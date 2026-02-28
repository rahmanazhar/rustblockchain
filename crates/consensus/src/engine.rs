use crate::error::ConsensusError;
use crate::finality::{FinalityEngine, FinalityVote};
use crate::pos::ProposerSelection;
use crate::slashing::Slasher;
use crate::state::ChainState;
use crate::tx_pool::TransactionPool;
use parking_lot::RwLock;
use rustchain_core::*;
use rustchain_crypto::{Blake3Hash, KeyPair};
use rustchain_storage::ChainDatabase;
use rustchain_vm::WasmEngine;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

/// Consensus event for subscribers.
#[derive(Clone, Debug)]
pub enum ConsensusEvent {
    NewBlock(Box<Block>),
    BlockFinalized(BlockNumber),
    EpochChanged(EpochInfo),
    TransactionPooled(Blake3Hash),
}

/// Consensus engine configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusConfig {
    pub chain_id: ChainId,
    pub block_time_ms: u64,
    pub max_transactions_per_block: usize,
    pub mempool_size: usize,
    pub min_gas_price: GasPrice,
    pub enable_block_production: bool,
    pub consensus_params: ConsensusParams,
    pub gas_limit_per_block: Gas,
}

impl Default for ConsensusConfig {
    fn default() -> Self {
        Self {
            chain_id: 1,
            block_time_ms: 5000,
            max_transactions_per_block: 1000,
            mempool_size: 10000,
            min_gas_price: 1,
            enable_block_production: false,
            consensus_params: ConsensusParams::default(),
            gas_limit_per_block: 10_000_000,
        }
    }
}

/// Chain info snapshot.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChainInfo {
    pub chain_id: ChainId,
    pub height: BlockNumber,
    pub best_block_hash: Blake3Hash,
    pub epoch: EpochNumber,
    pub finalized_height: BlockNumber,
    pub pending_transactions: usize,
    pub active_validators: usize,
}

/// The main consensus orchestrator.
#[allow(dead_code)]
pub struct ConsensusEngine {
    config: ConsensusConfig,
    chain_state: Arc<RwLock<ChainState>>,
    tx_pool: Arc<TransactionPool>,
    finality: Arc<FinalityEngine>,
    slasher: Slasher,
    vm_engine: Arc<WasmEngine>,
    storage: Arc<ChainDatabase>,
    event_tx: broadcast::Sender<ConsensusEvent>,
    keypair: Option<KeyPair>,
}

impl ConsensusEngine {
    /// Create a new consensus engine.
    pub fn new(
        config: ConsensusConfig,
        genesis: &GenesisConfig,
        storage: Arc<ChainDatabase>,
        vm_engine: Arc<WasmEngine>,
        keypair: Option<KeyPair>,
    ) -> Result<Self, ConsensusError> {
        let chain_state = ChainState::from_genesis(genesis, storage.clone())?;
        let validator_set = chain_state.current_epoch().validator_set.clone();

        let tx_pool = Arc::new(TransactionPool::new(
            config.mempool_size,
            config.min_gas_price,
        ));

        let finality = Arc::new(FinalityEngine::new(Arc::new(
            parking_lot::RwLock::new(validator_set),
        )));

        let slasher = Slasher::new(config.consensus_params.clone());

        let (event_tx, _) = broadcast::channel(1024);

        Ok(Self {
            config,
            chain_state: Arc::new(RwLock::new(chain_state)),
            tx_pool,
            finality,
            slasher,
            vm_engine,
            storage,
            event_tx,
            keypair,
        })
    }

    /// Start the consensus engine main loop.
    pub async fn start(&self, shutdown: CancellationToken) -> Result<(), ConsensusError> {
        info!("Consensus engine started");

        if !self.config.enable_block_production {
            info!("Block production disabled, running in follower mode");
            shutdown.cancelled().await;
            return Ok(());
        }

        let keypair = self.keypair.as_ref().ok_or(ConsensusError::NotValidator)?;
        let block_time = Duration::from_millis(self.config.block_time_ms);

        loop {
            tokio::select! {
                _ = shutdown.cancelled() => {
                    info!("Consensus engine shutting down");
                    break;
                }
                _ = tokio::time::sleep(block_time) => {
                    if let Err(e) = self.try_produce_block(keypair).await {
                        warn!("Block production failed: {}", e);
                    }
                }
            }
        }

        Ok(())
    }

    async fn try_produce_block(&self, keypair: &KeyPair) -> Result<(), ConsensusError> {
        let state = self.chain_state.read();
        let parent = state.head().clone();
        let epoch = state.current_epoch().clone();
        drop(state);

        // Check if we're the proposer for this slot
        let active = epoch.validator_set.active_validators();
        let next_number = parent.header.number + 1;

        let proposer =
            ProposerSelection::select_proposer(&active, &parent.hash(), next_number)?;

        if proposer != keypair.address() {
            debug!(
                "Not our turn to produce block {} (proposer: {})",
                next_number, proposer
            );
            return Ok(());
        }

        // Select transactions from pool
        let transactions = self.tx_pool.select_transactions(
            self.config.gas_limit_per_block,
            self.config.max_transactions_per_block,
        );

        // Build block
        let tx_hashes: Vec<Blake3Hash> = transactions.iter().map(|tx| tx.hash()).collect();
        let tx_data: Vec<&[u8]> = tx_hashes.iter().map(|h| h.as_ref()).collect();
        let transactions_root = rustchain_crypto::compute_merkle_root(&tx_data);

        let timestamp = chrono::Utc::now().timestamp_millis() as Timestamp;

        let header = BlockHeader {
            version: PROTOCOL_VERSION,
            chain_id: self.config.chain_id,
            number: next_number,
            timestamp,
            parent_hash: parent.hash(),
            state_root: Blake3Hash::ZERO, // Will be updated after execution
            transactions_root,
            receipts_root: Blake3Hash::ZERO,
            validator: keypair.address(),
            validator_pubkey: keypair.public_key(),
            validator_signature: rustchain_crypto::Signature::default(),
            epoch: epoch.epoch_number,
            gas_used: 0,
            gas_limit: self.config.gas_limit_per_block,
            extra_data: vec![],
        };

        let mut block = Block {
            header,
            transactions: transactions.clone(),
        };

        // Execute block and get receipts
        let mut state = self.chain_state.write();
        let receipts = state.apply_block(&block, &self.vm_engine)?;

        // Update header with actual values
        block.header.gas_used = receipts.iter().map(|r| r.gas_used).sum();

        // Sign the block
        block.header.sign(keypair);

        // Remove committed txs from pool
        let committed_hashes: Vec<Blake3Hash> = transactions.iter().map(|tx| tx.hash()).collect();
        self.tx_pool.prune_committed(&committed_hashes);

        // Check for epoch transition
        state.maybe_advance_epoch(&self.config.consensus_params)?;
        drop(state);

        // Create finality vote
        let vote = self.finality.create_vote(next_number, block.hash(), keypair);
        let _ = self.finality.add_vote(vote);

        // Broadcast events
        let _ = self.event_tx.send(ConsensusEvent::NewBlock(Box::new(block.clone())));

        info!(
            "Produced block {} with {} transactions",
            next_number,
            committed_hashes.len()
        );

        Ok(())
    }

    /// Handle a block received from the network.
    pub fn on_block_received(&self, block: Block) -> Result<(), ConsensusError> {
        // Validate basic structure
        block.validate_basic()?;

        // Validate proposer
        let state = self.chain_state.read();
        let epoch = state.current_epoch().clone();
        let parent_hash = state.head().hash();
        drop(state);

        let active = epoch.validator_set.active_validators();
        ProposerSelection::verify_proposer(
            &block.header.validator,
            &active,
            &parent_hash,
            block.header.number,
        )?;

        // Verify header signature
        block.header.verify_signature()?;

        // Apply block
        let mut state = self.chain_state.write();
        let _receipts = state.apply_block(&block, &self.vm_engine)?;

        // Prune committed transactions from pool
        let hashes: Vec<Blake3Hash> = block.transactions.iter().map(|tx| tx.hash()).collect();
        self.tx_pool.prune_committed(&hashes);

        // Check epoch transition
        state.maybe_advance_epoch(&self.config.consensus_params)?;
        drop(state);

        let _ = self.event_tx.send(ConsensusEvent::NewBlock(Box::new(block)));

        Ok(())
    }

    /// Handle a transaction received from the network.
    pub fn on_transaction_received(
        &self,
        tx: SignedTransaction,
    ) -> Result<(), ConsensusError> {
        let hash = tx.hash();
        self.tx_pool.insert(tx)?;
        let _ = self
            .event_tx
            .send(ConsensusEvent::TransactionPooled(hash));
        Ok(())
    }

    /// Handle a finality vote.
    pub fn on_vote_received(&self, vote: FinalityVote) -> Result<(), ConsensusError> {
        let status = self.finality.add_vote(vote)?;
        if let crate::finality::FinalityStatus::Finalized = status {
            let height = self.finality.finalized_height();
            let _ = self.event_tx.send(ConsensusEvent::BlockFinalized(height));
        }
        Ok(())
    }

    /// Subscribe to consensus events.
    pub fn subscribe(&self) -> broadcast::Receiver<ConsensusEvent> {
        self.event_tx.subscribe()
    }

    /// Get current chain info.
    pub fn chain_info(&self) -> ChainInfo {
        let state = self.chain_state.read();
        ChainInfo {
            chain_id: self.config.chain_id,
            height: state.height(),
            best_block_hash: state.head().hash(),
            epoch: state.current_epoch().epoch_number,
            finalized_height: self.finality.finalized_height(),
            pending_transactions: self.tx_pool.pending_count(),
            active_validators: state.current_epoch().validator_set.active_validators().len(),
        }
    }

    pub fn tx_pool(&self) -> &Arc<TransactionPool> {
        &self.tx_pool
    }

    pub fn chain_state(&self) -> &Arc<RwLock<ChainState>> {
        &self.chain_state
    }

    pub fn finality_engine(&self) -> &Arc<FinalityEngine> {
        &self.finality
    }
}

use crate::error::ConsensusError;
use rustchain_core::*;
use rustchain_crypto::{Address, Blake3Hash};
use rustchain_storage::ChainDatabase;
use rustchain_vm::{ExecutionContext, StateReader, VmError, WasmEngine};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info};

/// In-memory state reader backed by the database.
pub struct DbStateReader {
    db: Arc<ChainDatabase>,
}

impl DbStateReader {
    pub fn new(db: Arc<ChainDatabase>) -> Self {
        Self { db }
    }
}

impl StateReader for DbStateReader {
    fn get_account(&self, addr: &Address) -> Result<Option<Account>, VmError> {
        self.db
            .get_account(addr)
            .map_err(|e| VmError::State(e.to_string()))
    }

    fn get_storage(&self, addr: &Address, key: &[u8]) -> Result<Option<Vec<u8>>, VmError> {
        self.db
            .get_contract_storage(addr, key)
            .map_err(|e| VmError::State(e.to_string()))
    }

    fn get_code(&self, code_hash: &Blake3Hash) -> Result<Option<Vec<u8>>, VmError> {
        self.db
            .get_contract_code(code_hash)
            .map_err(|e| VmError::State(e.to_string()))
    }
}

/// Manages the authoritative chain state.
pub struct ChainState {
    head: Block,
    height: BlockNumber,
    current_epoch: EpochInfo,
    storage: Arc<ChainDatabase>,
}

impl ChainState {
    /// Initialize from genesis.
    pub fn from_genesis(
        genesis: &GenesisConfig,
        storage: Arc<ChainDatabase>,
    ) -> Result<Self, ConsensusError> {
        genesis.validate()?;

        let genesis_block = genesis.to_genesis_block();
        let genesis_hash = genesis_block.hash();

        // Check if already initialized
        if let Some(existing_hash) = storage.get_genesis_hash()? {
            if existing_hash != genesis_hash {
                return Err(ConsensusError::StateTransition(
                    "genesis hash mismatch with existing database".to_string(),
                ));
            }

            // Already initialized, load current state
            let height = storage.get_chain_height()?.unwrap_or(0);
            let head = storage
                .get_block_by_number(height)?
                .ok_or_else(|| ConsensusError::StateTransition("head block not found".to_string()))?;

            let epoch_number = height / genesis.consensus_params.epoch_length;
            let epoch_start = epoch_number * genesis.consensus_params.epoch_length;
            let epoch_end = epoch_start + genesis.consensus_params.epoch_length - 1;

            let validator_infos = genesis.to_validator_infos();
            let vs = ValidatorSet::new(
                validator_infos,
                genesis.consensus_params.min_stake,
                genesis.consensus_params.max_validators,
            );

            let current_epoch = EpochInfo {
                epoch_number,
                start_block: epoch_start,
                end_block: epoch_end,
                validator_set: vs,
                finalized_block: None,
            };

            info!("Loaded existing chain at height {}", height);
            return Ok(Self {
                head,
                height,
                current_epoch,
                storage,
            });
        }

        // First-time initialization
        storage.put_block(&genesis_block)?;
        storage.set_chain_height(0)?;
        storage.set_genesis_hash(&genesis_hash)?;

        // Create initial accounts
        for acct in &genesis.initial_accounts {
            let account = Account::new(acct.address, acct.balance);
            storage.put_account(&account)?;
        }

        // Create initial validators
        let validator_infos = genesis.to_validator_infos();
        for vi in &validator_infos {
            storage.put_validator(vi)?;
        }

        let vs = ValidatorSet::new(
            validator_infos,
            genesis.consensus_params.min_stake,
            genesis.consensus_params.max_validators,
        );

        let current_epoch = EpochInfo {
            epoch_number: 0,
            start_block: 0,
            end_block: genesis.consensus_params.epoch_length - 1,
            validator_set: vs,
            finalized_block: None,
        };

        storage.put_epoch(&current_epoch)?;

        info!("Initialized chain from genesis: {}", genesis.chain_name);

        Ok(Self {
            head: genesis_block,
            height: 0,
            current_epoch,
            storage,
        })
    }

    pub fn head(&self) -> &Block {
        &self.head
    }

    pub fn height(&self) -> BlockNumber {
        self.height
    }

    pub fn current_epoch(&self) -> &EpochInfo {
        &self.current_epoch
    }

    pub fn storage(&self) -> &Arc<ChainDatabase> {
        &self.storage
    }

    /// Apply a validated block to the state, executing all transactions.
    pub fn apply_block(
        &mut self,
        block: &Block,
        vm: &WasmEngine,
    ) -> Result<Vec<TransactionReceipt>, ConsensusError> {
        // Validate parent hash
        let expected_parent = self.head.hash();
        if block.header.parent_hash != expected_parent {
            return Err(ConsensusError::InvalidBlock(format!(
                "parent hash mismatch: expected {}, got {}",
                expected_parent, block.header.parent_hash
            )));
        }

        // Validate block number
        if block.header.number != self.height + 1 {
            return Err(ConsensusError::InvalidBlock(format!(
                "block number mismatch: expected {}, got {}",
                self.height + 1,
                block.header.number
            )));
        }

        let state_reader = Arc::new(DbStateReader::new(self.storage.clone()));
        let mut receipts = Vec::new();
        let mut total_gas_used: Gas = 0;
        let mut modified_accounts: HashMap<Address, Account> = HashMap::new();

        // Execute each transaction
        for (i, signed_tx) in block.transactions.iter().enumerate() {
            let tx = &signed_tx.transaction;
            let sender = signed_tx.sender();

            // Get or create sender account
            let mut sender_account = modified_accounts
                .get(&sender)
                .cloned()
                .or_else(|| self.storage.get_account(&sender).ok().flatten())
                .unwrap_or_else(|| Account::new(sender, 0));

            // Verify nonce
            if tx.nonce != sender_account.nonce {
                let receipt = TransactionReceipt {
                    tx_hash: signed_tx.hash(),
                    block_number: block.header.number,
                    block_hash: block.hash(),
                    index: i as u32,
                    status: TxStatus::Failure(format!(
                        "nonce mismatch: expected {}, got {}",
                        sender_account.nonce, tx.nonce
                    )),
                    gas_used: 0,
                    logs: vec![],
                    contract_address: None,
                    return_data: vec![],
                };
                receipts.push(receipt);
                continue;
            }

            // Check balance for gas + value
            let total_cost = tx.total_cost();
            if !sender_account.has_sufficient_balance(total_cost) {
                let receipt = TransactionReceipt {
                    tx_hash: signed_tx.hash(),
                    block_number: block.header.number,
                    block_hash: block.hash(),
                    index: i as u32,
                    status: TxStatus::Failure(format!(
                        "insufficient balance: have {}, need {}",
                        sender_account.balance, total_cost
                    )),
                    gas_used: 0,
                    logs: vec![],
                    contract_address: None,
                    return_data: vec![],
                };
                receipts.push(receipt);
                continue;
            }

            // Deduct gas cost upfront
            sender_account.balance -= tx.max_gas_cost();
            sender_account.increment_nonce();

            let (receipt, gas_used) = match tx.tx_type {
                TxType::Transfer => {
                    let to_addr = tx.to.unwrap_or(Address::ZERO);
                    let mut to_account = modified_accounts
                        .get(&to_addr)
                        .cloned()
                        .or_else(|| self.storage.get_account(&to_addr).ok().flatten())
                        .unwrap_or_else(|| Account::new(to_addr, 0));

                    sender_account.balance -= tx.value;
                    to_account.balance += tx.value;
                    modified_accounts.insert(to_addr, to_account);

                    let base_gas: Gas = 21000;
                    let gas_refund = tx.gas_limit.saturating_sub(base_gas) * tx.gas_price;
                    sender_account.balance += gas_refund as Balance;

                    let receipt = TransactionReceipt {
                        tx_hash: signed_tx.hash(),
                        block_number: block.header.number,
                        block_hash: block.hash(),
                        index: i as u32,
                        status: TxStatus::Success,
                        gas_used: base_gas,
                        logs: vec![],
                        contract_address: None,
                        return_data: vec![],
                    };
                    (receipt, base_gas)
                }
                TxType::ContractDeploy => {
                    let mut context = ExecutionContext::new(
                        sender,
                        sender,
                        Address::ZERO,
                        tx.value,
                        block.header.number,
                        block.header.timestamp,
                        block.header.chain_id,
                        tx.gas_limit,
                        state_reader.clone(),
                    );

                    match vm.deploy_contract(&tx.data, &[], &mut context) {
                        Ok(result) => {
                            let code_hash = result.code_hash;
                            let contract_addr = result.contract_address;

                            // Store contract code
                            let _ = self.storage.put_contract_code(&code_hash, &tx.data);

                            // Create contract account
                            let contract_account =
                                Account::new_contract(contract_addr, tx.value, code_hash);
                            modified_accounts.insert(contract_addr, contract_account);

                            // Apply storage changes
                            for ((addr, key), value) in &context.storage_changes {
                                match value {
                                    Some(v) => { let _ = self.storage.put_contract_storage(addr, key, v); },
                                    None => { let _ = self.storage.delete_contract_storage(addr, key); },
                                }
                            }

                            sender_account.balance -= tx.value;
                            let gas_refund = tx.gas_limit.saturating_sub(result.gas_used) * tx.gas_price;
                            sender_account.balance += gas_refund as Balance;

                            let receipt = TransactionReceipt {
                                tx_hash: signed_tx.hash(),
                                block_number: block.header.number,
                                block_hash: block.hash(),
                                index: i as u32,
                                status: TxStatus::Success,
                                gas_used: result.gas_used,
                                logs: result.logs,
                                contract_address: Some(contract_addr),
                                return_data: vec![],
                            };
                            (receipt, result.gas_used)
                        }
                        Err(e) => {
                            let receipt = TransactionReceipt {
                                tx_hash: signed_tx.hash(),
                                block_number: block.header.number,
                                block_hash: block.hash(),
                                index: i as u32,
                                status: TxStatus::Failure(e.to_string()),
                                gas_used: tx.gas_limit,
                                logs: vec![],
                                contract_address: None,
                                return_data: vec![],
                            };
                            (receipt, tx.gas_limit)
                        }
                    }
                }
                TxType::ContractCall => {
                    let to_addr = tx.to.unwrap_or(Address::ZERO);
                    let to_account = modified_accounts
                        .get(&to_addr)
                        .cloned()
                        .or_else(|| self.storage.get_account(&to_addr).ok().flatten());

                    if let Some(acct) = to_account {
                        if let Some(code_hash) = acct.code_hash {
                            let mut context = ExecutionContext::new(
                                sender,
                                sender,
                                to_addr,
                                tx.value,
                                block.header.number,
                                block.header.timestamp,
                                block.header.chain_id,
                                tx.gas_limit,
                                state_reader.clone(),
                            );

                            // Extract function name from data (first 32 bytes)
                            let func_name = if tx.data.len() >= 4 {
                                String::from_utf8_lossy(&tx.data[..4]).to_string()
                            } else {
                                "call".to_string()
                            };

                            match vm.call_contract(&code_hash, &func_name, &tx.data, &mut context) {
                                Ok(result) => {
                                    // Apply storage changes on success
                                    if matches!(result.status, TxStatus::Success) {
                                        for ((addr, key), value) in &context.storage_changes {
                                            match value {
                                                Some(v) => { let _ = self.storage.put_contract_storage(addr, key, v); },
                                                None => { let _ = self.storage.delete_contract_storage(addr, key); },
                                            }
                                        }
                                    }

                                    let gas_refund = tx.gas_limit.saturating_sub(result.gas_used) * tx.gas_price;
                                    sender_account.balance += gas_refund as Balance;

                                    let receipt = TransactionReceipt {
                                        tx_hash: signed_tx.hash(),
                                        block_number: block.header.number,
                                        block_hash: block.hash(),
                                        index: i as u32,
                                        status: result.status,
                                        gas_used: result.gas_used,
                                        logs: result.logs,
                                        contract_address: None,
                                        return_data: result.return_data,
                                    };
                                    (receipt, result.gas_used)
                                }
                                Err(e) => {
                                    let receipt = TransactionReceipt {
                                        tx_hash: signed_tx.hash(),
                                        block_number: block.header.number,
                                        block_hash: block.hash(),
                                        index: i as u32,
                                        status: TxStatus::Failure(e.to_string()),
                                        gas_used: tx.gas_limit,
                                        logs: vec![],
                                        contract_address: None,
                                        return_data: vec![],
                                    };
                                    (receipt, tx.gas_limit)
                                }
                            }
                        } else {
                            // Not a contract, treat as transfer
                            let receipt = TransactionReceipt {
                                tx_hash: signed_tx.hash(),
                                block_number: block.header.number,
                                block_hash: block.hash(),
                                index: i as u32,
                                status: TxStatus::Failure("target is not a contract".to_string()),
                                gas_used: 21000,
                                logs: vec![],
                                contract_address: None,
                                return_data: vec![],
                            };
                            (receipt, 21000)
                        }
                    } else {
                        let receipt = TransactionReceipt {
                            tx_hash: signed_tx.hash(),
                            block_number: block.header.number,
                            block_hash: block.hash(),
                            index: i as u32,
                            status: TxStatus::Failure("contract not found".to_string()),
                            gas_used: 21000,
                            logs: vec![],
                            contract_address: None,
                            return_data: vec![],
                        };
                        (receipt, 21000)
                    }
                }
                TxType::Stake => {
                    sender_account.balance -= tx.value;
                    // In production, update validator set
                    let receipt = TransactionReceipt {
                        tx_hash: signed_tx.hash(),
                        block_number: block.header.number,
                        block_hash: block.hash(),
                        index: i as u32,
                        status: TxStatus::Success,
                        gas_used: 21000,
                        logs: vec![],
                        contract_address: None,
                        return_data: vec![],
                    };
                    (receipt, 21000)
                }
                TxType::Unstake => {
                    // In production, initiate unbonding period
                    let receipt = TransactionReceipt {
                        tx_hash: signed_tx.hash(),
                        block_number: block.header.number,
                        block_hash: block.hash(),
                        index: i as u32,
                        status: TxStatus::Success,
                        gas_used: 21000,
                        logs: vec![],
                        contract_address: None,
                        return_data: vec![],
                    };
                    (receipt, 21000)
                }
            };

            total_gas_used += gas_used;
            modified_accounts.insert(sender, sender_account);
            receipts.push(receipt);
        }

        // Commit all changes atomically
        let mut batch = rustchain_storage::BlockCommitBatch::new(&self.storage);
        batch.put_block(block)?;
        batch.put_transactions(&block.transactions, block.header.number)?;
        batch.put_receipts(&receipts)?;
        batch.update_accounts(&modified_accounts.values().cloned().collect::<Vec<_>>())?;
        batch.set_chain_height(block.header.number)?;
        batch.set_best_block_hash(&block.hash())?;
        batch.commit()?;

        self.head = block.clone();
        self.height = block.header.number;

        debug!(
            "Applied block {} with {} txs, gas used: {}",
            block.header.number,
            block.transactions.len(),
            total_gas_used
        );

        Ok(receipts)
    }

    /// Check and advance epoch if at epoch boundary.
    pub fn maybe_advance_epoch(
        &mut self,
        consensus_params: &ConsensusParams,
    ) -> Result<Option<EpochInfo>, ConsensusError> {
        if !self.current_epoch.is_last_block(self.height) {
            return Ok(None);
        }

        let new_epoch_number = self.current_epoch.epoch_number + 1;
        let start_block = self.height + 1;
        let end_block = start_block + consensus_params.epoch_length - 1;

        // For now, carry forward the same validator set
        // In production, recalculate based on staking changes
        let new_epoch = EpochInfo {
            epoch_number: new_epoch_number,
            start_block,
            end_block,
            validator_set: self.current_epoch.validator_set.clone(),
            finalized_block: None,
        };

        self.storage.put_epoch(&new_epoch)?;
        self.current_epoch = new_epoch.clone();

        info!(
            "Advanced to epoch {} (blocks {}-{})",
            new_epoch_number, start_block, end_block
        );

        Ok(Some(new_epoch))
    }
}

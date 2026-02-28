use crate::config::NodeConfig;
use rustchain_api::{metrics::MetricsRegistry, server::ApiServer};
use rustchain_consensus::ConsensusEngine;
use rustchain_network::{NetworkEvent, NetworkService};
use rustchain_storage::ChainDatabase;
use rustchain_vm::WasmEngine;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

/// Main application that wires all components together.
pub struct Application {
    config: NodeConfig,
    keypair: Option<rustchain_crypto::KeyPair>,
}

impl Application {
    pub fn new(config: NodeConfig, keypair: Option<rustchain_crypto::KeyPair>) -> Self {
        Self { config, keypair }
    }

    /// Run the application until shutdown.
    pub async fn run(self) -> anyhow::Result<()> {
        let shutdown = CancellationToken::new();

        // 1. Open storage
        info!("Opening database at {:?}", self.config.storage.path);
        let storage = Arc::new(ChainDatabase::open(&self.config.storage)?);

        // 2. Initialize VM engine
        info!("Initializing WASM VM engine");
        let vm_engine = Arc::new(WasmEngine::new(&self.config.vm)?);

        // 3. Use provided keypair or generate one for validator mode
        let keypair = if self.config.consensus.enable_block_production {
            let kp = self.keypair.unwrap_or_else(rustchain_crypto::KeyPair::generate);
            info!("Validator address: {}", kp.address());
            Some(kp)
        } else {
            None
        };

        // 4. Initialize consensus engine
        info!("Starting consensus engine");
        let consensus = Arc::new(ConsensusEngine::new(
            self.config.consensus.clone(),
            &self.config.genesis,
            storage.clone(),
            vm_engine.clone(),
            keypair,
        )?);

        // 5. Start network service
        info!("Starting network service");
        let (network_service, mut network_events, _network_commands) =
            NetworkService::new(self.config.network.clone())?;

        let network_shutdown = shutdown.clone();
        let network_handle = tokio::spawn(async move {
            if let Err(e) = network_service.start(network_shutdown).await {
                error!("Network service error: {}", e);
            }
        });

        // 6. Start API server
        info!("Starting API server on {}", self.config.api.bind_address);
        let metrics = Arc::new(MetricsRegistry::new());
        let api_server = ApiServer::new(
            self.config.api.clone(),
            consensus.clone(),
            storage.clone(),
            metrics.clone(),
        );

        let _api_shutdown = shutdown.clone();
        let api_handle = tokio::spawn(async move {
            if let Err(e) = api_server.serve().await {
                error!("API server error: {}", e);
            }
        });

        // 7. Start consensus engine
        let consensus_clone = consensus.clone();
        let consensus_shutdown = shutdown.clone();
        let consensus_handle = tokio::spawn(async move {
            if let Err(e) = consensus_clone.start(consensus_shutdown).await {
                error!("Consensus engine error: {}", e);
            }
        });

        // 8. Network event handler - forward events to consensus
        let consensus_for_events = consensus.clone();
        let event_shutdown = shutdown.clone();
        let event_handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = event_shutdown.cancelled() => break,
                    event = network_events.recv() => {
                        match event {
                            Some(NetworkEvent::BlockReceived { block, peer }) => {
                                if let Err(e) = consensus_for_events.on_block_received(block) {
                                    tracing::warn!("Failed to process block from {}: {}", peer, e);
                                }
                            }
                            Some(NetworkEvent::TransactionReceived { tx, peer }) => {
                                if let Err(e) = consensus_for_events.on_transaction_received(tx) {
                                    tracing::debug!("Failed to process tx from {}: {}", peer, e);
                                }
                            }
                            Some(NetworkEvent::VoteReceived { vote, peer }) => {
                                if let Err(e) = consensus_for_events.on_vote_received(vote) {
                                    tracing::debug!("Failed to process vote from {}: {}", peer, e);
                                }
                            }
                            None => break,
                            _ => {}
                        }
                    }
                }
            }
        });

        info!("RustChain node started successfully");
        info!("Chain ID: {}", self.config.genesis.chain_id);
        info!("Chain name: {}", self.config.genesis.chain_name);

        // Wait for shutdown signal
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                info!("Received Ctrl+C, initiating shutdown...");
            }
        }

        // Graceful shutdown
        shutdown.cancel();

        // Wait for all tasks to complete
        let _ = tokio::join!(
            network_handle,
            api_handle,
            consensus_handle,
            event_handle,
        );

        // Flush storage
        storage.flush()?;
        info!("Node shut down gracefully");

        Ok(())
    }
}

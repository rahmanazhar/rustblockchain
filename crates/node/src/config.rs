use rustchain_api::config::ApiConfig;
use rustchain_consensus::ConsensusConfig;
use rustchain_core::GenesisConfig;
use rustchain_network::NetworkConfig;
use rustchain_storage::StorageConfig;
use rustchain_vm::VmConfig;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Logging configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub format: String,
    pub file: Option<String>,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            format: "pretty".to_string(),
            file: None,
        }
    }
}

/// Complete node configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeConfig {
    pub genesis: GenesisConfig,
    pub storage: StorageConfig,
    pub network: NetworkConfig,
    pub consensus: ConsensusConfig,
    pub api: ApiConfig,
    pub vm: VmConfig,
    pub logging: LoggingConfig,
}

impl NodeConfig {
    /// Load configuration from a TOML file.
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let data = std::fs::read_to_string(path)?;
        let config: Self = toml::from_str(&data)?;
        Ok(config)
    }

    /// Create a default development configuration, returning the validator keypair.
    pub fn devnet() -> (Self, rustchain_crypto::KeyPair) {
        let (genesis, keypair) = GenesisConfig::devnet_default();
        let config = Self {
            consensus: ConsensusConfig {
                chain_id: genesis.chain_id,
                block_time_ms: genesis.consensus_params.block_time_ms,
                consensus_params: genesis.consensus_params.clone(),
                gas_limit_per_block: genesis.gas_limit,
                enable_block_production: true,
                ..ConsensusConfig::default()
            },
            genesis,
            storage: StorageConfig::default(),
            network: NetworkConfig::default(),
            api: ApiConfig::default(),
            vm: VmConfig::default(),
            logging: LoggingConfig::default(),
        };
        (config, keypair)
    }
}

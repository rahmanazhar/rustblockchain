use crate::block::Block;
use crate::error::CoreError;
use crate::types::*;
use crate::validator::ValidatorInfo;
use rustchain_crypto::{Address, PublicKey};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Consensus parameters.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConsensusParams {
    pub block_time_ms: u64,
    pub epoch_length: u64,
    pub min_validators: u32,
    pub max_validators: u32,
    pub slash_fraction_double_sign: u16,
    pub slash_fraction_downtime: u16,
    pub downtime_jail_duration_ms: u64,
    pub min_stake: Balance,
    pub unbonding_period_epochs: u64,
    pub max_missed_blocks: u64,
    pub signed_blocks_window: u64,
}

impl Default for ConsensusParams {
    fn default() -> Self {
        Self {
            block_time_ms: 5000,
            epoch_length: 100,
            min_validators: 1,
            max_validators: 100,
            slash_fraction_double_sign: 500, // 5% in basis points
            slash_fraction_downtime: 100,    // 1%
            downtime_jail_duration_ms: 600_000, // 10 minutes
            min_stake: 1_000_000_000_000_000_000, // 1 token (18 decimals)
            unbonding_period_epochs: 10,
            max_missed_blocks: 50,
            signed_blocks_window: 100,
        }
    }
}

/// A validator defined in the genesis.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GenesisValidator {
    pub address: Address,
    pub public_key: PublicKey,
    pub stake: Balance,
}

/// An account with initial balance defined in the genesis.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GenesisAccount {
    pub address: Address,
    pub balance: Balance,
}

/// Genesis configuration for the blockchain.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GenesisConfig {
    pub chain_id: ChainId,
    pub chain_name: String,
    pub timestamp: Timestamp,
    pub initial_validators: Vec<GenesisValidator>,
    pub initial_accounts: Vec<GenesisAccount>,
    pub consensus_params: ConsensusParams,
    pub gas_limit: Gas,
}

impl GenesisConfig {
    /// Load genesis configuration from a TOML file.
    pub fn load(path: &Path) -> Result<Self, CoreError> {
        let data = std::fs::read_to_string(path)
            .map_err(|e| CoreError::Genesis(format!("failed to read genesis file: {}", e)))?;
        toml::from_str(&data)
            .map_err(|e| CoreError::Genesis(format!("failed to parse genesis: {}", e)))
    }

    /// Validate the genesis configuration.
    pub fn validate(&self) -> Result<(), CoreError> {
        if self.initial_validators.is_empty() {
            return Err(CoreError::Genesis(
                "genesis must have at least one validator".to_string(),
            ));
        }

        if self.initial_validators.len() < self.consensus_params.min_validators as usize {
            return Err(CoreError::Genesis(format!(
                "need at least {} validators, got {}",
                self.consensus_params.min_validators,
                self.initial_validators.len()
            )));
        }

        for v in &self.initial_validators {
            if v.stake < self.consensus_params.min_stake {
                return Err(CoreError::Genesis(format!(
                    "validator {} has insufficient stake: {} < {}",
                    v.address, v.stake, self.consensus_params.min_stake
                )));
            }
        }

        if self.gas_limit == 0 {
            return Err(CoreError::Genesis("gas limit must be > 0".to_string()));
        }

        Ok(())
    }

    /// Create the genesis block from this configuration.
    pub fn to_genesis_block(&self) -> Block {
        let first_validator = &self.initial_validators[0];
        Block::genesis(
            self.chain_id,
            self.timestamp,
            first_validator.address,
            first_validator.public_key,
            self.gas_limit,
            format!("rustchain genesis: {}", self.chain_name).into_bytes(),
        )
    }

    /// Convert genesis validators to ValidatorInfo structs.
    pub fn to_validator_infos(&self) -> Vec<ValidatorInfo> {
        self.initial_validators
            .iter()
            .map(|gv| ValidatorInfo::new(gv.address, gv.public_key, gv.stake))
            .collect()
    }

    /// Create a default devnet genesis config, returning the validator keypair.
    pub fn devnet_default() -> (Self, rustchain_crypto::KeyPair) {
        let kp = rustchain_crypto::KeyPair::generate();
        let config = Self {
            chain_id: 9999,
            chain_name: "rustchain-devnet".to_string(),
            timestamp: chrono::Utc::now().timestamp_millis() as u64,
            initial_validators: vec![GenesisValidator {
                address: kp.address(),
                public_key: kp.public_key(),
                stake: 10_000_000_000_000_000_000, // 10 tokens
            }],
            initial_accounts: vec![GenesisAccount {
                address: kp.address(),
                balance: 1_000_000_000_000_000_000_000, // 1000 tokens
            }],
            consensus_params: ConsensusParams {
                min_validators: 1,
                ..ConsensusParams::default()
            },
            gas_limit: 10_000_000,
        };
        (config, kp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_devnet_genesis_valid() {
        let (config, _kp) = GenesisConfig::devnet_default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_genesis_block_creation() {
        let (config, _kp) = GenesisConfig::devnet_default();
        let block = config.to_genesis_block();
        assert_eq!(block.header.number, 0);
        assert_eq!(block.header.chain_id, 9999);
    }

    #[test]
    fn test_empty_validators_invalid() {
        let (mut config, _kp) = GenesisConfig::devnet_default();
        config.initial_validators.clear();
        assert!(config.validate().is_err());
    }
}

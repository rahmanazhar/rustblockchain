use crate::genesis::ConsensusParams;
use crate::types::*;
use rustchain_crypto::Address;
use serde::{Deserialize, Serialize};

/// Network type configuration.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum NetworkType {
    Public,
    Private {
        allowed_validators: Vec<Address>,
    },
    Consortium {
        member_orgs: Vec<String>,
        allowed_validators: Vec<Address>,
    },
}

/// Chain-level configuration.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChainConfig {
    pub chain_id: ChainId,
    pub chain_name: String,
    pub network_type: NetworkType,
    pub consensus: ConsensusParams,
    pub gas_limit_per_block: Gas,
    pub base_gas_price: GasPrice,
    pub max_tx_size: usize,
    pub max_block_transactions: usize,
}

impl Default for ChainConfig {
    fn default() -> Self {
        Self {
            chain_id: 1,
            chain_name: "rustchain".to_string(),
            network_type: NetworkType::Public,
            consensus: ConsensusParams::default(),
            gas_limit_per_block: 10_000_000,
            base_gas_price: 1,
            max_tx_size: MAX_TRANSACTION_DATA_SIZE,
            max_block_transactions: MAX_BLOCK_TRANSACTIONS,
        }
    }
}

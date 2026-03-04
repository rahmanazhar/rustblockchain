//! Chain adapters — pluggable interfaces for connecting to external blockchains.

pub mod bitcoin;
pub mod ethereum;
pub mod solana;

use crate::error::BridgeError;
use serde::{Deserialize, Serialize};

/// Information about a lock/deposit event on an external chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalLockEvent {
    /// Transaction hash on the external chain
    pub tx_hash: String,
    /// Sender address on the external chain
    pub sender: String,
    /// Recipient address on RustChain (hex)
    pub recipient: String,
    /// Token symbol
    pub token_symbol: String,
    /// Amount in the token's smallest unit
    pub amount: u128,
    /// Block number on the external chain
    pub block_number: u64,
    /// Number of confirmations
    pub confirmations: u64,
}

/// Trait for chain-specific adapters.
///
/// Each adapter knows how to:
/// - Monitor for lock/deposit events on the external chain
/// - Submit unlock/withdrawal transactions on the external chain
/// - Verify proofs from the external chain
pub trait ChainAdapter: Send + Sync {
    /// Get the chain name.
    fn chain_name(&self) -> &str;

    /// Get the RPC endpoint URL.
    fn rpc_url(&self) -> &str;

    /// Check if the adapter is connected and healthy.
    fn is_connected(&self) -> bool;

    /// Get the current block number on the external chain.
    fn current_block_number(&self) -> Result<u64, BridgeError>;

    /// Get the required number of confirmations for this chain.
    fn required_confirmations(&self) -> u64;

    /// Check recent lock events on the external chain.
    ///
    /// Returns events that have sufficient confirmations.
    fn poll_lock_events(
        &self,
        from_block: u64,
        to_block: u64,
    ) -> Result<Vec<ExternalLockEvent>, BridgeError>;

    /// Submit an unlock transaction on the external chain.
    ///
    /// Returns the transaction hash on the external chain.
    fn submit_unlock(
        &self,
        recipient: &str,
        token_symbol: &str,
        amount: u128,
    ) -> Result<String, BridgeError>;

    /// Verify a transaction proof from the external chain.
    fn verify_tx_proof(
        &self,
        tx_hash: &str,
        expected_amount: u128,
    ) -> Result<bool, BridgeError>;

    /// Get the bridge contract address on the external chain.
    fn bridge_contract_address(&self) -> &str;

    /// Estimate the gas cost for an unlock transaction.
    fn estimate_unlock_gas(&self) -> Result<u128, BridgeError>;
}

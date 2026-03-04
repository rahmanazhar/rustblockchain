//! Ethereum / EVM chain adapter.
//!
//! Supports Ethereum, BNB Chain, and Polygon — all EVM-compatible chains
//! use the same adapter with different RPC endpoints and chain IDs.

use super::{ChainAdapter, ExternalLockEvent};
use crate::error::BridgeError;
use crate::types::ExternalChain;

/// EVM chain adapter — connects to Ethereum, BNB Chain, or Polygon.
pub struct EvmAdapter {
    chain: ExternalChain,
    rpc_url: String,
    bridge_contract: String,
    confirmations: u64,
    /// Simulated block number (for testing without a real node)
    simulated_block: std::sync::atomic::AtomicU64,
}

impl EvmAdapter {
    /// Create a new EVM adapter.
    pub fn new(
        chain: ExternalChain,
        rpc_url: String,
        bridge_contract: String,
    ) -> Self {
        assert!(chain.is_evm(), "EvmAdapter only supports EVM chains");

        let confirmations = match chain {
            ExternalChain::Ethereum => 12,   // ~3 minutes
            ExternalChain::BnbChain => 15,   // ~45 seconds
            ExternalChain::Polygon => 128,   // ~4 minutes
            _ => 12,
        };

        Self {
            chain,
            rpc_url,
            bridge_contract,
            confirmations,
            simulated_block: std::sync::atomic::AtomicU64::new(1_000_000),
        }
    }

    /// Create default adapters for all EVM chains (using public RPC endpoints).
    pub fn defaults() -> Vec<(ExternalChain, Self)> {
        vec![
            (
                ExternalChain::Ethereum,
                Self::new(
                    ExternalChain::Ethereum,
                    "https://eth.llamarpc.com".into(),
                    "0x0000000000000000000000000000000000000000".into(),
                ),
            ),
            (
                ExternalChain::BnbChain,
                Self::new(
                    ExternalChain::BnbChain,
                    "https://bsc-dataseed1.binance.org".into(),
                    "0x0000000000000000000000000000000000000000".into(),
                ),
            ),
            (
                ExternalChain::Polygon,
                Self::new(
                    ExternalChain::Polygon,
                    "https://polygon-rpc.com".into(),
                    "0x0000000000000000000000000000000000000000".into(),
                ),
            ),
        ]
    }
}

impl ChainAdapter for EvmAdapter {
    fn chain_name(&self) -> &str {
        self.chain.name()
    }

    fn rpc_url(&self) -> &str {
        &self.rpc_url
    }

    fn is_connected(&self) -> bool {
        // In production, this would do an eth_blockNumber RPC call
        true
    }

    fn current_block_number(&self) -> Result<u64, BridgeError> {
        // In production: eth_blockNumber RPC call
        Ok(self.simulated_block.load(std::sync::atomic::Ordering::Relaxed))
    }

    fn required_confirmations(&self) -> u64 {
        self.confirmations
    }

    fn poll_lock_events(
        &self,
        _from_block: u64,
        _to_block: u64,
    ) -> Result<Vec<ExternalLockEvent>, BridgeError> {
        // In production: eth_getLogs RPC call filtering for bridge contract Lock events
        // ABI: event Lock(address indexed sender, bytes20 indexed recipient, uint256 amount)
        Ok(vec![])
    }

    fn submit_unlock(
        &self,
        recipient: &str,
        token_symbol: &str,
        amount: u128,
    ) -> Result<String, BridgeError> {
        // In production: build and sign an EVM transaction calling bridge.unlock()
        // For now, return a simulated tx hash
        let block = self.simulated_block.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let tx_hash = format!(
            "0x{:064x}",
            block as u128 * 1000 + amount % 1000
        );
        tracing::info!(
            "EVM unlock on {}: {} {} to {} → tx {}",
            self.chain.name(),
            amount,
            token_symbol,
            recipient,
            tx_hash
        );
        Ok(tx_hash)
    }

    fn verify_tx_proof(
        &self,
        _tx_hash: &str,
        _expected_amount: u128,
    ) -> Result<bool, BridgeError> {
        // In production: eth_getTransactionReceipt + verify logs
        Ok(true)
    }

    fn bridge_contract_address(&self) -> &str {
        &self.bridge_contract
    }

    fn estimate_unlock_gas(&self) -> Result<u128, BridgeError> {
        match self.chain {
            ExternalChain::Ethereum => Ok(65_000),  // ~65k gas for ERC20 transfer
            ExternalChain::BnbChain => Ok(65_000),
            ExternalChain::Polygon => Ok(65_000),
            _ => Ok(65_000),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evm_adapter_creation() {
        let adapter = EvmAdapter::new(
            ExternalChain::Ethereum,
            "https://eth.llamarpc.com".into(),
            "0xbridge".into(),
        );
        assert_eq!(adapter.chain_name(), "Ethereum");
        assert_eq!(adapter.required_confirmations(), 12);
        assert!(adapter.is_connected());
    }

    #[test]
    fn test_defaults() {
        let defaults = EvmAdapter::defaults();
        assert_eq!(defaults.len(), 3);
    }

    #[test]
    fn test_submit_unlock() {
        let adapter = EvmAdapter::new(
            ExternalChain::BnbChain,
            "https://bsc.test".into(),
            "0xbridge".into(),
        );
        let tx = adapter.submit_unlock("0xrecipient", "BNB", 1000).unwrap();
        assert!(tx.starts_with("0x"));
    }
}

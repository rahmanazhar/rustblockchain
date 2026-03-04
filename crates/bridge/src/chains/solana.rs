//! Solana chain adapter.
//!
//! Solana uses Ed25519 signatures (same as RustChain) which makes key compatibility
//! straightforward. Bridge operations use Solana program accounts.

use super::{ChainAdapter, ExternalLockEvent};
use crate::error::BridgeError;

/// Solana chain adapter.
pub struct SolanaAdapter {
    rpc_url: String,
    /// Bridge program ID (base58 pubkey)
    bridge_program_id: String,
    /// Simulated slot (for testing)
    simulated_slot: std::sync::atomic::AtomicU64,
}

impl SolanaAdapter {
    pub fn new(rpc_url: String, bridge_program_id: String) -> Self {
        Self {
            rpc_url,
            bridge_program_id,
            simulated_slot: std::sync::atomic::AtomicU64::new(200_000_000),
        }
    }

    pub fn default_devnet() -> Self {
        Self::new(
            "https://api.devnet.solana.com".into(),
            "Bridge1111111111111111111111111111111111111".into(),
        )
    }
}

impl ChainAdapter for SolanaAdapter {
    fn chain_name(&self) -> &str {
        "Solana"
    }

    fn rpc_url(&self) -> &str {
        &self.rpc_url
    }

    fn is_connected(&self) -> bool {
        true
    }

    fn current_block_number(&self) -> Result<u64, BridgeError> {
        // Solana uses "slots" rather than blocks
        Ok(self.simulated_slot.load(std::sync::atomic::Ordering::Relaxed))
    }

    fn required_confirmations(&self) -> u64 {
        32 // Solana finality is ~32 slots (~12.8 seconds)
    }

    fn poll_lock_events(
        &self,
        _from_block: u64,
        _to_block: u64,
    ) -> Result<Vec<ExternalLockEvent>, BridgeError> {
        // In production: use getSignaturesForAddress + getTransaction
        // to find bridge program interactions
        Ok(vec![])
    }

    fn submit_unlock(
        &self,
        recipient: &str,
        token_symbol: &str,
        amount: u128,
    ) -> Result<String, BridgeError> {
        // In production: build a Solana transaction calling the bridge program's
        // unlock instruction
        let slot = self.simulated_slot.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        // Solana tx signatures are base58-encoded 64-byte Ed25519 signatures
        let sig = format!(
            "{}{}{}",
            &hex::encode(slot.to_le_bytes()),
            &hex::encode(amount.to_le_bytes())[..16],
            "simulated00000000000000000000000000000000000000000000000000000000000000000000"
        );
        let sig = &sig[..88]; // Trim to ~88 chars (base58 signature length)
        tracing::info!(
            "Solana unlock: {} {} to {} → sig {}",
            amount,
            token_symbol,
            recipient,
            sig
        );
        Ok(sig.to_string())
    }

    fn verify_tx_proof(
        &self,
        _tx_hash: &str,
        _expected_amount: u128,
    ) -> Result<bool, BridgeError> {
        // In production: getTransaction + verify program logs
        Ok(true)
    }

    fn bridge_contract_address(&self) -> &str {
        &self.bridge_program_id
    }

    fn estimate_unlock_gas(&self) -> Result<u128, BridgeError> {
        // Solana fees are very low (~5000 lamports = 0.000005 SOL)
        Ok(5_000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_solana_adapter() {
        let adapter = SolanaAdapter::default_devnet();
        assert_eq!(adapter.chain_name(), "Solana");
        assert_eq!(adapter.required_confirmations(), 32);
        assert!(adapter.is_connected());
    }

    #[test]
    fn test_submit_unlock() {
        let adapter = SolanaAdapter::default_devnet();
        let sig = adapter
            .submit_unlock("So1recipient...", "SOL", 1_000_000_000)
            .unwrap();
        assert!(!sig.is_empty());
    }
}

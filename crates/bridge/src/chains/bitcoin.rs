//! Bitcoin chain adapter.
//!
//! Bitcoin uses HTLC scripts for atomic swaps rather than smart contract bridges.
//! This adapter interfaces with Bitcoin nodes to monitor and submit HTLC transactions.

use super::{ChainAdapter, ExternalLockEvent};
use crate::error::BridgeError;

/// Bitcoin chain adapter.
pub struct BitcoinAdapter {
    rpc_url: String,
    /// Bitcoin network: "mainnet", "testnet", or "regtest"
    network: String,
    /// Simulated block height (for testing)
    simulated_block: std::sync::atomic::AtomicU64,
}

impl BitcoinAdapter {
    pub fn new(rpc_url: String, network: String) -> Self {
        Self {
            rpc_url,
            network,
            simulated_block: std::sync::atomic::AtomicU64::new(800_000),
        }
    }

    pub fn default_testnet() -> Self {
        Self::new(
            "http://localhost:18332".into(),
            "testnet".into(),
        )
    }

    /// Generate a P2SH HTLC script for atomic swap.
    ///
    /// Script structure:
    /// ```text
    /// OP_IF
    ///   OP_SHA256 <hash_lock> OP_EQUALVERIFY
    ///   <recipient_pubkey> OP_CHECKSIG
    /// OP_ELSE
    ///   <timelock> OP_CHECKLOCKTIMEVERIFY OP_DROP
    ///   <sender_pubkey> OP_CHECKSIG
    /// OP_ENDIF
    /// ```
    pub fn build_htlc_script(
        hash_lock: &[u8; 32],
        recipient_pubkey: &[u8],
        sender_pubkey: &[u8],
        timelock: u32,
    ) -> Vec<u8> {
        let mut script = Vec::new();

        // OP_IF
        script.push(0x63);
        // OP_SHA256
        script.push(0xa8);
        // Push 32 bytes (hash_lock)
        script.push(0x20);
        script.extend_from_slice(hash_lock);
        // OP_EQUALVERIFY
        script.push(0x88);
        // Push recipient pubkey
        script.push(recipient_pubkey.len() as u8);
        script.extend_from_slice(recipient_pubkey);
        // OP_CHECKSIG
        script.push(0xac);
        // OP_ELSE
        script.push(0x67);
        // Push timelock (4 bytes LE)
        script.push(0x04);
        script.extend_from_slice(&timelock.to_le_bytes());
        // OP_CHECKLOCKTIMEVERIFY
        script.push(0xb1);
        // OP_DROP
        script.push(0x75);
        // Push sender pubkey
        script.push(sender_pubkey.len() as u8);
        script.extend_from_slice(sender_pubkey);
        // OP_CHECKSIG
        script.push(0xac);
        // OP_ENDIF
        script.push(0x68);

        script
    }
}

impl ChainAdapter for BitcoinAdapter {
    fn chain_name(&self) -> &str {
        "Bitcoin"
    }

    fn rpc_url(&self) -> &str {
        &self.rpc_url
    }

    fn is_connected(&self) -> bool {
        true
    }

    fn current_block_number(&self) -> Result<u64, BridgeError> {
        Ok(self.simulated_block.load(std::sync::atomic::Ordering::Relaxed))
    }

    fn required_confirmations(&self) -> u64 {
        6 // Standard Bitcoin confirmation requirement
    }

    fn poll_lock_events(
        &self,
        _from_block: u64,
        _to_block: u64,
    ) -> Result<Vec<ExternalLockEvent>, BridgeError> {
        // In production: scan blocks for HTLC script outputs matching our bridge
        Ok(vec![])
    }

    fn submit_unlock(
        &self,
        recipient: &str,
        _token_symbol: &str,
        amount: u128,
    ) -> Result<String, BridgeError> {
        // In production: build a Bitcoin transaction spending the HTLC output
        let block = self.simulated_block.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let txid = format!("{:064x}", block as u128 * 1000 + amount % 1000);
        tracing::info!(
            "Bitcoin HTLC claim: {} satoshis to {} ({}) → txid {}",
            amount,
            recipient,
            self.network,
            txid
        );
        Ok(txid)
    }

    fn verify_tx_proof(
        &self,
        _tx_hash: &str,
        _expected_amount: u128,
    ) -> Result<bool, BridgeError> {
        // In production: getrawtransaction + verify outputs
        Ok(true)
    }

    fn bridge_contract_address(&self) -> &str {
        // Bitcoin doesn't have contracts; this returns the HTLC script hash
        "htlc-script"
    }

    fn estimate_unlock_gas(&self) -> Result<u128, BridgeError> {
        // Bitcoin fee is in satoshis, roughly 250 bytes * fee_rate
        Ok(5_000) // ~5000 satoshis
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bitcoin_adapter() {
        let adapter = BitcoinAdapter::default_testnet();
        assert_eq!(adapter.chain_name(), "Bitcoin");
        assert_eq!(adapter.required_confirmations(), 6);
    }

    #[test]
    fn test_htlc_script_generation() {
        let hash_lock = [0xAB; 32];
        let recipient_pk = vec![0x02; 33]; // compressed pubkey
        let sender_pk = vec![0x03; 33];
        let timelock = 800_000u32;

        let script = BitcoinAdapter::build_htlc_script(
            &hash_lock,
            &recipient_pk,
            &sender_pk,
            timelock,
        );

        // Verify script structure
        assert_eq!(script[0], 0x63); // OP_IF
        assert_eq!(script[1], 0xa8); // OP_SHA256
        assert!(!script.is_empty());
    }

    #[test]
    fn test_submit_unlock() {
        let adapter = BitcoinAdapter::default_testnet();
        let txid = adapter.submit_unlock("bc1q...", "BTC", 100_000).unwrap();
        assert_eq!(txid.len(), 64); // 32 bytes hex
    }
}

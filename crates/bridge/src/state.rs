//! Bridge state management — tracks locked tokens, wrapped supply, and transfer history.

use crate::error::BridgeError;
use crate::types::*;
use dashmap::DashMap;
use parking_lot::RwLock;
use rustchain_crypto::Address;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Wrapped token balances for a single chain.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WrappedTokenState {
    /// Total supply of wrapped tokens minted
    pub total_minted: u128,
    /// Total burned (redeemed back to external chain)
    pub total_burned: u128,
    /// Per-address balances of wrapped tokens
    pub balances: std::collections::HashMap<String, u128>,
}

impl WrappedTokenState {
    pub fn circulating_supply(&self) -> u128 {
        self.total_minted.saturating_sub(self.total_burned)
    }
}

/// Complete bridge state.
pub struct BridgeState {
    /// Transfer records keyed by transfer ID
    transfers: Arc<DashMap<String, BridgeTransfer>>,
    /// Transfers indexed by sender address
    by_sender: Arc<DashMap<String, Vec<String>>>,
    /// Locked token amounts per chain+token
    locked: Arc<DashMap<String, u128>>,
    /// Wrapped token state per chain
    wrapped: Arc<DashMap<String, WrappedTokenState>>,
    /// Bridge configuration
    config: Arc<RwLock<BridgeConfig>>,
    /// Whether the bridge is paused
    paused: Arc<RwLock<bool>>,
    /// Transfer counter for unique IDs
    counter: Arc<RwLock<u64>>,
}

impl BridgeState {
    pub fn new(config: BridgeConfig) -> Self {
        Self {
            transfers: Arc::new(DashMap::new()),
            by_sender: Arc::new(DashMap::new()),
            locked: Arc::new(DashMap::new()),
            wrapped: Arc::new(DashMap::new()),
            config: Arc::new(RwLock::new(config)),
            paused: Arc::new(RwLock::new(false)),
            counter: Arc::new(RwLock::new(0)),
        }
    }

    fn lock_key(chain: &ExternalChain, token: &str) -> String {
        format!("{}:{}", chain.name(), token)
    }

    fn next_id(&self) -> String {
        let mut counter = self.counter.write();
        *counter += 1;
        let now = chrono::Utc::now().timestamp_millis() as u64;
        format!("bridge-{}-{}", now, counter)
    }

    /// Check if bridge is paused.
    pub fn is_paused(&self) -> bool {
        *self.paused.read()
    }

    /// Pause the bridge.
    pub fn pause(&self) {
        *self.paused.write() = true;
    }

    /// Unpause the bridge.
    pub fn unpause(&self) {
        *self.paused.write() = false;
    }

    /// Get the current bridge configuration.
    pub fn config(&self) -> BridgeConfig {
        self.config.read().clone()
    }

    /// Initiate an outbound bridge transfer (RustChain → external chain).
    ///
    /// This locks tokens on RustChain. Once validators confirm, the relayer
    /// will unlock/mint tokens on the destination chain.
    pub fn initiate_outbound(
        &self,
        sender: Address,
        dest_chain: ExternalChain,
        recipient: String,
        token_symbol: String,
        amount: u128,
        fee: u128,
    ) -> Result<BridgeTransfer, BridgeError> {
        if self.is_paused() {
            return Err(BridgeError::BridgePaused);
        }

        let config = self.config.read();
        if !config.allow_outbound {
            return Err(BridgeError::Unauthorized("Outbound transfers disabled".into()));
        }

        let now = chrono::Utc::now().timestamp_millis() as u64;
        let id = self.next_id();

        let transfer = BridgeTransfer {
            id: id.clone(),
            direction: BridgeDirection::Outbound,
            source_chain: "RustChain".into(),
            dest_chain: dest_chain.name().into(),
            sender: sender.to_hex(),
            recipient,
            token_symbol,
            amount,
            fee,
            status: BridgeTransferStatus::Pending,
            source_tx_hash: None,
            dest_tx_hash: None,
            confirmations: 0,
            required_confirmations: config.min_signatures,
            created_at: now,
            updated_at: now,
            expires_at: now + config.transfer_timeout_seconds * 1000,
        };

        // Lock tokens
        let key = Self::lock_key(&dest_chain, &transfer.token_symbol);
        *self.locked.entry(key).or_insert(0) += amount;

        self.transfers.insert(id.clone(), transfer.clone());
        self.by_sender
            .entry(sender.to_hex())
            .or_default()
            .push(id);

        Ok(transfer)
    }

    /// Record an inbound bridge transfer (external chain → RustChain).
    ///
    /// Called when a relayer detects a lock on the external chain and submits
    /// a proof to RustChain.
    #[allow(clippy::too_many_arguments)]
    pub fn record_inbound(
        &self,
        source_chain: ExternalChain,
        sender: String,
        recipient: Address,
        token_symbol: String,
        amount: u128,
        fee: u128,
        source_tx_hash: String,
    ) -> Result<BridgeTransfer, BridgeError> {
        if self.is_paused() {
            return Err(BridgeError::BridgePaused);
        }

        let config = self.config.read();
        if !config.accept_inbound {
            return Err(BridgeError::Unauthorized("Inbound transfers disabled".into()));
        }

        let now = chrono::Utc::now().timestamp_millis() as u64;
        let id = self.next_id();

        let transfer = BridgeTransfer {
            id: id.clone(),
            direction: BridgeDirection::Inbound,
            source_chain: source_chain.name().into(),
            dest_chain: "RustChain".into(),
            sender,
            recipient: recipient.to_hex(),
            token_symbol: token_symbol.clone(),
            amount,
            fee,
            status: BridgeTransferStatus::SourceConfirmed,
            source_tx_hash: Some(source_tx_hash),
            dest_tx_hash: None,
            confirmations: 0,
            required_confirmations: config.min_signatures,
            created_at: now,
            updated_at: now,
            expires_at: now + config.transfer_timeout_seconds * 1000,
        };

        // Track wrapped token minting
        let key = Self::lock_key(&source_chain, &token_symbol);
        let net_amount = amount.saturating_sub(fee);
        self.wrapped.entry(key).or_default().total_minted += net_amount;

        self.transfers.insert(id.clone(), transfer.clone());

        Ok(transfer)
    }

    /// Add a validator confirmation to a transfer.
    pub fn add_confirmation(
        &self,
        transfer_id: &str,
        _validator: &Address,
    ) -> Result<BridgeTransfer, BridgeError> {
        let mut transfer = self
            .transfers
            .get_mut(transfer_id)
            .ok_or_else(|| BridgeError::TransferNotFound(transfer_id.into()))?;

        if transfer.status == BridgeTransferStatus::Completed {
            return Err(BridgeError::TransferAlreadyProcessed);
        }

        transfer.confirmations += 1;
        transfer.updated_at = chrono::Utc::now().timestamp_millis() as u64;

        if transfer.confirmations >= transfer.required_confirmations {
            transfer.status = BridgeTransferStatus::Validated;
        }

        Ok(transfer.clone())
    }

    /// Mark a transfer as completed.
    pub fn complete_transfer(
        &self,
        transfer_id: &str,
        dest_tx_hash: String,
    ) -> Result<BridgeTransfer, BridgeError> {
        let mut transfer = self
            .transfers
            .get_mut(transfer_id)
            .ok_or_else(|| BridgeError::TransferNotFound(transfer_id.into()))?;

        transfer.status = BridgeTransferStatus::Completed;
        transfer.dest_tx_hash = Some(dest_tx_hash);
        transfer.updated_at = chrono::Utc::now().timestamp_millis() as u64;

        Ok(transfer.clone())
    }

    /// Mark a transfer as failed.
    pub fn fail_transfer(&self, transfer_id: &str) -> Result<BridgeTransfer, BridgeError> {
        let mut transfer = self
            .transfers
            .get_mut(transfer_id)
            .ok_or_else(|| BridgeError::TransferNotFound(transfer_id.into()))?;

        transfer.status = BridgeTransferStatus::Failed;
        transfer.updated_at = chrono::Utc::now().timestamp_millis() as u64;

        Ok(transfer.clone())
    }

    /// Get a transfer by ID.
    pub fn get_transfer(&self, id: &str) -> Option<BridgeTransfer> {
        self.transfers.get(id).map(|t| t.clone())
    }

    /// Get all transfers for a sender address.
    pub fn get_transfers_by_sender(&self, sender: &str) -> Vec<BridgeTransfer> {
        self.by_sender
            .get(sender)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.transfers.get(id).map(|t| t.clone()))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// List all transfers (newest first).
    pub fn list_transfers(&self, limit: usize, offset: usize) -> Vec<BridgeTransfer> {
        let mut transfers: Vec<BridgeTransfer> =
            self.transfers.iter().map(|t| t.value().clone()).collect();
        transfers.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        transfers.into_iter().skip(offset).take(limit).collect()
    }

    /// Get bridge liquidity information for each chain+token.
    pub fn get_liquidity(&self) -> Vec<BridgeLiquidity> {
        let mut result = Vec::new();
        for chain in ExternalChain::all() {
            let key = Self::lock_key(chain, chain.symbol());
            let locked = self.locked.get(&key).map(|v| *v).unwrap_or(0);
            let wrapped = self
                .wrapped
                .get(&key)
                .map(|w| w.circulating_supply())
                .unwrap_or(0);

            result.push(BridgeLiquidity {
                chain: *chain,
                token_symbol: chain.symbol().into(),
                locked_amount: locked,
                wrapped_supply: wrapped,
                available_outbound: locked.saturating_sub(wrapped),
            });
        }
        result
    }

    /// Get the total number of transfers.
    pub fn total_transfers(&self) -> usize {
        self.transfers.len()
    }

    /// Get the number of pending transfers.
    pub fn pending_transfers(&self) -> usize {
        self.transfers
            .iter()
            .filter(|t| {
                matches!(
                    t.status,
                    BridgeTransferStatus::Pending
                        | BridgeTransferStatus::SourceConfirmed
                        | BridgeTransferStatus::RelaySubmitted
                )
            })
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_address(byte: u8) -> Address {
        Address::from_bytes([byte; 20])
    }

    #[test]
    fn test_outbound_transfer() {
        let state = BridgeState::new(BridgeConfig::default());
        let sender = test_address(1);

        let transfer = state
            .initiate_outbound(
                sender,
                ExternalChain::Ethereum,
                "0xabc".into(),
                "RCT".into(),
                1000,
                10,
            )
            .unwrap();

        assert_eq!(transfer.status, BridgeTransferStatus::Pending);
        assert_eq!(transfer.amount, 1000);
        assert_eq!(transfer.direction, BridgeDirection::Outbound);

        let fetched = state.get_transfer(&transfer.id).unwrap();
        assert_eq!(fetched.id, transfer.id);
    }

    #[test]
    fn test_inbound_transfer() {
        let state = BridgeState::new(BridgeConfig::default());
        let recipient = test_address(2);

        let transfer = state
            .record_inbound(
                ExternalChain::Bitcoin,
                "bc1q...".into(),
                recipient,
                "BTC".into(),
                50000,
                50,
                "0xdeadbeef".into(),
            )
            .unwrap();

        assert_eq!(transfer.status, BridgeTransferStatus::SourceConfirmed);
        assert_eq!(transfer.direction, BridgeDirection::Inbound);
    }

    #[test]
    fn test_confirmations_and_completion() {
        let state = BridgeState::new(BridgeConfig {
            min_signatures: 2,
            ..Default::default()
        });
        let sender = test_address(1);
        let val1 = test_address(10);
        let val2 = test_address(11);

        let transfer = state
            .initiate_outbound(
                sender,
                ExternalChain::Polygon,
                "0xdef".into(),
                "RCT".into(),
                500,
                5,
            )
            .unwrap();

        // First confirmation — not yet validated
        let t = state.add_confirmation(&transfer.id, &val1).unwrap();
        assert_eq!(t.confirmations, 1);
        assert_eq!(t.status, BridgeTransferStatus::Pending);

        // Second confirmation — now validated
        let t = state.add_confirmation(&transfer.id, &val2).unwrap();
        assert_eq!(t.confirmations, 2);
        assert_eq!(t.status, BridgeTransferStatus::Validated);

        // Complete
        let t = state
            .complete_transfer(&transfer.id, "0xfinaltx".into())
            .unwrap();
        assert_eq!(t.status, BridgeTransferStatus::Completed);
    }

    #[test]
    fn test_paused_bridge() {
        let state = BridgeState::new(BridgeConfig::default());
        state.pause();

        let result = state.initiate_outbound(
            test_address(1),
            ExternalChain::Ethereum,
            "0x".into(),
            "RCT".into(),
            100,
            1,
        );
        assert!(matches!(result, Err(BridgeError::BridgePaused)));

        state.unpause();
        let result = state.initiate_outbound(
            test_address(1),
            ExternalChain::Ethereum,
            "0x".into(),
            "RCT".into(),
            100,
            1,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_liquidity_tracking() {
        let state = BridgeState::new(BridgeConfig::default());

        // Lock some RCT for Ethereum
        state
            .initiate_outbound(
                test_address(1),
                ExternalChain::Ethereum,
                "0x".into(),
                "ETH".into(),
                1000,
                10,
            )
            .unwrap();

        let liquidity = state.get_liquidity();
        let eth = liquidity
            .iter()
            .find(|l| l.chain == ExternalChain::Ethereum)
            .unwrap();
        assert_eq!(eth.locked_amount, 1000);
    }
}

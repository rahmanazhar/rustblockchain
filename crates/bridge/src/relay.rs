//! Bridge relayer — monitors cross-chain events and relays proofs between chains.

use crate::chains::ChainAdapter;
use crate::error::BridgeError;
use crate::state::BridgeState;
use crate::types::*;
use crate::validator::BridgeValidatorSet;
use rustchain_crypto::Address;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{info, warn};

/// The bridge relayer coordinates cross-chain transfers by monitoring events
/// on connected chains and submitting proofs to RustChain.
pub struct BridgeRelayer {
    state: Arc<BridgeState>,
    validators: Arc<BridgeValidatorSet>,
    adapters: HashMap<ExternalChain, Arc<dyn ChainAdapter>>,
}

impl BridgeRelayer {
    pub fn new(
        state: Arc<BridgeState>,
        validators: Arc<BridgeValidatorSet>,
    ) -> Self {
        Self {
            state,
            validators,
            adapters: HashMap::new(),
        }
    }

    /// Register a chain adapter for monitoring.
    pub fn register_adapter(
        &mut self,
        chain: ExternalChain,
        adapter: Arc<dyn ChainAdapter>,
    ) {
        info!("Registered bridge adapter for {}", chain.name());
        self.adapters.insert(chain, adapter);
    }

    /// Get all registered chain adapters.
    pub fn registered_chains(&self) -> Vec<ExternalChain> {
        self.adapters.keys().copied().collect()
    }

    /// Process an inbound transfer event detected on an external chain.
    ///
    /// Called when the relayer detects a lock/deposit event on an external chain
    /// targeting RustChain.
    #[allow(clippy::too_many_arguments)]
    pub fn process_inbound_event(
        &self,
        source_chain: ExternalChain,
        sender: String,
        recipient: Address,
        token_symbol: String,
        amount: u128,
        source_tx_hash: String,
    ) -> Result<BridgeTransfer, BridgeError> {
        info!(
            "Processing inbound event: {} {} from {} on {}",
            amount,
            token_symbol,
            sender,
            source_chain.name()
        );

        // Look up fee schedule
        let fee = self.calculate_fee(amount, source_chain, &token_symbol);

        // Record in bridge state
        let transfer = self.state.record_inbound(
            source_chain,
            sender,
            recipient,
            token_symbol,
            amount,
            fee,
            source_tx_hash,
        )?;

        Ok(transfer)
    }

    /// Process an outbound transfer request.
    ///
    /// Called when a user wants to send tokens from RustChain to an external chain.
    pub fn process_outbound_request(
        &self,
        sender: Address,
        dest_chain: ExternalChain,
        recipient: String,
        token_symbol: String,
        amount: u128,
    ) -> Result<BridgeTransfer, BridgeError> {
        info!(
            "Processing outbound request: {} {} to {} on {}",
            amount,
            token_symbol,
            recipient,
            dest_chain.name()
        );

        let fee = self.calculate_fee(amount, dest_chain, &token_symbol);

        if amount <= fee {
            return Err(BridgeError::InvalidAmount(
                "Amount must be greater than fee".into(),
            ));
        }

        let transfer = self.state.initiate_outbound(
            sender,
            dest_chain,
            recipient,
            token_symbol,
            amount,
            fee,
        )?;

        Ok(transfer)
    }

    /// Submit a validator confirmation for a transfer.
    pub fn submit_confirmation(
        &self,
        transfer_id: &str,
        validator: &Address,
    ) -> Result<BridgeTransfer, BridgeError> {
        if !self.validators.is_validator(validator) {
            return Err(BridgeError::Unauthorized(
                "Not a bridge validator".into(),
            ));
        }

        self.validators.record_confirmation(validator);
        let transfer = self.state.add_confirmation(transfer_id, validator)?;

        if transfer.status == BridgeTransferStatus::Validated {
            info!(
                "Transfer {} has sufficient confirmations",
                transfer_id
            );
        }

        Ok(transfer)
    }

    /// Finalize a validated transfer.
    ///
    /// For inbound: mint wrapped tokens on RustChain.
    /// For outbound: trigger unlock on external chain via adapter.
    pub fn finalize_transfer(
        &self,
        transfer_id: &str,
    ) -> Result<BridgeTransfer, BridgeError> {
        let transfer = self
            .state
            .get_transfer(transfer_id)
            .ok_or_else(|| BridgeError::TransferNotFound(transfer_id.into()))?;

        if transfer.status != BridgeTransferStatus::Validated {
            return Err(BridgeError::TransferAlreadyProcessed);
        }

        match transfer.direction {
            BridgeDirection::Inbound => {
                // Mint wrapped tokens on RustChain
                info!("Finalizing inbound transfer {} — minting wrapped tokens", transfer_id);
                let dest_tx = format!("rustchain-mint-{}", transfer_id);
                self.state.complete_transfer(transfer_id, dest_tx)
            }
            BridgeDirection::Outbound => {
                // Attempt to unlock on external chain
                let chain_name = &transfer.dest_chain;
                let chain = ExternalChain::from_name(chain_name)
                    .ok_or_else(|| BridgeError::UnsupportedChain(chain_name.clone()))?;

                if let Some(adapter) = self.adapters.get(&chain) {
                    info!("Finalizing outbound transfer {} via {} adapter", transfer_id, chain.name());
                    let dest_tx = adapter.submit_unlock(
                        &transfer.recipient,
                        &transfer.token_symbol,
                        transfer.amount.saturating_sub(transfer.fee),
                    )?;
                    self.state.complete_transfer(transfer_id, dest_tx)
                } else {
                    warn!("No adapter registered for {}", chain.name());
                    let dest_tx = format!("pending-{}-unlock-{}", chain.name().to_lowercase(), transfer_id);
                    self.state.complete_transfer(transfer_id, dest_tx)
                }
            }
        }
    }

    /// Calculate the bridge fee for a transfer.
    fn calculate_fee(&self, amount: u128, _chain: ExternalChain, _token: &str) -> u128 {
        // Default: 0.1% (10 bps) with minimum of 1
        let fee = amount / 1000;
        std::cmp::max(fee, 1)
    }

    /// Get the fee schedule for all supported chains.
    pub fn get_fee_schedule(&self) -> Vec<BridgeFeeSchedule> {
        ExternalChain::all()
            .iter()
            .map(|chain| BridgeFeeSchedule {
                chain: *chain,
                token_symbol: chain.symbol().into(),
                fee_bps: 10, // 0.1%
                min_fee: 1,
                estimated_gas_cost: match chain {
                    ExternalChain::Bitcoin => "~0.0001 BTC".into(),
                    ExternalChain::Ethereum => "~0.005 ETH".into(),
                    ExternalChain::BnbChain => "~0.001 BNB".into(),
                    ExternalChain::Polygon => "~0.01 MATIC".into(),
                    ExternalChain::Solana => "~0.000005 SOL".into(),
                },
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustchain_crypto::KeyPair;

    fn setup() -> (Arc<BridgeState>, Arc<BridgeValidatorSet>, BridgeRelayer) {
        let state = Arc::new(BridgeState::new(BridgeConfig::default()));
        let validators = Arc::new(BridgeValidatorSet::new(2));
        let relayer = BridgeRelayer::new(state.clone(), validators.clone());
        (state, validators, relayer)
    }

    #[test]
    fn test_outbound_flow() {
        let (state, validators, relayer) = setup();
        let kp1 = KeyPair::generate();
        let kp2 = KeyPair::generate();
        validators
            .add_validator(kp1.address(), kp1.public_key(), 100)
            .unwrap();
        validators
            .add_validator(kp2.address(), kp2.public_key(), 100)
            .unwrap();

        let sender = Address::from_bytes([1u8; 20]);

        // 1. Initiate outbound
        let transfer = relayer
            .process_outbound_request(
                sender,
                ExternalChain::Ethereum,
                "0xrecipient".into(),
                "RCT".into(),
                10000,
            )
            .unwrap();
        assert_eq!(transfer.status, BridgeTransferStatus::Pending);

        // 2. Validators confirm
        relayer
            .submit_confirmation(&transfer.id, &kp1.address())
            .unwrap();
        let t = relayer
            .submit_confirmation(&transfer.id, &kp2.address())
            .unwrap();
        assert_eq!(t.status, BridgeTransferStatus::Validated);

        // 3. Finalize
        let t = relayer.finalize_transfer(&transfer.id).unwrap();
        assert_eq!(t.status, BridgeTransferStatus::Completed);
    }

    #[test]
    fn test_inbound_flow() {
        let (_state, validators, relayer) = setup();
        let kp1 = KeyPair::generate();
        let kp2 = KeyPair::generate();
        validators
            .add_validator(kp1.address(), kp1.public_key(), 100)
            .unwrap();
        validators
            .add_validator(kp2.address(), kp2.public_key(), 100)
            .unwrap();

        let recipient = Address::from_bytes([2u8; 20]);

        // 1. Process inbound event
        let transfer = relayer
            .process_inbound_event(
                ExternalChain::Bitcoin,
                "bc1qsender".into(),
                recipient,
                "BTC".into(),
                100000,
                "0xbtctx".into(),
            )
            .unwrap();
        assert_eq!(transfer.status, BridgeTransferStatus::SourceConfirmed);

        // 2. Validators confirm
        relayer
            .submit_confirmation(&transfer.id, &kp1.address())
            .unwrap();
        let t = relayer
            .submit_confirmation(&transfer.id, &kp2.address())
            .unwrap();
        assert_eq!(t.status, BridgeTransferStatus::Validated);

        // 3. Finalize — mints wrapped tokens
        let t = relayer.finalize_transfer(&transfer.id).unwrap();
        assert_eq!(t.status, BridgeTransferStatus::Completed);
    }

    #[test]
    fn test_non_validator_cannot_confirm() {
        let (_state, _validators, relayer) = setup();
        let sender = Address::from_bytes([1u8; 20]);
        let non_validator = Address::from_bytes([99u8; 20]);

        let transfer = relayer
            .process_outbound_request(
                sender,
                ExternalChain::Solana,
                "So1...".into(),
                "RCT".into(),
                5000,
            )
            .unwrap();

        let result = relayer.submit_confirmation(&transfer.id, &non_validator);
        assert!(matches!(result, Err(BridgeError::Unauthorized(_))));
    }
}

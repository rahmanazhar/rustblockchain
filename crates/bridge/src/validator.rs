//! Bridge validator set — manages the multi-sig validators that authorize
//! cross-chain transfers.

use crate::error::BridgeError;
use dashmap::DashMap;
use rustchain_crypto::{Address, PublicKey};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Information about a bridge validator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeValidator {
    pub address: Address,
    pub public_key: PublicKey,
    /// Stake weight (higher = more influence)
    pub weight: u64,
    pub is_active: bool,
    /// Number of transfers this validator has confirmed
    pub confirmations_count: u64,
    /// Timestamp when this validator was added
    pub added_at: u64,
}

/// Manages the set of bridge validators.
pub struct BridgeValidatorSet {
    validators: Arc<DashMap<Address, BridgeValidator>>,
    /// Minimum number of validators required to approve a transfer
    min_signatures: usize,
    /// Total weight of all active validators
    total_weight: Arc<parking_lot::RwLock<u64>>,
}

impl BridgeValidatorSet {
    pub fn new(min_signatures: usize) -> Self {
        Self {
            validators: Arc::new(DashMap::new()),
            min_signatures,
            total_weight: Arc::new(parking_lot::RwLock::new(0)),
        }
    }

    /// Add a validator to the bridge set.
    pub fn add_validator(
        &self,
        address: Address,
        public_key: PublicKey,
        weight: u64,
    ) -> Result<(), BridgeError> {
        let now = chrono::Utc::now().timestamp_millis() as u64;
        let validator = BridgeValidator {
            address,
            public_key,
            weight,
            is_active: true,
            confirmations_count: 0,
            added_at: now,
        };

        self.validators.insert(address, validator);
        *self.total_weight.write() += weight;
        Ok(())
    }

    /// Remove a validator from the bridge set.
    pub fn remove_validator(&self, address: &Address) -> Result<(), BridgeError> {
        if let Some((_, val)) = self.validators.remove(address) {
            *self.total_weight.write() -= val.weight;
        }
        Ok(())
    }

    /// Check if an address is an active bridge validator.
    pub fn is_validator(&self, address: &Address) -> bool {
        self.validators
            .get(address)
            .map(|v| v.is_active)
            .unwrap_or(false)
    }

    /// Get the minimum signatures required.
    pub fn min_signatures(&self) -> usize {
        self.min_signatures
    }

    /// Get total active validator count.
    pub fn active_count(&self) -> usize {
        self.validators.iter().filter(|v| v.is_active).count()
    }

    /// List all validators.
    pub fn list_validators(&self) -> Vec<BridgeValidator> {
        self.validators.iter().map(|v| v.value().clone()).collect()
    }

    /// Get a validator by address.
    pub fn get_validator(&self, address: &Address) -> Option<BridgeValidator> {
        self.validators.get(address).map(|v| v.clone())
    }

    /// Verify that a set of validator addresses meets the threshold.
    pub fn verify_threshold(&self, signers: &[Address]) -> Result<(), BridgeError> {
        let valid_count = signers
            .iter()
            .filter(|addr| self.is_validator(addr))
            .count();

        if valid_count < self.min_signatures {
            return Err(BridgeError::InsufficientSignatures {
                needed: self.min_signatures,
                got: valid_count,
            });
        }

        Ok(())
    }

    /// Record a confirmation by a validator.
    pub fn record_confirmation(&self, address: &Address) {
        if let Some(mut val) = self.validators.get_mut(address) {
            val.confirmations_count += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustchain_crypto::KeyPair;

    #[test]
    fn test_add_and_list_validators() {
        let set = BridgeValidatorSet::new(2);
        let kp1 = KeyPair::generate();
        let kp2 = KeyPair::generate();

        set.add_validator(kp1.address(), kp1.public_key(), 100)
            .unwrap();
        set.add_validator(kp2.address(), kp2.public_key(), 100)
            .unwrap();

        assert_eq!(set.active_count(), 2);
        assert!(set.is_validator(&kp1.address()));
        assert!(set.is_validator(&kp2.address()));
    }

    #[test]
    fn test_threshold_verification() {
        let set = BridgeValidatorSet::new(2);
        let kp1 = KeyPair::generate();
        let kp2 = KeyPair::generate();
        let kp3 = KeyPair::generate();

        set.add_validator(kp1.address(), kp1.public_key(), 100)
            .unwrap();
        set.add_validator(kp2.address(), kp2.public_key(), 100)
            .unwrap();

        // 2 valid signers — should pass
        assert!(set
            .verify_threshold(&[kp1.address(), kp2.address()])
            .is_ok());

        // 1 valid signer — should fail
        assert!(set.verify_threshold(&[kp1.address()]).is_err());

        // Unknown signer doesn't count
        assert!(set
            .verify_threshold(&[kp1.address(), kp3.address()])
            .is_err());
    }

    #[test]
    fn test_remove_validator() {
        let set = BridgeValidatorSet::new(1);
        let kp = KeyPair::generate();

        set.add_validator(kp.address(), kp.public_key(), 100)
            .unwrap();
        assert!(set.is_validator(&kp.address()));

        set.remove_validator(&kp.address()).unwrap();
        assert!(!set.is_validator(&kp.address()));
        assert_eq!(set.active_count(), 0);
    }
}

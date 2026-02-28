use crate::types::*;
use rustchain_crypto::{Address, PublicKey};
use serde::{Deserialize, Serialize};

/// Information about a validator.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ValidatorInfo {
    pub address: Address,
    pub public_key: PublicKey,
    pub stake: Balance,
    pub is_active: bool,
    pub commission_rate: u16,
    pub jailed_until: Option<Timestamp>,
    pub slash_count: u32,
    pub uptime_blocks: u64,
    pub total_blocks: u64,
}

/// The set of validators for a given epoch.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ValidatorSet {
    pub validators: Vec<ValidatorInfo>,
    pub total_stake: Balance,
    pub min_stake: Balance,
    pub max_validators: u32,
}

impl ValidatorInfo {
    pub fn new(address: Address, public_key: PublicKey, stake: Balance) -> Self {
        Self {
            address,
            public_key,
            stake,
            is_active: true,
            commission_rate: 0,
            jailed_until: None,
            slash_count: 0,
            uptime_blocks: 0,
            total_blocks: 0,
        }
    }

    pub fn is_jailed(&self, current_time: Timestamp) -> bool {
        self.jailed_until.is_some_and(|until| current_time < until)
    }
}

impl ValidatorSet {
    pub fn new(validators: Vec<ValidatorInfo>, min_stake: Balance, max_validators: u32) -> Self {
        let total_stake = validators.iter().filter(|v| v.is_active).map(|v| v.stake).sum();
        Self {
            validators,
            total_stake,
            min_stake,
            max_validators,
        }
    }

    /// Get all active (non-jailed) validators.
    pub fn active_validators(&self) -> Vec<&ValidatorInfo> {
        self.validators.iter().filter(|v| v.is_active && v.jailed_until.is_none()).collect()
    }

    /// Look up a validator by address.
    pub fn get_validator(&self, addr: &Address) -> Option<&ValidatorInfo> {
        self.validators.iter().find(|v| v.address == *addr)
    }

    /// Get a mutable reference to a validator by address.
    pub fn get_validator_mut(&mut self, addr: &Address) -> Option<&mut ValidatorInfo> {
        self.validators.iter_mut().find(|v| v.address == *addr)
    }

    /// Total stake of active validators.
    pub fn total_active_stake(&self) -> Balance {
        self.active_validators().iter().map(|v| v.stake).sum()
    }

    /// Whether an address is an active validator.
    pub fn is_validator(&self, addr: &Address) -> bool {
        self.active_validators().iter().any(|v| v.address == *addr)
    }

    /// 2/3 quorum threshold of active stake.
    pub fn quorum_threshold(&self) -> Balance {
        let total = self.total_active_stake();
        (total * 2).div_ceil(3) // Ceiling division
    }

    /// Recalculate total stake.
    pub fn recalculate_total_stake(&mut self) {
        self.total_stake = self.validators.iter().filter(|v| v.is_active).map(|v| v.stake).sum();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustchain_crypto::KeyPair;

    fn make_validator(stake: Balance) -> ValidatorInfo {
        let kp = KeyPair::generate();
        ValidatorInfo::new(kp.address(), kp.public_key(), stake)
    }

    #[test]
    fn test_quorum_threshold() {
        let validators = vec![
            make_validator(100),
            make_validator(200),
            make_validator(300),
        ];
        let set = ValidatorSet::new(validators, 10, 100);
        // Total = 600, 2/3 = 400
        assert_eq!(set.total_active_stake(), 600);
        assert_eq!(set.quorum_threshold(), 400); // ceil(600*2/3) = 400
    }

    #[test]
    fn test_active_validators() {
        let mut v1 = make_validator(100);
        let v2 = make_validator(200);
        v1.is_active = false;
        let set = ValidatorSet::new(vec![v1, v2], 10, 100);
        assert_eq!(set.active_validators().len(), 1);
    }

    #[test]
    fn test_jailed_validator() {
        let mut v = make_validator(100);
        v.jailed_until = Some(5000);
        assert!(v.is_jailed(4000));
        assert!(!v.is_jailed(5000));
        assert!(!v.is_jailed(6000));
    }
}

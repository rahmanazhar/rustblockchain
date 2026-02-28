use crate::error::ConsensusError;
use rustchain_core::{Balance, BlockNumber, ConsensusParams, Timestamp, ValidatorInfo};
use rustchain_crypto::Blake3Hash;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

/// Reason for slashing a validator.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SlashReason {
    /// Validator signed two different blocks at the same height.
    DoubleSign {
        block_a: Blake3Hash,
        block_b: Blake3Hash,
        height: BlockNumber,
    },
    /// Validator was offline for too many consecutive blocks.
    Downtime {
        missed_blocks: u64,
        window: u64,
    },
}

/// The penalty resulting from a slash.
#[derive(Debug, Clone)]
pub struct SlashPenalty {
    pub amount: Balance,
    pub jail: bool,
    pub jail_duration_ms: u64,
}

/// Slashing engine for punishing misbehaving validators.
pub struct Slasher {
    params: ConsensusParams,
}

impl Slasher {
    pub fn new(params: ConsensusParams) -> Self {
        Self { params }
    }

    /// Evaluate evidence and compute the penalty.
    pub fn evaluate_evidence(
        &self,
        reason: &SlashReason,
        validator: &ValidatorInfo,
    ) -> Option<SlashPenalty> {
        match reason {
            SlashReason::DoubleSign { .. } => {
                let fraction = self.params.slash_fraction_double_sign as u128;
                let amount = (validator.stake * fraction) / 10_000;
                Some(SlashPenalty {
                    amount,
                    jail: true,
                    jail_duration_ms: self.params.downtime_jail_duration_ms * 10, // 10x for double sign
                })
            }
            SlashReason::Downtime { missed_blocks, window: _ } => {
                if *missed_blocks > self.params.max_missed_blocks {
                    let fraction = self.params.slash_fraction_downtime as u128;
                    let amount = (validator.stake * fraction) / 10_000;
                    Some(SlashPenalty {
                        amount,
                        jail: true,
                        jail_duration_ms: self.params.downtime_jail_duration_ms,
                    })
                } else {
                    None
                }
            }
        }
    }

    /// Apply the slash penalty to a validator.
    pub fn apply_slash(
        &self,
        validator: &mut ValidatorInfo,
        penalty: &SlashPenalty,
        current_time: Timestamp,
    ) -> Result<(), ConsensusError> {
        // Reduce stake
        let slash_amount = penalty.amount.min(validator.stake);
        validator.stake -= slash_amount;
        validator.slash_count += 1;

        info!(
            "Slashed validator {} for {} tokens (slash #{})",
            validator.address, slash_amount, validator.slash_count
        );

        // Jail if required
        if penalty.jail {
            validator.jailed_until = Some(current_time + penalty.jail_duration_ms);
            validator.is_active = false;
            info!(
                "Jailed validator {} until {}",
                validator.address,
                current_time + penalty.jail_duration_ms
            );
        }

        // Deactivate if stake falls below minimum
        if validator.stake < self.params.min_stake {
            validator.is_active = false;
            warn!(
                "Validator {} deactivated: stake {} below minimum {}",
                validator.address, validator.stake, self.params.min_stake
            );
        }

        Ok(())
    }

    /// Detect double signing from two competing block headers at the same height.
    pub fn detect_double_sign(
        &self,
        hash_a: &Blake3Hash,
        hash_b: &Blake3Hash,
        height: BlockNumber,
    ) -> Option<SlashReason> {
        if hash_a != hash_b {
            Some(SlashReason::DoubleSign {
                block_a: *hash_a,
                block_b: *hash_b,
                height,
            })
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustchain_crypto::{hash, KeyPair};

    fn make_slasher() -> Slasher {
        Slasher::new(ConsensusParams::default())
    }

    fn make_validator(stake: Balance) -> ValidatorInfo {
        let kp = KeyPair::generate();
        ValidatorInfo::new(kp.address(), kp.public_key(), stake)
    }

    #[test]
    fn test_double_sign_slash() {
        let slasher = make_slasher();
        let mut validator = make_validator(1_000_000_000_000_000_000);

        let reason = SlashReason::DoubleSign {
            block_a: hash(b"block_a"),
            block_b: hash(b"block_b"),
            height: 100,
        };

        let penalty = slasher.evaluate_evidence(&reason, &validator).unwrap();
        assert!(penalty.jail);
        assert!(penalty.amount > 0);

        let original_stake = validator.stake;
        slasher.apply_slash(&mut validator, &penalty, 1000).unwrap();
        assert!(validator.stake < original_stake);
        assert!(validator.jailed_until.is_some());
        assert_eq!(validator.slash_count, 1);
    }

    #[test]
    fn test_downtime_below_threshold() {
        let slasher = make_slasher();
        let validator = make_validator(1_000_000_000_000_000_000);

        let reason = SlashReason::Downtime {
            missed_blocks: 10, // Below default threshold of 50
            window: 100,
        };

        assert!(slasher.evaluate_evidence(&reason, &validator).is_none());
    }

    #[test]
    fn test_detect_double_sign() {
        let slasher = make_slasher();
        let h1 = hash(b"block1");
        let h2 = hash(b"block2");

        assert!(slasher.detect_double_sign(&h1, &h2, 100).is_some());
        assert!(slasher.detect_double_sign(&h1, &h1, 100).is_none());
    }
}

use crate::error::ConsensusError;
use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;
use rustchain_core::{Balance, BlockNumber, ValidatorInfo};
use rustchain_crypto::{hash_multiple, Address, Blake3Hash};

/// Weighted random validator selection for PoS.
pub struct ProposerSelection;

impl ProposerSelection {
    /// Select the block proposer for a given block height.
    /// Uses deterministic randomness derived from parent_hash + block_number.
    /// Selection probability is proportional to stake.
    pub fn select_proposer(
        active_validators: &[&ValidatorInfo],
        parent_hash: &Blake3Hash,
        block_number: BlockNumber,
    ) -> Result<Address, ConsensusError> {
        if active_validators.is_empty() {
            return Err(ConsensusError::Epoch(
                "no active validators".to_string(),
            ));
        }

        if active_validators.len() == 1 {
            return Ok(active_validators[0].address);
        }

        // Generate deterministic seed
        let seed = hash_multiple(&[
            parent_hash.as_bytes(),
            &block_number.to_le_bytes(),
            b"proposer_selection",
        ]);

        let mut rng = ChaCha20Rng::from_seed(*seed.as_bytes());

        // Weighted selection based on stake
        let total_stake: Balance = active_validators.iter().map(|v| v.stake).sum();
        if total_stake == 0 {
            return Err(ConsensusError::Epoch("total stake is zero".to_string()));
        }

        // Generate a random number in [0, total_stake)
        use rand::Rng;
        let target: u128 = rng.gen_range(0..total_stake);

        let mut cumulative: Balance = 0;
        for validator in active_validators {
            cumulative += validator.stake;
            if target < cumulative {
                return Ok(validator.address);
            }
        }

        // Fallback (should not reach here)
        Ok(active_validators.last().unwrap().address)
    }

    /// Verify that a given address is the correct proposer for a block.
    pub fn verify_proposer(
        proposer: &Address,
        active_validators: &[&ValidatorInfo],
        parent_hash: &Blake3Hash,
        block_number: BlockNumber,
    ) -> Result<(), ConsensusError> {
        let expected = Self::select_proposer(active_validators, parent_hash, block_number)?;
        if *proposer != expected {
            return Err(ConsensusError::WrongProposer {
                expected: expected.to_hex(),
                got: proposer.to_hex(),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustchain_crypto::{hash, KeyPair};

    fn make_validators(stakes: &[Balance]) -> Vec<ValidatorInfo> {
        stakes
            .iter()
            .map(|&stake| {
                let kp = KeyPair::generate();
                ValidatorInfo::new(kp.address(), kp.public_key(), stake)
            })
            .collect()
    }

    #[test]
    fn test_single_validator() {
        let validators = make_validators(&[1000]);
        let refs: Vec<&ValidatorInfo> = validators.iter().collect();
        let parent = hash(b"parent");
        let proposer = ProposerSelection::select_proposer(&refs, &parent, 1).unwrap();
        assert_eq!(proposer, validators[0].address);
    }

    #[test]
    fn test_deterministic_selection() {
        let validators = make_validators(&[100, 200, 300]);
        let refs: Vec<&ValidatorInfo> = validators.iter().collect();
        let parent = hash(b"parent");

        let p1 = ProposerSelection::select_proposer(&refs, &parent, 1).unwrap();
        let p2 = ProposerSelection::select_proposer(&refs, &parent, 1).unwrap();
        assert_eq!(p1, p2);
    }

    #[test]
    fn test_different_blocks_different_proposers() {
        let validators = make_validators(&[100, 100, 100]);
        let refs: Vec<&ValidatorInfo> = validators.iter().collect();
        let parent = hash(b"parent");

        // Over many blocks, different validators should be selected
        let mut proposers = std::collections::HashSet::new();
        for i in 0..100 {
            let p = ProposerSelection::select_proposer(&refs, &parent, i).unwrap();
            proposers.insert(p);
        }
        // With 3 equal-stake validators over 100 blocks, we should see all of them
        assert!(proposers.len() > 1);
    }

    #[test]
    fn test_empty_validators() {
        let refs: Vec<&ValidatorInfo> = vec![];
        let parent = hash(b"parent");
        assert!(ProposerSelection::select_proposer(&refs, &parent, 1).is_err());
    }

    #[test]
    fn test_weighted_selection_bias() {
        // Validator with 90% of stake should be selected most of the time
        let validators = make_validators(&[9000, 500, 500]);
        let refs: Vec<&ValidatorInfo> = validators.iter().collect();
        let parent = hash(b"test");

        let mut counts = [0u32; 3];
        for i in 0..1000 {
            let p = ProposerSelection::select_proposer(&refs, &parent, i).unwrap();
            for (j, v) in validators.iter().enumerate() {
                if p == v.address {
                    counts[j] += 1;
                }
            }
        }

        // First validator should have significantly more selections
        assert!(counts[0] > counts[1] + counts[2]);
    }
}

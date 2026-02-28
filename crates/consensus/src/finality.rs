use crate::error::ConsensusError;
use dashmap::DashMap;
use rustchain_core::{Balance, BlockNumber, Timestamp, ValidatorSet};
use rustchain_crypto::{Address, Blake3Hash, KeyPair, PublicKey, Signature};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use parking_lot::RwLock;
use tracing::info;

/// A finality vote from a validator.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FinalityVote {
    pub block_number: BlockNumber,
    pub block_hash: Blake3Hash,
    pub validator: Address,
    pub validator_pubkey: PublicKey,
    pub signature: Signature,
    pub timestamp: Timestamp,
}

impl FinalityVote {
    /// Create a new finality vote.
    pub fn new(block_number: BlockNumber, block_hash: Blake3Hash, keypair: &KeyPair) -> Self {
        let mut vote_data = Vec::new();
        vote_data.extend_from_slice(&block_number.to_le_bytes());
        vote_data.extend_from_slice(block_hash.as_bytes());
        vote_data.extend_from_slice(b"finality_vote");

        let signature = keypair.sign(&vote_data);

        Self {
            block_number,
            block_hash,
            validator: keypair.address(),
            validator_pubkey: keypair.public_key(),
            signature,
            timestamp: chrono::Utc::now().timestamp_millis() as u64,
        }
    }

    /// Verify this vote's signature.
    pub fn verify(&self) -> Result<(), ConsensusError> {
        let mut vote_data = Vec::new();
        vote_data.extend_from_slice(&self.block_number.to_le_bytes());
        vote_data.extend_from_slice(self.block_hash.as_bytes());
        vote_data.extend_from_slice(b"finality_vote");

        self.validator_pubkey
            .verify(&vote_data, &self.signature)
            .map_err(|_| ConsensusError::Finality("invalid vote signature".to_string()))
    }
}

/// Status of finality for a block.
#[derive(Debug, Clone)]
pub enum FinalityStatus {
    Pending { voted_stake: Balance, needed: Balance },
    Finalized,
}

/// BFT-inspired finality engine.
/// A block is finalized when 2/3+ of active stake votes for it.
pub struct FinalityEngine {
    votes: DashMap<BlockNumber, Vec<FinalityVote>>,
    finalized_height: AtomicU64,
    validator_set: Arc<RwLock<ValidatorSet>>,
}

impl FinalityEngine {
    pub fn new(validator_set: Arc<RwLock<ValidatorSet>>) -> Self {
        Self {
            votes: DashMap::new(),
            finalized_height: AtomicU64::new(0),
            validator_set,
        }
    }

    /// Add a vote and check if quorum is reached.
    pub fn add_vote(&self, vote: FinalityVote) -> Result<FinalityStatus, ConsensusError> {
        // Verify vote signature
        vote.verify()?;

        // Check that the voter is a known active validator
        let vs = self.validator_set.read();
        if !vs.is_validator(&vote.validator) {
            return Err(ConsensusError::Finality(format!(
                "voter {} is not an active validator",
                vote.validator
            )));
        }
        drop(vs);

        // Don't accept votes for already-finalized blocks
        if vote.block_number <= self.finalized_height.load(Ordering::Relaxed) {
            return Ok(FinalityStatus::Finalized);
        }

        // Add vote
        self.votes
            .entry(vote.block_number)
            .or_default()
            .push(vote.clone());

        // Check quorum
        self.check_quorum(vote.block_number)
    }

    fn check_quorum(&self, block_number: BlockNumber) -> Result<FinalityStatus, ConsensusError> {
        let vs = self.validator_set.read();
        let threshold = vs.quorum_threshold();

        let voted_stake: Balance = if let Some(votes) = self.votes.get(&block_number) {
            votes
                .iter()
                .filter_map(|v| vs.get_validator(&v.validator).map(|vi| vi.stake))
                .sum()
        } else {
            return Ok(FinalityStatus::Pending {
                voted_stake: 0,
                needed: threshold,
            });
        };

        if voted_stake >= threshold {
            // Block is finalized
            let current = self.finalized_height.load(Ordering::Relaxed);
            if block_number > current {
                self.finalized_height.store(block_number, Ordering::Relaxed);
            }

            // Clean up old votes
            let to_remove: Vec<BlockNumber> = self
                .votes
                .iter()
                .filter(|e| *e.key() <= block_number)
                .map(|e| *e.key())
                .collect();
            for bn in to_remove {
                self.votes.remove(&bn);
            }

            info!("Block {} finalized with stake {}/{}", block_number, voted_stake, threshold);
            Ok(FinalityStatus::Finalized)
        } else {
            Ok(FinalityStatus::Pending {
                voted_stake,
                needed: threshold,
            })
        }
    }

    /// Check if a block is finalized.
    pub fn is_finalized(&self, block_number: BlockNumber) -> bool {
        block_number <= self.finalized_height.load(Ordering::Relaxed)
    }

    /// Get the latest finalized block height.
    pub fn finalized_height(&self) -> BlockNumber {
        self.finalized_height.load(Ordering::Relaxed)
    }

    /// Create a finality vote for a block.
    pub fn create_vote(
        &self,
        block_number: BlockNumber,
        block_hash: Blake3Hash,
        keypair: &KeyPair,
    ) -> FinalityVote {
        FinalityVote::new(block_number, block_hash, keypair)
    }

    /// Update the validator set (e.g., on epoch change).
    pub fn update_validator_set(&self, vs: ValidatorSet) {
        *self.validator_set.write() = vs;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustchain_core::ValidatorInfo;
    use rustchain_crypto::hash;

    fn make_finality_engine(num_validators: usize) -> (FinalityEngine, Vec<KeyPair>) {
        let keypairs: Vec<KeyPair> = (0..num_validators).map(|_| KeyPair::generate()).collect();
        let validators: Vec<ValidatorInfo> = keypairs
            .iter()
            .map(|kp| ValidatorInfo::new(kp.address(), kp.public_key(), 100))
            .collect();
        let vs = ValidatorSet::new(validators, 10, 100);
        let engine = FinalityEngine::new(Arc::new(RwLock::new(vs)));
        (engine, keypairs)
    }

    #[test]
    fn test_finality_with_quorum() {
        let (engine, keypairs) = make_finality_engine(3);
        let block_hash = hash(b"block1");

        // 2 out of 3 should reach quorum (2/3 of 300 = 200)
        let vote1 = FinalityVote::new(1, block_hash, &keypairs[0]);
        let status = engine.add_vote(vote1).unwrap();
        assert!(matches!(status, FinalityStatus::Pending { .. }));

        let vote2 = FinalityVote::new(1, block_hash, &keypairs[1]);
        let status = engine.add_vote(vote2).unwrap();
        assert!(matches!(status, FinalityStatus::Finalized));

        assert!(engine.is_finalized(1));
        assert_eq!(engine.finalized_height(), 1);
    }

    #[test]
    fn test_single_vote_not_enough() {
        let (engine, keypairs) = make_finality_engine(3);
        let block_hash = hash(b"block1");

        let vote = FinalityVote::new(1, block_hash, &keypairs[0]);
        let status = engine.add_vote(vote).unwrap();
        assert!(matches!(status, FinalityStatus::Pending { .. }));
        assert!(!engine.is_finalized(1));
    }

    #[test]
    fn test_invalid_voter() {
        let (engine, _) = make_finality_engine(3);
        let block_hash = hash(b"block1");
        let rogue = KeyPair::generate();

        let vote = FinalityVote::new(1, block_hash, &rogue);
        assert!(engine.add_vote(vote).is_err());
    }
}

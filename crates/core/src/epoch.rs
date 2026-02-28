use crate::types::*;
use crate::validator::ValidatorSet;
use serde::{Deserialize, Serialize};

/// Information about a consensus epoch.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EpochInfo {
    pub epoch_number: EpochNumber,
    pub start_block: BlockNumber,
    pub end_block: BlockNumber,
    pub validator_set: ValidatorSet,
    pub finalized_block: Option<BlockNumber>,
}

impl EpochInfo {
    pub fn contains_block(&self, block_number: BlockNumber) -> bool {
        block_number >= self.start_block && block_number <= self.end_block
    }

    pub fn is_last_block(&self, block_number: BlockNumber) -> bool {
        block_number == self.end_block
    }
}

use crate::error::CoreError;
use crate::transaction::SignedTransaction;
use crate::types::*;
use rustchain_crypto::{
    compute_merkle_root, hash, Address, Blake3Hash, KeyPair, PublicKey, Signature,
};
use serde::{Deserialize, Serialize};

/// Block header containing metadata and state commitments.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockHeader {
    pub version: u32,
    pub chain_id: ChainId,
    pub number: BlockNumber,
    pub timestamp: Timestamp,
    pub parent_hash: Blake3Hash,
    pub state_root: Blake3Hash,
    pub transactions_root: Blake3Hash,
    pub receipts_root: Blake3Hash,
    pub validator: Address,
    pub validator_pubkey: PublicKey,
    pub validator_signature: Signature,
    pub epoch: EpochNumber,
    pub gas_used: Gas,
    pub gas_limit: Gas,
    pub extra_data: Vec<u8>,
}

/// A complete block with header and transactions.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Block {
    pub header: BlockHeader,
    pub transactions: Vec<SignedTransaction>,
}

impl BlockHeader {
    /// Compute the hash of this header (excludes the validator signature).
    pub fn hash(&self) -> Blake3Hash {
        let mut header_for_hash = self.clone();
        header_for_hash.validator_signature = Signature::default();
        let encoded =
            bincode::serialize(&header_for_hash).expect("block header serialization cannot fail");
        hash(&encoded)
    }

    /// Sign this header with the validator's keypair.
    pub fn sign(&mut self, keypair: &KeyPair) {
        let h = self.hash();
        self.validator_signature = keypair.sign(h.as_bytes());
    }

    /// Verify the validator's signature on this header.
    pub fn verify_signature(&self) -> Result<(), CoreError> {
        let h = self.hash();
        self.validator_pubkey
            .verify(h.as_bytes(), &self.validator_signature)
            .map_err(CoreError::InvalidSignature)
    }

    /// Basic structural validation (no state access).
    pub fn validate_basic(&self) -> Result<(), CoreError> {
        if self.version != PROTOCOL_VERSION {
            return Err(CoreError::InvalidBlock(format!(
                "unsupported version: {}",
                self.version
            )));
        }

        if self.extra_data.len() > MAX_EXTRA_DATA_SIZE {
            return Err(CoreError::DataTooLarge {
                max: MAX_EXTRA_DATA_SIZE,
                got: self.extra_data.len(),
            });
        }

        if self.gas_used > self.gas_limit {
            return Err(CoreError::GasLimitExceeded {
                limit: self.gas_limit,
                used: self.gas_used,
            });
        }

        // Verify validator address matches the pubkey
        let derived = self.validator_pubkey.to_address();
        if derived != self.validator {
            return Err(CoreError::InvalidBlock(
                "validator address does not match public key".to_string(),
            ));
        }

        Ok(())
    }
}

impl Block {
    /// Compute the block hash.
    pub fn hash(&self) -> Blake3Hash {
        self.header.hash()
    }

    /// Number of transactions.
    pub fn tx_count(&self) -> usize {
        self.transactions.len()
    }

    /// Compute the Merkle root of transaction hashes.
    pub fn compute_transactions_root(&self) -> Blake3Hash {
        if self.transactions.is_empty() {
            return Blake3Hash::ZERO;
        }
        let tx_hashes: Vec<Blake3Hash> = self.transactions.iter().map(|tx| tx.hash()).collect();
        let data: Vec<&[u8]> = tx_hashes.iter().map(|h| h.as_ref()).collect();
        compute_merkle_root(&data)
    }

    /// Basic validation of the block structure.
    pub fn validate_basic(&self) -> Result<(), CoreError> {
        self.header.validate_basic()?;

        if self.transactions.len() > MAX_BLOCK_TRANSACTIONS {
            return Err(CoreError::InvalidBlock(format!(
                "too many transactions: {} (max {})",
                self.transactions.len(),
                MAX_BLOCK_TRANSACTIONS
            )));
        }

        // Verify transaction root
        let computed_root = self.compute_transactions_root();
        if computed_root != self.header.transactions_root {
            return Err(CoreError::InvalidBlock(
                "transactions root mismatch".to_string(),
            ));
        }

        // Verify all transaction signatures
        for (i, tx) in self.transactions.iter().enumerate() {
            tx.verify().map_err(|e| {
                CoreError::InvalidBlock(format!("invalid transaction {}: {}", i, e))
            })?;
        }

        Ok(())
    }

    /// Create the genesis block.
    pub fn genesis(
        chain_id: ChainId,
        timestamp: Timestamp,
        validator: Address,
        validator_pubkey: PublicKey,
        gas_limit: Gas,
        extra_data: Vec<u8>,
    ) -> Self {
        let header = BlockHeader {
            version: PROTOCOL_VERSION,
            chain_id,
            number: 0,
            timestamp,
            parent_hash: Blake3Hash::ZERO,
            state_root: Blake3Hash::ZERO,
            transactions_root: Blake3Hash::ZERO,
            receipts_root: Blake3Hash::ZERO,
            validator,
            validator_pubkey,
            validator_signature: Signature::default(),
            epoch: 0,
            gas_used: 0,
            gas_limit,
            extra_data,
        };

        Block {
            header,
            transactions: vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_genesis() -> (Block, KeyPair) {
        let kp = KeyPair::generate();
        let block = Block::genesis(
            1,
            1000,
            kp.address(),
            kp.public_key(),
            1_000_000,
            b"genesis".to_vec(),
        );
        (block, kp)
    }

    #[test]
    fn test_genesis_block() {
        let (block, _kp) = make_genesis();
        assert_eq!(block.header.number, 0);
        assert_eq!(block.header.parent_hash, Blake3Hash::ZERO);
        assert_eq!(block.transactions.len(), 0);
    }

    #[test]
    fn test_block_hash_deterministic() {
        let (block, _kp) = make_genesis();
        assert_eq!(block.hash(), block.hash());
    }

    #[test]
    fn test_sign_and_verify_header() {
        let (mut block, kp) = make_genesis();
        block.header.sign(&kp);
        assert!(block.header.verify_signature().is_ok());
    }

    #[test]
    fn test_validate_basic_genesis() {
        let (block, _kp) = make_genesis();
        assert!(block.validate_basic().is_ok());
    }

    #[test]
    fn test_extra_data_too_large() {
        let kp = KeyPair::generate();
        let block = Block::genesis(
            1,
            1000,
            kp.address(),
            kp.public_key(),
            1_000_000,
            vec![0u8; MAX_EXTRA_DATA_SIZE + 1],
        );
        assert!(block.header.validate_basic().is_err());
    }
}

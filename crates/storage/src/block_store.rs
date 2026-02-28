use crate::columns::{cf, meta_keys};
use crate::db::{encode_block_number, ChainDatabase};
use crate::error::StorageError;
use rustchain_core::{Block, BlockNumber};
use rustchain_crypto::Blake3Hash;

impl ChainDatabase {
    /// Store a block.
    pub fn put_block(&self, block: &Block) -> Result<(), StorageError> {
        let number_key = encode_block_number(block.header.number);
        let block_hash = block.hash();

        let block_bytes =
            bincode::serialize(block).map_err(|e| StorageError::Serialization(e.to_string()))?;

        let mut batch = self.new_write_batch();

        // Store block by number
        let blocks_cf = self.cf_handle(cf::BLOCKS)?;
        batch.put_cf(blocks_cf, number_key, &block_bytes);

        // Store block number by hash
        let hashes_cf = self.cf_handle(cf::BLOCK_HASHES)?;
        batch.put_cf(hashes_cf, block_hash.as_bytes(), number_key);

        self.write_batch(batch)
    }

    /// Get a block by number.
    pub fn get_block_by_number(&self, number: BlockNumber) -> Result<Option<Block>, StorageError> {
        let key = encode_block_number(number);
        match self.get_cf(cf::BLOCKS, &key)? {
            Some(bytes) => {
                let block = bincode::deserialize(&bytes)
                    .map_err(|e| StorageError::Deserialization(e.to_string()))?;
                Ok(Some(block))
            }
            None => Ok(None),
        }
    }

    /// Get a block by hash.
    pub fn get_block_by_hash(&self, hash: &Blake3Hash) -> Result<Option<Block>, StorageError> {
        match self.get_cf(cf::BLOCK_HASHES, hash.as_bytes())? {
            Some(number_bytes) => {
                let number = u64::from_be_bytes(
                    number_bytes
                        .try_into()
                        .map_err(|_| StorageError::Deserialization("invalid block number".into()))?,
                );
                self.get_block_by_number(number)
            }
            None => Ok(None),
        }
    }

    /// Get the chain height.
    pub fn get_chain_height(&self) -> Result<Option<BlockNumber>, StorageError> {
        match self.get_cf(cf::META, meta_keys::CHAIN_HEIGHT)? {
            Some(bytes) => {
                let height = bincode::deserialize(&bytes)
                    .map_err(|e| StorageError::Deserialization(e.to_string()))?;
                Ok(Some(height))
            }
            None => Ok(None),
        }
    }

    /// Set the chain height.
    pub fn set_chain_height(&self, height: BlockNumber) -> Result<(), StorageError> {
        let bytes = bincode::serialize(&height)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        self.put_cf(cf::META, meta_keys::CHAIN_HEIGHT, &bytes)
    }

    /// Get the latest block.
    pub fn get_latest_block(&self) -> Result<Option<Block>, StorageError> {
        match self.get_chain_height()? {
            Some(height) => self.get_block_by_number(height),
            None => Ok(None),
        }
    }

    /// Store the genesis hash.
    pub fn set_genesis_hash(&self, hash: &Blake3Hash) -> Result<(), StorageError> {
        self.put_cf(cf::META, meta_keys::GENESIS_HASH, hash.as_bytes())
    }

    /// Get the genesis hash.
    pub fn get_genesis_hash(&self) -> Result<Option<Blake3Hash>, StorageError> {
        match self.get_cf(cf::META, meta_keys::GENESIS_HASH)? {
            Some(bytes) => {
                if bytes.len() != 32 {
                    return Err(StorageError::Deserialization("invalid hash length".into()));
                }
                let mut arr = [0u8; 32];
                arr.copy_from_slice(&bytes);
                Ok(Some(Blake3Hash::from_bytes(arr)))
            }
            None => Ok(None),
        }
    }
}

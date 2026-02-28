use crate::columns::cf;
use crate::db::{encode_block_number, ChainDatabase};
use crate::error::StorageError;
use rocksdb::WriteBatch;
use rustchain_core::{Account, Block, BlockNumber, SignedTransaction, TransactionReceipt};
use rustchain_crypto::Blake3Hash;

/// Atomic batch writer for committing an entire block.
pub struct BlockCommitBatch<'a> {
    batch: WriteBatch,
    db: &'a ChainDatabase,
}

impl<'a> BlockCommitBatch<'a> {
    pub fn new(db: &'a ChainDatabase) -> Self {
        Self {
            batch: WriteBatch::default(),
            db,
        }
    }

    /// Add a block to the batch.
    pub fn put_block(&mut self, block: &Block) -> Result<(), StorageError> {
        let number_key = encode_block_number(block.header.number);
        let block_hash = block.hash();
        let block_bytes =
            bincode::serialize(block).map_err(|e| StorageError::Serialization(e.to_string()))?;

        let blocks_cf = self.db.cf_handle(cf::BLOCKS)?;
        self.batch.put_cf(blocks_cf, number_key, &block_bytes);

        let hashes_cf = self.db.cf_handle(cf::BLOCK_HASHES)?;
        self.batch
            .put_cf(hashes_cf, block_hash.as_bytes(), number_key);

        Ok(())
    }

    /// Add transactions to the batch.
    pub fn put_transactions(
        &mut self,
        txs: &[SignedTransaction],
        block_number: BlockNumber,
    ) -> Result<(), StorageError> {
        let tx_cf = self.db.cf_handle(cf::TRANSACTIONS)?;
        let idx_cf = self.db.cf_handle(cf::TX_INDEX)?;

        for (i, tx) in txs.iter().enumerate() {
            let tx_bytes = bincode::serialize(tx)
                .map_err(|e| StorageError::Serialization(e.to_string()))?;
            self.batch.put_cf(tx_cf, tx.tx_hash.as_bytes(), &tx_bytes);

            let index_data = bincode::serialize(&(block_number, i as u32))
                .map_err(|e| StorageError::Serialization(e.to_string()))?;
            self.batch
                .put_cf(idx_cf, tx.tx_hash.as_bytes(), &index_data);
        }

        Ok(())
    }

    /// Add receipts to the batch.
    pub fn put_receipts(
        &mut self,
        receipts: &[TransactionReceipt],
    ) -> Result<(), StorageError> {
        let cf = self.db.cf_handle(cf::RECEIPTS)?;
        for receipt in receipts {
            let bytes = bincode::serialize(receipt)
                .map_err(|e| StorageError::Serialization(e.to_string()))?;
            self.batch.put_cf(cf, receipt.tx_hash.as_bytes(), &bytes);
        }
        Ok(())
    }

    /// Update accounts in the batch.
    pub fn update_accounts(&mut self, accounts: &[Account]) -> Result<(), StorageError> {
        let cf = self.db.cf_handle(cf::ACCOUNTS)?;
        for account in accounts {
            let bytes = bincode::serialize(account)
                .map_err(|e| StorageError::Serialization(e.to_string()))?;
            self.batch.put_cf(cf, account.address.as_bytes(), &bytes);
        }
        Ok(())
    }

    /// Update chain height in the batch.
    pub fn set_chain_height(&mut self, height: BlockNumber) -> Result<(), StorageError> {
        let cf = self.db.cf_handle(cf::META)?;
        let bytes = bincode::serialize(&height)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        self.batch
            .put_cf(cf, crate::columns::meta_keys::CHAIN_HEIGHT, &bytes);
        Ok(())
    }

    /// Set the best block hash.
    pub fn set_best_block_hash(&mut self, hash: &Blake3Hash) -> Result<(), StorageError> {
        let cf = self.db.cf_handle(cf::META)?;
        self.batch
            .put_cf(cf, crate::columns::meta_keys::BEST_BLOCK_HASH, hash.as_bytes());
        Ok(())
    }

    /// Commit the batch atomically.
    pub fn commit(self) -> Result<(), StorageError> {
        self.db.write_batch(self.batch)
    }
}

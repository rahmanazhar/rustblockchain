use crate::columns::cf;
use crate::db::ChainDatabase;
use crate::error::StorageError;
use rustchain_core::{BlockNumber, SignedTransaction, TransactionReceipt};
use rustchain_crypto::Blake3Hash;

impl ChainDatabase {
    /// Store a signed transaction.
    pub fn put_transaction(&self, tx: &SignedTransaction) -> Result<(), StorageError> {
        let bytes =
            bincode::serialize(tx).map_err(|e| StorageError::Serialization(e.to_string()))?;
        self.put_cf(cf::TRANSACTIONS, tx.tx_hash.as_bytes(), &bytes)
    }

    /// Store the tx index (block number + index within block).
    pub fn put_tx_index(
        &self,
        tx_hash: &Blake3Hash,
        block_number: BlockNumber,
        tx_index: u32,
    ) -> Result<(), StorageError> {
        let index_data = bincode::serialize(&(block_number, tx_index))
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        self.put_cf(cf::TX_INDEX, tx_hash.as_bytes(), &index_data)
    }

    /// Get a transaction by hash.
    pub fn get_transaction(
        &self,
        hash: &Blake3Hash,
    ) -> Result<Option<SignedTransaction>, StorageError> {
        match self.get_cf(cf::TRANSACTIONS, hash.as_bytes())? {
            Some(bytes) => {
                let tx = bincode::deserialize(&bytes)
                    .map_err(|e| StorageError::Deserialization(e.to_string()))?;
                Ok(Some(tx))
            }
            None => Ok(None),
        }
    }

    /// Get the block number and index for a transaction.
    pub fn get_tx_location(
        &self,
        hash: &Blake3Hash,
    ) -> Result<Option<(BlockNumber, u32)>, StorageError> {
        match self.get_cf(cf::TX_INDEX, hash.as_bytes())? {
            Some(bytes) => {
                let location = bincode::deserialize(&bytes)
                    .map_err(|e| StorageError::Deserialization(e.to_string()))?;
                Ok(Some(location))
            }
            None => Ok(None),
        }
    }

    /// Store a transaction receipt.
    pub fn put_receipt(&self, receipt: &TransactionReceipt) -> Result<(), StorageError> {
        let bytes = bincode::serialize(receipt)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        self.put_cf(cf::RECEIPTS, receipt.tx_hash.as_bytes(), &bytes)
    }

    /// Get a transaction receipt by tx hash.
    pub fn get_receipt(
        &self,
        tx_hash: &Blake3Hash,
    ) -> Result<Option<TransactionReceipt>, StorageError> {
        match self.get_cf(cf::RECEIPTS, tx_hash.as_bytes())? {
            Some(bytes) => {
                let receipt = bincode::deserialize(&bytes)
                    .map_err(|e| StorageError::Deserialization(e.to_string()))?;
                Ok(Some(receipt))
            }
            None => Ok(None),
        }
    }
}

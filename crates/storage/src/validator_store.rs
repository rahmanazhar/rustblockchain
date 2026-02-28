use crate::columns::cf;
use crate::db::{encode_block_number, ChainDatabase};
use crate::error::StorageError;
use rustchain_core::{EpochInfo, EpochNumber, ValidatorInfo};
use rustchain_crypto::Address;

impl ChainDatabase {
    /// Store a validator.
    pub fn put_validator(&self, validator: &ValidatorInfo) -> Result<(), StorageError> {
        let bytes = bincode::serialize(validator)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        self.put_cf(cf::VALIDATORS, validator.address.as_bytes(), &bytes)
    }

    /// Get a validator by address.
    pub fn get_validator(
        &self,
        address: &Address,
    ) -> Result<Option<ValidatorInfo>, StorageError> {
        match self.get_cf(cf::VALIDATORS, address.as_bytes())? {
            Some(bytes) => {
                let v = bincode::deserialize(&bytes)
                    .map_err(|e| StorageError::Deserialization(e.to_string()))?;
                Ok(Some(v))
            }
            None => Ok(None),
        }
    }

    /// Store epoch info.
    pub fn put_epoch(&self, epoch: &EpochInfo) -> Result<(), StorageError> {
        let key = encode_block_number(epoch.epoch_number);
        let bytes =
            bincode::serialize(epoch).map_err(|e| StorageError::Serialization(e.to_string()))?;
        self.put_cf(cf::EPOCHS, &key, &bytes)
    }

    /// Get epoch info by number.
    pub fn get_epoch(&self, epoch_number: EpochNumber) -> Result<Option<EpochInfo>, StorageError> {
        let key = encode_block_number(epoch_number);
        match self.get_cf(cf::EPOCHS, &key)? {
            Some(bytes) => {
                let epoch = bincode::deserialize(&bytes)
                    .map_err(|e| StorageError::Deserialization(e.to_string()))?;
                Ok(Some(epoch))
            }
            None => Ok(None),
        }
    }
}

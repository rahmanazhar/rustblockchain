use crate::columns::cf;
use crate::db::{contract_storage_key, ChainDatabase};
use crate::error::StorageError;
use rustchain_core::{Account, Balance, Nonce};
use rustchain_crypto::Address;

impl ChainDatabase {
    /// Get an account by address.
    pub fn get_account(&self, address: &Address) -> Result<Option<Account>, StorageError> {
        match self.get_cf(cf::ACCOUNTS, address.as_bytes())? {
            Some(bytes) => {
                let account = bincode::deserialize(&bytes)
                    .map_err(|e| StorageError::Deserialization(e.to_string()))?;
                Ok(Some(account))
            }
            None => Ok(None),
        }
    }

    /// Store an account.
    pub fn put_account(&self, account: &Account) -> Result<(), StorageError> {
        let bytes = bincode::serialize(account)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        self.put_cf(cf::ACCOUNTS, account.address.as_bytes(), &bytes)
    }

    /// Get account balance (returns 0 for non-existent accounts).
    pub fn get_balance(&self, address: &Address) -> Result<Balance, StorageError> {
        Ok(self.get_account(address)?.map_or(0, |a| a.balance))
    }

    /// Get account nonce (returns 0 for non-existent accounts).
    pub fn get_nonce(&self, address: &Address) -> Result<Nonce, StorageError> {
        Ok(self.get_account(address)?.map_or(0, |a| a.nonce))
    }

    /// Get contract storage value.
    pub fn get_contract_storage(
        &self,
        addr: &Address,
        key: &[u8],
    ) -> Result<Option<Vec<u8>>, StorageError> {
        let composite_key = contract_storage_key(addr.as_bytes(), key);
        self.get_cf(cf::CONTRACT_STORAGE, &composite_key)
    }

    /// Set contract storage value.
    pub fn put_contract_storage(
        &self,
        addr: &Address,
        key: &[u8],
        value: &[u8],
    ) -> Result<(), StorageError> {
        let composite_key = contract_storage_key(addr.as_bytes(), key);
        self.put_cf(cf::CONTRACT_STORAGE, &composite_key, value)
    }

    /// Delete contract storage value.
    pub fn delete_contract_storage(
        &self,
        addr: &Address,
        key: &[u8],
    ) -> Result<(), StorageError> {
        let composite_key = contract_storage_key(addr.as_bytes(), key);
        self.delete_cf(cf::CONTRACT_STORAGE, &composite_key)
    }

    /// Store contract bytecode by its code hash.
    pub fn put_contract_code(
        &self,
        code_hash: &rustchain_crypto::Blake3Hash,
        code: &[u8],
    ) -> Result<(), StorageError> {
        self.put_cf(cf::CONTRACTS, code_hash.as_bytes(), code)
    }

    /// Get contract bytecode by code hash.
    pub fn get_contract_code(
        &self,
        code_hash: &rustchain_crypto::Blake3Hash,
    ) -> Result<Option<Vec<u8>>, StorageError> {
        self.get_cf(cf::CONTRACTS, code_hash.as_bytes())
    }
}

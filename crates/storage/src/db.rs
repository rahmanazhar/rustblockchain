use crate::columns::{cf, meta_keys, CURRENT_SCHEMA_VERSION};
use crate::error::StorageError;
use rocksdb::{ColumnFamilyDescriptor, Options, WriteBatch, DB};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::info;

/// Configuration for the chain database.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    pub path: PathBuf,
    pub cache_size_mb: usize,
    pub max_open_files: i32,
    pub write_buffer_size_mb: usize,
    pub enable_statistics: bool,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            path: PathBuf::from("./data/chaindb"),
            cache_size_mb: 256,
            max_open_files: 512,
            write_buffer_size_mb: 64,
            enable_statistics: false,
        }
    }
}

/// Main database wrapper around RocksDB.
pub struct ChainDatabase {
    db: DB,
    path: PathBuf,
}

impl ChainDatabase {
    /// Open the database with the given configuration.
    pub fn open(config: &StorageConfig) -> Result<Self, StorageError> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);
        opts.set_max_open_files(config.max_open_files);
        opts.set_write_buffer_size(config.write_buffer_size_mb * 1024 * 1024);
        opts.set_max_write_buffer_number(3);
        opts.set_target_file_size_base(64 * 1024 * 1024);
        opts.set_level_compaction_dynamic_level_bytes(true);
        opts.set_bytes_per_sync(1048576);

        let cf_descriptors: Vec<ColumnFamilyDescriptor> = cf::ALL
            .iter()
            .map(|name| {
                let mut cf_opts = Options::default();
                cf_opts.set_write_buffer_size(config.write_buffer_size_mb * 1024 * 1024);
                ColumnFamilyDescriptor::new(*name, cf_opts)
            })
            .collect();

        let db = DB::open_cf_descriptors(&opts, &config.path, cf_descriptors)?;
        let db_instance = Self {
            db,
            path: config.path.clone(),
        };

        // Check schema version
        db_instance.check_or_set_schema_version()?;

        info!("Database opened at {:?}", config.path);
        Ok(db_instance)
    }

    fn check_or_set_schema_version(&self) -> Result<(), StorageError> {
        match self.get_cf(cf::META, meta_keys::SCHEMA_VERSION)? {
            Some(bytes) => {
                let version: u32 = bincode::deserialize(&bytes)
                    .map_err(|e| StorageError::Deserialization(e.to_string()))?;
                if version != CURRENT_SCHEMA_VERSION {
                    return Err(StorageError::SchemaMismatch {
                        expected: CURRENT_SCHEMA_VERSION,
                        actual: version,
                    });
                }
                Ok(())
            }
            None => {
                let bytes = bincode::serialize(&CURRENT_SCHEMA_VERSION)
                    .map_err(|e| StorageError::Serialization(e.to_string()))?;
                self.put_cf(cf::META, meta_keys::SCHEMA_VERSION, &bytes)?;
                Ok(())
            }
        }
    }

    /// Get a value from a column family.
    pub fn get_cf(&self, cf_name: &str, key: &[u8]) -> Result<Option<Vec<u8>>, StorageError> {
        let cf = self
            .db
            .cf_handle(cf_name)
            .ok_or_else(|| StorageError::ColumnFamilyNotFound(cf_name.to_string()))?;
        Ok(self.db.get_cf(&cf, key)?)
    }

    /// Put a key-value pair in a column family.
    pub fn put_cf(
        &self,
        cf_name: &str,
        key: &[u8],
        value: &[u8],
    ) -> Result<(), StorageError> {
        let cf = self
            .db
            .cf_handle(cf_name)
            .ok_or_else(|| StorageError::ColumnFamilyNotFound(cf_name.to_string()))?;
        self.db.put_cf(&cf, key, value)?;
        Ok(())
    }

    /// Delete a key from a column family.
    pub fn delete_cf(&self, cf_name: &str, key: &[u8]) -> Result<(), StorageError> {
        let cf = self
            .db
            .cf_handle(cf_name)
            .ok_or_else(|| StorageError::ColumnFamilyNotFound(cf_name.to_string()))?;
        self.db.delete_cf(&cf, key)?;
        Ok(())
    }

    /// Write a batch atomically.
    pub fn write_batch(&self, batch: WriteBatch) -> Result<(), StorageError> {
        self.db.write(batch)?;
        Ok(())
    }

    /// Get a RocksDB column family handle.
    pub fn cf_handle(&self, name: &str) -> Result<&rocksdb::ColumnFamily, StorageError> {
        self.db
            .cf_handle(name)
            .ok_or_else(|| StorageError::ColumnFamilyNotFound(name.to_string()))
    }

    /// Create a new write batch.
    pub fn new_write_batch(&self) -> WriteBatch {
        WriteBatch::default()
    }

    /// Flush all column families.
    pub fn flush(&self) -> Result<(), StorageError> {
        for cf_name in cf::ALL {
            if let Some(cf) = self.db.cf_handle(cf_name) {
                self.db.flush_cf(&cf)?;
            }
        }
        Ok(())
    }

    /// Get the database path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Get the underlying DB reference (for advanced operations).
    pub fn inner(&self) -> &DB {
        &self.db
    }
}

/// Helper: encode u64 as big-endian bytes (for ordered key storage).
pub fn encode_block_number(n: u64) -> [u8; 8] {
    n.to_be_bytes()
}

/// Helper: decode u64 from big-endian bytes.
pub fn decode_block_number(bytes: &[u8]) -> Result<u64, StorageError> {
    if bytes.len() != 8 {
        return Err(StorageError::Deserialization(format!(
            "expected 8 bytes for block number, got {}",
            bytes.len()
        )));
    }
    let mut arr = [0u8; 8];
    arr.copy_from_slice(bytes);
    Ok(u64::from_be_bytes(arr))
}

/// Helper: build a composite key for contract storage (address + key).
pub fn contract_storage_key(address: &[u8; 20], key: &[u8]) -> Vec<u8> {
    let mut composite = Vec::with_capacity(20 + key.len());
    composite.extend_from_slice(address);
    composite.extend_from_slice(key);
    composite
}

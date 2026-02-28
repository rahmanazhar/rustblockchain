use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("database error: {0}")]
    Database(String),

    #[error("serialization error: {0}")]
    Serialization(String),

    #[error("deserialization error: {0}")]
    Deserialization(String),

    #[error("key not found: {0}")]
    NotFound(String),

    #[error("column family not found: {0}")]
    ColumnFamilyNotFound(String),

    #[error("schema version mismatch: expected {expected}, got {actual}")]
    SchemaMismatch { expected: u32, actual: u32 },

    #[error("corruption detected: {0}")]
    Corruption(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

impl From<rocksdb::Error> for StorageError {
    fn from(e: rocksdb::Error) -> Self {
        StorageError::Database(e.to_string())
    }
}

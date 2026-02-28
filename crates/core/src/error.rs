use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("invalid block: {0}")]
    InvalidBlock(String),

    #[error("invalid transaction: {0}")]
    InvalidTransaction(String),

    #[error("invalid signature: {0}")]
    InvalidSignature(#[from] rustchain_crypto::CryptoError),

    #[error("insufficient balance: have {have}, need {need}")]
    InsufficientBalance { have: u128, need: u128 },

    #[error("invalid nonce: expected {expected}, got {got}")]
    InvalidNonce { expected: u64, got: u64 },

    #[error("gas limit exceeded: limit {limit}, used {used}")]
    GasLimitExceeded { limit: u64, used: u64 },

    #[error("data too large: max {max}, got {got}")]
    DataTooLarge { max: usize, got: usize },

    #[error("genesis error: {0}")]
    Genesis(String),

    #[error("config error: {0}")]
    Config(String),

    #[error("serialization error: {0}")]
    Serialization(String),

    #[error("chain id mismatch: expected {expected}, got {got}")]
    ChainIdMismatch { expected: u64, got: u64 },
}

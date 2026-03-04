use thiserror::Error;

#[derive(Debug, Error)]
pub enum BridgeError {
    #[error("Unsupported chain: {0}")]
    UnsupportedChain(String),

    #[error("Invalid amount: {0}")]
    InvalidAmount(String),

    #[error("HTLC not found: {0}")]
    HtlcNotFound(String),

    #[error("HTLC already exists: {0}")]
    HtlcAlreadyExists(String),

    #[error("HTLC expired")]
    HtlcExpired,

    #[error("HTLC not expired yet")]
    HtlcNotExpired,

    #[error("HTLC already claimed")]
    HtlcAlreadyClaimed,

    #[error("HTLC already refunded")]
    HtlcAlreadyRefunded,

    #[error("Invalid hash preimage")]
    InvalidPreimage,

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Bridge transfer not found: {0}")]
    TransferNotFound(String),

    #[error("Bridge transfer already processed")]
    TransferAlreadyProcessed,

    #[error("Insufficient bridge liquidity")]
    InsufficientLiquidity,

    #[error("Insufficient validator signatures: need {needed}, got {got}")]
    InsufficientSignatures { needed: usize, got: usize },

    #[error("Invalid validator signature")]
    InvalidSignature,

    #[error("Chain adapter error: {0}")]
    AdapterError(String),

    #[error("Bridge paused")]
    BridgePaused,

    #[error("Amount below minimum: min {min}, got {got}")]
    BelowMinimum { min: u128, got: u128 },

    #[error("Amount above maximum: max {max}, got {got}")]
    AboveMaximum { max: u128, got: u128 },

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Serialization error: {0}")]
    Serialization(String),
}

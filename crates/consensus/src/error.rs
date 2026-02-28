use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConsensusError {
    #[error("invalid block: {0}")]
    InvalidBlock(String),

    #[error("block validation failed: {0}")]
    BlockValidation(String),

    #[error("unknown parent block: {0}")]
    UnknownParent(String),

    #[error("wrong proposer: expected {expected}, got {got}")]
    WrongProposer { expected: String, got: String },

    #[error("state transition error: {0}")]
    StateTransition(String),

    #[error("transaction pool error: {0}")]
    TransactionPool(String),

    #[error("transaction execution error: {0}")]
    TransactionExecution(String),

    #[error("finality error: {0}")]
    Finality(String),

    #[error("epoch error: {0}")]
    Epoch(String),

    #[error("slashing error: {0}")]
    Slashing(String),

    #[error("not a validator")]
    NotValidator,

    #[error("storage error: {0}")]
    Storage(#[from] rustchain_storage::StorageError),

    #[error("core error: {0}")]
    Core(#[from] rustchain_core::CoreError),

    #[error("vm error: {0}")]
    Vm(#[from] rustchain_vm::VmError),

    #[error("crypto error: {0}")]
    Crypto(#[from] rustchain_crypto::CryptoError),
}

use thiserror::Error;

#[derive(Debug, Error)]
pub enum VmError {
    #[error("compilation error: {0}")]
    Compilation(String),

    #[error("execution error: {0}")]
    Execution(String),

    #[error("out of gas: limit {limit}, used {used}")]
    OutOfGas { limit: u64, used: u64 },

    #[error("memory access out of bounds: offset {offset}, len {len}, memory_size {memory_size}")]
    MemoryOutOfBounds {
        offset: u32,
        len: u32,
        memory_size: u32,
    },

    #[error("call depth exceeded: max {max}")]
    CallDepthExceeded { max: u32 },

    #[error("code too large: max {max}, got {got}")]
    CodeTooLarge { max: usize, got: usize },

    #[error("invalid module: {0}")]
    InvalidModule(String),

    #[error("function not found: {0}")]
    FunctionNotFound(String),

    #[error("contract not found: {0}")]
    ContractNotFound(String),

    #[error("host function error: {0}")]
    HostFunction(String),

    #[error("trap: {0}")]
    Trap(String),

    #[error("state error: {0}")]
    State(String),
}

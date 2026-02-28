/// Balance in the smallest unit (like wei in Ethereum).
pub type Balance = u128;

/// Transaction nonce for replay protection.
pub type Nonce = u64;

/// Gas unit for computation metering.
pub type Gas = u64;

/// Gas price in the smallest unit per gas.
pub type GasPrice = u64;

/// Block number / height.
pub type BlockNumber = u64;

/// Epoch number for PoS.
pub type EpochNumber = u64;

/// Unix timestamp in milliseconds.
pub type Timestamp = u64;

/// Chain identifier for replay protection.
pub type ChainId = u64;

/// Maximum size of extra_data in block header (256 bytes).
pub const MAX_EXTRA_DATA_SIZE: usize = 256;

/// Maximum transaction data size (512 KB).
pub const MAX_TRANSACTION_DATA_SIZE: usize = 512 * 1024;

/// Maximum transactions per block.
pub const MAX_BLOCK_TRANSACTIONS: usize = 10_000;

/// Protocol version.
pub const PROTOCOL_VERSION: u32 = 1;

/// RocksDB column family names.
pub mod cf {
    /// BlockNumber (u64 BE) -> Block (bincode)
    pub const BLOCKS: &str = "blocks";

    /// Blake3Hash (32 bytes) -> BlockNumber (u64 BE)
    pub const BLOCK_HASHES: &str = "block_hashes";

    /// TxHash (Blake3Hash) -> SignedTransaction (bincode)
    pub const TRANSACTIONS: &str = "transactions";

    /// TxHash -> (BlockNumber, TxIndex) for fast lookup
    pub const TX_INDEX: &str = "tx_index";

    /// Address (20 bytes) -> Account (bincode)
    pub const ACCOUNTS: &str = "accounts";

    /// TxHash -> TransactionReceipt (bincode)
    pub const RECEIPTS: &str = "receipts";

    /// Address -> ValidatorInfo (bincode)
    pub const VALIDATORS: &str = "validators";

    /// EpochNumber (u64 BE) -> EpochInfo (bincode)
    pub const EPOCHS: &str = "epochs";

    /// CodeHash (Blake3Hash) -> WASM bytecode
    pub const CONTRACTS: &str = "contracts";

    /// (Address, key) -> value for contract storage
    pub const CONTRACT_STORAGE: &str = "contract_storage";

    /// String key -> value for chain metadata
    pub const META: &str = "meta";

    pub const ALL: &[&str] = &[
        BLOCKS,
        BLOCK_HASHES,
        TRANSACTIONS,
        TX_INDEX,
        ACCOUNTS,
        RECEIPTS,
        VALIDATORS,
        EPOCHS,
        CONTRACTS,
        CONTRACT_STORAGE,
        META,
    ];
}

// Meta keys
pub mod meta_keys {
    pub const CHAIN_HEIGHT: &[u8] = b"chain_height";
    pub const BEST_BLOCK_HASH: &[u8] = b"best_block_hash";
    pub const SCHEMA_VERSION: &[u8] = b"schema_version";
    pub const GENESIS_HASH: &[u8] = b"genesis_hash";
    pub const CURRENT_EPOCH: &[u8] = b"current_epoch";
}

pub const CURRENT_SCHEMA_VERSION: u32 = 1;

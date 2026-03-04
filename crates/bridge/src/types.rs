use rustchain_crypto::Address;
use serde::{Deserialize, Serialize};

/// Supported external blockchain networks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ExternalChain {
    /// Bitcoin mainnet / testnet
    Bitcoin,
    /// Ethereum (EVM)
    Ethereum,
    /// BNB Smart Chain (EVM)
    BnbChain,
    /// Polygon PoS (EVM)
    Polygon,
    /// Solana
    Solana,
}

impl ExternalChain {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Bitcoin => "Bitcoin",
            Self::Ethereum => "Ethereum",
            Self::BnbChain => "BNB Chain",
            Self::Polygon => "Polygon",
            Self::Solana => "Solana",
        }
    }

    pub fn symbol(&self) -> &'static str {
        match self {
            Self::Bitcoin => "BTC",
            Self::Ethereum => "ETH",
            Self::BnbChain => "BNB",
            Self::Polygon => "MATIC",
            Self::Solana => "SOL",
        }
    }

    pub fn chain_id(&self) -> u64 {
        match self {
            Self::Bitcoin => 0,
            Self::Ethereum => 1,
            Self::BnbChain => 56,
            Self::Polygon => 137,
            Self::Solana => 501,
        }
    }

    pub fn is_evm(&self) -> bool {
        matches!(self, Self::Ethereum | Self::BnbChain | Self::Polygon)
    }

    pub fn address_length(&self) -> usize {
        match self {
            Self::Bitcoin => 34,   // Base58 address
            Self::Ethereum | Self::BnbChain | Self::Polygon => 20, // 20 bytes
            Self::Solana => 32,    // Ed25519 public key
        }
    }

    pub fn all() -> &'static [ExternalChain] {
        &[
            Self::Bitcoin,
            Self::Ethereum,
            Self::BnbChain,
            Self::Polygon,
            Self::Solana,
        ]
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "bitcoin" | "btc" => Some(Self::Bitcoin),
            "ethereum" | "eth" => Some(Self::Ethereum),
            "bnb" | "bnbchain" | "bsc" | "binance" => Some(Self::BnbChain),
            "polygon" | "matic" => Some(Self::Polygon),
            "solana" | "sol" => Some(Self::Solana),
            _ => None,
        }
    }
}

impl std::fmt::Display for ExternalChain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Represents a token that can be bridged.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeToken {
    /// Native symbol on the external chain (e.g., "ETH", "BTC")
    pub symbol: String,
    /// Full name
    pub name: String,
    /// Decimal places
    pub decimals: u8,
    /// The external chain this token originates from
    pub origin_chain: ExternalChain,
    /// Contract address on the external chain (empty for native tokens)
    pub external_address: String,
    /// Whether this token is currently enabled for bridging
    pub enabled: bool,
    /// Minimum bridge amount (in smallest unit)
    pub min_amount: u128,
    /// Maximum bridge amount per transaction
    pub max_amount: u128,
    /// Bridge fee in basis points (1 bp = 0.01%)
    pub fee_bps: u16,
}

/// Direction of a bridge transfer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BridgeDirection {
    /// Incoming: external chain → RustChain (lock on external, mint wrapped on RustChain)
    Inbound,
    /// Outgoing: RustChain → external chain (burn wrapped on RustChain, unlock on external)
    Outbound,
}

/// Status of a bridge transfer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BridgeTransferStatus {
    /// Transfer initiated, waiting for confirmation
    Pending,
    /// Source chain transaction confirmed, waiting for relay
    SourceConfirmed,
    /// Relayer has submitted the proof, waiting for validator signatures
    RelaySubmitted,
    /// Sufficient validator signatures collected
    Validated,
    /// Destination chain transaction executed (tokens minted/unlocked)
    Completed,
    /// Transfer failed or rejected
    Failed,
    /// Transfer expired (no action within timeout)
    Expired,
    /// Transfer was refunded
    Refunded,
}

/// A cross-chain bridge transfer record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeTransfer {
    /// Unique transfer ID (hash of transfer parameters)
    pub id: String,
    /// Direction of transfer
    pub direction: BridgeDirection,
    /// Source chain (where tokens are locked/burned)
    pub source_chain: String,
    /// Destination chain (where tokens are minted/unlocked)
    pub dest_chain: String,
    /// Sender address on source chain
    pub sender: String,
    /// Recipient address on destination chain
    pub recipient: String,
    /// Token being bridged
    pub token_symbol: String,
    /// Amount being transferred (smallest unit)
    pub amount: u128,
    /// Bridge fee deducted
    pub fee: u128,
    /// Current status
    pub status: BridgeTransferStatus,
    /// Source chain transaction hash (when available)
    pub source_tx_hash: Option<String>,
    /// Destination chain transaction hash (when available)
    pub dest_tx_hash: Option<String>,
    /// Number of validator confirmations
    pub confirmations: usize,
    /// Required confirmations
    pub required_confirmations: usize,
    /// Creation timestamp (Unix ms)
    pub created_at: u64,
    /// Last update timestamp
    pub updated_at: u64,
    /// Expiry timestamp
    pub expires_at: u64,
}

/// HTLC (Hash Time-Locked Contract) status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HtlcStatus {
    /// Active — awaiting claim or expiry
    Active,
    /// Claimed — preimage revealed, tokens transferred
    Claimed,
    /// Refunded — expired, tokens returned to sender
    Refunded,
    /// Expired — past timelock, eligible for refund
    Expired,
}

/// A bridge fee schedule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeFeeSchedule {
    pub chain: ExternalChain,
    pub token_symbol: String,
    /// Fee in basis points (100 = 1%)
    pub fee_bps: u16,
    /// Minimum fee in token's smallest unit
    pub min_fee: u128,
    /// Estimated gas cost on the destination chain (informational)
    pub estimated_gas_cost: String,
}

/// Summary of bridge liquidity for a token pair.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeLiquidity {
    pub chain: ExternalChain,
    pub token_symbol: String,
    /// Total locked on RustChain side
    pub locked_amount: u128,
    /// Total wrapped tokens minted
    pub wrapped_supply: u128,
    /// Available for outbound transfers
    pub available_outbound: u128,
}

/// Configuration for the bridge system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeConfig {
    /// Whether the bridge is enabled
    pub enabled: bool,
    /// Minimum number of validator signatures required
    pub min_signatures: usize,
    /// HTLC default timelock in seconds
    pub htlc_timelock_seconds: u64,
    /// Maximum transfer timeout in seconds
    pub transfer_timeout_seconds: u64,
    /// Bridge operator address on RustChain
    pub operator: Address,
    /// Whether to accept inbound transfers
    pub accept_inbound: bool,
    /// Whether to allow outbound transfers
    pub allow_outbound: bool,
}

impl Default for BridgeConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            min_signatures: 2,
            htlc_timelock_seconds: 3600,     // 1 hour
            transfer_timeout_seconds: 86400,  // 24 hours
            operator: Address::ZERO,
            accept_inbound: true,
            allow_outbound: true,
        }
    }
}

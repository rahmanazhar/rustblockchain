//! RustChain Cross-Chain Bridge
//!
//! Provides cross-chain interoperability with Bitcoin, Ethereum, BNB Chain,
//! Polygon, and Solana via:
//!
//! - **HTLC Atomic Swaps** — trustless peer-to-peer token exchanges using
//!   hash time-locked contracts
//! - **Bridge Protocol** — lock-and-mint / burn-and-unlock bridge with
//!   multi-validator threshold signing
//! - **Chain Adapters** — pluggable adapters for each target blockchain
//!
//! # Architecture
//!
//! ```text
//! ┌──────────────┐     ┌──────────────┐     ┌──────────────┐
//! │  RustChain   │────▶│    Bridge     │────▶│   External   │
//! │  (Source)    │     │   Relayer     │     │   Chain      │
//! └──────────────┘     └──────────────┘     └──────────────┘
//!       │                    │                     │
//!       ▼                    ▼                     ▼
//!  Lock tokens          Verify proof         Mint wrapped
//!  on RustChain         of lock              tokens
//! ```

pub mod chains;
pub mod error;
pub mod htlc;
pub mod registry;
pub mod relay;
pub mod state;
pub mod types;
pub mod validator;

pub use error::BridgeError;
pub use htlc::{HtlcManager, HtlcSwap};
pub use registry::ChainRegistry;
pub use relay::BridgeRelayer;
pub use state::BridgeState;
pub use types::*;
pub use validator::BridgeValidatorSet;

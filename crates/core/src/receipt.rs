use crate::types::*;
use rustchain_crypto::{Address, Blake3Hash};
use serde::{Deserialize, Serialize};

/// Status of a transaction execution.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum TxStatus {
    Success,
    Failure(String),
    OutOfGas,
}

/// An event log emitted by a smart contract.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct EventLog {
    /// Contract address that emitted this event.
    pub address: Address,
    /// Indexed topics (up to 4).
    pub topics: Vec<Blake3Hash>,
    /// Non-indexed event data.
    pub data: Vec<u8>,
}

/// Receipt of a transaction execution.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TransactionReceipt {
    pub tx_hash: Blake3Hash,
    pub block_number: BlockNumber,
    pub block_hash: Blake3Hash,
    pub index: u32,
    pub status: TxStatus,
    pub gas_used: Gas,
    pub logs: Vec<EventLog>,
    pub contract_address: Option<Address>,
    pub return_data: Vec<u8>,
}

impl TransactionReceipt {
    /// Whether the transaction succeeded.
    pub fn is_success(&self) -> bool {
        matches!(self.status, TxStatus::Success)
    }
}

use rustchain_core::{Account, Block, SignedTransaction, ValidatorInfo};
use rustchain_consensus::ChainInfo;
use serde::{Deserialize, Serialize};

/// Generic API response wrapper.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T: Serialize> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn err(message: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message.into()),
        }
    }
}

/// Block data transfer object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockDto {
    pub number: u64,
    pub hash: String,
    pub parent_hash: String,
    pub timestamp: u64,
    pub validator: String,
    pub epoch: u64,
    pub gas_used: u64,
    pub gas_limit: u64,
    pub tx_count: usize,
    pub state_root: String,
    pub transactions_root: String,
    pub transactions: Vec<TransactionDto>,
}

impl From<&Block> for BlockDto {
    fn from(block: &Block) -> Self {
        let h = &block.header;
        Self {
            number: h.number,
            hash: block.hash().to_string(),
            parent_hash: h.parent_hash.to_string(),
            timestamp: h.timestamp,
            validator: h.validator.to_hex(),
            epoch: h.epoch,
            gas_used: h.gas_used,
            gas_limit: h.gas_limit,
            tx_count: block.transactions.len(),
            state_root: h.state_root.to_string(),
            transactions_root: h.transactions_root.to_string(),
            transactions: block.transactions.iter().map(TransactionDto::from).collect(),
        }
    }
}

/// Transaction data transfer object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionDto {
    pub hash: String,
    pub from: String,
    pub to: Option<String>,
    pub value: String,
    pub nonce: u64,
    pub tx_type: String,
    pub gas_limit: u64,
    pub gas_price: u64,
    pub data_size: usize,
    pub timestamp: u64,
    pub chain_id: u64,
}

impl From<&SignedTransaction> for TransactionDto {
    fn from(signed: &SignedTransaction) -> Self {
        let tx = &signed.transaction;
        Self {
            hash: signed.hash().to_string(),
            from: tx.from.to_hex(),
            to: tx.to.map(|a| a.to_hex()),
            value: tx.value.to_string(),
            nonce: tx.nonce,
            tx_type: format!("{:?}", tx.tx_type),
            gas_limit: tx.gas_limit,
            gas_price: tx.gas_price,
            data_size: tx.data.len(),
            timestamp: tx.timestamp,
            chain_id: tx.chain_id,
        }
    }
}

/// Account data transfer object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountDto {
    pub address: String,
    pub balance: String,
    pub nonce: u64,
    pub is_contract: bool,
    pub code_hash: Option<String>,
    pub storage_root: String,
}

impl From<&Account> for AccountDto {
    fn from(acct: &Account) -> Self {
        Self {
            address: acct.address.to_hex(),
            balance: acct.balance.to_string(),
            nonce: acct.nonce,
            is_contract: acct.is_contract(),
            code_hash: acct.code_hash.map(|h| h.to_string()),
            storage_root: acct.storage_root.to_string(),
        }
    }
}

/// Validator data transfer object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorDto {
    pub address: String,
    pub public_key: String,
    pub stake: String,
    pub is_active: bool,
    pub commission_rate: u16,
    pub jailed_until: Option<u64>,
    pub slash_count: u32,
    pub uptime_blocks: u64,
    pub total_blocks: u64,
}

impl From<&ValidatorInfo> for ValidatorDto {
    fn from(v: &ValidatorInfo) -> Self {
        Self {
            address: v.address.to_hex(),
            public_key: v.public_key.to_hex(),
            stake: v.stake.to_string(),
            is_active: v.is_active,
            commission_rate: v.commission_rate,
            jailed_until: v.jailed_until,
            slash_count: v.slash_count,
            uptime_blocks: v.uptime_blocks,
            total_blocks: v.total_blocks,
        }
    }
}

/// Request to submit a signed transaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmitTransactionRequest {
    pub chain_id: u64,
    pub nonce: u64,
    pub from: String,
    pub to: Option<String>,
    pub value: String,
    pub tx_type: String,
    pub gas_limit: u64,
    pub gas_price: u64,
    pub data: String,
    pub timestamp: u64,
    pub public_key: String,
    pub signature: String,
}

/// Response after submitting a transaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmitTransactionResponse {
    pub tx_hash: String,
}

/// Chain information DTO.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainInfoDto {
    pub chain_id: u64,
    pub height: u64,
    pub best_block_hash: String,
    pub epoch: u64,
    pub finalized_height: u64,
    pub pending_transactions: usize,
    pub active_validators: usize,
}

impl From<&ChainInfo> for ChainInfoDto {
    fn from(info: &ChainInfo) -> Self {
        Self {
            chain_id: info.chain_id,
            height: info.height,
            best_block_hash: info.best_block_hash.to_string(),
            epoch: info.epoch,
            finalized_height: info.finalized_height,
            pending_transactions: info.pending_transactions,
            active_validators: info.active_validators,
        }
    }
}

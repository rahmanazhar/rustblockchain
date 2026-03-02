use crate::dto::{ApiResponse, SubmitTransactionRequest, SubmitTransactionResponse, TransactionDto};
use crate::error::ApiError;
use crate::AppState;
use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use rustchain_core::{SignedTransaction, Transaction, TxType};
use rustchain_crypto::{Address, Blake3Hash, PublicKey, Signature};
use std::sync::Arc;

/// GET /tx/:hash
/// Retrieve a transaction by its hex hash.
async fn get_transaction(
    State(state): State<Arc<AppState>>,
    Path(hash_str): Path<String>,
) -> Result<Json<ApiResponse<TransactionDto>>, ApiError> {
    let hash = Blake3Hash::from_hex(&hash_str)
        .map_err(|_| ApiError::BadRequest(format!("Invalid transaction hash: {}", hash_str)))?;

    let tx = state
        .storage
        .get_transaction(&hash)?
        .ok_or_else(|| ApiError::NotFound(format!("Transaction not found: {}", hash_str)))?;

    Ok(Json(ApiResponse::ok(TransactionDto::from(&tx))))
}

/// Parse a TxType string back into the enum.
fn parse_tx_type(s: &str) -> Result<TxType, ApiError> {
    match s.to_lowercase().as_str() {
        "transfer" => Ok(TxType::Transfer),
        "contractdeploy" | "contract_deploy" => Ok(TxType::ContractDeploy),
        "contractcall" | "contract_call" => Ok(TxType::ContractCall),
        "stake" => Ok(TxType::Stake),
        "unstake" => Ok(TxType::Unstake),
        other => Err(ApiError::BadRequest(format!("Unknown tx_type: {}", other))),
    }
}

/// POST /tx
/// Submit a pre-signed transaction to the mempool.
async fn submit_transaction(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SubmitTransactionRequest>,
) -> Result<Json<ApiResponse<SubmitTransactionResponse>>, ApiError> {
    let from = Address::from_hex(&req.from)
        .map_err(|e| ApiError::BadRequest(format!("Invalid from address: {}", e)))?;

    let to = req
        .to
        .as_deref()
        .map(Address::from_hex)
        .transpose()
        .map_err(|e| ApiError::BadRequest(format!("Invalid to address: {}", e)))?;

    let value: u128 = req
        .value
        .parse()
        .map_err(|_| ApiError::BadRequest("Invalid value: must be a u128 integer".to_string()))?;

    let tx_type = parse_tx_type(&req.tx_type)?;

    let data = hex::decode(req.data.strip_prefix("0x").unwrap_or(&req.data))
        .map_err(|e| ApiError::BadRequest(format!("Invalid hex data: {}", e)))?;

    let public_key = PublicKey::from_hex(&req.public_key)
        .map_err(|e| ApiError::BadRequest(format!("Invalid public key: {}", e)))?;

    let signature = Signature::from_hex(&req.signature)
        .map_err(|e| ApiError::BadRequest(format!("Invalid signature: {}", e)))?;

    let transaction = Transaction {
        chain_id: req.chain_id,
        nonce: req.nonce,
        from,
        to,
        value,
        tx_type,
        gas_limit: req.gas_limit,
        gas_price: req.gas_price,
        data,
        timestamp: req.timestamp,
    };

    // Compute the tx hash the same way SignedTransaction::new does.
    let _signing_hash = transaction.signing_hash();
    let mut hash_input =
        bincode::serialize(&transaction).map_err(|e| ApiError::Serialization(e.to_string()))?;
    hash_input.extend_from_slice(signature.as_bytes());
    let tx_hash = rustchain_crypto::hash(&hash_input);

    let signed = SignedTransaction {
        transaction,
        public_key,
        signature,
        tx_hash,
    };

    // Validate before submitting
    signed
        .verify()
        .map_err(|e| ApiError::BadRequest(format!("Transaction verification failed: {}", e)))?;

    state.consensus.on_transaction_received(signed)?;

    Ok(Json(ApiResponse::ok(SubmitTransactionResponse {
        tx_hash: tx_hash.to_string(),
    })))
}

pub fn transactions_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/:hash", get(get_transaction))
        .route("/", post(submit_transaction))
}

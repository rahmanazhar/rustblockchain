use crate::dto::ApiResponse;
use crate::error::ApiError;
use crate::AppState;
use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};
use rustchain_crypto::Blake3Hash;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Receipt data transfer object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceiptDto {
    pub tx_hash: String,
    pub block_number: u64,
    pub block_hash: String,
    pub index: u32,
    pub status: String,
    pub gas_used: u64,
    pub logs_count: usize,
    pub contract_address: Option<String>,
    pub return_data: String,
}

/// GET /receipts/:hash
async fn get_receipt(
    State(state): State<Arc<AppState>>,
    Path(hash_str): Path<String>,
) -> Result<Json<ApiResponse<ReceiptDto>>, ApiError> {
    let hash = Blake3Hash::from_hex(&hash_str)
        .map_err(|_| ApiError::BadRequest(format!("Invalid hash: {}", hash_str)))?;

    let receipt = state
        .storage
        .get_receipt(&hash)?
        .ok_or_else(|| ApiError::NotFound(format!("Receipt not found: {}", hash_str)))?;

    let dto = ReceiptDto {
        tx_hash: receipt.tx_hash.to_string(),
        block_number: receipt.block_number,
        block_hash: receipt.block_hash.to_string(),
        index: receipt.index,
        status: format!("{:?}", receipt.status),
        gas_used: receipt.gas_used,
        logs_count: receipt.logs.len(),
        contract_address: receipt.contract_address.map(|a| a.to_hex()),
        return_data: hex::encode(&receipt.return_data),
    };

    Ok(Json(ApiResponse::ok(dto)))
}

pub fn receipts_router() -> Router<Arc<AppState>> {
    Router::new().route("/{hash}", get(get_receipt))
}

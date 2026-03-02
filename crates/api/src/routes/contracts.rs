use crate::dto::ApiResponse;
use crate::error::ApiError;
use crate::AppState;
use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};
use rustchain_crypto::Address;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Contract info DTO.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractInfoDto {
    pub address: String,
    pub balance: String,
    pub code_hash: String,
    pub nonce: u64,
}

/// Contract storage value DTO.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageValueDto {
    pub key: String,
    pub value: String,
}

/// GET /contracts/:address
async fn get_contract_info(
    State(state): State<Arc<AppState>>,
    Path(addr_str): Path<String>,
) -> Result<Json<ApiResponse<ContractInfoDto>>, ApiError> {
    let addr = Address::from_hex(&addr_str)
        .map_err(|e| ApiError::BadRequest(format!("Invalid address: {}", e)))?;

    let account = state
        .storage
        .get_account(&addr)?
        .ok_or_else(|| ApiError::NotFound(format!("Account not found: {}", addr_str)))?;

    let code_hash = account
        .code_hash
        .ok_or_else(|| ApiError::NotFound("Not a contract account".to_string()))?;

    let dto = ContractInfoDto {
        address: addr.to_hex(),
        balance: account.balance.to_string(),
        code_hash: code_hash.to_string(),
        nonce: account.nonce,
    };

    Ok(Json(ApiResponse::ok(dto)))
}

/// GET /contracts/:address/storage/:key
async fn get_contract_storage(
    State(state): State<Arc<AppState>>,
    Path((addr_str, key_hex)): Path<(String, String)>,
) -> Result<Json<ApiResponse<StorageValueDto>>, ApiError> {
    let addr = Address::from_hex(&addr_str)
        .map_err(|e| ApiError::BadRequest(format!("Invalid address: {}", e)))?;

    let key = hex::decode(key_hex.strip_prefix("0x").unwrap_or(&key_hex))
        .map_err(|e| ApiError::BadRequest(format!("Invalid hex key: {}", e)))?;

    let value = state
        .storage
        .get_contract_storage(&addr, &key)?
        .ok_or_else(|| ApiError::NotFound("Storage key not found".to_string()))?;

    let dto = StorageValueDto {
        key: hex::encode(&key),
        value: hex::encode(&value),
    };

    Ok(Json(ApiResponse::ok(dto)))
}

/// GET /contracts/:address/code
async fn get_contract_code(
    State(state): State<Arc<AppState>>,
    Path(addr_str): Path<String>,
) -> Result<Json<ApiResponse<serde_json::Value>>, ApiError> {
    let addr = Address::from_hex(&addr_str)
        .map_err(|e| ApiError::BadRequest(format!("Invalid address: {}", e)))?;

    let account = state
        .storage
        .get_account(&addr)?
        .ok_or_else(|| ApiError::NotFound(format!("Account not found: {}", addr_str)))?;

    let code_hash = account
        .code_hash
        .ok_or_else(|| ApiError::NotFound("Not a contract account".to_string()))?;

    let code = state
        .storage
        .get_contract_code(&code_hash)?
        .ok_or_else(|| ApiError::NotFound("Contract code not found".to_string()))?;

    Ok(Json(ApiResponse::ok(serde_json::json!({
        "code_hash": code_hash.to_string(),
        "code_size": code.len(),
        "code": hex::encode(&code),
    }))))
}

pub fn contracts_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/:address", get(get_contract_info))
        .route("/:address/storage/:key", get(get_contract_storage))
        .route("/:address/code", get(get_contract_code))
}

use crate::dto::{AccountDto, ApiResponse};
use crate::error::ApiError;
use crate::AppState;
use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};
use rustchain_crypto::Address;
use serde::Serialize;
use std::sync::Arc;

/// Balance-only response.
#[derive(Debug, Clone, Serialize)]
pub struct BalanceResponse {
    pub address: String,
    pub balance: String,
}

/// GET /accounts/:addr
/// Retrieve the full account state for an address.
async fn get_account(
    State(state): State<Arc<AppState>>,
    Path(addr_str): Path<String>,
) -> Result<Json<ApiResponse<AccountDto>>, ApiError> {
    let address = Address::from_hex(&addr_str)
        .map_err(|e| ApiError::BadRequest(format!("Invalid address: {}", e)))?;

    let account = state
        .storage
        .get_account(&address)?
        .ok_or_else(|| ApiError::NotFound(format!("Account not found: {}", addr_str)))?;

    Ok(Json(ApiResponse::ok(AccountDto::from(&account))))
}

/// GET /accounts/:addr/balance
/// Retrieve just the balance for an address (returns 0 for non-existent accounts).
async fn get_balance(
    State(state): State<Arc<AppState>>,
    Path(addr_str): Path<String>,
) -> Result<Json<ApiResponse<BalanceResponse>>, ApiError> {
    let address = Address::from_hex(&addr_str)
        .map_err(|e| ApiError::BadRequest(format!("Invalid address: {}", e)))?;

    let balance = state.storage.get_balance(&address)?;

    Ok(Json(ApiResponse::ok(BalanceResponse {
        address: address.to_hex(),
        balance: balance.to_string(),
    })))
}

pub fn accounts_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/{addr}", get(get_account))
        .route("/{addr}/balance", get(get_balance))
}

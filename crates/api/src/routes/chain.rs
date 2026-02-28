use crate::dto::{ApiResponse, ChainInfoDto};
use crate::error::ApiError;
use crate::AppState;
use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;
use std::sync::Arc;

/// Lightweight chain status snapshot.
#[derive(Debug, Clone, Serialize)]
pub struct ChainStatus {
    pub height: u64,
    pub finalized_height: u64,
    pub pending_transactions: usize,
    pub syncing: bool,
}

/// GET /chain/info
/// Returns comprehensive chain metadata.
async fn chain_info(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ApiResponse<ChainInfoDto>>, ApiError> {
    let info = state.consensus.chain_info();
    Ok(Json(ApiResponse::ok(ChainInfoDto::from(&info))))
}

/// GET /chain/status
/// Returns a lightweight status snapshot (useful for monitoring).
async fn chain_status(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ApiResponse<ChainStatus>>, ApiError> {
    let info = state.consensus.chain_info();
    Ok(Json(ApiResponse::ok(ChainStatus {
        height: info.height,
        finalized_height: info.finalized_height,
        pending_transactions: info.pending_transactions,
        syncing: false, // placeholder -- real implementation would check sync state
    })))
}

pub fn chain_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/info", get(chain_info))
        .route("/status", get(chain_status))
}

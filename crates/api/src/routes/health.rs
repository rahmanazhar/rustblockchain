use crate::dto::ApiResponse;
use crate::error::ApiError;
use crate::AppState;
use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;
use std::sync::Arc;

/// Health check response.
#[derive(Debug, Clone, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

/// Readiness probe response.
#[derive(Debug, Clone, Serialize)]
pub struct ReadyResponse {
    pub ready: bool,
    pub storage_ok: bool,
    pub consensus_ok: bool,
}

/// GET /health
/// Simple liveness probe -- if the server is up this always returns OK.
async fn health() -> Result<Json<ApiResponse<HealthResponse>>, ApiError> {
    Ok(Json(ApiResponse::ok(HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })))
}

/// GET /health/ready
/// Readiness probe -- checks that storage and consensus are operational.
async fn readiness(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ApiResponse<ReadyResponse>>, ApiError> {
    let storage_ok = state.storage.get_chain_height().is_ok();
    let consensus_ok = {
        let info = state.consensus.chain_info();
        info.height > 0 || info.active_validators > 0
    };

    let ready = storage_ok && consensus_ok;

    Ok(Json(ApiResponse::ok(ReadyResponse {
        ready,
        storage_ok,
        consensus_ok,
    })))
}

pub fn health_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(health))
        .route("/ready", get(readiness))
}

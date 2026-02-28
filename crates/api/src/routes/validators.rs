use crate::dto::{ApiResponse, ValidatorDto};
use crate::error::ApiError;
use crate::AppState;
use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use std::sync::Arc;

/// GET /validators
/// Returns the current validator set from the consensus engine.
async fn list_validators(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ApiResponse<Vec<ValidatorDto>>>, ApiError> {
    let chain_state = state.consensus.chain_state();
    let reader = chain_state.read();
    let epoch = reader.current_epoch();

    let validators: Vec<ValidatorDto> = epoch
        .validator_set
        .validators
        .iter()
        .map(ValidatorDto::from)
        .collect();

    Ok(Json(ApiResponse::ok(validators)))
}

pub fn validators_router() -> Router<Arc<AppState>> {
    Router::new().route("/", get(list_validators))
}

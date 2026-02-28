use crate::dto::{ApiResponse, BlockDto};
use crate::error::ApiError;
use crate::AppState;
use axum::extract::{Path, Query, State};
use axum::routing::get;
use axum::{Json, Router};
use rustchain_crypto::Blake3Hash;
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct PaginationParams {
    #[serde(default = "default_page")]
    pub page: u64,
    #[serde(default = "default_per_page")]
    pub per_page: u64,
}

fn default_page() -> u64 {
    1
}
fn default_per_page() -> u64 {
    20
}

/// GET /blocks?page=1&per_page=20
/// Returns a paginated list of blocks from newest to oldest.
async fn list_blocks(
    State(state): State<Arc<AppState>>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<ApiResponse<Vec<BlockDto>>>, ApiError> {
    let info = state.consensus.chain_info();
    let height = info.height;

    let per_page = params.per_page.clamp(1, 100);
    let page = params.page.max(1);
    let start = height.saturating_sub((page - 1) * per_page);

    let mut blocks = Vec::new();
    for i in 0..per_page {
        if start < i {
            break;
        }
        let block_num = start - i;
        if let Some(block) = state.storage.get_block_by_number(block_num)? {
            blocks.push(BlockDto::from(&block));
        }
    }

    Ok(Json(ApiResponse::ok(blocks)))
}

/// GET /blocks/latest
/// Returns the most recent block.
async fn get_latest_block(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ApiResponse<BlockDto>>, ApiError> {
    let block = state
        .storage
        .get_latest_block()?
        .ok_or_else(|| ApiError::NotFound("No blocks in chain".to_string()))?;

    Ok(Json(ApiResponse::ok(BlockDto::from(&block))))
}

/// GET /blocks/:id
/// Looks up a block by number (pure digits) or by hex hash.
async fn get_block(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<BlockDto>>, ApiError> {
    let block = if let Ok(number) = id.parse::<u64>() {
        state.storage.get_block_by_number(number)?
    } else {
        let hash = Blake3Hash::from_hex(&id)
            .map_err(|_| ApiError::BadRequest(format!("Invalid block identifier: {}", id)))?;
        state.storage.get_block_by_hash(&hash)?
    };

    let block =
        block.ok_or_else(|| ApiError::NotFound(format!("Block not found: {}", id)))?;

    Ok(Json(ApiResponse::ok(BlockDto::from(&block))))
}

pub fn blocks_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(list_blocks))
        .route("/latest", get(get_latest_block))
        .route("/{id}", get(get_block))
}

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

/// API error types with HTTP status code mapping.
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("Internal server error: {0}")]
    Internal(String),

    #[error("Rate limit exceeded")]
    RateLimit,

    #[error("Serialization error: {0}")]
    Serialization(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            ApiError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg.clone()),
            ApiError::Forbidden(msg) => (StatusCode::FORBIDDEN, msg.clone()),
            ApiError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
            ApiError::RateLimit => (
                StatusCode::TOO_MANY_REQUESTS,
                "Rate limit exceeded".to_string(),
            ),
            ApiError::Serialization(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
        };

        let body = json!({
            "success": false,
            "error": {
                "code": status.as_u16(),
                "message": message,
            }
        });

        (status, Json(body)).into_response()
    }
}

impl From<serde_json::Error> for ApiError {
    fn from(err: serde_json::Error) -> Self {
        ApiError::Serialization(err.to_string())
    }
}

impl From<rustchain_storage::StorageError> for ApiError {
    fn from(err: rustchain_storage::StorageError) -> Self {
        ApiError::Internal(format!("Storage error: {}", err))
    }
}

impl From<rustchain_consensus::ConsensusError> for ApiError {
    fn from(err: rustchain_consensus::ConsensusError) -> Self {
        ApiError::Internal(format!("Consensus error: {}", err))
    }
}

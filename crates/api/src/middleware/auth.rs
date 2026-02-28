use crate::config::AuthConfig;
use crate::error::ApiError;
use axum::body::Body;
use axum::extract::State;
use axum::http::Request;
use axum::middleware::Next;
use axum::response::Response;
use chrono::Utc;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// User roles for RBAC.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Admin,
    Validator,
    User,
    ReadOnly,
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Role::Admin => write!(f, "admin"),
            Role::Validator => write!(f, "validator"),
            Role::User => write!(f, "user"),
            Role::ReadOnly => write!(f, "readonly"),
        }
    }
}

/// JWT claims payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// Subject (typically the user's blockchain address).
    pub sub: String,
    /// User role.
    pub role: Role,
    /// Expiration timestamp (seconds since epoch).
    pub exp: u64,
    /// Issued at timestamp (seconds since epoch).
    pub iat: u64,
}

/// Generate a signed JWT token for the given subject and role.
pub fn generate_token(
    config: &AuthConfig,
    subject: &str,
    role: Role,
) -> Result<String, ApiError> {
    let now = Utc::now().timestamp() as u64;
    let claims = Claims {
        sub: subject.to_string(),
        role,
        exp: now + config.jwt_expiration_secs,
        iat: now,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(config.jwt_secret.as_bytes()),
    )
    .map_err(|e| ApiError::Internal(format!("Token generation failed: {}", e)))
}

/// Validate a JWT token and return the decoded claims.
pub fn validate_token(config: &AuthConfig, token: &str) -> Result<Claims, ApiError> {
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(config.jwt_secret.as_bytes()),
        &Validation::default(),
    )
    .map_err(|e| ApiError::Unauthorized(format!("Invalid token: {}", e)))?;

    Ok(token_data.claims)
}

/// Axum middleware that validates the JWT Bearer token on every request.
///
/// Extracts the token from the `Authorization: Bearer <token>` header,
/// validates it, and injects the `Claims` into request extensions.
pub async fn auth_middleware(
    State(auth_config): State<Arc<AuthConfig>>,
    mut request: Request<Body>,
    next: Next,
) -> Result<Response, ApiError> {
    let auth_header = request
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| ApiError::Unauthorized("Missing Authorization header".to_string()))?;

    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or_else(|| ApiError::Unauthorized("Invalid Authorization header format".to_string()))?;

    let claims = validate_token(&auth_config, token)?;

    // Insert claims into request extensions so handlers can access them.
    request.extensions_mut().insert(claims);

    Ok(next.run(request).await)
}

/// Axum middleware that enforces a minimum role level.
///
/// Must be applied *after* `auth_middleware` so that `Claims` are available.
pub async fn require_role(
    required: Role,
    request: Request<Body>,
    next: Next,
) -> Result<Response, ApiError> {
    let claims = request
        .extensions()
        .get::<Claims>()
        .ok_or_else(|| ApiError::Unauthorized("No authentication claims found".to_string()))?;

    let authorized = match required {
        Role::ReadOnly => true,
        Role::User => matches!(claims.role, Role::User | Role::Validator | Role::Admin),
        Role::Validator => matches!(claims.role, Role::Validator | Role::Admin),
        Role::Admin => matches!(claims.role, Role::Admin),
    };

    if !authorized {
        return Err(ApiError::Forbidden(format!(
            "Role '{}' required, but user has role '{}'",
            required, claims.role
        )));
    }

    Ok(next.run(request).await)
}

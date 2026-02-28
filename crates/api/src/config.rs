use rustchain_crypto::Address;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

/// TLS configuration for HTTPS support.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfig {
    pub cert_path: String,
    pub key_path: String,
}

/// Rate limiting configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    /// Maximum sustained requests per second per IP.
    pub requests_per_second: u32,
    /// Burst allowance above the sustained rate.
    pub burst_size: u32,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            requests_per_second: 100,
            burst_size: 200,
        }
    }
}

/// Authentication and authorization configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub jwt_secret: String,
    pub jwt_expiration_secs: u64,
    pub enable_rbac: bool,
    pub admin_addresses: Vec<Address>,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            jwt_secret: "change-me-in-production".to_string(),
            jwt_expiration_secs: 3600,
            enable_rbac: false,
            admin_addresses: vec![],
        }
    }
}

/// Top-level API server configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    pub bind_address: SocketAddr,
    pub tls: Option<TlsConfig>,
    pub cors_origins: Vec<String>,
    pub max_request_body_size: usize,
    pub rate_limit: RateLimitConfig,
    pub auth: AuthConfig,
    pub ws_max_connections: usize,
    pub metrics_enabled: bool,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            bind_address: SocketAddr::from(([127, 0, 0, 1], 8080)),
            tls: None,
            cors_origins: vec!["*".to_string()],
            max_request_body_size: 2 * 1024 * 1024, // 2 MB
            rate_limit: RateLimitConfig::default(),
            auth: AuthConfig::default(),
            ws_max_connections: 1024,
            metrics_enabled: true,
        }
    }
}

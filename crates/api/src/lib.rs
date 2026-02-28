pub mod config;
pub mod dto;
pub mod error;
pub mod metrics;
pub mod middleware;
pub mod routes;
pub mod server;
pub mod ws;

use crate::config::AuthConfig;
use crate::metrics::MetricsRegistry;
use rustchain_consensus::ConsensusEngine;
use rustchain_storage::ChainDatabase;
use std::sync::Arc;

/// Shared application state passed to every handler via Axum's `State` extractor.
pub struct AppState {
    pub consensus: Arc<ConsensusEngine>,
    pub storage: Arc<ChainDatabase>,
    pub metrics: Arc<MetricsRegistry>,
    pub auth_config: AuthConfig,
}

// Re-exports for downstream convenience.
pub use config::ApiConfig;
pub use error::ApiError;
pub use metrics::MetricsRegistry as Metrics;
pub use server::ApiServer;

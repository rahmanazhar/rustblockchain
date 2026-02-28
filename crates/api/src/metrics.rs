use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use prometheus::{
    Encoder, IntCounter, IntGauge, Registry, TextEncoder,
};
use std::sync::Arc;

/// Prometheus metrics registry for the API / blockchain node.
#[derive(Clone)]
pub struct MetricsRegistry {
    pub registry: Registry,
    pub blocks_processed: IntCounter,
    pub transactions_processed: IntCounter,
    pub active_connections: IntGauge,
    pub chain_height: IntGauge,
    pub peer_count: IntGauge,
}

impl MetricsRegistry {
    /// Create and register all metrics.
    pub fn new() -> Self {
        let registry = Registry::new();

        let blocks_processed = IntCounter::new(
            "rustchain_blocks_processed_total",
            "Total number of blocks processed",
        )
        .expect("metric can be created");

        let transactions_processed = IntCounter::new(
            "rustchain_transactions_processed_total",
            "Total number of transactions processed",
        )
        .expect("metric can be created");

        let active_connections = IntGauge::new(
            "rustchain_active_connections",
            "Number of active API / WebSocket connections",
        )
        .expect("metric can be created");

        let chain_height = IntGauge::new(
            "rustchain_chain_height",
            "Current blockchain height",
        )
        .expect("metric can be created");

        let peer_count = IntGauge::new(
            "rustchain_peer_count",
            "Number of connected P2P peers",
        )
        .expect("metric can be created");

        registry
            .register(Box::new(blocks_processed.clone()))
            .expect("collector can be registered");
        registry
            .register(Box::new(transactions_processed.clone()))
            .expect("collector can be registered");
        registry
            .register(Box::new(active_connections.clone()))
            .expect("collector can be registered");
        registry
            .register(Box::new(chain_height.clone()))
            .expect("collector can be registered");
        registry
            .register(Box::new(peer_count.clone()))
            .expect("collector can be registered");

        Self {
            registry,
            blocks_processed,
            transactions_processed,
            active_connections,
            chain_height,
            peer_count,
        }
    }

    /// Encode the metrics into Prometheus text format.
    pub fn encode(&self) -> Result<String, String> {
        let encoder = TextEncoder::new();
        let metric_families = self.registry.gather();
        let mut buffer = Vec::new();
        encoder
            .encode(&metric_families, &mut buffer)
            .map_err(|e| e.to_string())?;
        String::from_utf8(buffer).map_err(|e| e.to_string())
    }
}

impl Default for MetricsRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// GET /metrics  --  Prometheus scrape endpoint.
pub async fn metrics_handler(
    State(metrics): State<Arc<MetricsRegistry>>,
) -> impl IntoResponse {
    match metrics.encode() {
        Ok(body) => (
            StatusCode::OK,
            [("content-type", "text/plain; version=0.0.4; charset=utf-8")],
            body,
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to encode metrics: {}", e),
        )
            .into_response(),
    }
}

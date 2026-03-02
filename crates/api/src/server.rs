use crate::config::ApiConfig;
use crate::metrics::{metrics_handler, MetricsRegistry};
use crate::middleware::auth::auth_middleware;
use crate::middleware::rate_limit::{rate_limit_middleware, RateLimiter};
use crate::routes;
use crate::ws::handler::ws_upgrade;
use crate::AppState;
use axum::middleware;
use axum::response::Html;
use axum::routing::get;
use axum::Router;
use rustchain_consensus::ConsensusEngine;
use rustchain_storage::ChainDatabase;
use std::fs;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;
use tower_http::cors::{Any, CorsLayer};
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::trace::TraceLayer;
use tracing::info;

const INDEX_HTML: &str = include_str!("../static/index.html");

async fn serve_index() -> Html<&'static str> {
    Html(INDEX_HTML)
}

/// The API server that ties together all routes, middleware, and listeners.
pub struct ApiServer {
    config: ApiConfig,
    state: Arc<AppState>,
}

impl ApiServer {
    /// Create a new API server.
    pub fn new(
        config: ApiConfig,
        consensus: Arc<ConsensusEngine>,
        storage: Arc<ChainDatabase>,
        metrics: Arc<MetricsRegistry>,
    ) -> Self {
        let state = Arc::new(AppState {
            consensus,
            storage,
            metrics,
            auth_config: config.auth.clone(),
        });
        Self { config, state }
    }

    /// Build the complete axum Router with all routes and middleware.
    pub fn router(&self) -> Router {
        // --- CORS layer ---
        let cors = if self.config.cors_origins.iter().any(|o| o == "*") {
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any)
        } else {
            let origins: Vec<_> = self
                .config
                .cors_origins
                .iter()
                .filter_map(|o| o.parse().ok())
                .collect();
            CorsLayer::new()
                .allow_origin(origins)
                .allow_methods(Any)
                .allow_headers(Any)
        };

        // --- Rate limiter ---
        let limiter = Arc::new(RateLimiter::new(self.config.rate_limit.clone()));
        let limiter_clone = limiter.clone();

        // --- Read-only routes (no auth needed) ---
        let read_routes = Router::new()
            .nest("/blocks", routes::blocks_router())
            .nest("/accounts", routes::accounts_router())
            .nest("/validators", routes::validators_router())
            .nest("/chain", routes::chain_router())
            .nest("/health", routes::health_router())
            .nest("/receipts", routes::receipts_router())
            .nest("/contracts", routes::contracts_router())
            .route("/ws", get(ws_upgrade));

        // --- Write routes (auth required when RBAC enabled) ---
        let write_routes = if self.config.auth.enable_rbac {
            let auth_config = Arc::new(self.config.auth.clone());
            Router::new()
                .nest("/tx", routes::transactions_router())
                .layer(middleware::from_fn_with_state(auth_config, auth_middleware))
        } else {
            Router::new()
                .nest("/tx", routes::transactions_router())
        };

        // --- Wallet routes (password-based auth via encrypted keystore) ---
        let wallet_routes = Router::new().nest("/wallet", routes::wallet_router());

        let mut app = read_routes
            .merge(write_routes)
            .merge(wallet_routes)
            .route("/", get(serve_index))
            .with_state(self.state.clone());

        // --- Metrics endpoint (separate state) ---
        if self.config.metrics_enabled {
            let metrics_state = self.state.metrics.clone();
            app = app.route(
                "/metrics",
                get(metrics_handler).with_state(metrics_state),
            );
        }

        // --- Apply tower middleware (outermost first) ---
        app.layer(RequestBodyLimitLayer::new(self.config.max_request_body_size))
            .layer(cors)
            .layer(TraceLayer::new_for_http())
            .layer(middleware::from_fn(move |req, next| {
                let lim = limiter_clone.clone();
                rate_limit_middleware(lim, req, next)
            }))
    }

    /// Start serving HTTP (or HTTPS if TLS is configured).
    pub async fn serve(self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let router = self.router();
        let addr = self.config.bind_address;

        if let Some(tls_cfg) = &self.config.tls {
            // ---- TLS path ----
            let cert_pem = fs::read(&tls_cfg.cert_path)?;
            let key_pem = fs::read(&tls_cfg.key_path)?;

            let certs = rustls_pemfile::certs(&mut cert_pem.as_slice())
                .filter_map(|c| c.ok())
                .collect::<Vec<_>>();

            let key = rustls_pemfile::private_key(&mut key_pem.as_slice())?
                .ok_or("No private key found in PEM file")?;

            let server_config = rustls::ServerConfig::builder()
                .with_no_client_auth()
                .with_single_cert(certs, key)?;

            let tls_acceptor = TlsAcceptor::from(Arc::new(server_config));
            let listener = TcpListener::bind(addr).await?;

            info!("API server listening on https://{}", addr);

            loop {
                let (stream, _remote_addr) = listener.accept().await?;
                let acceptor = tls_acceptor.clone();
                let svc = router.clone();

                tokio::spawn(async move {
                    match acceptor.accept(stream).await {
                        Ok(tls_stream) => {
                            let io = hyper_util::rt::TokioIo::new(tls_stream);
                            let service = hyper::service::service_fn(move |req| {
                                let svc = svc.clone();
                                async move {
                                    let response = tower::ServiceExt::oneshot(svc, req).await;
                                    response
                                }
                            });
                            if let Err(e) =
                                hyper_util::server::conn::auto::Builder::new(
                                    hyper_util::rt::TokioExecutor::new(),
                                )
                                .serve_connection(io, service)
                                .await
                            {
                                tracing::warn!("TLS connection error: {}", e);
                            }
                        }
                        Err(e) => {
                            tracing::warn!("TLS handshake failed: {}", e);
                        }
                    }
                });
            }
        } else {
            // ---- Plain HTTP path ----
            let listener = TcpListener::bind(addr).await?;
            info!("API server listening on http://{}", addr);
            axum::serve(listener, router).await?;
            Ok(())
        }
    }
}

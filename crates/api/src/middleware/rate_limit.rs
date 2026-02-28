use crate::config::RateLimitConfig;
use crate::error::ApiError;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::Request;
use axum::middleware::Next;
use axum::response::Response;
use dashmap::DashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Instant;

/// Per-IP rate tracking entry.
struct RateEntry {
    count: u32,
    last_reset: Instant,
}

/// In-memory, per-IP token-bucket rate limiter.
///
/// Each IP is allowed `burst_size` requests immediately, then
/// `requests_per_second` sustained. The bucket is refilled every second.
#[derive(Clone)]
pub struct RateLimiter {
    config: RateLimitConfig,
    buckets: Arc<DashMap<IpAddr, RateEntry>>,
}

impl RateLimiter {
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            config,
            buckets: Arc::new(DashMap::new()),
        }
    }

    /// Check whether a request from `ip` is allowed.
    /// Returns `true` if allowed, `false` if rate-limited.
    fn check(&self, ip: IpAddr) -> bool {
        let now = Instant::now();
        let mut entry = self.buckets.entry(ip).or_insert_with(|| RateEntry {
            count: 0,
            last_reset: now,
        });

        let elapsed = now.duration_since(entry.last_reset);
        if elapsed.as_secs() >= 1 {
            // Reset the window
            entry.count = 1;
            entry.last_reset = now;
            true
        } else {
            entry.count += 1;
            entry.count <= self.config.burst_size
        }
    }
}

/// Axum middleware layer that enforces per-IP rate limiting.
pub async fn rate_limit_middleware(
    limiter: Arc<RateLimiter>,
    request: Request<Body>,
    next: Next,
) -> Result<Response, ApiError> {
    // Try to extract the client IP from ConnectInfo, then forwarded headers,
    // and finally fall back to a loopback address.
    let ip = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|ci| ci.0.ip())
        .or_else(|| {
            request
                .headers()
                .get("X-Forwarded-For")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.split(',').next())
                .and_then(|s| s.trim().parse::<IpAddr>().ok())
        })
        .unwrap_or(IpAddr::from([127, 0, 0, 1]));

    if !limiter.check(ip) {
        return Err(ApiError::RateLimit);
    }

    Ok(next.run(request).await)
}

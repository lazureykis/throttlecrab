//! HTTP/JSON transport for easy integration
//!
//! This transport provides a REST API with JSON payloads, making it easy
//! to integrate with any programming language or tool that supports HTTP.
//!
//! # API Endpoints
//!
//! ## POST /throttle
//!
//! Check rate limit for a key.
//!
//! ### Request Body
//!
//! ```json
//! {
//!   "key": "user:123",
//!   "max_burst": 10,
//!   "count_per_period": 100,
//!   "period": 60,
//!   "quantity": 1
//! }
//! ```
//!
//! - `quantity` is optional (defaults to 1)
//!
//! ### Response
//!
//! ```json
//! {
//!   "allowed": true,
//!   "limit": 10,
//!   "remaining": 9,
//!   "reset_after": 60,
//!   "retry_after": 0
//! }
//! ```
//!
//! ## GET /health
//!
//! Health check endpoint. Returns "OK" with 200 status.

use super::Transport;
use crate::actor::RateLimiterHandle;
use crate::metrics::{Metrics, Transport as MetricsTransport};
use crate::types::{ThrottleRequest as InternalRequest, ThrottleResponse};
use anyhow::Result;
use async_trait::async_trait;
use axum::{
    Router,
    extract::State,
    http::StatusCode,
    response::Json,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Instant, SystemTime};

/// HTTP request format for rate limiting
#[derive(Debug, Serialize, Deserialize)]
pub struct HttpThrottleRequest {
    /// The key to rate limit
    pub key: String,
    /// Maximum burst capacity
    pub max_burst: i64,
    /// Total requests allowed per period
    pub count_per_period: i64,
    /// Time period in seconds
    pub period: i64,
    /// Number of tokens to consume (optional, defaults to 1)
    pub quantity: Option<i64>,
}

/// Error response format
#[derive(Debug, Serialize, Deserialize)]
pub struct HttpErrorResponse {
    /// Error message
    pub error: String,
}

/// HTTP transport implementation
///
/// Provides a REST API with JSON payloads for easy integration.
pub struct HttpTransport {
    addr: SocketAddr,
    metrics: Arc<Metrics>,
}

impl HttpTransport {
    pub fn new(host: &str, port: u16, metrics: Arc<Metrics>) -> Self {
        let addr = format!("{host}:{port}").parse().expect("Invalid address");
        Self { addr, metrics }
    }
}

#[async_trait]
impl Transport for HttpTransport {
    async fn start(self, limiter: RateLimiterHandle) -> Result<()> {
        let metrics = Arc::clone(&self.metrics);
        let app_state = Arc::new(AppState { limiter, metrics });

        let app = Router::new()
            .route("/throttle", post(handle_throttle))
            .route("/health", get(|| async { "OK" }))
            .route("/metrics", get(handle_metrics))
            .with_state(app_state);

        tracing::info!("HTTP server listening on {}", self.addr);

        let listener = tokio::net::TcpListener::bind(self.addr).await?;
        axum::serve(listener, app).await?;

        Ok(())
    }
}

struct AppState {
    limiter: RateLimiterHandle,
    metrics: Arc<Metrics>,
}

async fn handle_throttle(
    State(state): State<Arc<AppState>>,
    Json(req): Json<HttpThrottleRequest>,
) -> Result<Json<ThrottleResponse>, (StatusCode, Json<HttpErrorResponse>)> {
    let start = Instant::now();

    // Always use server timestamp
    let timestamp = SystemTime::now();

    let internal_req = InternalRequest {
        key: req.key.clone(),
        max_burst: req.max_burst,
        count_per_period: req.count_per_period,
        period: req.period,
        quantity: req.quantity.unwrap_or(1),
        timestamp,
    };

    match state.limiter.throttle(internal_req).await {
        Ok(response) => {
            let latency_us = start.elapsed().as_micros() as u64;
            state.metrics.record_request_with_key(
                MetricsTransport::Http,
                latency_us,
                response.allowed,
                &req.key,
            );
            Ok(Json(response))
        }
        Err(e) => {
            tracing::error!("Rate limiter error: {}", e);
            let latency_us = start.elapsed().as_micros() as u64;
            state
                .metrics
                .record_error(MetricsTransport::Http, latency_us);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(HttpErrorResponse {
                    error: format!("Internal server error: {e}"),
                }),
            ))
        }
    }
}

async fn handle_metrics(State(state): State<Arc<AppState>>) -> Result<String, StatusCode> {
    Ok(state.metrics.export_prometheus())
}

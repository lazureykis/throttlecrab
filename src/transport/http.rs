use super::Transport;
use crate::actor::RateLimiterHandle;
use crate::types::{ThrottleRequest as InternalRequest, ThrottleResponse};
use anyhow::Result;
use async_trait::async_trait;
use axum::{Router, extract::State, http::StatusCode, response::Json, routing::post};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::SystemTime;

#[derive(Debug, Serialize, Deserialize)]
pub struct HttpThrottleRequest {
    pub key: String,
    pub max_burst: i64,
    pub count_per_period: i64,
    pub period: i64,
    pub quantity: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HttpErrorResponse {
    pub error: String,
}

pub struct HttpTransport {
    addr: SocketAddr,
}

impl HttpTransport {
    pub fn new(host: &str, port: u16) -> Self {
        let addr = format!("{host}:{port}").parse().expect("Invalid address");
        Self { addr }
    }
}

#[async_trait]
impl Transport for HttpTransport {
    async fn start(self, limiter: RateLimiterHandle) -> Result<()> {
        let app_state = Arc::new(AppState { limiter });

        let app = Router::new()
            .route("/throttle", post(handle_throttle))
            .route("/health", axum::routing::get(|| async { "OK" }))
            .with_state(app_state);

        tracing::info!("HTTP server listening on {}", self.addr);

        let listener = tokio::net::TcpListener::bind(self.addr).await?;
        axum::serve(listener, app).await?;

        Ok(())
    }
}

struct AppState {
    limiter: RateLimiterHandle,
}

async fn handle_throttle(
    State(state): State<Arc<AppState>>,
    Json(req): Json<HttpThrottleRequest>,
) -> Result<Json<ThrottleResponse>, (StatusCode, Json<HttpErrorResponse>)> {
    let internal_req = InternalRequest {
        key: req.key,
        max_burst: req.max_burst,
        count_per_period: req.count_per_period,
        period: req.period,
        quantity: req.quantity.unwrap_or(1),
        timestamp: SystemTime::now(),
    };

    match state.limiter.throttle(internal_req).await {
        Ok(response) => Ok(Json(response)),
        Err(e) => {
            tracing::error!("Rate limiter error: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(HttpErrorResponse {
                    error: format!("Internal server error: {e}"),
                }),
            ))
        }
    }
}

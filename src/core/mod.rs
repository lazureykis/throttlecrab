pub mod actor;
pub mod rate;
pub mod rate_limiter;
pub mod store;

pub use actor::{RateLimiterActor, RateLimiterHandle, RateLimiterMessage};
pub use rate::Rate;
pub use rate_limiter::{RateLimiter, RateLimitResult};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThrottleRequest {
    pub key: String,
    pub max_burst: i64,
    pub count_per_period: i64,
    pub period: i64, // seconds
    #[serde(default = "default_quantity")]
    pub quantity: i64,
}

fn default_quantity() -> i64 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThrottleResponse {
    pub allowed: bool,
    pub limit: i64,
    pub remaining: i64,
    pub reset_after: i64,  // seconds
    pub retry_after: i64,  // seconds
}

impl From<(bool, RateLimitResult)> for ThrottleResponse {
    fn from((allowed, result): (bool, RateLimitResult)) -> Self {
        ThrottleResponse {
            allowed,
            limit: result.limit,
            remaining: result.remaining,
            reset_after: result.reset_after.as_secs() as i64,
            retry_after: result.retry_after.as_secs() as i64,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CellError {
    #[error("negative quantity: {0}")]
    NegativeQuantity(i64),
    
    #[error("invalid rate limit parameters")]
    InvalidRateLimit,
    
    #[error("internal error: {0}")]
    Internal(String),
}
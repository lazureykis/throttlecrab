use serde::{Deserialize, Serialize};
use throttlecrab::RateLimitResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThrottleRequest {
    pub key: String,
    pub max_burst: i64,
    pub count_per_period: i64,
    pub period: i64, // seconds
    #[serde(default = "default_quantity")]
    pub quantity: i64,
    #[serde(default = "default_timestamp")]
    pub timestamp: i64, // seconds since UNIX epoch
}

fn default_quantity() -> i64 {
    1
}

fn default_timestamp() -> i64 {
    // If no timestamp provided, use current time
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThrottleResponse {
    pub allowed: bool,
    pub limit: i64,
    pub remaining: i64,
    pub reset_after: i64, // seconds
    pub retry_after: i64, // seconds
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

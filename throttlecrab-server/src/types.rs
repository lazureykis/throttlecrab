use serde::{Deserialize, Serialize};
use std::time::SystemTime;
use throttlecrab::RateLimitResult;

#[derive(Debug, Clone)]
pub struct ThrottleRequest {
    pub key: String,
    pub max_burst: i64,
    pub count_per_period: i64,
    pub period: i64, // seconds
    pub quantity: i64,
    pub timestamp: SystemTime,
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

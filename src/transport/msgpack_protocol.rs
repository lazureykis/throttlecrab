use crate::types::{ThrottleRequest, ThrottleResponse};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MsgPackRequest {
    pub cmd: u8, // 1 = throttle
    pub key: String,
    pub burst: i64,
    pub rate: i64,
    pub period: i64,
    #[serde(default = "default_quantity")]
    pub quantity: i64,
}

fn default_quantity() -> i64 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MsgPackResponse {
    pub ok: bool,
    pub allowed: u8, // 0 or 1
    pub limit: i64,
    pub remaining: i64,
    pub retry_after: i64,
    pub reset_after: i64,
}

impl From<MsgPackRequest> for ThrottleRequest {
    fn from(req: MsgPackRequest) -> Self {
        ThrottleRequest {
            key: req.key,
            max_burst: req.burst,
            count_per_period: req.rate,
            period: req.period,
            quantity: req.quantity,
        }
    }
}

impl From<ThrottleResponse> for MsgPackResponse {
    fn from(resp: ThrottleResponse) -> Self {
        MsgPackResponse {
            ok: true,
            allowed: if resp.allowed { 1 } else { 0 },
            limit: resp.limit,
            remaining: resp.remaining,
            retry_after: resp.retry_after,
            reset_after: resp.reset_after,
        }
    }
}

impl MsgPackResponse {
    pub fn error(message: &str) -> Self {
        tracing::error!("Error response: {}", message);
        MsgPackResponse {
            ok: false,
            allowed: 0,
            limit: 0,
            remaining: 0,
            retry_after: 0,
            reset_after: 0,
        }
    }
}

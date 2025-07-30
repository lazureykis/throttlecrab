pub mod compact_protocol;
pub mod msgpack;
pub mod msgpack_optimized;
pub mod msgpack_protocol;

use crate::actor::RateLimiterHandle;
use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait Transport {
    async fn start(self, limiter: RateLimiterHandle) -> Result<()>;
}

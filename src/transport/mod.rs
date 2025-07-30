pub mod compact_protocol;
pub mod grpc;
pub mod http;
pub mod msgpack;
pub mod msgpack_optimized;
pub mod msgpack_protocol;

#[cfg(test)]
mod http_test;

use crate::actor::RateLimiterHandle;
use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait Transport {
    async fn start(self, limiter: RateLimiterHandle) -> Result<()>;
}

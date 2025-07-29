pub mod rate;
pub mod rate_limiter;
pub mod store;
#[cfg(test)]
mod tests;

pub use rate::Rate;
pub use rate_limiter::{RateLimitResult, RateLimiter};
pub use store::{MemoryStore, Store};

#[derive(Debug, thiserror::Error)]
pub enum CellError {
    #[error("negative quantity: {0}")]
    NegativeQuantity(i64),

    #[error("invalid rate limit parameters")]
    InvalidRateLimit,

    #[error("internal error: {0}")]
    Internal(String),
}

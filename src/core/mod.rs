pub mod rate;
pub mod rate_limiter;
pub mod store;
#[cfg(test)]
mod tests;

pub use rate::Rate;
pub use rate_limiter::{RateLimitResult, RateLimiter};
pub use store::{MemoryStore, Store};

use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub enum CellError {
    NegativeQuantity(i64),
    InvalidRateLimit,
    Internal(String),
}

impl fmt::Display for CellError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CellError::NegativeQuantity(n) => write!(f, "negative quantity: {n}"),
            CellError::InvalidRateLimit => write!(f, "invalid rate limit parameters"),
            CellError::Internal(msg) => write!(f, "internal error: {msg}"),
        }
    }
}

impl Error for CellError {}

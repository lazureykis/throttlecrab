//! Core components of the throttlecrab rate limiting library
//!
//! This module contains the fundamental building blocks:
//! - [`rate`]: Rate calculation and emission intervals
//! - [`rate_limiter`]: The main GCRA rate limiter implementation
//! - [`store`]: Storage backends for rate limit state

pub mod rate;
pub mod rate_limiter;
pub mod store;
#[cfg(test)]
mod tests;

pub use rate::Rate;
pub use rate_limiter::{RateLimitResult, RateLimiter};
pub use store::{
    AdaptiveStore, AdaptiveStoreBuilder, PeriodicStore, PeriodicStoreBuilder, ProbabilisticStore,
    ProbabilisticStoreBuilder, Store,
};

use std::error::Error;
use std::fmt;

/// Errors that can occur during rate limiting operations
///
/// # Variants
///
/// - [`NegativeQuantity`](CellError::NegativeQuantity): The quantity parameter was negative
/// - [`InvalidRateLimit`](CellError::InvalidRateLimit): Rate limit parameters are invalid (e.g., zero or negative)
/// - [`Internal`](CellError::Internal): An internal error occurred (e.g., time calculation error)
///
/// # Example
///
/// ```
/// use throttlecrab::{RateLimiter, PeriodicStore, CellError};
/// use std::time::SystemTime;
///
/// let mut limiter = RateLimiter::new(PeriodicStore::new());
///
/// // This will return an error because quantity is negative
/// match limiter.rate_limit("key", 10, 100, 60, -1, SystemTime::now()) {
///     Err(CellError::NegativeQuantity(n)) => {
///         println!("Error: negative quantity {}", n);
///     }
///     _ => {}
/// }
/// ```
#[derive(Debug)]
pub enum CellError {
    /// The quantity parameter was negative
    NegativeQuantity(i64),
    /// Rate limit parameters are invalid (max_burst, count_per_period, or period <= 0)
    InvalidRateLimit,
    /// An internal error occurred
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

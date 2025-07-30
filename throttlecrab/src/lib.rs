pub mod core;

pub use core::{
    AdaptiveStore, CellError, PeriodicStore, ProbabilisticStore, Rate, RateLimitResult,
    RateLimiter, Store,
};

// Re-export the store module so benchmarks can access it
pub use crate::core::store;

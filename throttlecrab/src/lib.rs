pub mod core;

pub use core::{
    AdaptiveStore, AdaptiveStoreBuilder, CellError, PeriodicStore, PeriodicStoreBuilder,
    ProbabilisticStore, ProbabilisticStoreBuilder, Rate, RateLimitResult, RateLimiter, Store,
};

// Re-export the store module so benchmarks can access it
pub use crate::core::store;

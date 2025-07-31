//! # ThrottleCrab
//!
//! A high-performance GCRA (Generic Cell Rate Algorithm) rate limiter library for Rust.
//!
//! ## Overview
//!
//! ThrottleCrab implements the Generic Cell Rate Algorithm (GCRA), which provides:
//! - **Smooth traffic shaping**: No sudden bursts followed by long waits
//! - **Precise rate limiting**: Exact control over request rates
//! - **Fairness**: All clients get predictable access to resources
//! - **Memory efficiency**: O(1) space per key
//!
//! ## Quick Start
//!
//! ```
//! use throttlecrab::{RateLimiter, AdaptiveStore};
//! use std::time::SystemTime;
//!
//! // Create a rate limiter with adaptive store
//! let mut limiter = RateLimiter::new(AdaptiveStore::new());
//!
//! // Check rate limit: 10 burst, 100 requests per 60 seconds
//! let (allowed, result) = limiter
//!     .rate_limit("user:123", 10, 100, 60, 1, SystemTime::now())
//!     .unwrap();
//!
//! if allowed {
//!     println!("Request allowed! Remaining: {}", result.remaining);
//! } else {
//!     println!("Rate limited! Retry after: {} seconds", result.retry_after.as_secs());
//! }
//! ```
//!
//! ## Store Types
//!
//! ThrottleCrab provides several store implementations optimized for different use cases:
//!
//! ### [`AdaptiveStore`]
//! Dynamically adjusts cleanup frequency based on usage patterns. Best for variable workloads.
//!
//! ```
//! use throttlecrab::AdaptiveStore;
//!
//! let store = AdaptiveStore::builder()
//!     .capacity(1_000_000)
//!     .min_interval(std::time::Duration::from_secs(5))
//!     .max_interval(std::time::Duration::from_secs(300))
//!     .build();
//! ```
//!
//! ### [`PeriodicStore`]
//! Cleans up expired entries at fixed intervals. Best for predictable workloads.
//!
//! ```
//! use throttlecrab::PeriodicStore;
//!
//! let store = PeriodicStore::builder()
//!     .capacity(500_000)
//!     .cleanup_interval(std::time::Duration::from_secs(60))
//!     .build();
//! ```
//!
//! ### [`ProbabilisticStore`]
//! Uses random sampling for cleanup. Best for high-throughput scenarios.
//!
//! ```
//! use throttlecrab::ProbabilisticStore;
//!
//! let store = ProbabilisticStore::builder()
//!     .capacity(2_000_000)
//!     .cleanup_probability(10_000) // 1 in 10,000 chance
//!     .build();
//! ```
//!
//! ## Common Use Cases
//!
//! ### API Rate Limiting
//! ```
//! use throttlecrab::{RateLimiter, AdaptiveStore};
//! use std::time::SystemTime;
//!
//! let mut limiter = RateLimiter::new(AdaptiveStore::new());
//!
//! // Limit each API key to 1000 requests per minute with burst of 50
//! let api_key = "api_key_12345";
//! let (allowed, result) = limiter
//!     .rate_limit(api_key, 50, 1000, 60, 1, SystemTime::now())?;
//! # Ok::<(), throttlecrab::CellError>(())
//! ```
//!
//! ### User Action Throttling
//! ```
//! use throttlecrab::{RateLimiter, PeriodicStore};
//! use std::time::SystemTime;
//!
//! let mut limiter = RateLimiter::new(PeriodicStore::new());
//!
//! // Limit password reset attempts: 3 per hour, minimal burst
//! let user_id = "user:456";
//! let (allowed, _) = limiter
//!     .rate_limit(&format!("password_reset:{}", user_id), 1, 3, 3600, 1, SystemTime::now())?;
//! # Ok::<(), throttlecrab::CellError>(())
//! ```
//!
//! ### Resource Protection
//! ```
//! use throttlecrab::{RateLimiter, ProbabilisticStore};
//! use std::time::SystemTime;
//!
//! let mut limiter = RateLimiter::new(ProbabilisticStore::new());
//!
//! // Limit expensive operations: 10 per minute with burst of 2
//! let (allowed, _) = limiter
//!     .rate_limit("expensive_operation", 2, 10, 60, 1, SystemTime::now())?;
//! # Ok::<(), throttlecrab::CellError>(())
//! ```
//!
//! ## Understanding GCRA Parameters
//!
//! - **`max_burst`**: Maximum number of requests allowed in a burst
//! - **`count_per_period`**: Number of requests allowed per time period
//! - **`period`**: Time period in seconds
//! - **`quantity`**: Number of tokens to consume (default: 1)
//!
//! ## Thread Safety
//!
//! The rate limiter itself is not thread-safe. For concurrent access, wrap it in a mutex:
//!
//! ```
//! use std::sync::{Arc, Mutex};
//! use throttlecrab::{RateLimiter, AdaptiveStore};
//!
//! let limiter = Arc::new(Mutex::new(RateLimiter::new(AdaptiveStore::new())));
//! ```
//!
//! ## Features
//!
//! - `ahash` (default): Use AHash for faster hashing

pub mod core;

pub use core::{
    AdaptiveStore, AdaptiveStoreBuilder, CellError, PeriodicStore, PeriodicStoreBuilder,
    ProbabilisticStore, ProbabilisticStoreBuilder, Rate, RateLimitResult, RateLimiter, Store,
};

// Re-export the store module so benchmarks can access it
pub use crate::core::store;

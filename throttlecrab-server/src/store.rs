//! Store factory for creating rate limiter instances
//!
//! This module provides a factory function to create the appropriate
//! rate limiter store based on configuration.
//!
//! # Store Types
//!
//! The server supports three different store implementations:
//!
//! ## Periodic Store
//! - Cleanups occur at fixed intervals
//! - Predictable memory usage patterns
//! - Best for: Consistent workloads with predictable traffic
//!
//! ## Probabilistic Store
//! - Cleanups occur randomly based on probability
//! - Lower overhead but less predictable
//! - Best for: Variable workloads where cleanup timing isn't critical
//!
//! ## Adaptive Store
//! - Cleanup frequency adjusts based on load
//! - Balances performance and memory usage
//! - Best for: Workloads with varying traffic patterns

use crate::actor::{RateLimiterActor, RateLimiterHandle};
use crate::config::{StoreConfig, StoreType};
use std::time::Duration;
use throttlecrab::{AdaptiveStore, PeriodicStore, ProbabilisticStore};

/// Create a rate limiter actor with the configured store
///
/// This factory function creates the appropriate store type based on
/// configuration and spawns an actor to manage it.
///
/// # Parameters
///
/// - `config`: Store configuration specifying type and parameters
/// - `buffer_size`: Channel buffer size for actor communication
///
/// # Returns
///
/// A handle to communicate with the spawned rate limiter actor
///
/// # Example
///
/// ```ignore
/// let config = StoreConfig {
///     store_type: StoreType::Adaptive,
///     capacity: 100_000,
///     // ... other fields
/// };
/// let limiter = create_rate_limiter(&config, 10_000);
/// ```
pub fn create_rate_limiter(config: &StoreConfig, buffer_size: usize) -> RateLimiterHandle {
    match config.store_type {
        StoreType::Periodic => {
            let store = PeriodicStore::builder()
                .capacity(config.capacity)
                .cleanup_interval(Duration::from_secs(config.cleanup_interval))
                .build();
            RateLimiterActor::spawn_periodic(buffer_size, store)
        }
        StoreType::Probabilistic => {
            let store = ProbabilisticStore::builder()
                .capacity(config.capacity)
                .cleanup_probability(config.cleanup_probability)
                .build();
            RateLimiterActor::spawn_probabilistic(buffer_size, store)
        }
        StoreType::Adaptive => {
            let store = AdaptiveStore::builder()
                .capacity(config.capacity)
                .min_interval(Duration::from_secs(config.min_interval))
                .max_interval(Duration::from_secs(config.max_interval))
                .max_operations(config.max_operations)
                .build();
            RateLimiterActor::spawn_adaptive(buffer_size, store)
        }
    }
}

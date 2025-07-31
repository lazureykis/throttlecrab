use crate::actor::{RateLimiterActor, RateLimiterHandle};
use crate::config::{StoreConfig, StoreType};
use std::time::Duration;
use throttlecrab::{AdaptiveStore, PeriodicStore, ProbabilisticStore};

/// Create a rate limiter actor with the configured store
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

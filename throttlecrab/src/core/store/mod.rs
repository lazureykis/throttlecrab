use std::time::{Duration, SystemTime};

#[cfg(test)]
mod tests;

mod adaptive_cleanup;
mod fast_hasher;
mod periodic;
mod probabilistic;

pub use adaptive_cleanup::{AdaptiveStore, AdaptiveStoreBuilder};
pub use periodic::{PeriodicStore, PeriodicStoreBuilder};
pub use probabilistic::{ProbabilisticStore, ProbabilisticStoreBuilder};

#[cfg(test)]
mod cleanup_test;

#[cfg(test)]
mod store_test_suite;

#[cfg(test)]
mod tests_builders;

/// Store trait for rate limiter state storage (similar to redis-cell)
pub trait Store {
    /// Compare and swap with TTL
    fn compare_and_swap_with_ttl(
        &mut self,
        key: &str,
        old: i64,
        new: i64,
        ttl: Duration,
        now: SystemTime,
    ) -> Result<bool, String>;

    /// Get value
    fn get(&self, key: &str, now: SystemTime) -> Result<Option<i64>, String>;

    /// Set if not exists with TTL
    fn set_if_not_exists_with_ttl(
        &mut self,
        key: &str,
        value: i64,
        ttl: Duration,
        now: SystemTime,
    ) -> Result<bool, String>;
}

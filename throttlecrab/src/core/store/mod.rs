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

/// Storage backend for rate limiter state
///
/// The `Store` trait defines the interface for persisting rate limiter state.
/// Implementations manage the storage and retrieval of rate limit data with
/// support for atomic operations and TTL (time-to-live) expiration.
///
/// # Thread Safety
///
/// Store implementations are not required to be thread-safe. For concurrent
/// access, wrap the rate limiter in appropriate synchronization primitives.
///
/// # Example Implementation
///
/// ```ignore
/// use std::time::{Duration, SystemTime};
/// use throttlecrab::Store;
///
/// struct MyStore {
///     // Your storage implementation
/// }
///
/// impl Store for MyStore {
///     fn compare_and_swap_with_ttl(
///         &mut self,
///         key: &str,
///         old: i64,
///         new: i64,
///         ttl: Duration,
///         now: SystemTime,
///     ) -> Result<bool, String> {
///         // Implement atomic compare-and-swap
///         Ok(true)
///     }
///
///     fn get(&self, key: &str, now: SystemTime) -> Result<Option<i64>, String> {
///         // Retrieve value for key
///         Ok(None)
///     }
///
///     fn set_if_not_exists_with_ttl(
///         &mut self,
///         key: &str,
///         value: i64,
///         ttl: Duration,
///         now: SystemTime,
///     ) -> Result<bool, String> {
///         // Set value only if key doesn't exist
///         Ok(true)
///     }
/// }
/// ```
pub trait Store {
    /// Atomically compare and swap a value with TTL
    ///
    /// Updates the value for `key` from `old` to `new` only if the current
    /// value matches `old`. The entry will expire after `ttl` duration.
    ///
    /// # Returns
    ///
    /// - `Ok(true)` if the swap was successful
    /// - `Ok(false)` if the current value doesn't match `old`
    /// - `Err` if an error occurred
    fn compare_and_swap_with_ttl(
        &mut self,
        key: &str,
        old: i64,
        new: i64,
        ttl: Duration,
        now: SystemTime,
    ) -> Result<bool, String>;

    /// Get the current value for a key
    ///
    /// Returns the value associated with `key`, or `None` if the key doesn't
    /// exist or has expired.
    ///
    /// # Parameters
    ///
    /// - `key`: The key to look up
    /// - `now`: Current time for expiration checks
    fn get(&self, key: &str, now: SystemTime) -> Result<Option<i64>, String>;

    /// Set a value with TTL if the key doesn't exist
    ///
    /// Creates a new entry with the given value and TTL only if the key
    /// doesn't already exist.
    ///
    /// # Returns
    ///
    /// - `Ok(true)` if the value was set
    /// - `Ok(false)` if the key already exists
    /// - `Err` if an error occurred
    fn set_if_not_exists_with_ttl(
        &mut self,
        key: &str,
        value: i64,
        ttl: Duration,
        now: SystemTime,
    ) -> Result<bool, String>;
}

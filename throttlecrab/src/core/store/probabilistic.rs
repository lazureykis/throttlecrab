use super::Store;
use std::time::{Duration, SystemTime};

#[cfg(feature = "ahash")]
use ahash::AHashMap as HashMap;
#[cfg(not(feature = "ahash"))]
use std::collections::HashMap;

// Configuration constants
const DEFAULT_CAPACITY: usize = 1000;
const CAPACITY_OVERHEAD_FACTOR: f64 = 1.3;
const PROBABILISTIC_CLEANUP_MODULO: u64 = 1000; // 0.1% chance

/// Random-sampling cleanup store implementation
///
/// This store uses probabilistic cleanup where each operation has a small
/// chance to trigger a cleanup cycle. Best suited for high-throughput
/// applications where periodic cleanup would cause latency spikes.
///
/// # Features
///
/// - Cleanup probability configurable (e.g., 1 in 10,000 operations)
/// - Distributes cleanup cost across all operations
/// - No periodic latency spikes
/// - Excellent for very high request rates
///
/// # Example
///
/// ```
/// use throttlecrab::{RateLimiter, ProbabilisticStore};
/// use std::time::SystemTime;
///
/// // Clean up with 1 in 5000 probability (0.02% chance per operation)
/// let store = ProbabilisticStore::builder()
///     .cleanup_probability(5000)
///     .build();
/// let mut limiter = RateLimiter::new(store);
/// ```
///
/// # Cleanup Strategy
///
/// Uses a deterministic pseudo-random approach based on operation count,
/// ensuring uniform distribution of cleanup operations over time.
pub struct ProbabilisticStore {
    data: HashMap<String, (i64, Option<SystemTime>)>,
    operations_count: u64,
    cleanup_probability: u64,
}

/// Builder for configuring a ProbabilisticStore
///
/// Provides a fluent interface for customizing the probabilistic store's behavior.
///
/// # Example
///
/// ```
/// use throttlecrab::ProbabilisticStore;
///
/// let store = ProbabilisticStore::builder()
///     .capacity(1_000_000)
///     .cleanup_probability(10_000) // 1 in 10,000 chance
///     .build();
/// ```
pub struct ProbabilisticStoreBuilder {
    capacity: usize,
    cleanup_probability: u64,
}

impl ProbabilisticStore {
    /// Create a new ProbabilisticStore with default configuration
    ///
    /// Uses a default capacity of 1000 entries and cleanup probability of 1/1000.
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_CAPACITY)
    }

    /// Create a new ProbabilisticStore with specified capacity
    ///
    /// The store will allocate 30% more space to reduce hash collisions.
    ///
    /// # Parameters
    ///
    /// - `capacity`: Expected number of unique keys to track
    pub fn with_capacity(capacity: usize) -> Self {
        ProbabilisticStore {
            data: HashMap::with_capacity((capacity as f64 * CAPACITY_OVERHEAD_FACTOR) as usize),
            operations_count: 0,
            cleanup_probability: PROBABILISTIC_CLEANUP_MODULO,
        }
    }

    /// Create a new builder for configuring a ProbabilisticStore
    ///
    /// Provides fine-grained control over store configuration.
    pub fn builder() -> ProbabilisticStoreBuilder {
        ProbabilisticStoreBuilder {
            capacity: DEFAULT_CAPACITY,
            cleanup_probability: PROBABILISTIC_CLEANUP_MODULO,
        }
    }

    fn with_config(capacity: usize, cleanup_probability: u64) -> Self {
        ProbabilisticStore {
            data: HashMap::with_capacity((capacity as f64 * CAPACITY_OVERHEAD_FACTOR) as usize),
            operations_count: 0,
            cleanup_probability,
        }
    }

    fn maybe_cleanup(&mut self, now: SystemTime) {
        self.operations_count += 1;

        // Simple pseudo-random using operations count
        // This gives uniform distribution over time while being deterministic
        let hash = self.operations_count.wrapping_mul(2654435761); // Prime multiplier
        if hash % self.cleanup_probability == 0 {
            self.data.retain(|_, (_, expiry)| {
                if let Some(exp) = expiry {
                    *exp > now
                } else {
                    true
                }
            });
        }
    }
}

impl Default for ProbabilisticStore {
    fn default() -> Self {
        Self::new()
    }
}

impl Store for ProbabilisticStore {
    fn compare_and_swap_with_ttl(
        &mut self,
        key: &str,
        old: i64,
        new: i64,
        ttl: Duration,
        now: SystemTime,
    ) -> Result<bool, String> {
        self.maybe_cleanup(now);

        match self.data.get(key) {
            Some((_current, Some(expiry))) if *expiry <= now => Ok(false),
            Some((current, _)) if *current == old => {
                let expiry = now + ttl;
                self.data.insert(key.to_string(), (new, Some(expiry)));
                Ok(true)
            }
            Some(_) => Ok(false),
            None => Ok(false),
        }
    }

    fn get(&self, key: &str, now: SystemTime) -> Result<Option<i64>, String> {
        match self.data.get(key) {
            Some((value, Some(expiry))) if *expiry > now => Ok(Some(*value)),
            Some((value, None)) => Ok(Some(*value)),
            _ => Ok(None),
        }
    }

    fn set_if_not_exists_with_ttl(
        &mut self,
        key: &str,
        value: i64,
        ttl: Duration,
        now: SystemTime,
    ) -> Result<bool, String> {
        self.maybe_cleanup(now);

        match self.data.get(key) {
            Some((_, Some(expiry))) if *expiry > now => Ok(false),
            Some((_, None)) => Ok(false),
            _ => {
                let expiry = now + ttl;
                self.data.insert(key.to_string(), (value, Some(expiry)));
                Ok(true)
            }
        }
    }
}

impl Default for ProbabilisticStoreBuilder {
    fn default() -> Self {
        Self {
            capacity: DEFAULT_CAPACITY,
            cleanup_probability: PROBABILISTIC_CLEANUP_MODULO,
        }
    }
}

impl ProbabilisticStoreBuilder {
    /// Create a new builder with default settings
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the expected capacity (number of unique keys)
    ///
    /// The store will allocate 30% more space to reduce hash collisions.
    pub fn capacity(mut self, capacity: usize) -> Self {
        self.capacity = capacity;
        self
    }

    /// Set the cleanup probability denominator
    ///
    /// Cleanup will occur with probability 1/n where n is the value provided.
    /// For example, `cleanup_probability(10_000)` means cleanup happens
    /// approximately once every 10,000 operations.
    ///
    /// # Example
    ///
    /// ```
    /// use throttlecrab::ProbabilisticStore;
    ///
    /// let store = ProbabilisticStore::builder()
    ///     .cleanup_probability(5_000) // 1 in 5,000 chance (0.02%)
    ///     .build();
    /// ```
    pub fn cleanup_probability(mut self, probability: u64) -> Self {
        self.cleanup_probability = probability;
        self
    }

    /// Build the ProbabilisticStore with the configured settings
    pub fn build(self) -> ProbabilisticStore {
        ProbabilisticStore::with_config(self.capacity, self.cleanup_probability)
    }
}

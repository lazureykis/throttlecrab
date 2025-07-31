use super::Store;
use std::time::{Duration, SystemTime};

#[cfg(feature = "ahash")]
use ahash::AHashMap as HashMap;
#[cfg(not(feature = "ahash"))]
use std::collections::HashMap;

// Configuration constants
const DEFAULT_CAPACITY: usize = 1000;
const CAPACITY_OVERHEAD_FACTOR: f64 = 1.3;
const MIN_CLEANUP_INTERVAL_SECS: u64 = 1;
const MAX_CLEANUP_INTERVAL_SECS: u64 = 300; // 5 minutes
const DEFAULT_CLEANUP_INTERVAL_SECS: u64 = 5;
const MAX_OPERATIONS_BEFORE_CLEANUP: usize = 100_000;
const EXPIRED_RATIO_THRESHOLD: f64 = 0.2; // 20%

/// Adaptive cleanup store implementation
///
/// This store dynamically adjusts its cleanup frequency based on usage patterns,
/// making it ideal for variable workloads. It monitors the ratio of expired entries
/// and adjusts cleanup intervals accordingly.
///
/// # Features
///
/// - Self-tuning cleanup intervals
/// - Monitors expired entry ratio
/// - Adjusts between min and max cleanup intervals
/// - Triggers cleanup based on time or operation count
///
/// # Example
///
/// ```
/// use throttlecrab::{RateLimiter, AdaptiveStore};
/// use std::time::SystemTime;
///
/// let mut limiter = RateLimiter::new(AdaptiveStore::new());
/// ```
pub struct AdaptiveStore {
    data: HashMap<String, (i64, Option<SystemTime>)>,
    // Cleanup timing
    next_cleanup: SystemTime,
    min_cleanup_interval: Duration,
    max_cleanup_interval: Duration,
    current_cleanup_interval: Duration,
    // Cleanup triggers
    expired_count: usize,
    operations_since_cleanup: usize,
    max_operations_before_cleanup: usize,
    // Cleanup history for adaptation
    last_cleanup_removed: usize,
    last_cleanup_total: usize,
}

/// Builder for configuring an AdaptiveStore
///
/// Provides a fluent interface for customizing the adaptive store's behavior.
///
/// # Example
///
/// ```
/// use throttlecrab::AdaptiveStore;
///
/// let store = AdaptiveStore::builder()
///     .capacity(1_000_000)
///     .min_interval(std::time::Duration::from_secs(5))
///     .max_interval(std::time::Duration::from_secs(300))
///     .max_operations(100_000)
///     .build();
/// ```
pub struct AdaptiveStoreBuilder {
    capacity: usize,
    min_cleanup_interval: Duration,
    max_cleanup_interval: Duration,
    max_operations_before_cleanup: usize,
}

impl AdaptiveStore {
    /// Create a new AdaptiveStore with default configuration
    ///
    /// Uses a default capacity of 1000 entries and standard cleanup intervals.
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_CAPACITY)
    }

    /// Create a new AdaptiveStore with specified capacity
    ///
    /// # Parameters
    ///
    /// - `capacity`: Expected number of unique keys to track
    pub fn with_capacity(capacity: usize) -> Self {
        AdaptiveStore {
            data: HashMap::with_capacity((capacity as f64 * CAPACITY_OVERHEAD_FACTOR) as usize),
            next_cleanup: SystemTime::now() + Duration::from_secs(DEFAULT_CLEANUP_INTERVAL_SECS),
            min_cleanup_interval: Duration::from_secs(MIN_CLEANUP_INTERVAL_SECS),
            max_cleanup_interval: Duration::from_secs(MAX_CLEANUP_INTERVAL_SECS),
            current_cleanup_interval: Duration::from_secs(DEFAULT_CLEANUP_INTERVAL_SECS),
            expired_count: 0,
            operations_since_cleanup: 0,
            max_operations_before_cleanup: MAX_OPERATIONS_BEFORE_CLEANUP,
            last_cleanup_removed: 0,
            last_cleanup_total: 0,
        }
    }

    /// Create a new builder for configuring an AdaptiveStore
    ///
    /// Provides fine-grained control over store configuration.
    pub fn builder() -> AdaptiveStoreBuilder {
        AdaptiveStoreBuilder {
            capacity: DEFAULT_CAPACITY,
            min_cleanup_interval: Duration::from_secs(MIN_CLEANUP_INTERVAL_SECS),
            max_cleanup_interval: Duration::from_secs(MAX_CLEANUP_INTERVAL_SECS),
            max_operations_before_cleanup: MAX_OPERATIONS_BEFORE_CLEANUP,
        }
    }

    fn with_config(
        capacity: usize,
        min_cleanup_interval: Duration,
        max_cleanup_interval: Duration,
        max_operations_before_cleanup: usize,
    ) -> Self {
        AdaptiveStore {
            data: HashMap::with_capacity((capacity as f64 * CAPACITY_OVERHEAD_FACTOR) as usize),
            next_cleanup: SystemTime::now() + Duration::from_secs(DEFAULT_CLEANUP_INTERVAL_SECS),
            min_cleanup_interval,
            max_cleanup_interval,
            current_cleanup_interval: Duration::from_secs(DEFAULT_CLEANUP_INTERVAL_SECS),
            expired_count: 0,
            operations_since_cleanup: 0,
            max_operations_before_cleanup,
            last_cleanup_removed: 0,
            last_cleanup_total: 0,
        }
    }

    fn should_clean(&self, now: SystemTime) -> bool {
        // Time-based trigger
        if now >= self.next_cleanup {
            return true;
        }

        // Operation count trigger (prevent unbounded growth)
        if self.operations_since_cleanup >= self.max_operations_before_cleanup {
            return true;
        }

        // Expired percentage trigger with dynamic threshold
        if self.expired_count > 50 {
            let expired_ratio = self.expired_count as f64 / self.data.len().max(1) as f64;

            // More aggressive cleanup if we removed a lot last time
            let threshold = if self.last_cleanup_removed > self.last_cleanup_total / 4 {
                EXPIRED_RATIO_THRESHOLD / 2.0 // Clean at half threshold if last cleanup was productive
            } else {
                EXPIRED_RATIO_THRESHOLD * 1.25 // Otherwise wait until 125% of threshold
            };

            if expired_ratio > threshold {
                return true;
            }
        }

        // Memory pressure trigger (if HashMap is getting too large)
        if self.data.len() > self.data.capacity() * 3 / 4 {
            return true;
        }

        false
    }

    fn cleanup(&mut self, now: SystemTime) {
        let initial_len = self.data.len();

        self.data.retain(|_, (_, expiry)| {
            if let Some(exp) = expiry {
                *exp > now
            } else {
                true
            }
        });

        let removed = initial_len - self.data.len();

        // Adaptive interval adjustment
        if removed == 0 && self.expired_count == 0 {
            // No expired entries, increase interval
            self.current_cleanup_interval =
                (self.current_cleanup_interval * 2).min(self.max_cleanup_interval);
        } else if removed as f64 > initial_len as f64 * 0.5 {
            // Removed many entries, decrease interval
            self.current_cleanup_interval =
                (self.current_cleanup_interval / 2).max(self.min_cleanup_interval);
        }

        // Update state
        self.last_cleanup_removed = removed;
        self.last_cleanup_total = initial_len;
        self.next_cleanup = now + self.current_cleanup_interval;
        self.expired_count = 0;
        self.operations_since_cleanup = 0;
    }

    fn maybe_clean_expired(&mut self, now: SystemTime) {
        self.operations_since_cleanup += 1;

        if self.should_clean(now) {
            self.cleanup(now);
        }
    }
}

impl Default for AdaptiveStore {
    fn default() -> Self {
        Self::new()
    }
}

impl Store for AdaptiveStore {
    fn compare_and_swap_with_ttl(
        &mut self,
        key: &str,
        old: i64,
        new: i64,
        ttl: Duration,
        now: SystemTime,
    ) -> Result<bool, String> {
        self.maybe_clean_expired(now);

        match self.data.get(key) {
            Some((_current, Some(expiry))) if *expiry <= now => {
                self.expired_count += 1;
                Ok(false)
            }
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
        self.maybe_clean_expired(now);

        match self.data.get(key) {
            Some((_, Some(expiry))) if *expiry > now => Ok(false),
            Some((_, None)) => Ok(false),
            Some((_, Some(_expiry))) => {
                self.expired_count += 1;
                let expiry = now + ttl;
                self.data.insert(key.to_string(), (value, Some(expiry)));
                Ok(true)
            }
            None => {
                let expiry = now + ttl;
                self.data.insert(key.to_string(), (value, Some(expiry)));
                Ok(true)
            }
        }
    }
}

impl Default for AdaptiveStoreBuilder {
    fn default() -> Self {
        Self {
            capacity: DEFAULT_CAPACITY,
            min_cleanup_interval: Duration::from_secs(MIN_CLEANUP_INTERVAL_SECS),
            max_cleanup_interval: Duration::from_secs(MAX_CLEANUP_INTERVAL_SECS),
            max_operations_before_cleanup: MAX_OPERATIONS_BEFORE_CLEANUP,
        }
    }
}

impl AdaptiveStoreBuilder {
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

    /// Set the minimum cleanup interval
    ///
    /// Cleanup will never run more frequently than this interval.
    pub fn min_interval(mut self, interval: Duration) -> Self {
        self.min_cleanup_interval = interval;
        self
    }

    /// Set the maximum cleanup interval
    ///
    /// Cleanup will run at least this often, even with low expired entry ratios.
    pub fn max_interval(mut self, interval: Duration) -> Self {
        self.max_cleanup_interval = interval;
        self
    }

    /// Set the maximum operations before forcing a cleanup
    ///
    /// This prevents unbounded memory growth under high load.
    pub fn max_operations(mut self, max_ops: usize) -> Self {
        self.max_operations_before_cleanup = max_ops;
        self
    }

    /// Build the AdaptiveStore with the configured settings
    pub fn build(self) -> AdaptiveStore {
        AdaptiveStore::with_config(
            self.capacity,
            self.min_cleanup_interval,
            self.max_cleanup_interval,
            self.max_operations_before_cleanup,
        )
    }
}

// Note: A background cleanup approach could be implemented with async runtime
// and additional dependencies (Arc, parking_lot, tokio channels) but is
// omitted here to maintain zero dependencies

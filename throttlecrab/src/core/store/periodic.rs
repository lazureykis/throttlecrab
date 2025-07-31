use super::Store;
use std::time::{Duration, SystemTime};

#[cfg(feature = "ahash")]
use ahash::AHashMap as HashMap;
#[cfg(not(feature = "ahash"))]
use std::collections::HashMap;

// Configuration constants
const DEFAULT_CAPACITY: usize = 1000;
const CAPACITY_OVERHEAD_FACTOR: f64 = 1.3;
const DEFAULT_CLEANUP_INTERVAL_SECS: u64 = 60;

/// Periodic cleanup store implementation
/// Cleans up expired entries at regular time intervals
pub struct PeriodicStore {
    data: HashMap<String, (i64, Option<SystemTime>)>,
    // Track when next cleanup is needed
    next_cleanup: SystemTime,
    // Cleanup interval
    cleanup_interval: Duration,
    // Track number of expired entries
    expired_count: usize,
}

/// Builder for PeriodicStore
pub struct PeriodicStoreBuilder {
    capacity: usize,
    cleanup_interval: Duration,
}

impl PeriodicStore {
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_CAPACITY)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        PeriodicStore {
            // Pre-allocate with overhead to avoid rehashing
            data: HashMap::with_capacity((capacity as f64 * CAPACITY_OVERHEAD_FACTOR) as usize),
            next_cleanup: SystemTime::now() + Duration::from_secs(DEFAULT_CLEANUP_INTERVAL_SECS),
            cleanup_interval: Duration::from_secs(DEFAULT_CLEANUP_INTERVAL_SECS),
            expired_count: 0,
        }
    }

    pub fn builder() -> PeriodicStoreBuilder {
        PeriodicStoreBuilder {
            capacity: DEFAULT_CAPACITY,
            cleanup_interval: Duration::from_secs(DEFAULT_CLEANUP_INTERVAL_SECS),
        }
    }

    fn with_config(capacity: usize, cleanup_interval: Duration) -> Self {
        PeriodicStore {
            data: HashMap::with_capacity((capacity as f64 * CAPACITY_OVERHEAD_FACTOR) as usize),
            next_cleanup: SystemTime::now() + cleanup_interval,
            cleanup_interval,
            expired_count: 0,
        }
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    #[cfg(test)]
    pub fn expired_count(&self) -> usize {
        self.expired_count
    }

    fn maybe_clean_expired(&mut self, now: SystemTime) {
        // Clean periodically based on time
        if now >= self.next_cleanup {
            let before_count = self.data.len();
            self.data.retain(|_, (_, expiry)| {
                if let Some(exp) = expiry {
                    *exp > now
                } else {
                    true
                }
            });
            self.expired_count = before_count.saturating_sub(self.data.len());
            self.next_cleanup = now + self.cleanup_interval;
        }
    }
}

impl Default for PeriodicStore {
    fn default() -> Self {
        Self::new()
    }
}

impl Store for PeriodicStore {
    fn compare_and_swap_with_ttl(
        &mut self,
        key: &str,
        old: i64,
        new: i64,
        ttl: Duration,
        now: SystemTime,
    ) -> Result<bool, String> {
        // Only clean periodically, not on every operation
        self.maybe_clean_expired(now);

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
        self.maybe_clean_expired(now);

        // Check for existing non-expired key
        match self.data.get(key) {
            Some((_, Some(expiry))) if *expiry > now => Ok(false),
            Some((_, None)) => Ok(false),
            Some((_, Some(_expiry))) => {
                // Key is expired - insert the new value
                let expiry = now + ttl;
                self.data.insert(key.to_string(), (value, Some(expiry)));
                Ok(true)
            }
            None => {
                // Key doesn't exist
                let expiry = now + ttl;
                self.data.insert(key.to_string(), (value, Some(expiry)));
                Ok(true)
            }
        }
    }
}

impl PeriodicStoreBuilder {
    pub fn capacity(mut self, capacity: usize) -> Self {
        self.capacity = capacity;
        self
    }

    pub fn cleanup_interval(mut self, interval: Duration) -> Self {
        self.cleanup_interval = interval;
        self
    }

    pub fn build(self) -> PeriodicStore {
        PeriodicStore::with_config(self.capacity, self.cleanup_interval)
    }
}

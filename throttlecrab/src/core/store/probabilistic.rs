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

/// Probabilistic cleanup store implementation
/// Each operation has a small chance to trigger cleanup
pub struct ProbabilisticStore {
    data: HashMap<String, (i64, Option<SystemTime>)>,
    operations_count: u64,
    cleanup_probability: u64,
}

/// Builder for ProbabilisticStore
pub struct ProbabilisticStoreBuilder {
    capacity: usize,
    cleanup_probability: u64,
}

impl ProbabilisticStore {
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_CAPACITY)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        ProbabilisticStore {
            data: HashMap::with_capacity((capacity as f64 * CAPACITY_OVERHEAD_FACTOR) as usize),
            operations_count: 0,
            cleanup_probability: PROBABILISTIC_CLEANUP_MODULO,
        }
    }

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

    fn log_debug(&self, _message: &str) {
        // No-op in library
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

impl ProbabilisticStoreBuilder {
    pub fn capacity(mut self, capacity: usize) -> Self {
        self.capacity = capacity;
        self
    }

    pub fn cleanup_probability(mut self, probability: u64) -> Self {
        self.cleanup_probability = probability;
        self
    }

    pub fn build(self) -> ProbabilisticStore {
        ProbabilisticStore::with_config(self.capacity, self.cleanup_probability)
    }
}

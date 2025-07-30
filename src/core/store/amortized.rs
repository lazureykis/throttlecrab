use super::Store;
use std::time::{Duration, SystemTime};

#[cfg(feature = "ahash")]
use ahash::AHashMap as HashMap;
#[cfg(not(feature = "ahash"))]
use std::collections::HashMap;

// Configuration constants
const DEFAULT_CAPACITY: usize = 1000;
const CAPACITY_OVERHEAD_FACTOR: f64 = 1.3;
const DEFAULT_OPERATIONS_PER_CLEANUP: usize = 100;
const DEFAULT_ENTRIES_PER_CLEANUP: usize = 10;
const PROBABILISTIC_CLEANUP_MODULO: u64 = 1000; // 0.1% chance

/// Memory store with amortized cleanup - spreads cleanup cost across operations
pub struct AmortizedMemoryStore {
    data: HashMap<String, (i64, Option<SystemTime>)>,
    // Cleanup state - using iterator position instead of vector
    cleanup_position: usize, // Position in logical iteration order
    operations_count: usize,
    // Configuration
    operations_per_cleanup: usize,
    entries_per_cleanup: usize,
}

impl AmortizedMemoryStore {
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_CAPACITY)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        AmortizedMemoryStore {
            data: HashMap::with_capacity((capacity as f64 * CAPACITY_OVERHEAD_FACTOR) as usize),
            cleanup_position: 0,
            operations_count: 0,
            operations_per_cleanup: DEFAULT_OPERATIONS_PER_CLEANUP,
            entries_per_cleanup: DEFAULT_ENTRIES_PER_CLEANUP,
        }
    }

    fn amortized_cleanup(&mut self, now: SystemTime) {
        self.operations_count += 1;

        // Only cleanup every N operations
        if self.operations_count % self.operations_per_cleanup != 0 {
            return;
        }

        // Reset position if we've gone through all entries
        if self.cleanup_position >= self.data.len() {
            self.cleanup_position = 0;
        }

        // Clean a small batch using iterator
        let mut keys_to_remove = Vec::with_capacity(self.entries_per_cleanup);

        // Skip to our position and collect keys to remove
        for (i, (key, (_, expiry))) in self.data.iter().enumerate() {
            if i < self.cleanup_position {
                continue;
            }
            if i >= self.cleanup_position + self.entries_per_cleanup {
                break;
            }

            if let Some(exp) = expiry {
                if *exp <= now {
                    keys_to_remove.push(key.clone());
                }
            }
        }

        // Remove expired entries
        for key in keys_to_remove {
            self.data.remove(&key);
        }

        self.cleanup_position += self.entries_per_cleanup;

    }
}

impl Default for AmortizedMemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

impl Store for AmortizedMemoryStore {
    fn compare_and_swap_with_ttl(
        &mut self,
        key: &str,
        old: i64,
        new: i64,
        ttl: Duration,
        now: SystemTime,
    ) -> Result<bool, String> {
        self.amortized_cleanup(now);

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
        self.amortized_cleanup(now);

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

/// Probabilistic cleanup - each operation has a small chance to trigger cleanup
pub struct ProbabilisticMemoryStore {
    data: HashMap<String, (i64, Option<SystemTime>)>,
    operations_count: u64,
    #[allow(dead_code)]
    cleanup_probability: f64, // e.g., 0.001 = 0.1% chance per operation
}

impl ProbabilisticMemoryStore {
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_CAPACITY)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        ProbabilisticMemoryStore {
            data: HashMap::with_capacity((capacity as f64 * CAPACITY_OVERHEAD_FACTOR) as usize),
            operations_count: 0,
            cleanup_probability: 1.0 / PROBABILISTIC_CLEANUP_MODULO as f64,
        }
    }

    fn maybe_cleanup(&mut self, now: SystemTime) {
        self.operations_count += 1;

        // Simple pseudo-random using operations count
        // This is deterministic but spreads cleanups evenly
        let should_clean =
            (self.operations_count.wrapping_mul(2654435761) % PROBABILISTIC_CLEANUP_MODULO) < 1;

        if should_clean {
            let _before = self.data.len();
            self.data.retain(|_, (_, expiry)| {
                if let Some(exp) = expiry {
                    *exp > now
                } else {
                    true
                }
            });

            #[cfg(debug_assertions)]
            {
                let removed = _before - self.data.len();
                if removed > 0 {
                    eprintln!("Probabilistic cleanup: removed {removed} entries");
                }
            }
        }
    }
}

impl Default for ProbabilisticMemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

impl Store for ProbabilisticMemoryStore {
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

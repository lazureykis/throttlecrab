use std::time::{Duration, SystemTime};
use super::Store;

#[cfg(feature = "ahash")]
use ahash::AHashMap as HashMap;
#[cfg(not(feature = "ahash"))]
use std::collections::HashMap;

/// Memory store with amortized cleanup - spreads cleanup cost across operations
pub struct AmortizedMemoryStore {
    data: HashMap<String, (i64, Option<SystemTime>)>,
    // Cleanup state
    cleanup_cursor: Vec<String>, // Keys to check for cleanup
    cleanup_index: usize,
    operations_count: usize,
    // Configuration
    operations_per_cleanup: usize,
    entries_per_cleanup: usize,
}

impl AmortizedMemoryStore {
    pub fn new() -> Self {
        Self::with_capacity(1000)
    }
    
    pub fn with_capacity(capacity: usize) -> Self {
        AmortizedMemoryStore {
            data: HashMap::with_capacity((capacity as f64 * 1.3) as usize),
            cleanup_cursor: Vec::with_capacity(capacity),
            cleanup_index: 0,
            operations_count: 0,
            operations_per_cleanup: 100, // Check every 100 operations
            entries_per_cleanup: 10,      // Clean up to 10 entries each time
        }
    }

    fn amortized_cleanup(&mut self, now: SystemTime) {
        self.operations_count += 1;
        
        // Only cleanup every N operations
        if self.operations_count % self.operations_per_cleanup != 0 {
            return;
        }
        
        // Rebuild cursor if needed
        if self.cleanup_index >= self.cleanup_cursor.len() {
            self.cleanup_cursor.clear();
            self.cleanup_cursor.extend(self.data.keys().cloned());
            self.cleanup_index = 0;
        }
        
        // Clean a small batch
        let mut removed = 0;
        let end = (self.cleanup_index + self.entries_per_cleanup).min(self.cleanup_cursor.len());
        
        for i in self.cleanup_index..end {
            let key = &self.cleanup_cursor[i];
            let should_remove = match self.data.get(key) {
                Some((_, Some(expiry))) => *expiry <= now,
                _ => false,
            };
            
            if should_remove {
                self.data.remove(key);
                removed += 1;
            }
        }
        
        self.cleanup_index = end;
        
        #[cfg(debug_assertions)]
        if removed > 0 {
            eprintln!("Amortized cleanup: removed {} entries", removed);
        }
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
        Self::with_capacity(1000)
    }
    
    pub fn with_capacity(capacity: usize) -> Self {
        ProbabilisticMemoryStore {
            data: HashMap::with_capacity((capacity as f64 * 1.3) as usize),
            operations_count: 0,
            cleanup_probability: 0.001, // 0.1% chance = ~1 cleanup per 1000 ops
        }
    }

    fn maybe_cleanup(&mut self, now: SystemTime) {
        self.operations_count += 1;
        
        // Simple pseudo-random using operations count
        // This is deterministic but spreads cleanups evenly
        let should_clean = (self.operations_count.wrapping_mul(2654435761) % 1000) < 1;
        
        if should_clean {
            let before = self.data.len();
            self.data.retain(|_, (_, expiry)| {
                if let Some(exp) = expiry {
                    *exp > now
                } else {
                    true
                }
            });
            
            #[cfg(debug_assertions)]
            {
                let removed = before - self.data.len();
                if removed > 0 {
                    eprintln!("Probabilistic cleanup: removed {} entries", removed);
                }
            }
        }
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
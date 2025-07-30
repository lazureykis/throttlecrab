use std::time::{Duration, SystemTime};
use super::Store;

#[cfg(feature = "ahash")]
use ahash::AHashMap as HashMap;
#[cfg(not(feature = "ahash"))]
use std::collections::HashMap;

/// Optimized in-memory store implementation
pub struct OptimizedMemoryStore {
    data: HashMap<String, (i64, Option<SystemTime>)>,
    // Track when next cleanup is needed
    next_cleanup: SystemTime,
    // Cleanup interval
    cleanup_interval: Duration,
    // Track number of expired entries
    expired_count: usize,
}

impl OptimizedMemoryStore {
    pub fn new() -> Self {
        Self::with_capacity(1000)
    }
    
    pub fn with_capacity(capacity: usize) -> Self {
        OptimizedMemoryStore {
            // Pre-allocate with 30% overhead to avoid rehashing
            data: HashMap::with_capacity((capacity as f64 * 1.3) as usize),
            next_cleanup: SystemTime::now() + Duration::from_secs(60),
            cleanup_interval: Duration::from_secs(60),
            expired_count: 0,
        }
    }
    
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.data.len()
    }
    
    #[cfg(test)]
    pub fn expired_count(&self) -> usize {
        self.expired_count
    }

    fn maybe_clean_expired(&mut self, now: SystemTime) {
        // Clean if we've passed the cleanup time OR if expired entries exceed 20% of total
        let should_clean = now >= self.next_cleanup || 
                          (self.expired_count > 100 && self.expired_count > self.data.len() / 5);
        
        if should_clean {
            self.data.retain(|_, (_, expiry)| {
                if let Some(exp) = expiry {
                    *exp > now
                } else {
                    true
                }
            });
            self.next_cleanup = now + self.cleanup_interval;
            self.expired_count = 0;
        }
    }
}

impl Default for OptimizedMemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

impl Store for OptimizedMemoryStore {
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
            Some((_current, Some(expiry))) if *expiry <= now => {
                // Key expired, track it
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
        self.maybe_clean_expired(now);

        // Check for existing non-expired key
        match self.data.get(key) {
            Some((_, Some(expiry))) if *expiry > now => Ok(false),
            Some((_, None)) => Ok(false),
            Some((_, Some(_expiry))) => {
                // Key is expired
                self.expired_count += 1;
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

/// String interning store to reduce allocations
pub struct InternedMemoryStore {
    data: HashMap<usize, (i64, Option<SystemTime>)>,
    // Intern string keys to numeric IDs
    key_to_id: HashMap<String, usize>,
    next_id: usize,
    next_cleanup: SystemTime,
    cleanup_interval: Duration,
}

impl InternedMemoryStore {
    pub fn new() -> Self {
        Self::with_capacity(1000)
    }
    
    pub fn with_capacity(capacity: usize) -> Self {
        let adjusted_capacity = (capacity as f64 * 1.3) as usize;
        InternedMemoryStore {
            data: HashMap::with_capacity(adjusted_capacity),
            key_to_id: HashMap::with_capacity(adjusted_capacity),
            next_id: 0,
            next_cleanup: SystemTime::now() + Duration::from_secs(60),
            cleanup_interval: Duration::from_secs(60),
        }
    }

    fn get_or_create_id(&mut self, key: &str) -> usize {
        if let Some(&id) = self.key_to_id.get(key) {
            id
        } else {
            let id = self.next_id;
            self.next_id += 1;
            self.key_to_id.insert(key.to_string(), id);
            id
        }
    }

    fn maybe_clean_expired(&mut self, now: SystemTime) {
        if now >= self.next_cleanup {
            self.data.retain(|_, (_, expiry)| {
                if let Some(exp) = expiry {
                    *exp > now
                } else {
                    true
                }
            });
            // Also clean up unused key mappings
            let used_ids: std::collections::HashSet<_> = self.data.keys().copied().collect();
            self.key_to_id.retain(|_, id| used_ids.contains(id));
            
            self.next_cleanup = now + self.cleanup_interval;
        }
    }
}

impl Store for InternedMemoryStore {
    fn compare_and_swap_with_ttl(
        &mut self,
        key: &str,
        old: i64,
        new: i64,
        ttl: Duration,
        now: SystemTime,
    ) -> Result<bool, String> {
        self.maybe_clean_expired(now);
        
        let id = self.get_or_create_id(key);
        match self.data.get(&id) {
            Some((_current, Some(expiry))) if *expiry <= now => Ok(false),
            Some((current, _)) if *current == old => {
                let expiry = now + ttl;
                self.data.insert(id, (new, Some(expiry)));
                Ok(true)
            }
            Some(_) => Ok(false),
            None => Ok(false),
        }
    }

    fn get(&self, key: &str, now: SystemTime) -> Result<Option<i64>, String> {
        if let Some(&id) = self.key_to_id.get(key) {
            match self.data.get(&id) {
                Some((value, Some(expiry))) if *expiry > now => Ok(Some(*value)),
                Some((value, None)) => Ok(Some(*value)),
                _ => Ok(None),
            }
        } else {
            Ok(None)
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
        self.maybe_clean_expired(now);
        
        let id = self.get_or_create_id(key);
        match self.data.get(&id) {
            Some((_, Some(expiry))) if *expiry > now => Ok(false),
            Some((_, None)) => Ok(false),
            _ => {
                let expiry = now + ttl;
                self.data.insert(id, (value, Some(expiry)));
                Ok(true)
            }
        }
    }
}
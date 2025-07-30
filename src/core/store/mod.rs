use std::time::{Duration, SystemTime};

#[cfg(feature = "ahash")]
use ahash::AHashMap as HashMap;
#[cfg(not(feature = "ahash"))]
use std::collections::HashMap;

#[cfg(test)]
mod tests;

pub mod adaptive_cleanup;
pub mod amortized;
pub mod arena;
pub mod fast_hasher;
pub mod optimized;

#[cfg(test)]
mod cleanup_test;

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

    /// Log debug message
    fn log_debug(&self, message: &str);

    /// Set if not exists with TTL
    fn set_if_not_exists_with_ttl(
        &mut self,
        key: &str,
        value: i64,
        ttl: Duration,
        now: SystemTime,
    ) -> Result<bool, String>;
}

/// In-memory store implementation
pub struct MemoryStore {
    data: HashMap<String, (i64, Option<SystemTime>)>,
}

impl MemoryStore {
    pub fn new() -> Self {
        MemoryStore {
            data: HashMap::new(),
        }
    }

    fn clean_expired(&mut self, now: SystemTime) {
        self.data.retain(|_, (_, expiry)| {
            if let Some(exp) = expiry {
                *exp > now
            } else {
                true
            }
        });
    }
}

impl Default for MemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryStore {
    pub fn get(&self, key: &str, now: SystemTime) -> Result<Option<i64>, String> {
        match self.data.get(key) {
            Some((value, Some(expiry))) if *expiry > now => Ok(Some(*value)),
            Some((value, None)) => Ok(Some(*value)),
            _ => Ok(None),
        }
    }
}

impl Store for MemoryStore {
    fn compare_and_swap_with_ttl(
        &mut self,
        key: &str,
        old: i64,
        new: i64,
        ttl: Duration,
        now: SystemTime,
    ) -> Result<bool, String> {
        self.clean_expired(now);

        match self.data.get(key) {
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
        self.get(key, now)
    }

    fn log_debug(&self, _message: &str) {
        // No-op in library - binary can implement logging
    }

    fn set_if_not_exists_with_ttl(
        &mut self,
        key: &str,
        value: i64,
        ttl: Duration,
        now: SystemTime,
    ) -> Result<bool, String> {
        self.clean_expired(now);

        if self.data.contains_key(key) {
            Ok(false)
        } else {
            let expiry = now + ttl;
            self.data.insert(key.to_string(), (value, Some(expiry)));
            Ok(true)
        }
    }
}

impl Store for &mut MemoryStore {
    fn compare_and_swap_with_ttl(
        &mut self,
        key: &str,
        old: i64,
        new: i64,
        ttl: Duration,
        now: SystemTime,
    ) -> Result<bool, String> {
        self.clean_expired(now);

        match self.data.get(key) {
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
        (**self).get(key, now)
    }

    fn log_debug(&self, _message: &str) {
        // No-op in library - binary can implement logging
    }

    fn set_if_not_exists_with_ttl(
        &mut self,
        key: &str,
        value: i64,
        ttl: Duration,
        now: SystemTime,
    ) -> Result<bool, String> {
        self.clean_expired(now);

        if self.data.contains_key(key) {
            Ok(false)
        } else {
            let expiry = now + ttl;
            self.data.insert(key.to_string(), (value, Some(expiry)));
            Ok(true)
        }
    }
}

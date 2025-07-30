use ahash::AHashMap;
use std::time::{Duration, SystemTime};
use throttlecrab::Store;

/// Memory store using ahash for benchmarking only
pub struct AHashMemoryStore {
    data: AHashMap<String, (i64, Option<SystemTime>)>,
    next_cleanup: SystemTime,
    cleanup_interval: Duration,
    expired_count: usize,
}

impl Default for AHashMemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

impl AHashMemoryStore {
    pub fn new() -> Self {
        Self::with_capacity(1000)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        let data = AHashMap::with_capacity((capacity as f64 * 1.3) as usize);

        AHashMemoryStore {
            data,
            next_cleanup: SystemTime::now() + Duration::from_secs(60),
            cleanup_interval: Duration::from_secs(60),
            expired_count: 0,
        }
    }

    fn maybe_clean_expired(&mut self, now: SystemTime) {
        let should_clean = now >= self.next_cleanup
            || (self.expired_count > 100 && self.expired_count > self.data.len() / 5);

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

impl Store for AHashMemoryStore {
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

    fn log_debug(&self, _message: &str) {
        // No-op
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

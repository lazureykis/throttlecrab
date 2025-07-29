use std::collections::HashMap;
use std::time::{Duration, SystemTime};

/// Store trait for rate limiter state storage (similar to redis-cell)
pub trait Store {
    /// Compare and swap with TTL
    fn compare_and_swap_with_ttl(
        &mut self,
        key: &str,
        old: i64,
        new: i64,
        ttl: Duration,
    ) -> Result<bool, String>;

    /// Get value with current time
    fn get_with_time(&self, key: &str) -> Result<(Option<i64>, SystemTime), String>;

    /// Log debug message
    fn log_debug(&self, message: &str);

    /// Set if not exists with TTL
    fn set_if_not_exists_with_ttl(
        &mut self,
        key: &str,
        value: i64,
        ttl: Duration,
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

    fn clean_expired(&mut self) {
        let now = SystemTime::now();
        self.data.retain(|_, (_, expiry)| {
            if let Some(exp) = expiry {
                *exp > now
            } else {
                true
            }
        });
    }
}

impl Store for &mut MemoryStore {
    fn compare_and_swap_with_ttl(
        &mut self,
        key: &str,
        old: i64,
        new: i64,
        ttl: Duration,
    ) -> Result<bool, String> {
        self.clean_expired();
        
        match self.data.get(key) {
            Some((current, _)) if *current == old => {
                let expiry = SystemTime::now() + ttl;
                self.data.insert(key.to_string(), (new, Some(expiry)));
                Ok(true)
            }
            Some(_) => Ok(false),
            None => Ok(false),
        }
    }

    fn get_with_time(&self, key: &str) -> Result<(Option<i64>, SystemTime), String> {
        let now = SystemTime::now();
        
        match self.data.get(key) {
            Some((value, Some(expiry))) if *expiry > now => Ok((Some(*value), now)),
            Some((value, None)) => Ok((Some(*value), now)),
            _ => Ok((None, now)),
        }
    }

    fn log_debug(&self, message: &str) {
        tracing::debug!("{}", message);
    }

    fn set_if_not_exists_with_ttl(
        &mut self,
        key: &str,
        value: i64,
        ttl: Duration,
    ) -> Result<bool, String> {
        self.clean_expired();
        
        if self.data.contains_key(key) {
            Ok(false)
        } else {
            let expiry = SystemTime::now() + ttl;
            self.data.insert(key.to_string(), (value, Some(expiry)));
            Ok(true)
        }
    }
}
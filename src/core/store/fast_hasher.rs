use super::Store;
use std::collections::HashMap;
use std::hash::{BuildHasher, Hasher};
use std::time::{Duration, SystemTime};

// Note: These custom hashers are kept for benchmarking comparisons
// The default stores now use ahash when the feature is enabled

/// A very fast, non-cryptographic hasher based on FxHash
/// This is suitable for our use case since we don't need protection against HashDoS
#[derive(Default)]
pub struct FxHasher {
    hash: u64,
}

impl FxHasher {
    const K: u64 = 0x517cc1b727220a95;

    #[inline]
    fn add_to_hash(&mut self, i: u64) {
        self.hash = self.hash.rotate_left(5) ^ i;
        self.hash = self.hash.wrapping_mul(Self::K);
    }
}

impl Hasher for FxHasher {
    #[inline]
    fn finish(&self) -> u64 {
        self.hash
    }

    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        for chunk in bytes.chunks(8) {
            let mut buf = [0u8; 8];
            buf[..chunk.len()].copy_from_slice(chunk);
            self.add_to_hash(u64::from_ne_bytes(buf));
        }
    }

    #[inline]
    fn write_u8(&mut self, i: u8) {
        self.add_to_hash(i as u64);
    }

    #[inline]
    fn write_u16(&mut self, i: u16) {
        self.add_to_hash(i as u64);
    }

    #[inline]
    fn write_u32(&mut self, i: u32) {
        self.add_to_hash(i as u64);
    }

    #[inline]
    fn write_u64(&mut self, i: u64) {
        self.add_to_hash(i);
    }

    #[inline]
    fn write_usize(&mut self, i: usize) {
        self.add_to_hash(i as u64);
    }
}

/// BuildHasher for FxHasher
#[derive(Default)]
pub struct FxBuildHasher;

impl BuildHasher for FxBuildHasher {
    type Hasher = FxHasher;

    #[inline]
    fn build_hasher(&self) -> Self::Hasher {
        FxHasher::default()
    }
}

/// Type alias for HashMap using FxHasher
pub type FxHashMap<K, V> = HashMap<K, V, FxBuildHasher>;

/// Memory store using fast hasher
pub struct FastHashMemoryStore {
    data: FxHashMap<String, (i64, Option<SystemTime>)>,
    next_cleanup: SystemTime,
    cleanup_interval: Duration,
    expired_count: usize,
}

impl FastHashMemoryStore {
    pub fn new() -> Self {
        Self::with_capacity(1000)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        let mut data = FxHashMap::default();
        data.reserve((capacity as f64 * 1.3) as usize);

        FastHashMemoryStore {
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

impl Default for FastHashMemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

impl Store for FastHashMemoryStore {
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

/// Alternative: Use a simple multiplicative hash for even faster performance
/// This is less robust but extremely fast for string keys
#[derive(Default)]
pub struct SimpleHasher {
    hash: u64,
}

impl Hasher for SimpleHasher {
    #[inline]
    fn finish(&self) -> u64 {
        self.hash
    }

    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        // Simple multiplicative hash - very fast but less uniform distribution
        for &byte in bytes {
            self.hash = self.hash.wrapping_mul(31).wrapping_add(byte as u64);
        }
    }
}

/// BuildHasher for SimpleHasher
#[derive(Default)]
pub struct SimpleBuildHasher;

impl BuildHasher for SimpleBuildHasher {
    type Hasher = SimpleHasher;

    #[inline]
    fn build_hasher(&self) -> Self::Hasher {
        SimpleHasher::default()
    }
}

/// Type alias for HashMap using SimpleHasher
pub type SimpleHashMap<K, V> = HashMap<K, V, SimpleBuildHasher>;

/// Memory store using simple multiplicative hasher
pub struct SimpleHashMemoryStore {
    data: SimpleHashMap<String, (i64, Option<SystemTime>)>,
    next_cleanup: SystemTime,
    cleanup_interval: Duration,
    expired_count: usize,
}

impl SimpleHashMemoryStore {
    pub fn new() -> Self {
        Self::with_capacity(1000)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        let mut data = SimpleHashMap::default();
        data.reserve((capacity as f64 * 1.3) as usize);

        SimpleHashMemoryStore {
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

impl Default for SimpleHashMemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

impl Store for SimpleHashMemoryStore {
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

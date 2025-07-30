use super::Store;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[cfg(feature = "ahash")]
use ahash::AHashMap as HashMap;
#[cfg(not(feature = "ahash"))]
use std::collections::HashMap;

/// Compact memory store with reduced memory footprint
/// 
/// This implementation uses:
/// - Separate storage for TAT (i64) and expiry (u32)
/// - u32 timestamps for expiry (seconds since custom epoch)
/// - Short string optimization for keys < 16 bytes
pub struct CompactMemoryStore {
    // Separate maps for TAT values and expiry times
    tat_data: HashMap<CompactKey, i64>,
    expiry_data: HashMap<CompactKey, u32>,
    // Custom epoch to allow u32 timestamps (Jan 1, 2020)
    epoch: SystemTime,
    // Next cleanup time as u32 seconds since epoch
    next_cleanup: u32,
    cleanup_interval: u32,
}

/// Compact key representation with short string optimization
#[derive(Clone, PartialEq, Eq, Hash)]
enum CompactKey {
    // For keys <= 15 bytes, store inline
    Short([u8; 16]), // 15 bytes + 1 length byte
    // For longer keys, heap allocate
    Long(String),
}

impl CompactKey {
    fn new(key: &str) -> Self {
        let bytes = key.as_bytes();
        if bytes.len() <= 15 {
            let mut short = [0u8; 16];
            short[0] = bytes.len() as u8;
            short[1..=bytes.len()].copy_from_slice(bytes);
            CompactKey::Short(short)
        } else {
            CompactKey::Long(key.to_string())
        }
    }
    
    #[cfg(test)]
    fn as_str(&self) -> &str {
        match self {
            CompactKey::Short(bytes) => {
                let len = bytes[0] as usize;
                std::str::from_utf8(&bytes[1..=len]).unwrap()
            }
            CompactKey::Long(s) => s.as_str(),
        }
    }
}

impl CompactMemoryStore {
    pub fn new() -> Self {
        let epoch = UNIX_EPOCH + Duration::from_secs(1_577_836_800); // Jan 1, 2020
        CompactMemoryStore {
            tat_data: HashMap::new(),
            expiry_data: HashMap::new(),
            epoch,
            next_cleanup: 60, // 60 seconds from epoch
            cleanup_interval: 60,
        }
    }
    
    pub fn with_capacity(capacity: usize) -> Self {
        let epoch = UNIX_EPOCH + Duration::from_secs(1_577_836_800); // Jan 1, 2020
        CompactMemoryStore {
            tat_data: HashMap::with_capacity(capacity),
            expiry_data: HashMap::with_capacity(capacity),
            epoch,
            next_cleanup: 60,
            cleanup_interval: 60,
        }
    }
    
    /// Convert SystemTime to u32 seconds since our epoch
    fn time_to_u32(&self, time: SystemTime) -> Result<u32, String> {
        time.duration_since(self.epoch)
            .map(|d| d.as_secs() as u32)
            .map_err(|_| "Time before epoch".to_string())
    }
    
    /// Convert u32 seconds since epoch back to SystemTime
    #[allow(dead_code)]
    fn u32_to_time(&self, secs: u32) -> SystemTime {
        self.epoch + Duration::from_secs(secs as u64)
    }
    
    
    fn clean_expired(&mut self, now_u32: u32) {
        // Collect keys to remove
        let mut to_remove = Vec::new();
        for (key, &expiry) in &self.expiry_data {
            if expiry <= now_u32 {
                to_remove.push(key.clone());
            }
        }
        
        // Remove expired entries from both maps
        for key in to_remove {
            self.tat_data.remove(&key);
            self.expiry_data.remove(&key);
        }
        
        self.next_cleanup = now_u32 + self.cleanup_interval;
    }
    
    fn maybe_clean_expired(&mut self, now_u32: u32) {
        if now_u32 >= self.next_cleanup {
            self.clean_expired(now_u32);
        }
    }
}

impl Default for CompactMemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

impl Store for CompactMemoryStore {
    fn compare_and_swap_with_ttl(
        &mut self,
        key: &str,
        old: i64,
        new: i64,
        ttl: Duration,
        now: SystemTime,
    ) -> Result<bool, String> {
        let now_u32 = self.time_to_u32(now)?;
        self.maybe_clean_expired(now_u32);
        
        let compact_key = CompactKey::new(key);
        
        if let Some(&tat) = self.tat_data.get(&compact_key) {
            if let Some(&expiry) = self.expiry_data.get(&compact_key) {
                // Check if expired
                if expiry <= now_u32 {
                    return Ok(false);
                }
                
                if tat == old {
                    // Update with new value
                    let new_expiry = now_u32 + ttl.as_secs() as u32;
                    self.tat_data.insert(compact_key.clone(), new);
                    self.expiry_data.insert(compact_key, new_expiry);
                    Ok(true)
                } else {
                    Ok(false)
                }
            } else {
                Ok(false)
            }
        } else {
            Ok(false)
        }
    }
    
    fn get(&self, key: &str, now: SystemTime) -> Result<Option<i64>, String> {
        let now_u32 = self.time_to_u32(now)?;
        let compact_key = CompactKey::new(key);
        
        if let Some(&tat) = self.tat_data.get(&compact_key) {
            if let Some(&expiry) = self.expiry_data.get(&compact_key) {
                if expiry > now_u32 {
                    Ok(Some(tat))
                } else {
                    Ok(None)
                }
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
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
        let now_u32 = self.time_to_u32(now)?;
        let compact_key = CompactKey::new(key);
        
        // Check if key exists and is not expired
        if let Some(&expiry) = self.expiry_data.get(&compact_key) {
            if expiry > now_u32 {
                return Ok(false);
            }
        }
        
        // Clean expired entries periodically
        self.maybe_clean_expired(now_u32);
        
        // Insert new entry
        let expiry = now_u32 + ttl.as_secs() as u32;
        self.tat_data.insert(compact_key.clone(), value);
        self.expiry_data.insert(compact_key, expiry);
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_compact_key() {
        // Test short keys
        let short = CompactKey::new("hello");
        assert!(matches!(short, CompactKey::Short(_)));
        assert_eq!(short.as_str(), "hello");
        
        // Test long keys
        let long = CompactKey::new("this_is_a_very_long_key_that_exceeds_fifteen_bytes");
        assert!(matches!(long, CompactKey::Long(_)));
        assert_eq!(long.as_str(), "this_is_a_very_long_key_that_exceeds_fifteen_bytes");
        
        // Test 15-byte boundary
        let boundary = CompactKey::new("exactly15bytes!");
        assert!(matches!(boundary, CompactKey::Short(_)));
        assert_eq!(boundary.as_str(), "exactly15bytes!");
    }
    
    
    #[test]
    fn test_compact_basic_operations() {
        let mut store = CompactMemoryStore::new();
        let now = SystemTime::now();
        let ttl = Duration::from_secs(60);
        
        // Test set and get
        assert!(store.set_if_not_exists_with_ttl("key1", 100, ttl, now).unwrap());
        assert_eq!(store.get("key1", now).unwrap(), Some(100));
        
        // Test compare and swap
        assert!(store.compare_and_swap_with_ttl("key1", 100, 200, ttl, now).unwrap());
        assert_eq!(store.get("key1", now).unwrap(), Some(200));
        
        // Test non-existent key
        assert_eq!(store.get("key2", now).unwrap(), None);
    }
    
    #[test]
    fn test_compact_expiry() {
        let mut store = CompactMemoryStore::new();
        let now = SystemTime::now();
        let short_ttl = Duration::from_secs(1); // Changed from millis to secs
        
        // Set with short TTL
        assert!(store.set_if_not_exists_with_ttl("key1", 100, short_ttl, now).unwrap());
        assert_eq!(store.get("key1", now).unwrap(), Some(100));
        
        // Check after expiry
        let later = now + Duration::from_secs(2);
        assert_eq!(store.get("key1", later).unwrap(), None);
        
        // Can set again after expiry
        assert!(store.set_if_not_exists_with_ttl("key1", 200, Duration::from_secs(60), later).unwrap());
        assert_eq!(store.get("key1", later).unwrap(), Some(200));
    }
}
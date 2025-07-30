use super::Store;
use std::collections::HashMap;
use std::time::{Duration, SystemTime};

/// Arena-allocated store for reduced allocation pressure
/// 
/// This implementation pre-allocates memory in large chunks (arenas) and
/// manages allocations internally to reduce malloc/free overhead.
pub struct ArenaMemoryStore {
    // Main storage using indices instead of direct values
    indices: HashMap<String, ArenaIndex>,
    // Arena storage
    arena: Arena,
    // Next cleanup time
    next_cleanup: SystemTime,
    cleanup_interval: Duration,
}

#[derive(Clone, Copy, Debug)]
struct ArenaIndex {
    generation: u32,
    index: u32,
}

struct Arena {
    // Flat storage for values and expiry times
    values: Vec<i64>,
    expiries: Vec<Option<SystemTime>>,
    // Generation counters to detect stale indices
    generations: Vec<u32>,
    // Free list for recycling slots
    free_list: Vec<u32>,
    // Current capacity
    capacity: usize,
}

impl Arena {
    fn new(capacity: usize) -> Self {
        let mut arena = Arena {
            values: Vec::with_capacity(capacity),
            expiries: Vec::with_capacity(capacity),
            generations: Vec::with_capacity(capacity),
            free_list: Vec::with_capacity(capacity),
            capacity,
        };
        
        // Initialize all slots as free
        for i in 0..capacity {
            arena.values.push(0);
            arena.expiries.push(None);
            arena.generations.push(0);
            arena.free_list.push(i as u32);
        }
        
        arena
    }
    
    fn allocate(&mut self, value: i64, expiry: Option<SystemTime>) -> Option<ArenaIndex> {
        if let Some(index) = self.free_list.pop() {
            let idx = index as usize;
            self.values[idx] = value;
            self.expiries[idx] = expiry;
            // Increment generation to invalidate old references
            self.generations[idx] = self.generations[idx].wrapping_add(1);
            
            Some(ArenaIndex {
                generation: self.generations[idx],
                index,
            })
        } else {
            // Arena is full
            None
        }
    }
    
    fn deallocate(&mut self, index: ArenaIndex) {
        let idx = index.index as usize;
        if idx < self.capacity && self.generations[idx] == index.generation {
            // Mark as free
            self.generations[idx] = self.generations[idx].wrapping_add(1);
            self.free_list.push(index.index);
        }
    }
    
    fn get(&self, index: ArenaIndex) -> Option<(i64, Option<SystemTime>)> {
        let idx = index.index as usize;
        if idx < self.capacity && self.generations[idx] == index.generation {
            Some((self.values[idx], self.expiries[idx]))
        } else {
            None
        }
    }
    
    fn update(&mut self, index: ArenaIndex, value: i64, expiry: Option<SystemTime>) -> bool {
        let idx = index.index as usize;
        if idx < self.capacity && self.generations[idx] == index.generation {
            self.values[idx] = value;
            self.expiries[idx] = expiry;
            true
        } else {
            false
        }
    }
}

impl ArenaMemoryStore {
    pub fn new() -> Self {
        Self::with_capacity(10_000)
    }
    
    pub fn with_capacity(capacity: usize) -> Self {
        ArenaMemoryStore {
            indices: HashMap::with_capacity(capacity),
            arena: Arena::new(capacity),
            next_cleanup: SystemTime::now() + Duration::from_secs(60),
            cleanup_interval: Duration::from_secs(60),
        }
    }
    
    fn clean_expired(&mut self, now: SystemTime) {
        let mut expired_keys = Vec::new();
        
        // Find expired entries
        for (key, &index) in &self.indices {
            if let Some((_, Some(expiry))) = self.arena.get(index) {
                if expiry <= now {
                    expired_keys.push(key.clone());
                }
            }
        }
        
        // Remove expired entries and deallocate from arena
        for key in expired_keys {
            if let Some(index) = self.indices.remove(&key) {
                self.arena.deallocate(index);
            }
        }
        
        self.next_cleanup = now + self.cleanup_interval;
    }
    
    fn maybe_clean_expired(&mut self, now: SystemTime) {
        if now >= self.next_cleanup {
            self.clean_expired(now);
        }
    }
}

impl Default for ArenaMemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

impl Store for ArenaMemoryStore {
    fn compare_and_swap_with_ttl(
        &mut self,
        key: &str,
        old: i64,
        new: i64,
        ttl: Duration,
        now: SystemTime,
    ) -> Result<bool, String> {
        self.maybe_clean_expired(now);
        
        if let Some(&index) = self.indices.get(key) {
            if let Some((current, existing_expiry)) = self.arena.get(index) {
                // Check if expired
                if let Some(exp) = existing_expiry {
                    if exp <= now {
                        // Entry expired, remove it
                        self.indices.remove(key);
                        self.arena.deallocate(index);
                        return Ok(false);
                    }
                }
                
                if current == old {
                    // Update the value
                    let expiry = now + ttl;
                    if self.arena.update(index, new, Some(expiry)) {
                        Ok(true)
                    } else {
                        // Generation mismatch - entry was recycled
                        self.indices.remove(key);
                        Ok(false)
                    }
                } else {
                    Ok(false)
                }
            } else {
                // Index is stale
                self.indices.remove(key);
                Ok(false)
            }
        } else {
            Ok(false)
        }
    }
    
    fn get(&self, key: &str, now: SystemTime) -> Result<Option<i64>, String> {
        if let Some(&index) = self.indices.get(key) {
            if let Some((value, expiry)) = self.arena.get(index) {
                if let Some(exp) = expiry {
                    if exp > now {
                        Ok(Some(value))
                    } else {
                        Ok(None)
                    }
                } else {
                    Ok(Some(value))
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
        self.maybe_clean_expired(now);
        
        // Check if key already exists and is not expired
        if let Some(&index) = self.indices.get(key) {
            if let Some((_, expiry)) = self.arena.get(index) {
                if let Some(exp) = expiry {
                    if exp > now {
                        return Ok(false); // Key exists and not expired
                    }
                } else {
                    return Ok(false); // Key exists with no expiry
                }
            }
            // Stale index, remove it
            self.indices.remove(key);
            self.arena.deallocate(index);
        }
        
        // Allocate new entry
        let expiry = now + ttl;
        if let Some(index) = self.arena.allocate(value, Some(expiry)) {
            self.indices.insert(key.to_string(), index);
            Ok(true)
        } else {
            // Arena is full - try to clean up and retry
            self.clean_expired(now);
            
            if let Some(index) = self.arena.allocate(value, Some(expiry)) {
                self.indices.insert(key.to_string(), index);
                Ok(true)
            } else {
                Err("Arena capacity exceeded".to_string())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_arena_basic_operations() {
        let mut store = ArenaMemoryStore::with_capacity(10);
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
    fn test_arena_capacity_and_cleanup() {
        let mut store = ArenaMemoryStore::with_capacity(3);
        let now = SystemTime::now();
        let short_ttl = Duration::from_millis(100);
        let long_ttl = Duration::from_secs(60);
        
        // Fill the arena
        assert!(store.set_if_not_exists_with_ttl("key1", 1, short_ttl, now).unwrap());
        assert!(store.set_if_not_exists_with_ttl("key2", 2, short_ttl, now).unwrap());
        assert!(store.set_if_not_exists_with_ttl("key3", 3, long_ttl, now).unwrap());
        
        // Arena is full, but after expiry cleanup we should be able to add more
        let later = now + Duration::from_millis(200);
        assert!(store.set_if_not_exists_with_ttl("key4", 4, long_ttl, later).unwrap());
        
        // Old short TTL keys should be gone
        assert_eq!(store.get("key1", later).unwrap(), None);
        assert_eq!(store.get("key2", later).unwrap(), None);
        // Long TTL keys should still exist
        assert_eq!(store.get("key3", later).unwrap(), Some(3));
        assert_eq!(store.get("key4", later).unwrap(), Some(4));
    }
    
    #[test]
    fn test_arena_generation_tracking() {
        let mut store = ArenaMemoryStore::with_capacity(2);
        let now = SystemTime::now();
        let ttl = Duration::from_millis(100);
        
        // Add and remove entries to test generation tracking
        assert!(store.set_if_not_exists_with_ttl("key1", 1, ttl, now).unwrap());
        
        // Let it expire and add a new key that might reuse the same slot
        let later = now + Duration::from_millis(200);
        store.clean_expired(later);
        
        assert!(store.set_if_not_exists_with_ttl("key2", 2, Duration::from_secs(60), later).unwrap());
        
        // key1 should be gone, key2 should exist
        assert_eq!(store.get("key1", later).unwrap(), None);
        assert_eq!(store.get("key2", later).unwrap(), Some(2));
    }
}
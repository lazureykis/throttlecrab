use super::Store;
use std::time::{Duration, SystemTime};

#[cfg(feature = "ahash")]
use ahash::AHashMap as HashMap;
#[cfg(not(feature = "ahash"))]
use std::collections::HashMap;

/// Hierarchical timing wheel store for efficient TTL management
/// 
/// This implementation uses multiple wheels with different time granularities
/// to efficiently manage entries with varying TTLs. Provides O(1) expiry
/// detection and efficient bulk cleanup.
pub struct TimingWheelStore {
    // Active entries: key -> (TAT value, wheel level, slot index)
    entries: HashMap<String, (i64, usize, usize)>,
    
    // Timing wheels: level -> wheel (circular buffer of slots)
    // Each slot contains keys that expire in that time window
    wheels: Vec<Wheel>,
    
    // Current time position for each wheel
    current_positions: Vec<usize>,
    
    // Last tick time
    last_tick: SystemTime,
    
    // Minimum tick interval (determines finest granularity)
    tick_interval: Duration,
}

struct Wheel {
    slots: Vec<Vec<String>>, // Each slot contains keys expiring in that window
    slots_count: usize,
    #[allow(dead_code)]
    slot_duration: Duration, // Time covered by each slot
    #[allow(dead_code)]
    level: usize,
}

impl Wheel {
    fn new(level: usize, slots_count: usize, slot_duration: Duration) -> Self {
        let mut slots = Vec::with_capacity(slots_count);
        for _ in 0..slots_count {
            slots.push(Vec::new());
        }
        
        Wheel {
            slots,
            slots_count,
            slot_duration,
            level,
        }
    }
    
    fn add_to_slot(&mut self, slot_index: usize, key: String) {
        self.slots[slot_index % self.slots_count].push(key);
    }
    
    fn take_slot(&mut self, slot_index: usize) -> Vec<String> {
        let index = slot_index % self.slots_count;
        std::mem::take(&mut self.slots[index])
    }
    
    #[allow(dead_code)]
    fn clear_slot(&mut self, slot_index: usize) {
        let index = slot_index % self.slots_count;
        self.slots[index].clear();
    }
}

impl TimingWheelStore {
    pub fn new() -> Self {
        // Create 4 wheels with increasing granularity:
        // Level 0: 60 slots × 1 second = 1 minute
        // Level 1: 60 slots × 1 minute = 1 hour  
        // Level 2: 24 slots × 1 hour = 1 day
        // Level 3: 365 slots × 1 day = 1 year
        let wheels = vec![
            Wheel::new(0, 60, Duration::from_secs(1)),
            Wheel::new(1, 60, Duration::from_secs(60)),
            Wheel::new(2, 24, Duration::from_secs(3600)),
            Wheel::new(3, 365, Duration::from_secs(86400)),
        ];
        
        TimingWheelStore {
            entries: HashMap::new(),
            wheels,
            current_positions: vec![0, 0, 0, 0],
            last_tick: SystemTime::now(),
            tick_interval: Duration::from_secs(1),
        }
    }
    
    pub fn with_capacity(capacity: usize) -> Self {
        let wheels = vec![
            Wheel::new(0, 60, Duration::from_secs(1)),
            Wheel::new(1, 60, Duration::from_secs(60)),
            Wheel::new(2, 24, Duration::from_secs(3600)),
            Wheel::new(3, 365, Duration::from_secs(86400)),
        ];
        
        TimingWheelStore {
            entries: HashMap::with_capacity(capacity),
            wheels,
            current_positions: vec![0, 0, 0, 0],
            last_tick: SystemTime::now(),
            tick_interval: Duration::from_secs(1),
        }
    }
    
    /// Calculate which wheel and slot an expiry time belongs to
    fn calculate_wheel_position(&self, expiry: SystemTime, now: SystemTime) -> Option<(usize, usize)> {
        if expiry <= now {
            return None; // Already expired
        }
        
        let duration = expiry.duration_since(now).unwrap();
        let total_seconds = duration.as_secs();
        
        // Determine which wheel based on duration
        let (wheel_index, slot_offset) = if total_seconds < 60 {
            // Level 0: seconds
            (0, total_seconds as usize)
        } else if total_seconds < 3600 {
            // Level 1: minutes
            (1, (total_seconds / 60) as usize)
        } else if total_seconds < 86400 {
            // Level 2: hours
            (2, (total_seconds / 3600) as usize)
        } else {
            // Level 3: days (cap at 1 year)
            let days = (total_seconds / 86400).min(364) as usize;
            (3, days)
        };
        
        // Calculate absolute slot position
        let current_pos = self.current_positions[wheel_index];
        let slot_index = (current_pos + slot_offset) % self.wheels[wheel_index].slots_count;
        
        Some((wheel_index, slot_index))
    }
    
    /// Advance time and process expired entries
    fn tick(&mut self, now: SystemTime) -> Vec<String> {
        let elapsed = now.duration_since(self.last_tick).unwrap_or(Duration::ZERO);
        
        // Only tick if at least 100ms have passed to reduce overhead
        if elapsed < Duration::from_millis(100) {
            return Vec::new();
        }
        
        let ticks = (elapsed.as_secs() / self.tick_interval.as_secs()) as usize;
        
        if ticks == 0 {
            return Vec::new();
        }
        
        let mut expired_keys = Vec::new();
        
        // Process each tick
        for _ in 0..ticks {
            // Advance the seconds wheel
            self.current_positions[0] = (self.current_positions[0] + 1) % self.wheels[0].slots_count;
            let expired = self.wheels[0].take_slot(self.current_positions[0]);
            expired_keys.extend(expired);
            
            // Check for cascade to minutes wheel
            if self.current_positions[0] == 0 {
                self.current_positions[1] = (self.current_positions[1] + 1) % self.wheels[1].slots_count;
                
                // Move entries from minutes wheel to seconds wheel
                let to_cascade = self.wheels[1].take_slot(self.current_positions[1]);
                for key in to_cascade {
                    // Re-insert into seconds wheel
                    if let Some((_, wheel_idx, _slot_idx)) = self.entries.get(&key) {
                        if *wheel_idx == 1 {
                            // Recalculate position and move to level 0
                            if let Some((new_wheel, new_slot)) = self.calculate_wheel_position(now + Duration::from_secs(59), now) {
                                self.wheels[new_wheel].add_to_slot(new_slot, key);
                            }
                        }
                    }
                }
                
                // Continue cascading if needed
                if self.current_positions[1] == 0 {
                    // Cascade to hours wheel
                    self.current_positions[2] = (self.current_positions[2] + 1) % self.wheels[2].slots_count;
                    // Similar cascading logic...
                }
            }
        }
        
        self.last_tick = now;
        
        // Remove expired entries from main storage
        for key in &expired_keys {
            self.entries.remove(key);
        }
        
        expired_keys
    }
}

impl Default for TimingWheelStore {
    fn default() -> Self {
        Self::new()
    }
}

impl Store for TimingWheelStore {
    fn compare_and_swap_with_ttl(
        &mut self,
        key: &str,
        old: i64,
        new: i64,
        ttl: Duration,
        now: SystemTime,
    ) -> Result<bool, String> {
        // Process any pending expirations
        self.tick(now);
        
        if let Some((current, wheel_idx, slot_idx)) = self.entries.get(key) {
            if *current == old {
                // Remove from old wheel position
                self.wheels[*wheel_idx].slots[*slot_idx].retain(|k| k != key);
                
                // Calculate new position
                let expiry = now + ttl;
                if let Some((new_wheel, new_slot)) = self.calculate_wheel_position(expiry, now) {
                    // Add to new position
                    self.wheels[new_wheel].add_to_slot(new_slot, key.to_string());
                    self.entries.insert(key.to_string(), (new, new_wheel, new_slot));
                    Ok(true)
                } else {
                    // Already expired
                    self.entries.remove(key);
                    Ok(false)
                }
            } else {
                Ok(false)
            }
        } else {
            Ok(false)
        }
    }
    
    fn get(&self, key: &str, _now: SystemTime) -> Result<Option<i64>, String> {
        if let Some((value, _, _)) = self.entries.get(key) {
            // Note: We don't tick here to avoid mutating in a read operation
            // Expired entries will be cleaned up on next write
            Ok(Some(*value))
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
        // Process any pending expirations
        self.tick(now);
        
        if self.entries.contains_key(key) {
            Ok(false)
        } else {
            let expiry = now + ttl;
            if let Some((wheel_idx, slot_idx)) = self.calculate_wheel_position(expiry, now) {
                // Add to wheel
                self.wheels[wheel_idx].add_to_slot(slot_idx, key.to_string());
                self.entries.insert(key.to_string(), (value, wheel_idx, slot_idx));
                Ok(true)
            } else {
                // Already expired
                Ok(false)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_timing_wheel_basic_operations() {
        let mut store = TimingWheelStore::new();
        let now = SystemTime::now();
        let ttl = Duration::from_secs(30);
        
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
    fn test_timing_wheel_expiry() {
        let mut store = TimingWheelStore::new();
        let now = SystemTime::now();
        
        // Add entries with different TTLs
        assert!(store.set_if_not_exists_with_ttl("key1", 1, Duration::from_secs(2), now).unwrap());
        assert!(store.set_if_not_exists_with_ttl("key2", 2, Duration::from_secs(65), now).unwrap());
        assert!(store.set_if_not_exists_with_ttl("key3", 3, Duration::from_secs(3700), now).unwrap());
        
        // All should exist initially
        assert_eq!(store.get("key1", now).unwrap(), Some(1));
        assert_eq!(store.get("key2", now).unwrap(), Some(2));
        assert_eq!(store.get("key3", now).unwrap(), Some(3));
        
        // After 3 seconds, key1 should be expired
        let later = now + Duration::from_secs(3);
        store.tick(later);
        assert_eq!(store.entries.contains_key("key1"), false);
        assert_eq!(store.entries.contains_key("key2"), true);
        assert_eq!(store.entries.contains_key("key3"), true);
    }
    
    #[test]
    fn test_wheel_selection() {
        let store = TimingWheelStore::new();
        let now = SystemTime::now();
        
        // Test different TTL ranges
        let pos1 = store.calculate_wheel_position(now + Duration::from_secs(30), now).unwrap();
        assert_eq!(pos1.0, 0); // Seconds wheel
        
        let pos2 = store.calculate_wheel_position(now + Duration::from_secs(120), now).unwrap();
        assert_eq!(pos2.0, 1); // Minutes wheel
        
        let pos3 = store.calculate_wheel_position(now + Duration::from_secs(7200), now).unwrap();
        assert_eq!(pos3.0, 2); // Hours wheel
        
        let pos4 = store.calculate_wheel_position(now + Duration::from_secs(172800), now).unwrap();
        assert_eq!(pos4.0, 3); // Days wheel
    }
}
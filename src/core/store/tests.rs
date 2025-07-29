use super::{MemoryStore, Store};
use std::time::{Duration, SystemTime};

#[test]
fn test_memory_store_set_and_get() {
    let mut store = MemoryStore::new();
    let now = SystemTime::now();

    // Set a value
    let success = (&mut store)
        .set_if_not_exists_with_ttl("key1", 42, Duration::from_secs(60), now)
        .unwrap();
    assert!(success);

    // Get the value
    let value = store.get("key1", now).unwrap();
    assert_eq!(value, Some(42));

    // Try to set again - should fail
    let success = (&mut store)
        .set_if_not_exists_with_ttl("key1", 100, Duration::from_secs(60), now)
        .unwrap();
    assert!(!success);

    // Value should still be 42
    let value = store.get("key1", now).unwrap();
    assert_eq!(value, Some(42));
}

#[test]
fn test_memory_store_compare_and_swap() {
    let mut store = MemoryStore::new();
    let now = SystemTime::now();

    // Set initial value
    (&mut store)
        .set_if_not_exists_with_ttl("key1", 10, Duration::from_secs(60), now)
        .unwrap();

    // Successful CAS
    let success = (&mut store)
        .compare_and_swap_with_ttl("key1", 10, 20, Duration::from_secs(60), now)
        .unwrap();
    assert!(success);

    let value = store.get("key1", now).unwrap();
    assert_eq!(value, Some(20));

    // Failed CAS - old value doesn't match
    let success = (&mut store)
        .compare_and_swap_with_ttl("key1", 10, 30, Duration::from_secs(60), now)
        .unwrap();
    assert!(!success);

    let value = store.get("key1", now).unwrap();
    assert_eq!(value, Some(20)); // Still 20
}

#[test]
fn test_memory_store_ttl() {
    let mut store = MemoryStore::new();
    let now = SystemTime::now();

    // Set with very short TTL
    (&mut store)
        .set_if_not_exists_with_ttl("key1", 42, Duration::from_millis(100), now)
        .unwrap();

    // Value should exist immediately
    let value = store.get("key1", now).unwrap();
    assert_eq!(value, Some(42));

    // Simulate time passing
    let later = now + Duration::from_millis(200);

    // Trigger cleanup by trying to set a new value
    (&mut store)
        .set_if_not_exists_with_ttl("key2", 100, Duration::from_secs(60), later)
        .unwrap();

    // Original key should be gone after cleanup
    let value = store.get("key1", later).unwrap();
    assert_eq!(value, None);
}

#[test]
fn test_memory_store_get_nonexistent() {
    let store = MemoryStore::new();
    let now = SystemTime::now();

    let value = store.get("nonexistent", now).unwrap();
    assert_eq!(value, None);
}

#[test]
fn test_memory_store_multiple_keys() {
    let mut store = MemoryStore::new();
    let now = SystemTime::now();

    // Set multiple keys
    for i in 0..10 {
        let key = format!("key{i}");
        (&mut store)
            .set_if_not_exists_with_ttl(&key, i * 10, Duration::from_secs(60), now)
            .unwrap();
    }

    // Verify all keys
    for i in 0..10 {
        let key = format!("key{i}");
        let value = store.get(&key, now).unwrap();
        assert_eq!(value, Some(i * 10));
    }
}

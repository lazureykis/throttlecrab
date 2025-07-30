# Store Capacity Behavior Guide

This document explains what happens when different throttlecrab store implementations reach their capacity limits.

## Overview

Most stores use the `with_capacity()` constructor which provides a hint for initial allocation, but behavior varies significantly when capacity is exceeded.

## Store-by-Store Capacity Behavior

### 1. **Standard MemoryStore**
```rust
let store = MemoryStore::new();
```
- **Initial Capacity**: HashMap default (0, grows as needed)
- **When Full**: Grows dynamically, limited only by system memory
- **Performance Impact**: Reallocation causes temporary performance hit
- **Use When**: Unpredictable key count, memory not a constraint

### 2. **OptimizedMemoryStore**
```rust
let store = OptimizedMemoryStore::with_capacity(10_000);
```
- **Initial Capacity**: Pre-allocated HashMap with given capacity
- **When Full**: Grows dynamically like standard HashMap
- **Performance Impact**: Better initial performance, same reallocation cost
- **Use When**: Known approximate key count, want better startup performance

### 3. **InternedMemoryStore**
```rust
let store = InternedMemoryStore::with_capacity(10_000);
```
- **Initial Capacity**: Pre-allocated for both strings and entries
- **When Full**: Grows dynamically, but string interning has overhead
- **Performance Impact**: String table growth can be expensive
- **Use When**: Many duplicate keys expected

### 4. **ArenaMemoryStore** ⚠️
```rust
let store = ArenaMemoryStore::with_capacity(10_000);
```
- **Initial Capacity**: FIXED SIZE - pre-allocated arrays
- **When Full**: 
  - First attempts cleanup of expired entries
  - If still full: **Returns error "Arena capacity exceeded"**
- **Performance Impact**: Allocation failure breaks rate limiting
- **Use When**: Bounded, predictable key count
- **Critical**: Must size appropriately or implement fallback

### 5. **CompactMemoryStore**
```rust
let store = CompactMemoryStore::with_capacity(10_000);
```
- **Initial Capacity**: Pre-allocated HashMap
- **When Full**: Grows dynamically
- **Performance Impact**: Short string optimization reduces memory pressure
- **Use When**: Memory efficiency is important

### 6. **TimingWheelStore**
```rust
let store = TimingWheelStore::with_capacity(10_000);
```
- **Initial Capacity**: Fixed wheel sizes, dynamic entry storage
- **When Full**: Entries HashMap grows dynamically
- **Performance Impact**: Wheel overflow degrades to O(n) operations
- **Use When**: TTL-heavy workloads with predictable patterns

### 7. **Adaptive/Amortized/ProbabilisticMemoryStore**
```rust
let store = AdaptiveMemoryStore::with_capacity(10_000);
```
- **Initial Capacity**: Pre-allocated HashMap
- **When Full**: Grows dynamically
- **Special Behavior**: More aggressive cleanup may prevent growth
- **Performance Impact**: Cleanup strategies may free space before growth
- **Use When**: Want automatic memory management

### 8. **BTreeStore**
```rust
let store = BTreeStore::with_capacity(10_000); // Ignored
```
- **Initial Capacity**: N/A - BTreeMap doesn't support pre-allocation
- **When Full**: Always grows dynamically
- **Performance Impact**: O(log n) operations, predictable growth
- **Use When**: Need sorted keys or predictable performance

### 9. **HeapStore**
```rust
let store = HeapStore::with_capacity(10_000);
```
- **Initial Capacity**: HashMap and BinaryHeap pre-allocated
- **When Full**: Both structures grow dynamically
- **Performance Impact**: Heap reallocation can be expensive
- **Use When**: TTL-based cleanup is critical

### 10. **BloomFilterStore** ⚠️
```rust
let store = BloomFilterStore::with_config(inner, 10_000, 0.01);
```
- **Initial Capacity**: Fixed-size bit array based on parameters
- **When Full**: 
  - False positive rate increases beyond target
  - May regenerate filter (losing all entries temporarily)
- **Performance Impact**: Degraded filtering accuracy
- **Use When**: Can tolerate false positives

## Production Recommendations

### For Bounded Systems (e.g., Internal Services)
```rust
// Use Arena with 2-3x expected capacity
let capacity = expected_keys * 3;
let store = ArenaMemoryStore::with_capacity(capacity);
```

### For Unbounded Systems (e.g., Public APIs)
```rust
// Use Adaptive or Amortized with cleanup
let store = AdaptiveMemoryStore::with_capacity(expected_keys);
```

### For DDoS Protection
```rust
// Use Probabilistic with aggressive cleanup
let store = ProbabilisticMemoryStore::with_capacity(100_000);
```

### Handling Arena Capacity Errors
```rust
match limiter.rate_limit(key, burst, rate, period, cost, now) {
    Ok((allowed, result)) => { /* normal handling */ },
    Err(CellError::InternalError(msg)) if msg.contains("capacity") => {
        // Arena is full - options:
        // 1. Reject request (fail closed)
        // 2. Allow request (fail open)  
        // 3. Fall back to different store
        // 4. Trigger emergency cleanup
    },
    Err(e) => { /* other errors */ }
}
```

## Capacity Planning Formula

For most stores:
```
capacity = expected_unique_keys * growth_factor

Where growth_factor:
- 1.5x for stable systems
- 2-3x for growing systems  
- 5-10x for unpredictable/attack scenarios
```

For Arena (fixed capacity):
```
capacity = max_unique_keys * safety_factor

Where safety_factor:
- 1.2x minimum
- 2x recommended
- 3x for critical systems
```

## Memory Usage Estimation

| Store Type | Memory per Entry | Overhead |
|------------|-----------------|----------|
| Standard | ~200 bytes | HashMap overhead |
| Optimized | ~180 bytes | Pre-allocation |
| Interned | ~150 bytes | String table |
| Arena | ~100 bytes | Fixed allocation |
| Compact | ~80 bytes | Short string opt |
| Adaptive | ~200 bytes | Cleanup metadata |
| BTree | ~250 bytes | Tree structure |
| Heap | ~220 bytes | Heap + HashMap |
| BloomFilter | ~10 bits | Filter + inner store |

## Warning Signs

Monitor for these indicators of capacity issues:

1. **Arena Store**: "Arena capacity exceeded" errors
2. **High Memory Usage**: Unexpected growth beyond capacity
3. **Performance Degradation**: Reallocation pauses
4. **Increased GC Pressure**: Frequent cleanup cycles
5. **False Positive Rate**: BloomFilter accuracy degradation

## Best Practices

1. **Always set initial capacity** based on expected load
2. **Monitor actual vs expected capacity** in production
3. **Implement capacity alerts** before hitting limits
4. **Have a fallback strategy** for capacity errors
5. **Test capacity behavior** under load testing
6. **Size Arena conservatively** - it cannot grow
7. **Consider sharding** for very high cardinality
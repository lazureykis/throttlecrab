# Store Capacity Behavior Guide

This document explains what happens when different throttlecrab store implementations reach their capacity limits.

## Overview

Most stores use the `with_capacity()` constructor which provides a hint for initial allocation, but behavior varies significantly when capacity is exceeded.

## Store-by-Store Capacity Behavior

### 1. **OptimizedMemoryStore**
```rust
let store = OptimizedMemoryStore::with_capacity(10_000);
```
- **Initial Capacity**: Pre-allocated HashMap with given capacity
- **When Full**: Grows dynamically like standard HashMap
- **Performance Impact**: Better initial performance, same reallocation cost
- **Use When**: Known approximate key count, want better startup performance

### 2. **InternedMemoryStore**
```rust
let store = InternedMemoryStore::with_capacity(10_000);
```
- **Initial Capacity**: Pre-allocated for both strings and entries
- **When Full**: Grows dynamically, but string interning has overhead
- **Performance Impact**: String table growth can be expensive
- **Use When**: Many duplicate keys expected

### 3. **Adaptive/Amortized/ProbabilisticMemoryStore**
```rust
let store = AdaptiveMemoryStore::with_capacity(10_000);
```
- **Initial Capacity**: Pre-allocated HashMap
- **When Full**: Grows dynamically
- **Special Behavior**: More aggressive cleanup may prevent growth
- **Performance Impact**: Cleanup strategies may free space before growth
- **Use When**: Want automatic memory management

## Production Recommendations

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

## Capacity Planning Formula

For most stores:
```
capacity = expected_unique_keys * growth_factor

Where growth_factor:
- 1.5x for stable systems
- 2-3x for growing systems  
- 5-10x for unpredictable/attack scenarios
```

## Memory Usage Estimation

| Store Type | Memory per Entry | Overhead |
|------------|-----------------|----------|
| Optimized | ~180 bytes | Pre-allocation |
| Interned | ~150 bytes | String table |
| Compact | ~80 bytes | Short string opt |
| Adaptive | ~200 bytes | Cleanup metadata |
| Amortized | ~190 bytes | Basic cleanup |
| Probabilistic | ~195 bytes | Random cleanup |

## Warning Signs

Monitor for these indicators of capacity issues:

1. **High Memory Usage**: Unexpected growth beyond capacity
2. **Performance Degradation**: Reallocation pauses
3. **Increased GC Pressure**: Frequent cleanup cycles

## Best Practices

1. **Always set initial capacity** based on expected load
2. **Monitor actual vs expected capacity** in production
3. **Implement capacity alerts** before hitting limits
4. **Have a fallback strategy** for capacity errors
5. **Test capacity behavior** under load testing
6. **Consider sharding** for very high cardinality
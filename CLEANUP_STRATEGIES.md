# Cleanup Strategies Analysis

## The Problem
With HashMap-based storage, expired entries accumulate over time and must be cleaned up. The challenge is balancing:
- **Memory usage** - Don't let expired entries grow unbounded
- **Latency** - Avoid cleanup spikes that hurt P99 latency  
- **Throughput** - Minimize cleanup overhead

## Strategies Comparison

### 1. Eager Cleanup (Standard MemoryStore)
```rust
// Clean on EVERY operation
fn clean_expired(&mut self) {
    self.data.retain(|_, (_, expiry)| !is_expired(expiry));
}
```
- ❌ **O(n) on every operation** - Terrible for performance
- ✅ Memory always minimal
- ❌ Throughput: 143K req/s with 10K keys

### 2. Periodic Cleanup (OptimizedMemoryStore) 
```rust
// Clean every 60 seconds OR when >20% expired
if now >= next_cleanup || expired_ratio > 0.2 {
    cleanup();
}
```
- ✅ **45x faster** - 10.8M req/s with 10K keys
- ⚠️ Can have latency spikes during cleanup
- ⚠️ Memory can grow between cleanups

### 3. Amortized Cleanup (NEW)
```rust
// Clean small batch every 100 operations
if ops % 100 == 0 {
    clean_next_10_entries();
}
```
- ✅ **Consistent latency** - No spikes
- ✅ Bounded memory growth
- ✅ Predictable overhead: 10/100 = 10% of ops do cleanup

### 4. Probabilistic Cleanup
```rust
// 0.1% chance to cleanup on each operation
if random() < 0.001 {
    cleanup_all();
}
```
- ✅ Simple implementation
- ⚠️ Less predictable than amortized
- ⚠️ Can still have occasional spikes

### 5. Adaptive Cleanup
```rust
// Adjust cleanup frequency based on removal rate
if last_cleanup_removed_many {
    decrease_interval();
} else {
    increase_interval();
}
```
- ✅ Self-tuning to workload
- ⚠️ More complex
- ✅ Good for varying traffic patterns

## Recommendations

**For consistent low latency (P99):** Use AmortizedMemoryStore
- Spreads cleanup cost evenly
- No latency spikes
- Predictable performance

**For maximum throughput:** Use OptimizedMemoryStore with AHash
- Deferred cleanup minimizes overhead
- Accept occasional latency spikes
- Best average performance

**For production systems:** Consider:
1. Monitor memory usage
2. Set memory limits
3. Use amortized cleanup for user-facing services
4. Use periodic cleanup for batch processing

## Real-world Considerations

1. **TTL Distribution** matters:
   - Uniform TTLs → Periodic cleanup works well
   - Random TTLs → Amortized cleanup better

2. **Traffic Patterns**:
   - Steady traffic → Any strategy works
   - Bursty traffic → Avoid per-operation cleanup

3. **Memory Constraints**:
   - Tight memory → More frequent cleanup
   - Plenty of memory → Less frequent, larger cleanups

4. **SLA Requirements**:
   - Strict P99 → Amortized cleanup
   - Best average → Periodic cleanup
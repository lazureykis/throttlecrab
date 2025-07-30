# Benchmark Results Summary

## Store Performance Comparison

### 10,000 Unique Keys
| Store Type | Throughput | Time per Op | vs Standard |
|------------|------------|-------------|-------------|
| Standard | 143K req/s | 6.98 µs | 1x (baseline) |
| Optimized | 13.2M req/s | 75.5 ns | **92x faster** |
| Interned | 9.8M req/s | 102 ns | 68x faster |
| Fast Hash | 6.6M req/s | 152 ns | 46x faster |
| Simple Hash | 11.2M req/s | 89.4 ns | 78x faster |
| **AHash** | **17.1M req/s** | **58.4 ns** | **🚀 119x faster** |

### 100,000 Unique Keys
| Store Type | Throughput | Time per Op | vs Standard |
|------------|------------|-------------|-------------|
| Standard | 356K req/s | 2.81 µs | 1x (baseline) |
| Optimized | 7.95M req/s | 126 ns | **22x faster** |

## Key Optimizations

1. **Deferred Cleanup** - The biggest win
   - Standard: Cleans expired entries on every operation
   - Optimized: Cleans only every 60 seconds or when 20% are expired

2. **Pre-allocated Capacity** - Avoids rehashing
   - Allocates 1.3x expected capacity upfront

3. **Fast Hashing** - AHash wins
   - Standard HashMap uses SipHash (cryptographically secure but slower)
   - AHash uses SIMD-optimized non-cryptographic hash

## Performance by Key Count

### Standard MemoryStore Degradation
- 10 keys: ~18M req/s
- 100 keys: ~18M req/s  
- 1,000 keys: ~5.9M req/s (3x slower)
- 10,000 keys: ~240K req/s (75x slower)
- 100,000 keys: ~356K req/s (50x slower than 10 keys)

### AHash + Optimizations
- 10,000 keys: 17.1M req/s (only 1.5x slower than single key!)
- Maintains high performance even with many keys

## Recommendations

For production use with many unique keys:
1. Use deferred cleanup strategy
2. Pre-allocate HashMap capacity
3. Consider using AHash if adding dependencies is acceptable
4. For zero-dependency, OptimizedMemoryStore gives 92x improvement
# ThrottleCrab Benchmark Results

## Store Comparison Results

### Overall Performance (400K operations, 2K unique keys)

| Store Implementation | Throughput (ops/s) | Speedup vs Standard | Total Time (ms) |
|---------------------|-------------------|-------------------|----------------|
| Adaptive MemoryStore | 12,532,440 | 21.6x | 31.92 |
| Optimized MemoryStore | 11,429,782 | 19.7x | 35.00 |
| Amortized MemoryStore | 11,084,720 | 19.1x | 36.09 |
| Probabilistic MemoryStore | 10,061,743 | 17.3x | 39.75 |
| Compact MemoryStore | 9,169,798 | 15.8x | 43.62 |
| Interned MemoryStore | 9,040,961 | 15.6x | 44.24 |
| MemoryStore (baseline) | 580,000 | 1.0x | 689.66 |

## Access Pattern Performance (100K operations, 1K unique keys)

### Sequential Access Pattern
| Store | Throughput (ops/s) | Speedup | Characteristics |
|-------|-------------------|---------|-----------------|
| Interned | 9,978,836 | 17.3x | Best for sequential - key interning benefits from repeated patterns |
| Optimized | 8,410,664 | 14.6x | Good general performance |
| Compact | 7,895,126 | 13.7x | Memory efficient with reasonable speed |
| Amortized | 7,367,206 | 12.7x | Cleanup strategy adds some overhead |
| Probabilistic | 7,136,209 | 12.3x | Random cleanup slightly slower |
| Adaptive | 7,121,999 | 12.3x | Dynamic cleanup adjustment |
| MemoryStore | 578,040 | 1.0x | Baseline performance |

### Random Access Pattern
| Store | Throughput (ops/s) | Speedup | Characteristics |
|-------|-------------------|---------|-----------------|
| Optimized | 8,458,805 | 16.0x | HashMap optimizations shine |
| Probabilistic | 7,992,140 | 15.2x | Random cleanup works well with random access |
| Adaptive | 7,579,394 | 14.4x | Adapts well to unpredictable patterns |
| Amortized | 7,166,917 | 13.6x | Consistent performance |
| Compact | 5,672,404 | 10.8x | Key packing overhead more visible |
| Interned | 4,533,391 | 8.6x | String interning less effective for random keys |
| MemoryStore | 527,270 | 1.0x | Baseline performance |

### Hot Keys Pattern (80% requests on 20% keys)
| Store | Throughput (ops/s) | Speedup | Characteristics |
|-------|-------------------|---------|-----------------|
| Amortized | 12,041,120 | 2.0x | Cleanup focuses on hot keys efficiently |
| Interned | 11,480,671 | 1.9x | String interning excels with repeated keys |
| Probabilistic | 10,826,514 | 1.8x | Probabilistic cleanup handles hot keys well |
| Adaptive | 7,964,531 | 1.3x | Adapts cleanup frequency to hot key pattern |
| Compact | 7,390,345 | 1.2x | Reasonable performance |
| Optimized | 6,400,324 | 1.1x | Similar to baseline for this pattern |
| MemoryStore | 6,075,718 | 1.0x | Baseline (already good for hot keys) |

### Sparse Pattern (90% non-existent keys)
| Store | Throughput (ops/s) | Speedup | Characteristics |
|-------|-------------------|---------|-----------------|
| Adaptive | 9,282,932 | 15.2x | Adapts to high miss rate |
| Amortized | 9,174,522 | 15.0x | Efficient handling of non-existent keys |
| Probabilistic | 9,146,863 | 15.0x | Random cleanup effective |
| Interned | 8,871,769 | 14.5x | Pre-allocated IDs help |
| Compact | 8,638,375 | 14.2x | Compact representation efficient |
| Optimized | 6,204,147 | 10.2x | HashMap lookups for non-existent keys |
| MemoryStore | 609,740 | 1.0x | Baseline performance |

## Key Insights

### Best Use Cases by Store Type

1. **Adaptive MemoryStore** (Overall Winner)
   - Best adaptive performance: 21.6x speedup
   - Excels at sparse patterns and adapts to workload
   - Recommended for: General purpose, unpredictable workloads

2. **Interned MemoryStore**
   - Best for sequential access: 17.3x speedup
   - Excellent for hot keys: 1.9x over baseline
   - Recommended for: APIs with predictable key patterns

3. **Amortized MemoryStore**
   - Best for hot keys: 2.0x over baseline
   - Excellent sparse performance: 15.0x speedup
   - Recommended for: Workloads with popular endpoints

4. **Probabilistic MemoryStore**
   - Strong random access: 15.2x speedup
   - Good hot key handling: 1.8x over baseline
   - Recommended for: Unpredictable access patterns

5. **Compact MemoryStore**
   - Memory efficient: 15.8x speedup
   - Consistent across patterns
   - Recommended for: Memory-constrained environments


### Performance Summary

- **Biggest improvement**: Adaptive store with 21.6x speedup
- **Most consistent**: Optimized and Amortized stores
- **Best for hot keys**: Amortized and Interned stores
- **Best for sparse keys**: Adaptive and Amortized stores
- **Memory efficient**: Compact store with 15.8x speedup
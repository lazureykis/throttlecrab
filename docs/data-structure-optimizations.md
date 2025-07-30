# Data Structure Optimizations for ThrottleCrab

This document outlines planned data structure optimizations for the ThrottleCrab rate limiter. Each optimization targets specific performance characteristics and use cases.

## Current Implementations

Before diving into new optimizations, here's what we already have:

1. **Standard MemoryStore** - Basic HashMap with cleanup on every operation
2. **OptimizedMemoryStore** - Periodic cleanup instead of per-operation
3. **InternedMemoryStore** - String interning to reduce allocations
4. **AmortizedMemoryStore** - Spreads cleanup cost across operations
5. **ProbabilisticMemoryStore** - Random cleanup trigger
6. **AdaptiveMemoryStore** - Adapts cleanup frequency based on workload
7. **FastHashMemoryStore/SimpleHashMemoryStore** - Custom hash functions
8. **AHashMemoryStore** - Uses AHash, a fast hash algorithm

## Planned Optimizations

### 1. Arena-Allocated Store

**Goal**: Reduce allocator pressure and improve cache locality

**Implementation Details**:
- Pre-allocate memory in large contiguous chunks (arenas)
- Use custom allocator for key-value pairs
- Implement generational arena for efficient cleanup
- Store indices instead of pointers for better cache usage

**Benefits**:
- Reduced malloc/free overhead
- Better cache locality due to contiguous memory
- Predictable memory usage patterns
- Faster bulk operations

**Trade-offs**:
- More complex memory management
- Potential for internal fragmentation
- Fixed maximum capacity per arena

**Use Cases**:
- High-throughput scenarios with predictable load
- Systems with strict latency requirements
- Embedded or resource-constrained environments

### 2. Sharded Store

**Goal**: Improve concurrent access performance

**Implementation Details**:
- Partition keyspace across N internal stores
- Use consistent hashing for shard selection
- Per-shard locks or lock-free structures
- Configurable shard count based on CPU cores

**Benefits**:
- Reduced lock contention
- Better multi-core scalability
- Parallel cleanup operations
- Cache-line isolation between shards

**Trade-offs**:
- Increased memory overhead
- More complex implementation
- Potential for uneven shard distribution

**Use Cases**:
- Multi-threaded applications
- High-concurrency web services
- Systems with many CPU cores

### 3. Compact Key Representation

**Goal**: Reduce memory usage and improve cache efficiency

**Implementation Details**:
- Use u32 for timestamps (seconds since custom epoch)
- Pack TAT value and expiry into single u64
- Bit-packed flags for common states
- Short string optimization for keys < 16 bytes

**Benefits**:
- 50% reduction in value storage size
- Better cache line utilization
- Reduced memory bandwidth usage
- Faster comparisons

**Trade-offs**:
- Limited timestamp range (136 years with u32)
- Slightly more complex packing/unpacking
- Potential precision loss

**Use Cases**:
- Memory-constrained deployments
- Very high key count scenarios
- Edge computing environments

### 4. Lock-Free Data Structures

**Goal**: Eliminate lock contention entirely

**Implementation Details**:
- Use atomic operations for all updates
- Implement hazard pointers for safe memory reclamation
- Compare-and-swap loops for updates
- Wait-free read operations

**Benefits**:
- No blocking on locks
- Better worst-case latency
- Improved scalability
- Deadlock-free by design

**Trade-offs**:
- Complex implementation
- Potential for high CPU usage under contention
- Platform-specific atomic requirements
- Harder to debug

**Use Cases**:
- Ultra-low latency requirements
- Systems that cannot tolerate blocking
- Real-time applications

### 5. SIMD-Optimized Operations

**Goal**: Leverage CPU vector instructions for bulk operations

**Implementation Details**:
- Batch expiry checks using SIMD comparisons
- Vectorized hash computations
- Parallel key matching for cleanup
- Platform-specific implementations (AVX2, AVX-512, NEON)

**Benefits**:
- Process multiple entries per CPU cycle
- Significantly faster cleanup operations
- Better utilization of modern CPUs
- Reduced branching in hot paths

**Trade-offs**:
- Platform-specific code
- Increased binary size
- Requires runtime CPU detection
- Complex implementation

**Use Cases**:
- Batch processing systems
- Large-scale cleanup operations
- Systems with predictable access patterns

### 6. Hierarchical Timing Wheels

**Goal**: Efficient O(1) expiry tracking

**Implementation Details**:
- Multiple wheel levels for different time granularities
- Cascade entries between wheels as time advances
- Dedicated expiry buckets
- Lazy evaluation of expired entries

**Benefits**:
- O(1) insertion and expiry detection
- Efficient bulk expiry handling
- Predictable memory usage
- Natural batching of cleanup work

**Trade-offs**:
- Fixed time granularity
- Additional memory overhead
- More complex than simple expiry tracking
- Requires periodic tick processing

**Use Cases**:
- Systems with uniform TTLs
- High volume of expiring entries
- Predictable expiry patterns

### 7. Bloom Filter Pre-check

**Goal**: Reduce HashMap lookups for non-existent keys

**Implementation Details**:
- Probabilistic pre-filter for existence checks
- Multiple hash functions for low false positive rate
- Periodic filter regeneration
- Size based on expected key count

**Benefits**:
- Fast negative lookups (key doesn't exist)
- Reduced HashMap access
- Memory efficient for sparse keyspaces
- CPU cache friendly

**Trade-offs**:
- False positives require HashMap lookup
- Additional memory overhead
- Requires periodic maintenance
- No benefit for existing keys

**Use Cases**:
- APIs with many invalid key attempts
- Sparse keyspaces
- DDoS protection scenarios
- Cache-miss optimization

## Implementation Priority

Based on complexity and expected impact:

1. **High Priority**:
   - Arena-Allocated Store (good balance of benefit/complexity)
   - Sharded Store (immediate concurrency benefits)
   - Compact Key Representation (easy wins)

2. **Medium Priority**:
   - Hierarchical Timing Wheels (specific use cases)
   - Bloom Filter Pre-check (specific use cases)

3. **Low Priority**:
   - Lock-Free Data Structures (high complexity)
   - SIMD Operations (platform specific)

## Benchmarking Strategy

For each optimization:

1. Implement as separate Store trait implementation
2. Add to existing benchmark suite
3. Test with various:
   - Key counts (10 to 1M)
   - Access patterns (sequential, random, hot-key)
   - TTL distributions
   - Concurrency levels
4. Measure:
   - Throughput (ops/sec)
   - Latency (p50, p99, p999)
   - Memory usage
   - CPU usage
5. Compare against baseline implementations

## Integration Plan

1. Each optimization will be implemented as a separate module
2. All implementations will conform to the Store trait
3. Users can choose implementation based on their use case
4. Documentation will guide implementation selection
5. Default implementation remains unchanged for compatibility
# ThrottleCrab Integration Tests & Benchmarks

This directory contains comprehensive integration tests and performance benchmarks for the ThrottleCrab server.

## Quick Start

Run all benchmarks with default settings:
```bash
./run-benchmarks.sh
```

Run specific benchmark suite:
```bash
./run-benchmarks.sh transports  # Test all transport protocols
./run-benchmarks.sh stores      # Compare store types
./run-benchmarks.sh workloads   # Test workload patterns
./run-benchmarks.sh stress      # Run stress test
./run-benchmarks.sh multi       # Test multiple transports
```

## Benchmark Suites

### 1. Transport Benchmarks
Tests the performance of each transport protocol (HTTP, gRPC, Native) under steady load.

```bash
./run-benchmarks.sh -d 60 -r 50000 transports
```

### 2. Store Comparison
Compares the performance characteristics of different store types:
- **Periodic**: Fixed interval cleanup
- **Probabilistic**: Random cleanup based on probability
- **Adaptive**: Dynamic cleanup based on load

```bash
./run-benchmarks.sh -d 30 stores
```

### 3. Workload Patterns
Tests various realistic workload patterns:
- **Steady**: Constant request rate
- **Burst**: Alternating high/low traffic
- **Ramp**: Gradually increasing load
- **Wave**: Sinusoidal traffic pattern

```bash
./run-benchmarks.sh workloads
```

### 4. Stress Test
Gradually increases load to find the server's breaking point:

```bash
./run-benchmarks.sh -d 120 stress
```

### 5. Multi-Transport Test
Tests all transports concurrently sharing the same rate limiter:

```bash
./run-benchmarks.sh multi
```

## Advanced Options

### Custom Duration and RPS
```bash
./run-benchmarks.sh -d 120 -r 100000 all
```

### Save Results as JSON
```bash
./run-benchmarks.sh -j -o results-2024 stores
```

### Compare with Previous Run
```bash
./run-benchmarks.sh -c results-2024 stores
```

## Running Individual Tests

You can also run tests directly with cargo:

```bash
# Run all integration tests
cargo test --release -- --nocapture

# Run specific test
cargo test --release test_all_transports -- --nocapture

# Run benchmark executable
cargo test --release benchmark -- --nocapture
```

## Test Architecture

### Workload Generator
- Supports multiple traffic patterns
- Configurable key distribution (sequential, random, zipfian, user-resource)
- Tracks detailed latency percentiles
- Records success/failure/rate-limited requests

### Transport Tests
- Each transport runs in isolation with its own server instance
- Tests use realistic request patterns
- Measures end-to-end latency including network overhead

### Store Comparison
- Tests memory efficiency with high cardinality
- Cleanup performance with short TTL keys
- Burst handling capabilities
- Hotspot resilience with zipfian distribution

## Performance Metrics

Each benchmark reports:
- **Throughput**: Requests per second
- **Success Rate**: Percentage of allowed requests
- **Rate Limit Rate**: Percentage of rate-limited requests
- **Latency Percentiles**: P50, P90, P95, P99, P99.9
- **Error Rate**: Failed requests

## Interpreting Results

### Transport Performance
- **Native**: Lowest latency, highest throughput
- **gRPC**: Best for service-to-service communication
- **HTTP**: Most compatible, easiest to integrate

### Store Selection
- **Periodic**: Predictable cleanup overhead, good for steady workloads
- **Probabilistic**: Low overhead, good for high-throughput scenarios
- **Adaptive**: Best for variable workloads, self-tuning

### Expected Performance
On modern hardware (M1/M2 Mac, recent Intel/AMD):
- Single transport: 100k-500k RPS
- All transports concurrent: 200k-1M RPS total
- P99 latency: <1ms for Native, <5ms for HTTP/gRPC

## Troubleshooting

### Server fails to start
- Check ports are not in use: `lsof -i :8080`
- Ensure release build: `cargo build --release`

### Low performance
- Check CPU throttling: `pmset -g thermal`
- Disable debug logging: Use `--log-level warn`
- Increase file descriptor limit: `ulimit -n 65536`

### Inconsistent results
- Run longer tests: `-d 60` or more
- Use warmup period for JIT optimization
- Close other applications to reduce interference
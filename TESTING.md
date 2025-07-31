# Testing Guide

This document describes how to run tests and benchmarks for the ThrottleCrab project.

## Unit Tests

Run unit tests for all crates:

```bash
cargo test --all
```

Run tests for specific crates:

```bash
# Core library tests
cargo test -p throttlecrab

# Server tests
cargo test -p throttlecrab-server

# Client tests
cargo test -p throttlecrab-client
```

## Integration Tests

The `integration-tests/` directory contains comprehensive integration and performance tests.

### Running Integration Tests

```bash
cd integration-tests
cargo test --release
```

### Performance Testing Scripts

#### Basic Performance Test
Tests HTTP transport with standard configuration:
```bash
./run-perf-test.sh
```

#### Custom Performance Test
Configurable test with custom parameters:
```bash
./run-custom-test.sh [threads] [requests] [port] [store] [log_level]

# Example: 50 threads, 10000 requests each
./run-custom-test.sh 50 10000
```

#### Client Performance Test
Tests native protocol with connection pooling:
```bash
./run-client-perf-test.sh
```

#### Transport Comparison
Compares performance across all transport types:
```bash
./run-transport-test.sh
```

#### Heavy Load Test
Stress test with high concurrency:
```bash
./run-heavy-test.sh
```

## Benchmarks

### Library Benchmarks

The core library includes micro-benchmarks for different store implementations:

```bash
cd throttlecrab
cargo bench
```

To run specific benchmarks:
```bash
cargo bench store_comparison
cargo bench access_patterns
```

### Server Benchmarks

The server includes comprehensive benchmark suites:

```bash
cd throttlecrab-server/tests
./run-benchmarks.sh
```

Available benchmark suites:
- `transports` - Compare all transport protocols
- `stores` - Compare store implementations
- `workloads` - Test different traffic patterns
- `stress` - Find breaking points
- `multi` - Test concurrent transports

Run specific suite:
```bash
./run-benchmarks.sh transports
```

With custom parameters:
```bash
./run-benchmarks.sh -d 60 -r 100000 stores
```

## Performance Profiling

### Using Instruments (macOS)

```bash
# Build with debug symbols
cargo build --release

# Profile with Instruments
instruments -t "Time Profiler" target/release/throttlecrab-server
```

### Using perf (Linux)

```bash
# Build with debug symbols
cargo build --release

# Record performance data
perf record -g target/release/throttlecrab-server
perf report
```

### Flamegraphs

```bash
# Install flamegraph
cargo install flamegraph

# Generate flamegraph
cargo flamegraph --bin throttlecrab-server -- --native
```

## Load Testing

### Using the Integration Test Binary

```bash
cd integration-tests
cargo build --release

# Run load test
./target/release/throttlecrab-integration-tests perf-test \
    --threads 100 \
    --requests 10000 \
    --port 9090
```

### Using External Tools

#### wrk
```bash
# Install wrk
brew install wrk  # macOS

# Test HTTP endpoint
wrk -t12 -c400 -d30s \
    -s scripts/throttle.lua \
    http://localhost:8080/throttle
```

#### hey
```bash
# Install hey
go install github.com/rakyll/hey@latest

# Test HTTP endpoint
hey -n 100000 -c 100 -m POST \
    -H "Content-Type: application/json" \
    -d '{"key":"test","max_burst":10,"count_per_period":100,"period":60}' \
    http://localhost:8080/throttle
```

## Continuous Integration

Tests run automatically on every push via GitHub Actions:

- Unit tests for all crates
- Integration tests
- Clippy lints
- Format checking
- Code coverage reporting

See `.github/workflows/ci.yml` for details.

## Troubleshooting

### Tests Failing Due to Port Conflicts

Kill any running throttlecrab servers:
```bash
lsof -ti:9090 | xargs kill -9
lsof -ti:8080 | xargs kill -9
```

### Performance Test Variations

Results may vary due to:
- System load
- CPU throttling
- Background processes

For consistent results:
- Close other applications
- Disable CPU throttling
- Run tests multiple times
- Use longer test durations

### Debug Logging

Enable debug logs for tests:
```bash
RUST_LOG=debug cargo test -- --nocapture
```

For performance tests:
```bash
RUST_LOG=throttlecrab=debug ./run-perf-test.sh
```
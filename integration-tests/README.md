# ThrottleCrab Integration Tests

This directory contains integration tests for ThrottleCrab server and client.

## Available Tests

### Transport Performance Test

Tests the performance of different transport protocols:

```bash
# Test all transports
./run-transport-test.sh -t all

# Test specific transport with custom parameters
./run-transport-test.sh -t native -T 32 -r 10000

# Available transports: http, grpc, msgpack, native
```

### Client Library Test

Tests the throttlecrab-client library with connection pooling:

```bash
# Run with defaults (10 threads, 1000 requests each)
./run-client-test.sh

# Custom parameters
./run-client-test.sh 20 5000 9090 20
# (threads) (requests) (port) (pool_size)
```

## Test Binary

The integration test binary supports two commands:

```bash
# Run transport performance test
cargo run --release -- perf-test --threads 32 --requests 10000 --transport http

# Run client performance test  
cargo run --release -- client-perf-test --threads 20 --requests 5000 --pool-size 10
```

## Requirements

- ThrottleCrab server must be built in release mode
- Ports must be available (default: 58080 for HTTP, 58070 for gRPC, etc.)
- For best results, run on a machine with multiple cores

## Performance Tips

- Use release builds for accurate benchmarks
- Close other applications to reduce interference
- Run tests multiple times for consistency
- Adjust thread count based on your CPU cores
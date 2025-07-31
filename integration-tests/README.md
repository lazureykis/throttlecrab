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

# Available transports: http, grpc, native
```

## Test Binary

The integration test binary supports the following command:

```bash
# Run transport performance test
cargo run --release -- perf-test --threads 32 --requests 10000 --transport http
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
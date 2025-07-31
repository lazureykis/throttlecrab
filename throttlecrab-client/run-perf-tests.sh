#!/bin/bash

# Run performance tests for throttlecrab-client

set -e

echo "=== ThrottleCrab Client Performance Tests ==="
echo

# Check if we're in the right directory
if [ ! -f "Cargo.toml" ] || [ ! -d "src" ]; then
    echo "Error: This script must be run from the throttlecrab-client directory"
    exit 1
fi

# Build in release mode
echo "Building in release mode..."
cargo build --release --all-targets

echo
echo "Running performance tests..."
echo

# Run the basic performance test
echo "1. Running throughput test..."
cargo test --release test_client_throughput -- --nocapture --test-threads=1

echo
echo "2. Running pool configuration test..."
cargo test --release test_pool_configurations -- --nocapture --test-threads=1

echo
echo "3. Running connection recovery test..."
cargo test --release test_connection_recovery -- --nocapture --test-threads=1

echo
echo "4. Running latency percentiles test..."
cargo test --release test_latency_percentiles -- --nocapture --test-threads=1

echo
echo "5. Running protocol comparison test..."
cargo test --release test_protocol_performance_comparison -- --nocapture --test-threads=1

echo
echo "=== All performance tests completed ==="
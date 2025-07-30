#!/bin/bash

echo "ThrottleCrab Performance Benchmarks"
echo "==================================="
echo

# Build in release mode first
echo "Building in release mode..."
cargo build --release --examples
echo

# Run store comparison benchmark
echo "1. Running Overall Store Performance Comparison..."
echo "================================================="
cargo run --release --example store_comparison
echo
echo

# Run simple access patterns benchmark
echo "2. Running Access Pattern Benchmarks (Simple)..."
echo "==============================================="
cargo run --release --example access_patterns_simple
echo
echo

# Run full access patterns benchmark (without BloomFilter to avoid crash)
echo "3. Running Access Pattern Benchmarks (Full)..."
echo "============================================="
cargo run --release --example access_patterns
echo
echo

echo "All benchmarks completed!"
echo
echo "For detailed results, see: docs/benchmark-results.md"
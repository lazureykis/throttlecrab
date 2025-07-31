#!/bin/bash

# ThrottleCrab Performance Test Runner

set -e

echo "Building projects..."
cd .. && cargo build --release -p throttlecrab-server
cd integration-tests && cargo build --release

# Start server in background
echo -e "\nStarting ThrottleCrab server..."
../target/release/throttlecrab-server \
    --http --http-port 58080 \
    --store adaptive \
    --store-capacity 100000 \
    --buffer-size 100000 \
    --log-level warn &

SERVER_PID=$!
echo "Server started with PID: $SERVER_PID"

# Wait for server to start
sleep 2

# Run performance test
echo -e "\nRunning performance test..."
../target/release/throttlecrab-integration-tests perf-test \
    --threads 20 \
    --requests 5000 \
    --port 58080

# Stop server
echo -e "\nStopping server..."
kill $SERVER_PID
wait $SERVER_PID 2>/dev/null || true

echo "Test completed!"
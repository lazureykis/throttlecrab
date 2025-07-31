#!/bin/bash

# Custom performance test with configurable parameters

set -e

# Default values
THREADS=${1:-20}
REQUESTS=${2:-5000}
PORT=${3:-58080}
STORE=${4:-adaptive}
LOG_LEVEL=${5:-warn}

echo "=== Custom ThrottleCrab Performance Test ==="
echo "Threads: $THREADS"
echo "Requests per thread: $REQUESTS"
echo "Total requests: $((THREADS * REQUESTS))"
echo "Port: $PORT"
echo "Store type: $STORE"
echo "Log level: $LOG_LEVEL"
echo ""

# Build if needed
if [ ! -f "../target/release/throttlecrab-server" ] || [ ! -f "../target/release/throttlecrab-integration-tests" ]; then
    echo "Building projects..."
    cd .. && cargo build --release -p throttlecrab-server
    cd integration-tests && cargo build --release
fi

# Kill any existing server on the port
lsof -ti:$PORT | xargs kill -9 2>/dev/null || true

# Start server
echo "Starting server..."
../target/release/throttlecrab-server \
    --http --http-port $PORT \
    --store $STORE \
    --store-capacity 1000000 \
    --buffer-size 1000000 \
    --log-level $LOG_LEVEL &

SERVER_PID=$!
echo "Server started with PID: $SERVER_PID"

# Wait for server
sleep 2

# Run test
echo -e "\nRunning performance test..."
../target/release/throttlecrab-integration-tests perf-test \
    --threads $THREADS \
    --requests $REQUESTS \
    --port $PORT

# Cleanup
echo -e "\nStopping server..."
kill $SERVER_PID
wait $SERVER_PID 2>/dev/null || true

echo "Test completed!"
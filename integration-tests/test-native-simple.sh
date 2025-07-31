#!/bin/bash

# Kill any existing server
lsof -ti:58072 | xargs kill -9 2>/dev/null || true

# Start server
echo "Starting server..."
../target/release/throttlecrab-server \
    --native --native-port 58072 \
    --store adaptive \
    --store-capacity 1000000 \
    --buffer-size 1000000 \
    --log-level debug &

SERVER_PID=$!
sleep 2

# Compile and run simple test
echo -e "\nCompiling test..."
cargo build --release --bin native-test-simple
    -L ../target/release/deps \
    $(pkg-config --libs openssl) \
    -C opt-level=3

echo -e "\nRunning test..."
../target/release/native-test-simple

# Cleanup
rm -f native_test_simple
kill $SERVER_PID
wait $SERVER_PID 2>/dev/null || true

echo "Test completed!"
#!/bin/bash

# Performance test with configurable transport and parameters

set -e

# Function to show usage
usage() {
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "OPTIONS:"
    echo "  -t, --transport TRANSPORT    Transport to test (http, grpc) [default: http]"
    echo "  -T, --threads NUM           Number of threads [default: 20]"
    echo "  -r, --requests NUM          Requests per thread [default: 5000]"
    echo "  -s, --store TYPE            Store type (adaptive, periodic, probabilistic) [default: adaptive]"
    echo "  -l, --log-level LEVEL       Log level (trace, debug, info, warn, error) [default: warn]"
    echo "  -h, --help                  Show this help message"
    echo ""
    echo "Examples:"
    echo "  # Test HTTP with 32 threads, 100k requests each"
    echo "  $0 -t http -T 32 -r 100000"
    echo ""
    echo "  # Test gRPC with adaptive store"
    echo "  $0 -t grpc -T 16 -r 50000 -s adaptive"
    echo ""
    echo "  # Test all transports sequentially"
    echo "  $0 -t all"
    exit 1
}

# Default values
TRANSPORT="http"
THREADS=20
REQUESTS=5000
STORE="adaptive"
LOG_LEVEL="warn"

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -t|--transport)
            TRANSPORT="$2"
            shift 2
            ;;
        -T|--threads)
            THREADS="$2"
            shift 2
            ;;
        -r|--requests)
            REQUESTS="$2"
            shift 2
            ;;
        -s|--store)
            STORE="$2"
            shift 2
            ;;
        -l|--log-level)
            LOG_LEVEL="$2"
            shift 2
            ;;
        -h|--help)
            usage
            ;;
        *)
            echo "Unknown option: $1"
            usage
            ;;
    esac
done

# Validate transport
case $TRANSPORT in
    http|grpc|redis|all)
        ;;
    *)
        echo "Invalid transport: $TRANSPORT"
        echo "Valid options: http, grpc, redis, all"
        exit 1
        ;;
esac

# Set ports for different transports
HTTP_PORT=58080
GRPC_PORT=58070

# Function to run test for a specific transport
run_transport_test() {
    local transport=$1
    local port=$2

    echo "=== Testing $transport transport ==="
    echo "Threads: $THREADS"
    echo "Requests per thread: $REQUESTS"
    echo "Total requests: $((THREADS * REQUESTS))"
    echo "Port: $port"
    echo "Store type: $STORE"
    echo "Log level: $LOG_LEVEL"
    echo ""

    # Kill any existing server on the port
    lsof -ti:$port | xargs kill -9 2>/dev/null || true

    # Build transport-specific arguments
    case $transport in
        http)
            TRANSPORT_ARGS="--http --http-port $port"
            ;;
        grpc)
            TRANSPORT_ARGS="--grpc --grpc-port $port"
            ;;
        redis)
            TRANSPORT_ARGS="--redis --redis-port $port"
            ;;
    esac

    # Start server
    echo "Starting server with $transport transport..."
    ../target/release/throttlecrab-server \
        $TRANSPORT_ARGS \
        --store $STORE \
        --store-capacity 1000000 \
        --buffer-size 1000000 \
        --log-level $LOG_LEVEL &

    SERVER_PID=$!
    echo "Server started with PID: $SERVER_PID"

    # Wait for server
    sleep 2

    # Check if server is running
    if ! kill -0 $SERVER_PID 2>/dev/null; then
        echo "ERROR: Server failed to start"
        exit 1
    fi

    # Run test
    echo -e "\nRunning performance test..."

    ../target/release/throttlecrab-integration-tests perf-test \
        --threads $THREADS \
        --requests $REQUESTS \
        --port $port \
        --transport $transport

    # Cleanup
    echo -e "\nStopping server..."
    kill $SERVER_PID
    wait $SERVER_PID 2>/dev/null || true

    echo "Test completed for $transport transport!"
    echo ""
}

# Build if needed
echo "Building projects..."
cd .. && cargo build --release -p throttlecrab-server
cd integration-tests && cargo build --release

# Run tests based on transport selection
if [ "$TRANSPORT" = "all" ]; then
    # Test all transports
    for t in http grpc; do
        case $t in
            http) port=$HTTP_PORT ;;
            grpc) port=$GRPC_PORT ;;
            redis) port=$REDIS_PORT ;;
        esac
        run_transport_test $t $port
        sleep 2  # Brief pause between tests
    done
else
    # Test specific transport
    case $TRANSPORT in
        http) port=$HTTP_PORT ;;
        grpc) port=$GRPC_PORT ;;
        redis) port=$REDIS_PORT ;;
    esac
    run_transport_test $TRANSPORT $port
fi

echo "All tests completed!"

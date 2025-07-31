#!/bin/bash

# ThrottleCrab Benchmark Runner Script
# This script provides convenient ways to run various benchmark suites

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Default values
DURATION=30
RPS=10000
OUTPUT_DIR="benchmark-results-$(date +%Y%m%d-%H%M%S)"

# Function to print usage
usage() {
    echo "Usage: $0 [OPTIONS] [SUITE]"
    echo ""
    echo "SUITE can be one of:"
    echo "  transports    - Test all transport protocols"
    echo "  stores        - Compare different store types"
    echo "  workloads     - Test various workload patterns"
    echo "  stress        - Run stress test with increasing load"
    echo "  multi         - Test multiple transports concurrently"
    echo "  all           - Run all benchmark suites (default)"
    echo ""
    echo "OPTIONS:"
    echo "  -d, --duration SECONDS    Test duration (default: 30)"
    echo "  -r, --rps RPS            Target requests per second (default: 10000)"
    echo "  -o, --output DIR         Output directory for results"
    echo "  -j, --json               Save results as JSON"
    echo "  -c, --compare DIR        Compare with previous results"
    echo "  -h, --help               Show this help message"
    echo ""
    echo "Examples:"
    echo "  $0                       # Run all benchmarks with defaults"
    echo "  $0 transports            # Run only transport benchmarks"
    echo "  $0 -d 60 -r 50000 stress # Run stress test for 60s targeting 50k RPS"
    echo "  $0 -j -o results stores  # Run store comparison and save JSON results"
}

# Parse command line arguments
SUITE="all"
JSON_FLAG=""
COMPARE=""

while [[ $# -gt 0 ]]; do
    case $1 in
        -d|--duration)
            DURATION="$2"
            shift 2
            ;;
        -r|--rps)
            RPS="$2"
            shift 2
            ;;
        -o|--output)
            OUTPUT_DIR="$2"
            shift 2
            ;;
        -j|--json)
            JSON_FLAG="--json"
            shift
            ;;
        -c|--compare)
            COMPARE="--compare $2"
            shift 2
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        transports|stores|workloads|stress|multi|all)
            SUITE="$1"
            shift
            ;;
        *)
            echo -e "${RED}Unknown option: $1${NC}"
            usage
            exit 1
            ;;
    esac
done

# Ensure we're in the right directory
cd "$(dirname "$0")"

echo -e "${BLUE}Building release version...${NC}"
cargo build --release --bin throttlecrab

echo -e "${BLUE}Running benchmark suite: ${YELLOW}$SUITE${NC}"
echo -e "${BLUE}Duration: ${YELLOW}${DURATION}s${NC}, Target RPS: ${YELLOW}${RPS}${NC}"
echo -e "${BLUE}Output directory: ${YELLOW}${OUTPUT_DIR}${NC}"
echo ""

# Build the benchmark test
cargo test --release --no-run benchmark

# Find and run the benchmark binary directly  
BENCHMARK_BIN=$(find ../target/release/deps -name 'benchmark-*' -type f -perm +111 | grep -v '\.d$' | head -1)

if [ -z "$BENCHMARK_BIN" ]; then
    echo -e "${RED}Error: Could not find benchmark binary${NC}"
    exit 1
fi

# Run the benchmark
"$BENCHMARK_BIN" \
    --suite "$SUITE" \
    --duration "$DURATION" \
    --target-rps "$RPS" \
    --output-dir "$OUTPUT_DIR" \
    $JSON_FLAG \
    $COMPARE

echo ""
echo -e "${GREEN}Benchmark completed!${NC}"
echo -e "${BLUE}Results saved to: ${YELLOW}${OUTPUT_DIR}${NC}"

# If JSON results were generated, show a summary
if [[ -n "$JSON_FLAG" ]]; then
    echo ""
    echo -e "${BLUE}JSON results available at: ${YELLOW}${OUTPUT_DIR}/results.json${NC}"
fi
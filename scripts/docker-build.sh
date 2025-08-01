#!/bin/bash
set -euo pipefail

# Script to build and push Docker images for throttlecrab
# Usage: ./scripts/docker-build.sh [--push] [--platform PLATFORM]
#
# Options:
#   --push              Push images to Docker Hub
#   --platform PLATFORM Override platform for local builds (e.g., linux/amd64)

# Configuration
DOCKER_REGISTRY="docker.io"
DOCKER_USERNAME="${DOCKER_USERNAME:-lazureykis}"
IMAGE_NAME="throttlecrab"
PLATFORMS="linux/amd64,linux/arm64"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Parse command line arguments
PUSH_IMAGES=false
OVERRIDE_PLATFORM=""

while [[ $# -gt 0 ]]; do
    case $1 in
        --push)
            PUSH_IMAGES=true
            shift
            ;;
        --platform)
            OVERRIDE_PLATFORM="$2"
            shift 2
            ;;
        -h|--help)
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  --push              Build and push images to Docker Hub"
            echo "  --platform PLATFORM Override platform for local builds (e.g., linux/amd64)"
            echo "  -h, --help          Show this help message"
            echo ""
            echo "Examples:"
            echo "  $0                  # Build for current platform locally"
            echo "  $0 --push           # Build multi-platform and push to Docker Hub"
            echo "  $0 --platform linux/amd64  # Build for specific platform locally"
            exit 0
            ;;
        *)
            echo -e "${RED}Unknown option: $1${NC}"
            echo "Usage: $0 [--push] [--platform PLATFORM]"
            echo "Run '$0 --help' for more information"
            exit 1
            ;;
    esac
done

# Ensure we're in the project root
if [[ ! -f "Cargo.toml" ]]; then
    echo -e "${RED}Error: This script must be run from the project root directory${NC}"
    exit 1
fi

# Ensure throttlecrab's Cargo.toml exists
if [[ ! -f "throttlecrab/Cargo.toml" ]]; then
    echo -e "${RED}Error: throttlecrab/Cargo.toml not found. Are you in the project root?${NC}"
    exit 1
fi

# Get version from throttlecrab's Cargo.toml
VERSION=$(grep '^version' throttlecrab/Cargo.toml | head -1 | sed 's/version = //;s/"//g;s/ //g')
if [[ -z "$VERSION" ]]; then
    echo -e "${RED}Error: Could not extract version from throttlecrab/Cargo.toml${NC}"
    exit 1
fi

# Validate version format (should be x.y.z or x.y.z-suffix)
if ! [[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.-]+)?$ ]]; then
    echo -e "${RED}Error: Invalid version format: $VERSION${NC}"
    exit 1
fi

echo -e "${GREEN}Building throttlecrab Docker image v${VERSION}${NC}"
echo "Platforms: ${PLATFORMS}"

# Ensure Dockerfile exists
if [[ ! -f "Dockerfile" ]]; then
    echo -e "${RED}Error: Dockerfile not found in project root${NC}"
    exit 1
fi

# Check if Docker is installed and running
if ! docker info > /dev/null 2>&1; then
    echo -e "${RED}Error: Docker is not running or not installed${NC}"
    exit 1
fi

# Setup Docker buildx builder
BUILDER_NAME="throttlecrab-builder"

# Check if builder exists by trying to inspect it
if docker buildx inspect ${BUILDER_NAME} >/dev/null 2>&1; then
    echo -e "${YELLOW}Using existing ${BUILDER_NAME}${NC}"
    docker buildx use ${BUILDER_NAME}
else
    echo -e "${YELLOW}Creating Docker buildx builder...${NC}"
    if ! docker buildx create --name ${BUILDER_NAME} --use; then
        echo -e "${RED}Error: Failed to create buildx builder${NC}"
        exit 1
    fi
fi

# Ensure builder is bootstrapped
docker buildx inspect ${BUILDER_NAME} --bootstrap >/dev/null 2>&1

# Build tags
TAGS=(
    "${DOCKER_REGISTRY}/${DOCKER_USERNAME}/${IMAGE_NAME}:${VERSION}"
    "${DOCKER_REGISTRY}/${DOCKER_USERNAME}/${IMAGE_NAME}:latest"
)

# Construct tag arguments
TAG_ARGS=""
for tag in "${TAGS[@]}"; do
    TAG_ARGS="${TAG_ARGS} --tag ${tag}"
done

# Prepare for build
if [[ "${PUSH_IMAGES}" == "true" ]]; then
    echo -e "${YELLOW}Building and pushing images to Docker Hub...${NC}"
    
    # Verify Docker Hub authentication
    echo -e "${YELLOW}Verifying Docker Hub authentication...${NC}"
    # Try to get auth info from docker config
    AUTH_CONFIG="${HOME}/.docker/config.json"
    if [[ -f "$AUTH_CONFIG" ]] && grep -q "\"${DOCKER_REGISTRY}\"" "$AUTH_CONFIG" 2>/dev/null; then
        echo -e "${GREEN}Docker Hub credentials found${NC}"
    else
        echo -e "${YELLOW}Docker Hub credentials not found. Please authenticate:${NC}"
        docker login ${DOCKER_REGISTRY}
    fi
else
    echo -e "${YELLOW}Building images locally (use --push to push to registry)...${NC}"
    
    # Note: --load only works with single platform, so we'll build for current platform only
    if [[ -n "$OVERRIDE_PLATFORM" ]]; then
        CURRENT_PLATFORM="$OVERRIDE_PLATFORM"
        echo -e "${YELLOW}Using override platform: ${CURRENT_PLATFORM}${NC}"
    else
        # Use a more reliable method to detect platform
        CURRENT_PLATFORM="linux/$(uname -m | sed 's/x86_64/amd64/;s/aarch64/arm64/')"
    fi
fi

# Execute build directly without eval
if [[ "${PUSH_IMAGES}" == "true" ]]; then
    echo -e "${GREEN}Building multi-platform images and pushing...${NC}"
    docker buildx build \
        --platform "${PLATFORMS}" \
        ${TAG_ARGS} \
        --file Dockerfile \
        --push \
        .
else
    echo -e "${GREEN}Building for current platform: ${CURRENT_PLATFORM}${NC}"
    docker buildx build \
        --platform "${CURRENT_PLATFORM}" \
        ${TAG_ARGS} \
        --file Dockerfile \
        --load \
        .
fi

if [[ $? -eq 0 ]]; then
    echo -e "${GREEN}✓ Build successful!${NC}"
    
    if [[ "${PUSH_IMAGES}" == "true" ]]; then
        echo -e "${GREEN}✓ Images pushed to Docker Hub:${NC}"
        for tag in "${TAGS[@]}"; do
            echo "  - ${tag}"
        done
    else
        echo -e "${YELLOW}Images built locally. To push to Docker Hub, run:${NC}"
        echo "  ./scripts/docker-build.sh --push"
    fi
else
    echo -e "${RED}✗ Build failed!${NC}"
    exit 1
fi
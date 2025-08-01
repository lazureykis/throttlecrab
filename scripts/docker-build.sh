#!/bin/bash
set -euo pipefail

# Script to build and push Docker images for throttlecrab
# Usage: ./scripts/docker-build-push.sh [--push]

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

# Check if --push flag is provided
PUSH_IMAGES=false
if [[ "${1:-}" == "--push" ]]; then
    PUSH_IMAGES=true
fi

# Ensure we're in the project root
if [[ ! -f "Cargo.toml" ]]; then
    echo -e "${RED}Error: This script must be run from the project root directory${NC}"
    exit 1
fi

# Get version from throttlecrab's Cargo.toml
VERSION=$(grep '^version' throttlecrab/Cargo.toml | head -1 | sed 's/version = //;s/"//g;s/ //g')
if [[ -z "$VERSION" ]]; then
    echo -e "${RED}Error: Could not extract version from throttlecrab/Cargo.toml${NC}"
    exit 1
fi

# Validate version format (should be x.y.z)
if ! [[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
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
    # Try to inspect a known public image with our credentials
    if ! docker buildx imagetools inspect ${DOCKER_REGISTRY}/${DOCKER_USERNAME}/${IMAGE_NAME}:latest >/dev/null 2>&1; then
        # If that fails, we might not have pushed yet or not logged in
        echo -e "${YELLOW}Authentication check inconclusive. Attempting login...${NC}"
        docker login ${DOCKER_REGISTRY}
    fi
else
    echo -e "${YELLOW}Building images locally (use --push to push to registry)...${NC}"
    
    # Note: --load only works with single platform, so we'll build for current platform only
    # Use a more reliable method to detect platform
    CURRENT_PLATFORM="linux/$(uname -m | sed 's/x86_64/amd64/;s/aarch64/arm64/')"
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
        echo "  ./scripts/docker-build-push.sh --push"
    fi
else
    echo -e "${RED}✗ Build failed!${NC}"
    exit 1
fi
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

# Get version from Cargo.toml
VERSION=$(grep '^version' throttlecrab/Cargo.toml | head -1 | cut -d '"' -f 2)

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
if docker buildx ls | grep -q "throttlecrab-builder"; then
    echo -e "${YELLOW}Using existing throttlecrab-builder${NC}"
    docker buildx use throttlecrab-builder
else
    echo -e "${YELLOW}Creating Docker buildx builder...${NC}"
    if ! docker buildx create --name throttlecrab-builder --use; then
        echo -e "${RED}Error: Failed to create buildx builder${NC}"
        exit 1
    fi
    docker buildx inspect --bootstrap
fi

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

# Build command
BUILD_CMD="docker buildx build \
    --platform ${PLATFORMS} \
    ${TAG_ARGS} \
    --file Dockerfile"

if [[ "${PUSH_IMAGES}" == "true" ]]; then
    echo -e "${YELLOW}Building and pushing images to Docker Hub...${NC}"
    
    # Try to ensure we're logged in to Docker Hub
    echo -e "${YELLOW}Checking Docker Hub login...${NC}"
    if ! docker pull hello-world >/dev/null 2>&1; then
        echo -e "${YELLOW}Please log in to Docker Hub:${NC}"
        docker login ${DOCKER_REGISTRY}
    fi
    
    BUILD_CMD="${BUILD_CMD} --push"
else
    echo -e "${YELLOW}Building images locally (use --push to push to registry)...${NC}"
    BUILD_CMD="${BUILD_CMD} --load"
    
    # Note: --load only works with single platform, so we'll build for current platform only
    CURRENT_PLATFORM=$(docker version --format '{{.Server.Os}}/{{.Server.Arch}}')
    BUILD_CMD="docker buildx build \
        --platform ${CURRENT_PLATFORM} \
        ${TAG_ARGS} \
        --file Dockerfile \
        --load"
fi

# Add build context
BUILD_CMD="${BUILD_CMD} ."

# Execute build
echo -e "${GREEN}Executing: ${BUILD_CMD}${NC}"
eval ${BUILD_CMD}

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
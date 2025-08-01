#!/bin/bash
set -euo pipefail

# Quick Docker build and push script for throttlecrab
# Optimized for speed - builds only for current platform

# Configuration
DOCKER_USERNAME="${DOCKER_USERNAME:-lazureykis}"
IMAGE_NAME="throttlecrab"

# Ensure we're in the project root
if [[ ! -f "Cargo.toml" ]]; then
    echo "❌ Error: This script must be run from the project root directory"
    exit 1
fi

# Ensure Dockerfile exists
if [[ ! -f "Dockerfile" ]]; then
    echo "❌ Error: Dockerfile not found in project root"
    exit 1
fi

# Check if Docker is running
if ! docker info > /dev/null 2>&1; then
    echo "❌ Error: Docker is not running or not installed"
    exit 1
fi

# Get version from Cargo.toml
VERSION=$(grep '^version' throttlecrab/Cargo.toml | head -1 | cut -d '"' -f 2)

echo "🚀 Quick building throttlecrab v${VERSION}"

# Build for current platform only (much faster)
docker build -t ${DOCKER_USERNAME}/${IMAGE_NAME}:${VERSION} -t ${DOCKER_USERNAME}/${IMAGE_NAME}:latest .

# Push if requested
if [[ "${1:-}" == "--push" ]]; then
    echo "📤 Pushing to Docker Hub..."
    # Check if logged in
    echo "🔐 Checking Docker Hub login..."
    if ! docker pull hello-world >/dev/null 2>&1; then
        echo "🔐 Please log in to Docker Hub:"
        docker login
    fi
    docker push ${DOCKER_USERNAME}/${IMAGE_NAME}:${VERSION}
    docker push ${DOCKER_USERNAME}/${IMAGE_NAME}:latest
    echo "✅ Done!"
else
    echo "✅ Built locally. Run with --push to upload to Docker Hub"
fi
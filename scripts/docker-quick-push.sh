#!/bin/bash
set -euo pipefail

# Quick Docker build and push script for throttlecrab
# Optimized for speed - builds only for current platform

# Configuration
DOCKER_USERNAME="lazureykis"
IMAGE_NAME="throttlecrab"

# Get version from Cargo.toml
VERSION=$(grep '^version' throttlecrab/Cargo.toml | head -1 | cut -d '"' -f 2)

echo "🚀 Quick building throttlecrab v${VERSION}"

# Build for current platform only (much faster)
docker build -t ${DOCKER_USERNAME}/${IMAGE_NAME}:${VERSION} -t ${DOCKER_USERNAME}/${IMAGE_NAME}:latest .

# Push if requested
if [[ "${1:-}" == "--push" ]]; then
    echo "📤 Pushing to Docker Hub..."
    docker push ${DOCKER_USERNAME}/${IMAGE_NAME}:${VERSION}
    docker push ${DOCKER_USERNAME}/${IMAGE_NAME}:latest
    echo "✅ Done!"
else
    echo "✅ Built locally. Run with --push to upload to Docker Hub"
fi
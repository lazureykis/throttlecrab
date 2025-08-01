# ThrottleCrab Scripts

This directory contains utility scripts for building and managing ThrottleCrab.

## Docker Build Scripts

### docker-quick-push.sh

Fast Docker build script optimized for local development. Builds only for your current platform.

```bash
# Build locally
./scripts/docker-quick-push.sh

# Build and push to Docker Hub
./scripts/docker-quick-push.sh --push
```

### docker-build-push.sh

Full-featured Docker build script with multi-platform support (linux/amd64, linux/arm64).

```bash
# Build locally (current platform only)
./scripts/docker-build-push.sh

# Build and push multi-platform images to Docker Hub
./scripts/docker-build-push.sh --push
```

## Usage Tips

- Use `docker-quick-push.sh` for rapid iteration during development
- Use `docker-build-push.sh --push` for official releases with multi-platform support
- Both scripts automatically detect the version from `Cargo.toml`
- Images are tagged as both `version` and `latest`
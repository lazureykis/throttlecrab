# Docker Hub Automated Build Setup

This guide explains how to set up automated builds for throttlecrab on Docker Hub.

## Repository Structure

The repository is configured for Docker Hub automated builds:

```
throttlecrab/
├── Dockerfile          # Main Dockerfile (for local builds)
├── docker/
│   └── Dockerfile      # Docker Hub will use this
├── hooks/
│   └── build           # Custom build hook for Docker Hub
├── docker-compose.yml  # Example compose file
└── .dockerignore       # Excludes unnecessary files
```

## Docker Hub Configuration

### 1. Link GitHub Repository

1. Log in to [Docker Hub](https://hub.docker.com)
2. Go to your repository: `lazureykis/throttlecrab`
3. Click on "Builds" tab
4. Click "Configure Automated Builds"
5. Link your GitHub account if not already linked
6. Select the `lazureykis/throttlecrab` GitHub repository

### 2. Configure Build Rules

Add the following build rules:

#### Rule 1: Latest from main
- **Source Type**: Branch
- **Source**: main
- **Docker Tag**: latest
- **Dockerfile location**: /docker/Dockerfile
- **Build Context**: /

#### Rule 2: Version tags
- **Source Type**: Tag
- **Source**: /^v([0-9.]+)$/
- **Docker Tag**: {\1}
- **Dockerfile location**: /docker/Dockerfile
- **Build Context**: /

#### Rule 3: Major.Minor tags
- **Source Type**: Tag
- **Source**: /^v([0-9]+)\.([0-9]+)\.([0-9]+)$/
- **Docker Tag**: {\1}.{\2}
- **Dockerfile location**: /docker/Dockerfile
- **Build Context**: /

#### Rule 4: Major version tags
- **Source Type**: Tag
- **Source**: /^v([0-9]+)\.([0-9]+)\.([0-9]+)$/
- **Docker Tag**: {\1}
- **Dockerfile location**: /docker/Dockerfile
- **Build Context**: /

### 3. Build Settings

- **Build Caching**: Enabled
- **Repository Links**: Not needed
- **Build Environment Variables**: None required

### 4. Trigger Builds

Automated builds will trigger on:
- Push to `main` branch → Updates `latest` tag
- New version tag (e.g., `v0.2.4`) → Creates version tags

## Testing Locally

Before pushing, test the Docker build locally:

```bash
# Build using the same context as Docker Hub
docker build -f docker/Dockerfile -t throttlecrab:test .

# Run the test image
docker run --rm -p 8080:8080 throttlecrab:test
```

## Build Status

You can monitor build status at:
https://hub.docker.com/r/lazureykis/throttlecrab/builds

## Troubleshooting

### Build Failures

1. Check the build logs on Docker Hub
2. Ensure all dependencies are available during build
3. Verify the Dockerfile path is correct
4. Test the build locally first

### Common Issues

- **Missing protobuf compiler**: Already handled in Dockerfile
- **Rust version**: Using latest stable in Dockerfile
- **Build timeout**: Our multi-stage build is optimized for caching

## Manual Build Trigger

You can manually trigger a build:
1. Go to Builds tab on Docker Hub
2. Click "Trigger Build"
3. Select the branch or tag

## Multi-Architecture Builds

Docker Hub automated builds currently don't support multi-arch builds directly.
For multi-arch support, you would need to:
1. Use GitHub Actions (our previous setup)
2. Or build locally and push manually with buildx

## Security

- Docker Hub builds run in isolated environments
- No secrets are needed in the build process
- The image runs as non-root user (uid 1000)
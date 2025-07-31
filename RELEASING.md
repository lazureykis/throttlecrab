# Release Process

This document describes the process for releasing new versions of the throttlecrab crates.

## Overview

The project consists of two crates that need to be released in order:
1. `throttlecrab` - The core rate limiting library
2. `throttlecrab-server` - The server implementation that depends on `throttlecrab`

## Pre-Release Checklist

Before starting the release process, ensure:

- [ ] All CI checks are passing on `main` branch
- [ ] All planned features/fixes for the release are merged
- [ ] Dependencies are up to date (`cargo outdated`)
- [ ] No security vulnerabilities (`cargo audit`)
- [ ] Documentation is updated if needed
- [ ] CHANGELOG.md is updated (if maintained)

## Release Steps

### 1. Create Release Branch

```bash
git checkout main
git pull origin main
git checkout -b release/vX.Y.Z
```

### 2. Update Dependencies

Check for outdated dependencies:
```bash
cargo outdated
```

Update dependencies if needed (use flexible version constraints for libraries):
- Update workspace dependencies in root `Cargo.toml`
- Update crate-specific dependencies

### 3. Bump Version Numbers

Update version in the following files:
- `throttlecrab/Cargo.toml`
- `throttlecrab-server/Cargo.toml` 
- Update the `throttlecrab` dependency version in `throttlecrab-server/Cargo.toml`

### 4. Verify Everything Builds

```bash
# Check all crates compile
cargo check --all

# Run all tests
cargo test --all

# Run linter
cargo clippy --all-targets --all-features -- -D warnings

# Format code
cargo fmt --all

# Run benchmarks (optional)
cd throttlecrab-server
./run-criterion-benchmarks.sh
```

### 5. Commit Version Bump

```bash
git add -A
git commit -m "chore: bump version to X.Y.Z

- Update throttlecrab from A.B.C to X.Y.Z
- Update throttlecrab-server from A.B.C to X.Y.Z
- Update dependencies (if any)"
```

### 6. Push and Create PR

```bash
git push origin release/vX.Y.Z
```

Create a pull request from `release/vX.Y.Z` to `main` with:
- Title: "Release vX.Y.Z"
- Description: List of changes included in the release

### 7. Merge PR

After PR approval and CI passes, merge to main.

### 8. Tag the Release

```bash
git checkout main
git pull origin main
git tag -a vX.Y.Z -m "Release version X.Y.Z"
git push origin vX.Y.Z
```

### 9. Publish to crates.io

**IMPORTANT**: Publish crates in order due to dependencies.

First, do a dry run:
```bash
cd throttlecrab
cargo publish --dry-run

cd ../throttlecrab-server
cargo publish --dry-run
```

If dry run succeeds, publish for real:

```bash
# Publish core library first
cd throttlecrab
cargo publish

# Wait for throttlecrab to be available on crates.io (usually ~1 minute)
# You can check at https://crates.io/crates/throttlecrab

# Then publish the server
cd ../throttlecrab-server
cargo publish
```

### 10. Create GitHub Release

1. Go to https://github.com/lazureykis/throttlecrab/releases
2. Click "Create a new release"
3. Choose the tag `vX.Y.Z`
4. Title: "vX.Y.Z"
5. Description: Include the key changes and improvements
6. Publish release

## Post-Release

- [ ] Verify crates are available on crates.io
- [ ] Update any example repositories or documentation
- [ ] Announce the release if needed

## Version Numbering

Follow [Semantic Versioning](https://semver.org/):
- MAJOR version (X.0.0) - Incompatible API changes
- MINOR version (0.Y.0) - Backwards-compatible functionality additions
- PATCH version (0.0.Z) - Backwards-compatible bug fixes

## Troubleshooting

### Crate Publishing Fails

If `cargo publish` fails:
- Check you're logged in: `cargo login`
- Ensure you have publishing rights for the crate
- Verify all dependencies are published and available
- Check for any uncommitted changes

### Version Conflicts

If you get version conflicts:
- Ensure the `throttlecrab` version in `throttlecrab-server/Cargo.toml` matches the newly published version
- Run `cargo update` to update the lock file

### CI Failures

If CI fails after tagging:
- Fix the issues on a new branch
- Cherry-pick fixes to main
- Delete the tag: `git tag -d vX.Y.Z && git push origin :refs/tags/vX.Y.Z`
- Start the release process again
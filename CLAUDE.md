## Project Overview

This is a Rust project called "throttlecrab" using Rust edition 2024.

## Common Development Commands

### Build
```bash
cargo build
```

### Run
```bash
cargo run
```

### Test
```bash
cargo test
```

### Run a specific test
```bash
cargo test test_name
```

### Lint
```bash
cargo clippy --all-targets --all-features -- -D warnings
```

### Format
```bash
cargo fmt --all
```

### Check
```bash
cargo check
```

## Project Structure

The project follows the standard Rust project layout:
- `Cargo.toml` - Project manifest and dependencies
- `src/main.rs` - Application entry point

## Development Workflow

1. When adding new functionality, place it in appropriate modules under `src/`

2. Use `cargo check` for quick compilation checks during development

## Git Workflow

- Always create a new branch to implement anything
- After finishing work, push the branch and create a PR with a proper description
- After pushing more commits to an existing PR, update the PR summary accordingly
- Never commit into main branch

## Additional Git Workflow Guidance

- Always create a new branch from fresh main when working on a new feature
- After work, push the code and create a PR

## Pre-Commit and Pre-Push Checks

- Before creating a PR or pushing into an existing PR, ALWAYS check:
  - Run cargo test --all
  - Run cargo clippy --all-targets --all-features -- -D warnings
  - Run cargo fmt --all

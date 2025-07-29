# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

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
cargo clippy
```

### Format
```bash
cargo fmt
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

1. Before committing changes, always run:
   - `cargo fmt` to format code
   - `cargo clippy` to check for linting issues
   - `cargo test` to run tests

2. When adding new functionality, place it in appropriate modules under `src/`

3. Use `cargo check` for quick compilation checks during development

## Git Workflow

- Always create a new branch to implement anything
- After finishing work, push the branch and create a PR with a proper description
- After pushing more commits to an existing PR, update the PR summary accordingly

- Always run 'cargo fmt --all' and 'cargo clippy --all-targets --all-features -- -D warnings' before pushing code to github

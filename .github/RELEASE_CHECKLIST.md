# Release Checklist for vX.Y.Z

## Pre-Release Verification
- [ ] All CI checks passing on main
- [ ] No security vulnerabilities: `cargo audit`
- [ ] Dependencies checked: `cargo outdated`
- [ ] All tests passing: `cargo test --all`
- [ ] Clippy clean: `cargo clippy --all-targets --all-features -- -D warnings`
- [ ] Code formatted: `cargo fmt --all -- --check`

## Version Updates
- [ ] `throttlecrab/Cargo.toml` version updated to X.Y.Z
- [ ] `throttlecrab-server/Cargo.toml` version updated to X.Y.Z
- [ ] `throttlecrab` dependency version updated in `throttlecrab-server/Cargo.toml`
- [ ] Version numbers are consistent across all crates

## Testing
- [ ] Integration tests pass: `cd integration-tests && cargo test`
- [ ] Benchmarks run successfully: `cd throttlecrab-server && ./run-criterion-benchmarks.sh`
- [ ] Example clients work with new version

## Documentation
- [ ] README.md is up to date
- [ ] API documentation builds: `cargo doc --all --no-deps`
- [ ] Any breaking changes are documented
- [ ] Migration guide provided (if needed)

## Release Process
- [ ] Release branch created: `release/vX.Y.Z`
- [ ] PR created and reviewed
- [ ] PR merged to main
- [ ] Tag created: `git tag -a vX.Y.Z -m "Release version X.Y.Z"`
- [ ] Tag pushed: `git push origin vX.Y.Z`

## Publishing
- [ ] Dry run successful: `cargo publish --dry-run` (both crates)
- [ ] `throttlecrab` published to crates.io
- [ ] Waited for `throttlecrab` to be available on crates.io
- [ ] `throttlecrab-server` published to crates.io
- [ ] Both crates verified on crates.io

## Post-Release
- [ ] GitHub release created with release notes
- [ ] Any dependent projects updated
- [ ] Release announced (if applicable)

## Rollback Plan
If issues are discovered:
- [ ] Document the issue
- [ ] Yank affected versions if necessary: `cargo yank --vers X.Y.Z`
- [ ] Create patch release with fix
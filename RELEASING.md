# Release Process

Releases are fully automated by the `Release` workflow
(`.github/workflows/release.yml`). Do **not** bump versions, tag, or publish by
hand — the workflow does all of it, and a manual bump will collide with the
version it calculates.

## How to release

1. Make sure `main` is green and everything you want in the release is merged.
2. Go to [Actions → Release](https://github.com/lazureykis/throttlecrab/actions/workflows/release.yml)
   and click **Run workflow** (or `gh workflow run release.yml --ref main`).
3. Pick the inputs:
   - `version_bump` — `patch` (default), `minor`, or `major`
   - `run_tests` — leave `true`; runs `cargo test --all`, clippy, and fmt before publishing
   - `generate_ai_changelog` — generates the release notes with Claude; falls
     back to `.github/scripts/generate-simple-changelog.sh` when `false`

## What the workflow does

In order, from the current version in `throttlecrab/Cargo.toml`:

1. Calculates the new version and fails if its tag already exists
2. Updates the version in `throttlecrab/Cargo.toml`, `throttlecrab-server/Cargo.toml`,
   and the `throttlecrab` dependency in `throttlecrab-server`, then `cargo update --workspace`
3. Runs tests, clippy, and `cargo fmt --check` (when `run_tests` is true)
4. Commits `chore: bump version to X.Y.Z` and pushes it with tag `vX.Y.Z` to `main`
5. Publishes `throttlecrab` to crates.io, waits for propagation, then publishes `throttlecrab-server`
6. Creates the GitHub release with the generated changelog
7. Builds `linux/amd64` + `linux/arm64` binaries, pushes multi-arch images to
   `ghcr.io/lazureykis/throttlecrab`, and rolls out `deployment/throttlecrab` in
   the `production` namespace

Step 7 touches production. Only run the workflow when a production rollout is
acceptable.

## Dependency updates

Dependency bumps are ordinary PRs to `main` — not part of the release. Update
`Cargo.lock` with `cargo update`, or the workspace dependency versions in the
root `Cargo.toml` for semver-incompatible upgrades, and let CI verify. The next
release picks them up automatically.

## Required secrets

| Secret                    | Used for                                  |
| ------------------------- | ----------------------------------------- |
| `PUBLISH_TOKEN`           | Pushing the version commit and tag to `main` |
| `CARGO_REGISTRY_TOKEN`    | `cargo publish` for both crates           |
| `CLAUDE_CODE_OAUTH_TOKEN` | AI changelog generation                   |
| `KUBECONFIG`              | Production k3s rollout                    |

## If a release fails

The workflow's cleanup step deletes the tag it created, but anything already
completed stays done. Check, in order:

- Was the version commit pushed to `main`? Revert it if the release didn't finish.
- Did either crate reach crates.io? Publishes are **irreversible** — you cannot
  republish the same version. If `throttlecrab` published but `throttlecrab-server`
  failed, fix the problem and publish the server manually from that tag rather
  than re-running the whole workflow.
- Is there a partial GitHub release to clean up?

Then re-run the workflow, bumping to the next version if a crate was already
published.

## Local build prerequisites

Building `throttlecrab-server` requires `protoc` (Debian/Ubuntu:
`apt-get install protobuf-compiler`, macOS: `brew install protobuf`). Cargo
caches build-script failures, so if `protoc` was missing on an earlier build,
`touch throttlecrab-server/build.rs` to force the build script to re-run.

## Version numbering

[Semantic Versioning](https://semver.org/): MAJOR for incompatible API changes,
MINOR for backwards-compatible additions, PATCH for backwards-compatible fixes.

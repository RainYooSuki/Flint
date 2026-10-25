# Flint release workflow

The active workflow is `flint-release.yml`.

## Location

GitHub only discovers workflows under the repository-root `.github/workflows/` directory. When this project is published, `Flint/` should be the repository root; keep this file at `.github/workflows/flint-release.yml`.

## Triggers

- Every push to `main` or `dev` starts the release workflow, including pushes that contain only `VERSION`, Markdown/docs, workflow files, release scripts, or ignore metadata.
- Manual `workflow_dispatch` from a branch.

No changed-file exemptions apply: every push is considered, including docs/tooling-only and `VERSION`-only pushes.

The prepare source guard runs for every automatic push or manual branch-triggered run and rejects an empty commit or a commit whose only changed file is `VERSION`. Docs/tooling/workflow-only commits and `VERSION` plus other-file commits may proceed. A `VERSION`-only push starts the workflow and then fails in prepare.

## Maintainer expectations

The workflow computes `YYYY.M.N-sha7`, builds Windows x64, Linux x64, macOS x64, and macOS arm64, publishes raw files plus matching ZIPs and `sha.txt`, and verifies the exact release asset set. The release build uses `tauri build --no-bundle`; the workflow packages the raw executables into ZIPs and does not publish the platform installer bundles from an ordinary bundled build. Only after publication and verification does it commit the released value to `VERSION` as `github-actions[bot]` and push to the triggering branch. That bot push uses `GITHUB_TOKEN`, so it does not start another workflow run; a manual `VERSION`-only push does start the workflow and then fails the prepare source guard.

At post-release recording, if branch `VERSION` already equals the released version, the workflow exits without another commit. Otherwise it requires branch `VERSION` to equal the source-commit `VERSION`, writes the released value, and refuses any unexpected branch `VERSION` state rather than overwriting it.

## Parallel workspace test verification

The `verify` job has no dependency on `prepare`, so release metadata/static checks and the full Rust workspace tests start concurrently on separate Ubuntu runners. It installs the reviewed Linux Tauri prerequisites, Node 22/frontend dependencies, builds `dist` for the Tauri context, then runs:

```bash
cargo test --workspace --all-targets --locked --manifest-path src-tauri/Cargo.toml
```

The four-platform `build` matrix declares `needs: [prepare, verify]`, so neither a static/architecture failure nor a Rust test failure can publish a release. Windows/macOS compilation remains platform-specific; the workspace test suite executes on Linux.

## Phase 5 prepare gate

The `prepare` job installs frontend dependencies and runs:

```bash
node --test scripts/test-architecture-checker.mjs scripts/test-frontend-contracts.mjs
node scripts/check-architecture.mjs --phase 5
```

The strict gate allows `@tauri-apps/*` only in the exact five reviewed frontend adapters: `src/core/api/tauriInvoke.ts`, `src/core/events/tauriEvents.ts`, `src/core/platform/tauriDialogs.ts`, `src/core/platform/tauriOpener.ts`, and `src/core/platform/tauriWindow.ts`. It requires `HostServices` injection and registry overlays, rejects production shim/alias and migration-policy tokens, requires one shared actual-registry `ProviderRuntimeState`, and enforces the exact nineteen-package Cargo/source/dependency/typed-port rules, both provider compositions, the sole `ModSource`/`LoaderKind` bindings, and the 30-command/28-feature-command gate map. There are no temporary production exceptions. GitHub Actions is the final verification authority; local checks are not evidence that hosted CI is green.

Use `main` or `dev` for normal releases. Review [`../../RELEASE.md`](../../RELEASE.md) before changing tags, release permissions, or branch protection. Run the developer checks in that guide before pushing release-tooling changes; use the hosted workflow result as completion evidence.

# Flint Release Guide

This guide describes how Flint releases are built, published, and recorded.

## Release version

Flint release versions use:

```text
YYYY.M.N-<sha7>
```

- `YYYY.M` is the UTC year and month of the release source commit's committer timestamp.
- `N` is the one-based release ordinal for that UTC month. It resets to `1` in a new month.
- `sha7` is the first seven lowercase characters of the full source commit SHA.
- If SHA7 is all digits and starts with `0`, `g<sha7>` is used as the pre-release identifier so the version remains valid SemVer.
- The Git tag is `flint-v<version>`.

Examples: `2026.10.1-abc1234`, `2026.10.2-def5678`, `2026.11.1-987abcd`.

All Cargo workspace packages share this generated release version. `src-tauri/Cargo.toml` keeps the host `[package] version` and the `[workspace.package] version` identical; each non-root member crate declares `version.workspace = true`. Release tooling updates the `Cargo.lock` entry for every workspace-local package. No Cargo workspace package may use an independent `0.x` release version.

### Monthly ordinal calculation

The prepare job checks out full history, fetches all remote tags and branch tips, and reads the `flint-v*` tag inventory. Tags are resolved through `refs/tags/<tag>^{commit}`, so annotated and lightweight tags both map to the correct source commit.

For the source commit's UTC month, Flint counts distinct release-source commit SHAs represented by valid tags. Existing ordinals must form the contiguous sequence `1..count`; otherwise the run stops. Ordinary commits and VERSION-only ledger commits do not count. Legacy, malformed, source-mismatched, and ledger-source tags are ignored with warnings. A well-formed tag whose source commit cannot be resolved fails the run rather than silently reducing the monthly count.

If exactly one valid monthly release tag already points at the workflow `HEAD`, that tag's version and `N` are reused. Multiple valid tags at the same HEAD fail clearly. A new source commit gets the next monthly ordinal.

The release job fetches tags again and recomputes the version immediately before publishing. If the monthly inventory changed after the build and the prepared version is stale, publishing stops.

`VERSION` is a release ledger; generated package/Tauri/Cargo workspace metadata may lag it until the next build applies a computed version. Prepare runs the version-only `validate-release.mjs --allow-divergent-metadata` mode, which requires valid versions and internally consistent generated metadata (including `[workspace.package]` and every local `Cargo.lock` package entry), accepts a generated year/month/N at or behind the ledger, and does not use SHA prereleases for ordering. Generated metadata ahead of the ledger still fails. The mode cannot be combined with `--version`, `--require-exact`, or asset validation; those checks remain strict after `apply-version`.

## Triggers

The workflow is `.github/workflows/flint-release.yml`.

- Every push to `main` or `dev` starts the release workflow, including pushes that contain only `VERSION`, Markdown/docs, workflow files, release scripts, or ignore metadata.
- Manual `workflow_dispatch` from a selected branch.
- No changed-file exemptions apply: every push is considered, including docs/tooling-only and `VERSION`-only pushes.
- The prepare source guard runs for every automatic push or manual branch-triggered run and rejects an empty commit or a commit whose only changed file is `VERSION`. Docs/tooling/workflow-only commits and `VERSION` plus other-file commits may proceed. A `VERSION`-only push starts the workflow and then fails in prepare.
- Manual dispatch uses the exact workflow `GITHUB_SHA`.

A repository-wide concurrency group ensures one release workflow runs at a time. The workflow uses the repository `GITHUB_TOKEN`; the publish job requests `contents: write`. No personal access token is required.

## Build matrix

| Platform | Runner | Rust target | Raw file |
| --- | --- | --- | --- |
| `windows-x64` | `windows-latest` | `x86_64-pc-windows-msvc` | `flint.exe` |
| `linux-x64` | `ubuntu-22.04` | `x86_64-unknown-linux-gnu` | `flint` |
| `macos-x64` | `macos-14` | `x86_64-apple-darwin` | `flint` |
| `macos-arm64` | `macos-14` | `aarch64-apple-darwin` | `flint` |

The matrix uses `fail-fast: false`. Each job installs frontend dependencies, applies the computed version to the same five release paths (`VERSION`, `package.json`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`, and `src-tauri/Cargo.lock`), builds with `tauri build --no-bundle --target <triple>`, and verifies the executable architecture on Unix runners. The Cargo manifest update covers both host and workspace-package versions; the single lockfile update covers every local workspace package entry. Non-root member manifests inherit `[workspace.package]` and are not independently rewritten.

## Artifacts

Each release contains:

```text
Flint-windows-x64-<version>.exe
Flint-windows-x64-<version>.zip
Flint-linux-x64-<version>
Flint-linux-x64-<version>.zip
Flint-macos-x64-<version>
Flint-macos-x64-<version>.zip
Flint-macos-arm64-<version>
Flint-macos-arm64-<version>.zip
sha.txt
```

Every ZIP contains exactly the matching raw executable and `RUNNING.txt`. Raw/ZIP files must be non-empty, ZIP contents are decompressed and compared byte-for-byte with the raw asset, and `sha.txt` contains exactly eight SHA-256 entries, one for each raw/ZIP asset. Aggregate validation rejects missing or unexpected files before publishing.

## Publication

The release job downloads the four platform artifacts, generates and validates `sha.txt`, applies the computed version locally, and generates release notes. Notes use the nearest parseable previous `flint-v*` tag reachable from `HEAD`, excluding the current tag; the first release uses an initial-release fallback. Legacy and malformed tags are ignored with warnings.

The release is published or updated as `flint-v<version>` at `GITHUB_SHA` with:

- the exact nine assets;
- non-empty release notes;
- `draft: false` and `prerelease: false`;
- overwrite enabled for reruns at the same commit.

After publication, the workflow verifies the tag commit, release body, and exact non-empty asset set before making any VERSION commit.

## Post-release VERSION record

After release verification, the workflow:

1. Fetches the latest triggering branch.
2. Verifies the released source commit is still an ancestor of that branch.
3. Reads the branch's current `VERSION`.
4. If branch `VERSION` already equals the released version, exits successfully without another commit.
5. If it equals the source-commit `VERSION`, writes the released version and commits only `VERSION` as `github-actions[bot]`.
6. If it is any other value, refuses the unexpected branch `VERSION` state without overwriting it.
7. Pushes without force.

The post-release `VERSION`-only push does not start another workflow run because it is made with `GITHUB_TOKEN`. A manual `VERSION`-only push to `main` or `dev` does start the workflow and then fails the prepare source guard because its source commit is `VERSION`-only. The commit is a release record, not a source commit, so it must not be used as a release source.

## Reruns and recovery

- A workflow rerun uses the original `GITHUB_SHA`; an existing valid tag at that commit is reused, and an existing release is updated intentionally.
- A new manual dispatch at a newer source commit computes a new monthly version.
- A tag at a different commit is refused; tags are never moved automatically.
- Prepare rejects an empty or exactly `VERSION`-only release-source commit for every push or manual branch-triggered run. At post-release recording, an already-current branch `VERSION` succeeds without a commit, a branch `VERSION` equal to the source-commit value is updated, and any other `VERSION` state is refused rather than overwritten.
- If publication succeeds but the VERSION push is blocked, fix the branch permission/state and rerun the same workflow run.

### Branch protection

The bot push can fail if the branch requires pull requests, reviews, signed commits, or a ruleset that excludes `github-actions[bot]`. Configure an allowed automation path deliberately. The workflow never force-pushes.

## Local checks

Before changing release tooling:

```bash
node --check scripts/release-version.mjs
node --check scripts/version-files.mjs
node --check scripts/apply-version.mjs
node --check scripts/validate-release.mjs
node --check scripts/release-notes.mjs
node --check scripts/test-release-scripts.mjs
node scripts/test-release-scripts.mjs
node scripts/validate-release.mjs --require-exact
```

If actionlint is available:

```bash
actionlint .github/workflows/flint-release.yml
```

Markdown/YAML checks and a real CI run are still required before publishing.

## Operational constraints

- Releases require all four platform builds and the exact asset set; an incomplete build set cannot publish.
- macOS x64 is cross-built on the macOS arm64 runner and must pass the architecture check.
- Linux builds depend on the Ubuntu 22.04 WebKitGTK packages installed by the workflow.
- Release actions are declared by major version; pin them to audited commit SHAs if required by repository policy.
- Binaries are unsigned and macOS builds are not notarized; Windows SmartScreen and macOS Gatekeeper warnings are expected.
- Raw Unix downloads may lose the executable bit; use the ZIP or `chmod +x`.
- A frontend lockfile is recommended; without one the workflow falls back to `npm install`.
- Branch protection can block the post-release VERSION commit after a public release.
- A partial release must be retried from the same workflow commit; do not move the tag.

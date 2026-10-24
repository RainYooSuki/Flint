# Flint release workflow

The active workflow is `flint-release.yml`.

## Location

GitHub only discovers workflows under the repository-root `.github/workflows/` directory. When this project is published, `Flint/` should be the repository root; keep this file at `.github/workflows/flint-release.yml`.

## Triggers

- Pushes to `main` or `dev`.
- Manual `workflow_dispatch` from a branch.

Pushes containing only `VERSION`, Markdown/docs, workflow files, release scripts, or ignore metadata are skipped to avoid release loops and docs-only releases.

## Maintainer expectations

The workflow computes `YYYY.M.N-sha7`, builds Windows x64, Linux x64, macOS x64, and macOS arm64, publishes raw files plus matching ZIPs and `sha.txt`, and verifies the exact release asset set. Only after publication and verification does it commit the released value to `VERSION` as `github-actions[bot]` and push to the triggering branch.

Use `main` or `dev` for normal releases. Review [`../../RELEASE.md`](../../RELEASE.md) before changing tags, release permissions, or branch protection. Run the local checks in that guide before pushing release-tooling changes.
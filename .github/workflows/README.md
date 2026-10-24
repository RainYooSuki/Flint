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

The workflow computes `YYYY.M.N-sha7`, builds Windows x64, Linux x64, macOS x64, and macOS arm64, publishes raw files plus matching ZIPs and `sha.txt`, and verifies the exact release asset set. Only after publication and verification does it commit the released value to `VERSION` as `github-actions[bot]` and push to the triggering branch. That bot push uses `GITHUB_TOKEN`, so it does not start another workflow run; a manual `VERSION`-only push does start the workflow and then fails the prepare source guard.

At post-release recording, if branch `VERSION` already equals the released version, the workflow exits without another commit. Otherwise it requires branch `VERSION` to equal the source-commit `VERSION`, writes the released value, and refuses any unexpected branch `VERSION` state rather than overwriting it.

Use `main` or `dev` for normal releases. Review [`../../RELEASE.md`](../../RELEASE.md) before changing tags, release permissions, or branch protection. Run the local checks in that guide before pushing release-tooling changes.
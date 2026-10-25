# Flint

Flint is a desktop launcher for Minecraft Java Edition. It supports Microsoft and offline accounts, vanilla and modded instances, mod discovery and management, and local save management through a Tauri 2 desktop shell.

## Feature overview

- Microsoft device-code accounts and online launch, plus the built-in offline account.
- Local instance discovery, Java scanning, settings, and vanilla version installation.
- Forge, NeoForge, Fabric, Quilt, LiteLoader, and OptiFine installation.
- CurseForge and Modrinth mod search, dependency planning, and installed-mod management.
- Save listing, backup, export, import, rename, and delete.
- Cross-platform release builds for Windows x64, Linux x64, macOS x64, and macOS arm64.

## Accounts and online launch

The offline account is always available and uses the username configured in Flint. Microsoft sign-in uses OAuth 2.0 device-code flow and makes the signed-in profile the active account. Launch uses the active account automatically; Microsoft sessions are refreshed when needed.

Microsoft sign-in requires a Microsoft Entra public-client application ID. There is no client secret and no redirect URI in the device-code flow. The account must own Minecraft: Java Edition. Access and refresh tokens are stored only in the local account store and are never inserted into launch output.

See [`docs/INTEGRATIONS.md`](docs/INTEGRATIONS.md) for Azure app registration, key storage, and environment notes.

## Loader installation

Choose a game directory, base Minecraft version, loader version, and Java executable in the Downloads view. If the base version is missing, Flint installs the vanilla metadata, client, libraries, and assets first.

| Loader | Installation path |
| --- | --- |
| Forge | Official installer JAR; Java executable required |
| NeoForge | Official installer JAR; Java executable required |
| Fabric | Loader profile and libraries from Fabric metadata |
| Quilt | Loader profile and libraries from Quilt metadata |
| LiteLoader | LiteLoader metadata/profile extraction |
| OptiFine | Upstream listing and installer flow when a direct JAR is available |

Every successful install creates a local version that appears in the instance list. Installed files stay inside the selected game directory; when an installer runs, its log also stays there. Java is not bundled with Flint; Forge, NeoForge, and OptiFine installer flows and all loader launches need a working Java runtime.

OptiFine's upstream download is ad-gated and may not provide a direct JAR. Flint refuses to execute an HTML response and reports a clear failure; in that case, install OptiFine manually into the same game directory and refresh local versions.

## Mods

The mod discovery and installed-mod panels use the currently selected instance:

- Search is filtered by that instance's Minecraft version and loader. Mod search currently supports Fabric, Forge, NeoForge, and Quilt contexts.
- Modrinth works without additional configuration. CurseForge requires an API key in Settings.
- Version selection can resolve required dependencies recursively. Optional and incompatible dependencies are reported, while required dependencies without a safe download fail the operation.
- Mods are downloaded atomically into `<gameDir>/mods`, scanned for metadata, and never executed by Flint.
- Installed mods can be enabled, disabled, or deleted from the selected game directory. Disabled JARs use the `.jar.disabled` suffix.

## Save manager

The save manager uses the selected instance's game directory:

- Lists worlds from `<gameDir>/saves` with level name, last-played time, size, game mode, hardcore flag, version, and icon when available.
- Creates ZIP backups under `<gameDir>/backups/flint-saves`.
- Exports a save to a chosen ZIP, imports a ZIP containing one save folder or a top-level `level.dat`, renames the save folder, and deletes saves after confirmation.
- Rejects unsafe archive paths and save names during import and management.

## Integrations

Microsoft and CurseForge integrations are optional. Configure them in Settings under Integration services. Both values are user-supplied at runtime; no client secret, API key, or account token is embedded in a release build. See [`docs/INTEGRATIONS.md`](docs/INTEGRATIONS.md) for setup, local storage paths, and troubleshooting.

## Technology

Flint uses Tauri 2 with a nineteen-package Rust workspace and a React 18 + TypeScript frontend built by Vite. The Rust workspace separates the thin Tauri host, framework-neutral core/API crates, seven feature crates, two mod providers, and six loader providers. Provider-backed commands and feature availability share one actual-registry `ProviderRuntimeState`; the host owns lifecycle, availability IPC, command gating, and exit shutdown. Frontend Tauri access is confined to typed invoke/event/dialog/opener/window adapters injected as `HostServices`. The base command contract is in [`docs/API_CONTRACT.md`](docs/API_CONTRACT.md); account, loader, mod, and save extensions are in [`docs/FEATURE_API_CONTRACT.md`](docs/FEATURE_API_CONTRACT.md).

## Requirements

- Node.js 20 or newer.
- Rust 1.90 or newer.
- Tauri 2 system prerequisites:
  - Windows: Microsoft WebView2 and Visual Studio C++ build tools.
  - macOS: Xcode Command Line Tools.
  - Linux: WebKitGTK, GTK, and the usual build dependencies. The release workflow installs the required Ubuntu 22.04 packages.
- A Java runtime for launching and for Forge, NeoForge, and OptiFine installation. Flint discovers Java runtimes but does not download one.
- Network access to Mojang, Microsoft, Modrinth, CurseForge, and the selected loader service.
- A Microsoft Entra application ID for Microsoft sign-in, and a CurseForge API key for CurseForge search; both are optional until the corresponding feature is used.

See the [Tauri prerequisites guide](https://v2.tauri.app/start/prerequisites/) for platform details.

## Development

Install dependencies:

```bash
npm install
```

Run the desktop app:

```bash
npm run tauri -- dev
```

Run only the Vite frontend:

```bash
npm run dev
```

The development server listens on `http://localhost:1420`.

## Build and test

Install frontend dependencies:

```bash
npm install
```

Build the frontend:

```bash
npm run build
```

Build a raw native executable without OS bundles:

```bash
npm run tauri -- build --no-bundle
```

Build the configured native bundles:

```bash
npm run tauri -- build
```

The release workflow uses `tauri build --no-bundle --target <triple>` to produce the raw executable for each release target. It then packages that raw executable into the matching ZIP artifact; it does not publish the platform installer bundles produced by an ordinary bundled build. A local `npm run tauri -- build` uses the bundle formats configured in `src-tauri/tauri.conf.json`.

Run the available development checks:

```bash
cargo fmt --all --manifest-path src-tauri/Cargo.toml -- --check
cargo test --workspace --all-targets --locked --manifest-path src-tauri/Cargo.toml
node --test scripts/test-architecture-checker.mjs scripts/test-frontend-contracts.mjs
node scripts/check-architecture.mjs --phase 5
node scripts/test-release-scripts.mjs
node scripts/validate-release.mjs --require-exact
```

These commands are developer checks, not evidence that the hosted release workflow is green. GitHub Actions is the verification authority for completion and release.

Raw output locations without `--target` are `src-tauri/target/release/flint.exe` on Windows and `src-tauri/target/release/flint` on Linux/macOS. Target-specific builds are placed under `src-tauri/target/<triple>/release/`. Cargo resolves backend dependencies from `src-tauri/Cargo.lock`; no separate Rust dependency installation step is needed.

## Versioning

Release versions are generated from the exact source commit:

```text
YYYY.M.N-<sha7>
```

- `YYYY.M` is the UTC year and month of the source commit.
- `N` is the one-based release ordinal for that UTC month. It resets to `1` in a new month and is based on existing valid monthly release tags.
- `sha7` is the first seven lowercase characters of the source commit SHA.
- If an all-numeric SHA7 begins with `0`, the pre-release identifier is written as `g<sha7>` to keep valid SemVer.
- Release tags use the form `flint-v<version>`, for example `flint-v2026.10.2-def5678`.

The release workflow computes the version before building and applies it to the five-file release metadata set: `VERSION`, `package.json`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`, and `src-tauri/Cargo.lock`. Within `src-tauri/Cargo.toml`, both the host `[package] version` and `[workspace.package] version` are updated. The host package keeps its generated `[package] version`, `[workspace.package] version` matches it, and every non-root workspace member uses `version.workspace = true`. `Cargo.lock` is updated for every workspace-local package (`flint`, the three foundation crates, the seven `flint-feature-*` crates, the two `flint-mod-provider-*` crates, and the six `flint-loader-provider-*` crates). Member manifests are not independently versioned, and no Cargo workspace package may use an independent `0.x` release version. After a successful release, the workflow commits only the released value to `VERSION` on the triggering branch as `github-actions[bot]`. A rerun at the same source commit reuses an existing valid tag at that commit.

`VERSION` is the release ledger. Generated package/Tauri/Cargo workspace metadata can lag the ledger, but the host, `[workspace.package]`, member manifests, and every local `Cargo.lock` package entry must remain internally consistent at that generated value. SHA prereleases are identifiers rather than chronological ordering signals. The version-only prepare validation checks version syntax and internal metadata consistency without treating a SHA-only difference as "ahead"; it cannot be combined with `--version`, `--require-exact`, or asset validation. Exact checks after `apply-version` and `--require-exact` remain strict.

## Release artifacts

For version `<version>`, releases contain the Windows `.exe`, the Linux/macOS raw executables, a matching ZIP for every executable, and `sha.txt`. Files follow `Flint-<platform>-<version>` naming, with `.exe` on Windows and `.zip` for archives. Each ZIP contains the matching raw executable and a `RUNNING.txt`; `sha.txt` contains SHA-256 hashes for the eight raw/ZIP assets, not for itself.

## Release workflow

The workflow is [`.github/workflows/flint-release.yml`](.github/workflows/flint-release.yml).

- Every push to `main` or `dev` starts the release workflow, including docs/tooling-only pushes and pushes that change only `VERSION`, Markdown/docs, workflow files, release scripts, or ignore metadata. The `prepare` job runs the architecture and frontend contract tests, then `node scripts/check-architecture.mjs --phase 5`.
- Manual runs use `workflow_dispatch` from the selected branch.
- No changed-file exemptions apply. The prepare source guard runs for every automatic push or manual branch-triggered run and rejects an empty commit or a commit whose only changed file is `VERSION`. Docs/tooling/workflow-only commits and `VERSION` plus other-file commits may proceed. A `VERSION`-only push starts the workflow and then fails in prepare.
- After release verification, if branch `VERSION` already equals the released version, the workflow exits successfully without another commit; otherwise it requires the branch value to equal the source-commit `VERSION`, writes the released value, and refuses any unexpected branch `VERSION` state.
- The post-release `VERSION`-only bot push does not start another workflow run because it is made with `GITHUB_TOKEN`; a manual `VERSION`-only push does start one and then fails the prepare source guard.
- The build matrix creates Windows x64, Linux x64, macOS x64, and macOS arm64 assets.
- Packaging validation checks the raw executables, exact ZIP contents, eight release assets, and `sha.txt` before publishing.
- The release uses `flint-v<version>` and notes commits since the previous reachable parseable release tag.
- The publish job uses `GITHUB_TOKEN` with `contents: write`; no personal access token is required.
- GitHub must discover the workflow from the repository-root `.github/workflows/` directory, so `Flint/` should be published as the repository root.

## Current limitations

- Modpacks, resource packs, server browsing, skins, and capes are not implemented.
- Microsoft sign-in requires a user-supplied Azure public-client ID; CurseForge search requires a CurseForge API key. Offline launch and Modrinth do not require either.
- Java is not bundled or downloaded automatically.
- OptiFine may require a manual installer because upstream links can be ad-gated or unavailable.
- Release binaries are unsigned and macOS builds are not notarized.

## Project layout

```text
.github/workflows/   Release automation
docs/                API contracts, integration setup, and project notes
scripts/             Version, release, and validation tooling
src/                 React frontend
src-tauri/           Rust/Tauri backend
VERSION              Last recorded release version
README.md            Project overview
RELEASE.md           Release operator guide
```

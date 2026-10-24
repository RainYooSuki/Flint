# Flint

Flint is a desktop launcher for Minecraft Java Edition. It combines a Tauri 2 shell, a React interface, and a Rust backend for local instance management, Java discovery, version installation, and offline launch.

## Features

- Local instance discovery from a Minecraft `versions` directory.
- Java runtime discovery from `JAVA_HOME`, `PATH`, and common platform locations.
- Local settings for game directory, Java path, memory, window size, and launch arguments.
- Mojang version catalog browsing and vanilla version installation with progress events.
- Offline username launch for installed versions, with output written to a log file.
- Compact custom window chrome with minimize, maximize, and close controls.
- Release builds for Windows x64, Linux x64, macOS x64, and macOS arm64.

## Current limitations

- Microsoft/Mojang account sign-in and online authentication are not implemented.
- Modpacks, resource packs, server browsing, skins, and capes are not implemented.
- Release binaries are unsigned; macOS builds are not notarized.

## Technology

Flint uses Tauri 2 with a Rust backend and a React 18 + TypeScript frontend built by Vite. The frontend/backend interface is documented in [`docs/API_CONTRACT.md`](docs/API_CONTRACT.md).

## Requirements

- Node.js 20 or newer.
- Rust 1.90 or newer.
- Tauri 2 system prerequisites:
  - Windows: Microsoft WebView2 and Visual Studio C++ build tools.
  - macOS: Xcode Command Line Tools.
  - Linux: WebKitGTK, GTK, and the usual build dependencies. The release workflow installs the required Ubuntu 22.04 packages.
- See the [Tauri prerequisites guide](https://v2.tauri.app/start/prerequisites/) for platform details.

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

Run the available checks:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml --check
node scripts/test-release-scripts.mjs
node scripts/validate-release.mjs --require-exact
```

Raw output locations without `--target` are `src-tauri/target/release/flint.exe` on Windows and `src-tauri/target/release/flint` on Linux/macOS. Target-specific builds are placed under `src-tauri/target/<triple>/release/`.

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

The release workflow computes the version before building and applies it to `VERSION`, `package.json`, `tauri.conf.json`, `Cargo.toml`, and `Cargo.lock`. After a successful release, it commits only the released value to `VERSION` on the triggering branch as `github-actions[bot]`. A rerun at the same source commit reuses an existing valid tag at that commit.

## Release artifacts

For version `<version>`, releases contain:

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

Each ZIP contains the matching raw executable and a `RUNNING.txt`. `sha.txt` contains SHA-256 hashes for the eight raw/ZIP assets, not for itself.

## Release workflow

The workflow is [`.github/workflows/flint-release.yml`](.github/workflows/flint-release.yml).

- Automatic runs are triggered by pushes to `main` or `dev`; manual runs use `workflow_dispatch` from the selected branch.
- Pushes containing only `VERSION`, Markdown/docs, workflow files, release scripts, or ignore metadata do not start a release.
- The build matrix creates Windows x64, Linux x64, macOS x64, and macOS arm64 assets.
- Packaging validation checks the raw executables, exact ZIP contents, eight release assets, and `sha.txt` before publishing.
- The release uses `flint-v<version>` and notes commits since the previous parseable release tag reachable from `HEAD`; the current tag is excluded.
- The publish job uses `GITHUB_TOKEN` with `contents: write`; no personal access token is required.
- GitHub must discover the workflow from the repository-root `.github/workflows/` directory, so `Flint/` should be published as the repository root.

## Project layout

```text
.github/workflows/   Release automation
docs/                API contract and project notes
scripts/             Version, release, and validation tooling
src/                 React frontend
src-tauri/           Rust/Tauri backend
VERSION              Last recorded release version
README.md            Project overview
RELEASE.md           Release operator guide
```
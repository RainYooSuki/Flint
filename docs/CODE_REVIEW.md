# Flint Quality Summary

## Review scope

The final independent review covers the tracked application and release boundary:

- Microsoft device-code accounts, active-account selection, token refresh, and online launch;
- vanilla version installation and Forge, NeoForge, Fabric, Quilt, LiteLoader, and OptiFine loader flows;
- CurseForge and Modrinth search, required dependency planning, downloaded-mod installation, and installed-mod management;
- save discovery, backup, export, import, rename, and delete handling;
- the Tauri command surface, frontend API bridge, local settings/account stores, selected-instance context, Design B shell, and release automation.

The detailed command and model contracts live in `docs/API_CONTRACT.md` and `docs/FEATURE_API_CONTRACT.md`.

## Defects fixed in the final review

- The installed-mod toggle compared the post-toggle filename with the old list entry, so the row did not update after a successful enable/disable. It now replaces the pre-toggle entry.
- A stale loader-version request could leave the loader panel permanently in a loading state after the selected instance, game version, or loader changed. The reset path now clears that state.
- Mod discovery now accepts only the four documented loader contexts (Fabric, Forge, NeoForge, and Quilt); LiteLoader and OptiFine instances are shown as unsupported for mod search.
- Java is required for the Forge, NeoForge, and OptiFine installer flows rather than for profile-only loader installs.
- Save rename now edits and reports the folder name while explicitly preserving the in-game `LevelName`; import messaging no longer claims a same-name overwrite.
- Microsoft verification and skin URLs now require HTTPS, and device-code `slow_down` responses are propagated to the UI so it increases the polling interval.
- Unix settings and account files are tightened to `0600` when read and when atomically replaced.
- Provider mod downloads are rejected unless they are valid JAR/ZIP archives, and archive extraction paths are bounded during copying rather than only after a write.

## Static review notes

- The Rust command registration in `src-tauri/src/lib.rs` matches all 29 wrappers in `src/api.ts` and both contract documents.
- The frontend and Rust model fields match, including nullable Option fields and the `slowDown` polling flag. `AccountInfo` exposes profile metadata only, never access or refresh tokens.
- Account storage is local: `accounts.json` contains the Microsoft token store and is written with owner-only permissions on Unix. The account update path uses a temporary replacement file and validates active-account fallback.
- Microsoft device-code login requires a configured public-client application ID; token values are redacted from `LaunchResult.command`.
- Loader installation attempts to install the required vanilla base first, uses bounded installer/network operations, and rejects non-archive responses instead of executing HTML.
- Mod dependency planning is recursive with cycle, depth, and node limits. Required failures stop installation; optional and incompatible dependencies are reported only. Downloaded JARs are scanned for metadata and are not executed by Flint.
- Mod state changes are limited to safe file names under `<gameDir>/mods`; disabling uses the `.jar.disabled` suffix and deleting a mod also removes its sidecar.
- Save import validates ZIP structure, duplicate/unsafe paths, and symlink boundaries. Backups and exports are ZIP files; destructive actions have explicit UI confirmation.

## Checks run for the final tree

Passed:

```text
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo check --manifest-path src-tauri/Cargo.toml --all-targets --offline
cargo test --manifest-path src-tauri/Cargo.toml --offline --no-run
fresh pure Rust test harness (current source): 72 passed, 0 failed
node --check scripts/*.mjs
node scripts/test-release-scripts.mjs
node scripts/validate-release.mjs --require-exact
YAML parse with duplicate-key rejection
GitHub Script syntax checks and PowerShell parser checks
JSON, Markdown link/fence, CSS, TSX delimiter/JSX, API/model/command consistency,
prohibited-wording, secret-exposure, event-cleanup, and layout audits
git diff --check
```

The full `cargo test` binaries linked successfully, but this Windows host could not start the Tauri-linked test executable because `WebView2Loader.dll` reported `STATUS_ENTRYPOINT_NOT_FOUND`. The same current Rust modules were therefore executed through a fresh pure harness with the Tauri event dependency stubbed; all 72 tests passed. Git Bash `bash -n` could not run because the host returned a signal-pipe creation error, so workflow shell bodies received YAML, PowerShell, and GitHub Script static checks rather than a local Bash parse.

## Current readiness

The source is ready for hosted build and live integration validation. This review did not run the following:

- a full `npm install` / `tsc` / Vite/Tauri desktop build or target-OS rendering;
- real Microsoft device-code authorization with a configured Azure application;
- live CurseForge and Modrinth requests, including API-key quotas and provider file availability;
- real Forge/NeoForge/Fabric/Quilt/LiteLoader/OptiFine installations with a matching Java runtime;
- live launcher smoke tests for online accounts, modded instances, and save operations;
- hosted GitHub Actions execution.

## Operational risks

- Microsoft sign-in depends on the user's Azure public-client configuration, personal-account support, and Minecraft entitlement. Missing or misconfigured client IDs block only Microsoft features; offline launch remains available.
- Account-file updates are last-write-wins across concurrent command executions; the store is replaced atomically, but a globally serialized mutation queue is not implemented.
- CurseForge requires a user-provided API key and is subject to provider access, quotas, and file availability. Modrinth remains available without a key.
- Provider payloads and downloads are size-limited and parsed defensively, but live CDN redirects, removed files, and provider rate limits remain outside local validation.
- Loader installation depends on Java and upstream metadata/installer availability. A partially populated vanilla directory is considered ready when its metadata and client JAR exist, and installer profile discovery has live-runtime fallbacks.
- Mod dependency installs are not a single filesystem transaction; if a later dependency fails, earlier installed files may remain and are reported as a partial failure.
- OptiFine upstream links can be ad-gated or unavailable; the manual-install fallback is required in that case.
- Microsoft and CurseForge credentials are stored in local JSON; Unix owner-only permissions are implemented but were not executable on this Windows review host. Users must protect the OS account and data directory.
- macOS x64 is cross-built on a macOS runner and Windows/macOS binaries are unsigned. Linux builds depend on the workflow's WebKitGTK packages.
- Release actions are referenced by major version. Branch protection can block the post-release VERSION-only bot commit.


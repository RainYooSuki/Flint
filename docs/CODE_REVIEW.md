# Historical Quality Snapshot

> **Historical snapshot only.** This file records an earlier point-in-time review of the application and release boundary. It does not identify a current commit, has no recorded hosted-CI run, and is not evidence for the current working tree. Use [`ARCHITECTURE.md`](ARCHITECTURE.md), the authoritative contracts, and GitHub Actions for current status and verification.

## Review scope

The reviewed snapshot covered the tracked application and release boundary:

- Microsoft device-code accounts, active-account selection, token refresh, and online launch;
- vanilla version installation and Forge, NeoForge, Fabric, Quilt, LiteLoader, and OptiFine loader flows;
- CurseForge and Modrinth search, required dependency planning, downloaded-mod installation, and installed-mod management;
- save discovery, backup, export, import, rename, and delete handling;
- the Tauri command surface, host lifecycle/availability integration, typed frontend host services, contribution registry, and release automation.

The detailed command and model contracts live in [`API_CONTRACT.md`](API_CONTRACT.md) and [`FEATURE_API_CONTRACT.md`](FEATURE_API_CONTRACT.md).

## Defects addressed in the reviewed snapshot

- The installed-mod toggle compared the post-toggle filename with the old list entry, so the row did not update after enable/disable. It now replaces the pre-toggle entry.
- A stale loader-version request could leave the loader panel permanently loading after selected instance, game version, or loader changed. The reset path now clears that state.
- Mod discovery accepted only the four documented loader contexts (Fabric, Forge, NeoForge, and Quilt); LiteLoader and OptiFine instances are unsupported for mod search.
- Java was required for Forge, NeoForge, and OptiFine installer flows rather than profile-only loader installs.
- Save rename edited and reported the folder name while preserving the in-game `LevelName`; import messaging no longer claimed a same-name overwrite.
- Microsoft verification and skin URLs required HTTPS, and device-code `slow_down` responses propagated to the UI polling cadence.
- Unix settings and account files were tightened to `0600` when read and atomically replaced.
- Provider mod downloads were rejected unless they were valid JAR/ZIP archives, and archive copying was bounded during extraction rather than only after a write.

## Snapshot observations

- The compile-time command surface and DTO/client shapes were reviewed for consistency; `AccountInfo` exposed profile metadata only, never access or refresh tokens.
- Account storage was local and used owner-only Unix permissions where available. The store replacement was atomic, but account mutations remained last-write-wins across concurrent commands.
- Microsoft launch-command display redacted access tokens.
- Loader installation attempted to prepare the vanilla base first, used bounded installer/network operations, and rejected non-archive responses instead of executing HTML.
- Mod dependency planning was recursive with cycle, depth, and node limits. Required failures stopped installation; optional and incompatible dependencies were reported only. Downloaded JARs were scanned for metadata and never executed.
- Mod state changes were limited to safe file names under `<gameDir>/mods`; disabling used `.jar.disabled`, and deleting a mod also removed its sidecar.
- Save import validated ZIP structure, duplicate/unsafe paths, and symlink boundaries. Backups/exports were ZIP files, and destructive actions had explicit UI confirmation.

## Verification status

The historical snapshot included local formatting, compilation, unit-test, release-script, markup/static-audit, and consistency checks, but it did not record a successful hosted GitHub Actions run. Some Tauri-linked tests could not execute on the review host; an alternate harness was used at that time. Those historical results are not reproducible evidence for the current source and are retained only as context.

Not validated in that snapshot:

- a full hosted four-platform release workflow;
- real Microsoft device-code authorization with a configured application;
- live CurseForge or Modrinth requests, quotas, redirects, or provider file availability;
- real Forge/NeoForge/Fabric/Quilt/LiteLoader/OptiFine installations with matching Java runtimes;
- live desktop smoke tests for accounts, modded instances, dialogs/opener/window controls, and save operations.

## Historical operational risks

- Microsoft sign-in depends on the user's application configuration, personal-account support, and Minecraft entitlement.
- CurseForge requires a user-provided API key and is subject to provider access, quotas, and file availability. Modrinth does not require a key.
- Provider payloads and downloads are size-limited and parsed defensively, but live CDN redirects, removed files, and provider rate limits require runtime validation.
- Loader installation depends on Java and upstream metadata/installer availability.
- Mod dependency installation is not a single filesystem transaction; later failures may leave earlier installed files and must be reported as partial.
- OptiFine upstream links can be ad-gated or unavailable; manual installation may be required.
- Microsoft and CurseForge credentials are stored in local JSON; users must protect the operating-system account and data directory.
- Release binaries are unsigned and macOS builds are not notarized. macOS x64 is cross-built on an Apple-silicon runner.
- Branch protection can block the post-release `VERSION` record commit.

## Current authority

This snapshot must not be used to claim current test counts, current CI status, or current implementation completeness. Current architecture/status is owned by [`ARCHITECTURE.md`](ARCHITECTURE.md), policy by [`DEVELOPMENT_STANDARDS.md`](DEVELOPMENT_STANDARDS.md), and executable verification by GitHub Actions.

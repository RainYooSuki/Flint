# Flint Feature API Contract

This document extends `docs/API_CONTRACT.md` with account, loader-installation, mod-platform, mod-management and save-management capabilities. All serialized Rust structs use `#[serde(rename_all = "camelCase")]`.

All commands in this document are gated by their owning backend feature. The authoritative availability DTO, lifecycle states, unavailable-error format, and gating semantics are documented in [`API_CONTRACT.md#host-availability-and-command-gating`](API_CONTRACT.md#host-availability-and-command-gating).

## Existing model changes

`LauncherSettings` gains two locally stored integration values:

```ts
interface LauncherSettings {
  // existing fields unchanged
  microsoftClientId: string; // Azure public-client application ID
  curseforgeApiKey: string;  // CurseForge API key
}
```

Both default to `""` when an older settings file is loaded.

`InstanceInfo` gains the resolved base Minecraft version:

```ts
interface InstanceInfo {
  // existing fields unchanged
  gameVersion: string; // e.g. "1.21.1", resolved through inheritsFrom
}
```

## Accounts and Microsoft authentication

Microsoft login uses OAuth 2.0 Device Code Flow with scopes `XboxLive.signin offline_access`. The application ID is read from `LauncherSettings.microsoftClientId`. No client secret is stored or required.

```ts
type AccountType = "offline" | "microsoft";
type LoginStatus = "pending" | "success" | "error";

interface AccountInfo {
  id: string;                 // "offline" or Minecraft profile UUID
  username: string;
  uuid: string | null;
  accountType: AccountType;
  active: boolean;
  skinUrl: string | null;
  expiresAt: number | null;   // Unix seconds for Microsoft access token
}

interface MicrosoftDeviceCode {
  deviceCode: string;
  userCode: string;
  verificationUri: string;
  expiresIn: number;
  interval: number;
  message: string;
}

interface MicrosoftLoginPoll {
  status: LoginStatus;
  message: string;
  account: AccountInfo | null;
  slowDown: boolean;
}
```

Commands:

| Command | Arguments | Result |
| --- | --- | --- |
| `list_accounts` | none | `AccountInfo[]` |
| `begin_microsoft_login` | none | `MicrosoftDeviceCode` |
| `poll_microsoft_login` | `deviceCode: string` | `MicrosoftLoginPoll` |
| `select_account` | `accountId: string` | `AccountInfo[]` |
| `remove_account` | `accountId: string` | `AccountInfo[]` |
| `refresh_active_account` | none | `AccountInfo` |

Rules:

- `list_accounts` always includes a synthetic offline account with id `offline`, using `LauncherSettings.username`, followed by stored Microsoft accounts.
- Exactly one account is active. The active id defaults to `offline` and is persisted in the app data directory.
- A successful Microsoft login upserts by Minecraft UUID and makes that account active.
- `poll_microsoft_login` returns `pending` for normal device-flow polling statuses. `success` includes the account. Terminal failures return `error` with a readable message.
- `MicrosoftLoginPoll.slowDown` is true when Microsoft asks for a slower cadence; the frontend must increase the subsequent polling interval by at least five seconds.
- Access and refresh tokens are persisted in the app data directory; tokens must never be included in logs, returned API responses, launch result commands, or UI notices.
- `remove_account("offline")` is rejected. Removing the active Microsoft account falls back to offline.
- `launch_instance` resolves the active account automatically. Microsoft accounts use their Minecraft UUID, access token and `msa` user type; the access token is refreshed when needed. Offline accounts continue using `LaunchRequest.username`.
- `LaunchResult.command` is a display-only command string. Access tokens are replaced before formatting, while the spawned process receives the internal arguments separately. Tokens must never appear in returned commands, errors, events, logs, or UI notices.

## Loader installation

Loader identifiers are lowercase:

```ts
type LoaderKind = "forge" | "neoforge" | "fabric" | "quilt" | "liteloader" | "optifine";

interface LoaderVersionInfo {
  loader: LoaderKind;
  version: string;
  label: string;
  recommended: boolean;
  gameVersion: string;
  releaseTime: string | null;
}

interface LoaderInstallRequest {
  gameDir: string;
  gameVersion: string;
  loader: LoaderKind;
  loaderVersion: string;
  javaPath: string;
}

interface LoaderInstallResult {
  gameDir: string;
  gameVersion: string;
  loader: LoaderKind;
  loaderVersion: string;
  installedVersionId: string;
  installPath: string;
  logPath: string | null;
}

interface LoaderInstallProgress {
  loader: LoaderKind;
  gameVersion: string;
  loaderVersion: string;
  stage: string;
  completed: number;
  total: number;
  message: string;
}
```

Commands:

| Command | Arguments | Result |
| --- | --- | --- |
| `list_loader_versions` | `loader: LoaderKind`, `gameVersion: string` | `LoaderVersionInfo[]` |
| `install_loader` | `request: LoaderInstallRequest` | `LoaderInstallResult` |

`install_loader` emits `flint://loader-install-progress`.

Rules:

- If the selected vanilla version is absent, install the vanilla client metadata/client/libraries/assets first as part of the operation.
- Run official/manual installer JARs only with the supplied Java executable and a bounded timeout. Capture output to a log under the app data or game directory.
- Forge/NeoForge use their installer JARs. Fabric/Quilt build loader profiles from their metadata endpoints and download the required libraries into the selected game directory.
- LiteLoader and OptiFine must use their public version/download metadata and installer/profile extraction paths. Fail with a clear error when the upstream endpoint cannot provide a safe direct download.
- Validate all downloaded URLs are HTTPS, constrain archive extraction against path traversal, and return the installed version id so the UI can refresh local instances.
- No command may panic on malformed JSON/XML/HTML or network failure.

## Mod platforms, dependency resolution and mod management

```ts
type ModSource = "curseforge" | "modrinth";
type ModDependencyType = "required" | "optional" | "embedded" | "incompatible";

interface ModProject {
  id: string;
  source: ModSource;
  slug: string;
  name: string;
  summary: string;
  author: string;
  downloads: number;
  iconUrl: string | null;
  categories: string[];
  projectUrl: string;
}

interface ModSearchPage {
  source: ModSource;
  total: number;
  offset: number;
  limit: number;
  hits: ModProject[];
}

interface ModDependencyRef {
  source: ModSource;
  projectId: string;
  versionId: string | null;
  name: string;
  dependencyType: ModDependencyType;
}

interface ModVersionInfo {
  id: string;
  source: ModSource;
  projectId: string;
  name: string;
  versionNumber: string;
  fileName: string;
  downloadUrl: string | null;
  gameVersions: string[];
  loaders: string[];
  releaseType: string;
  publishedAt: string | null;
  dependencies: ModDependencyRef[];
}

interface ModDependencyPlan {
  root: ModVersionInfo;
  required: ModVersionInfo[];
  optional: ModDependencyRef[];
  conflicts: string[];
  warnings: string[];
}

interface InstalledModInfo {
  fileName: string;
  name: string;
  modId: string | null;
  version: string | null;
  loader: string | null;
  enabled: boolean;
  path: string;
  source: ModSource | null;
  projectId: string | null;
  versionId: string | null;
}

interface ModInstallProgress {
  projectName: string;
  fileName: string;
  stage: string;
  completed: number;
  total: number;
  message: string;
}

interface ModInstallResult {
  root: InstalledModInfo;
  dependencies: InstalledModInfo[];
  skipped: string[];
  warnings: string[];
}
```

Commands:

| Command | Arguments | Result |
| --- | --- | --- |
| `search_mods` | `source, query, gameVersion, loader, offset, limit` | `ModSearchPage` |
| `list_mod_versions` | `source, projectId, gameVersion, loader` | `ModVersionInfo[]` |
| `resolve_mod_dependencies` | `source, projectId, versionId, gameVersion, loader` | `ModDependencyPlan` |
| `install_mod` | `gameDir, source, projectId, versionId, gameVersion, loader, installDependencies` | `ModInstallResult` |
| `list_installed_mods` | `gameDir` | `InstalledModInfo[]` |
| `set_mod_enabled` | `gameDir, fileName, enabled` | `InstalledModInfo` |
| `delete_mod` | `gameDir, fileName` | `void` |

`install_mod` emits `flint://mod-install-progress`.

Rules:

- CurseForge calls require `LauncherSettings.curseforgeApiKey`; a missing key returns an actionable error. Modrinth calls require a descriptive `User-Agent` header.
- `search_mods`, `list_mod_versions`, `resolve_mod_dependencies`, and `install_mod` accept only the exact lowercase mod-search loader values `fabric`, `forge`, `neoforge`, and `quilt`. Legacy separator aliases, substrings, display-label matching, empty values, and other loaders fail with an actionable error before provider/network I/O. Only Minecraft Java mods are searched.
- `list_mod_versions` returns versions matching the requested game version and loader, newest first.
- Dependency resolution follows required dependencies recursively with cycle detection, a bounded node count, and a bounded depth. Optional and incompatible dependencies are reported but not installed.
- Required dependencies without a safe downloadable file are a hard error for `resolve_mod_dependencies`; `install_mod(..., installDependencies=true)` installs the resolved required graph.
- Downloads are written atomically into `<gameDir>/mods`, scanned safely with the `zip` crate for metadata. Supported metadata includes Fabric `fabric.mod.json`, Quilt `quilt.mod.json`, Forge/NeoForge `META-INF/mods.toml` or `META-INF/neoforge.mods.toml`, and legacy `mcmod.info`.
- Persist provider/project/version metadata in a small sidecar file; never execute downloaded JARs.
- Disabled mods are stored as `<name>.jar.disabled`; enabling removes `.disabled`. Path traversal in file names is rejected. Deleting a mod also removes its sidecar.

## Save management

```ts
interface SaveInfo {
  folderName: string;
  displayName: string;
  path: string;
  iconDataUrl: string | null;
  lastPlayed: number | null; // Unix milliseconds
  sizeBytes: number;
  gameMode: string | null;
  hardcore: boolean;
  versionName: string | null;
}

interface SaveTransferResult {
  path: string;
  save: SaveInfo | null;
}
```

Commands:

| Command | Arguments | Result |
| --- | --- | --- |
| `list_saves` | `gameDir: string` | `SaveInfo[]` |
| `backup_save` | `gameDir, folderName` | `SaveTransferResult` |
| `export_save` | `gameDir, folderName, destination` | `SaveTransferResult` |
| `import_save` | `gameDir, archivePath` | `SaveInfo` |
| `rename_save` | `gameDir, folderName, newName` | `SaveInfo` |
| `delete_save` | `gameDir, folderName` | `void` |

Rules:

- Saves live under `<gameDir>/saves`. Each immediate child directory containing `level.dat` is a save.
- Parse gzip-compressed NBT `level.dat` defensively for `Data.LevelName`, `Data.LastPlayed`, `Data.GameType`, `Data.hardcore`, and `Data.Version.Name`.
- If `icon.png` exists, return a PNG data URL; otherwise `null`.
- `backup_save` writes a ZIP under `<gameDir>/backups/flint-saves` and returns its path. Export uses the caller-selected destination path.
- Import accepts a ZIP containing either one save folder or a top-level `level.dat`. Reject archive path traversal and duplicate/unsafe names.
- Folder names are validated to a single safe path component. Destructive operations must never follow symlinks outside the saves directory.
- Delete and overwrite operations require a UI confirmation. UI should display the selected instance and current game directory as context.

## Events

| Event | Payload |
| --- | --- |
| `flint://install-progress` | existing `InstallProgress` |
| `flint://loader-install-progress` | `LoaderInstallProgress` |
| `flint://mod-install-progress` | `ModInstallProgress` |

Event names and payload field shapes are unchanged. Provider-influenced loader and mod progress messages, and loader-triggered vanilla progress messages, are sanitized before publication: credential/URL assignments are redacted, control characters and whitespace are normalized, and messages are bounded to 512 Unicode scalar values. Progress delivery remains best-effort; command results and committed durable state are authoritative.

## UI expectations

- Sidebar account block opens an account dialog for offline/Microsoft account management, device-code login, account selection and removal.
- Settings exposes integration fields for the Microsoft application ID and CurseForge API key with clear local-storage configuration text.
- Downloads page gains tabs for loaders, Modrinth/CurseForge search, mod-version selection, dependency review and installed-mod management.
- A new save-management page is available in the sidebar and uses the currently selected instance and game directory.
- Existing offline launch, settings, instance discovery, vanilla install and launch functionality must not regress.

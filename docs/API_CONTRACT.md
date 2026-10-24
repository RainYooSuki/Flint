# Flint frontend/backend API contract

Flint is a Tauri v2 launcher. This file documents the base launcher commands; account, loader, mod, and save extensions are documented in [`FEATURE_API_CONTRACT.md`](FEATURE_API_CONTRACT.md). All Rust structs exposed to the frontend use `#[serde(rename_all = "camelCase")]`.

## Commands

| Command | Arguments | Result |
| --- | --- | --- |
| `launcher_info` | none | `LauncherInfo` |
| `get_host_availability` | none | `HostAvailabilitySnapshot` |
| `load_settings` | none | `LauncherSettings` |
| `save_settings` | `settings: LauncherSettings` | `LauncherSettings` |
| `list_instances` | `gameDir: string` | `InstanceInfo[]` |
| `scan_java` | none | `JavaInfo[]` |
| `fetch_remote_versions` | none | `RemoteVersion[]` |
| `install_version` | `gameDir: string`, `versionId: string` | `InstallResult` |
| `launch_instance` | `request: LaunchRequest` | `LaunchResult` |

`install_version` emits `flint://install-progress` events with an `InstallProgress` payload. A failed install returns an error string; the version directory may be safely retried.

## Host availability and command gating

`get_host_availability` is ungated and always returns the current read-only lifecycle snapshot, including when host startup fails. `launcher_info` is also ungated. The other 28 existing commands in this contract and in [`FEATURE_API_CONTRACT.md`](FEATURE_API_CONTRACT.md) are feature commands gated by their owning backend feature.

```ts
type HostLifecycleState =
  | "starting"
  | "ready"
  | "degraded"
  | "startup_failed"
  | "runtime_failed"
  | "stopping"
  | "stopped";

type FeatureLifecycleState =
  | "starting"
  | "ready"
  | "failed"
  | "blocked_dependency"
  | "stopped";

interface FeatureAvailability {
  featureId: string;
  state: FeatureLifecycleState;
  reason: string | null;
}

interface HostAvailabilitySnapshot {
  revision: number;
  hostState: HostLifecycleState;
  lastFailure: string | null;
  features: FeatureAvailability[];
}
```

Rust serializes this shape as `flint_feature_api::FeatureAvailabilitySnapshot`; the JSON field and enum names are the compatibility surface.

- The snapshot contains exactly one entry for each of the seven backend feature IDs: `flint.settings`, `flint.accounts`, `flint.vanilla`, `flint.launch`, `flint.loaders`, `flint.mods`, and `flint.saves`.
- Command ownership is fixed: `load_settings`/`save_settings` -> `flint.settings`; account commands -> `flint.accounts`; `list_instances`, `scan_java`, `fetch_remote_versions`, and `install_version` -> `flint.vanilla`; `launch_instance` -> `flint.launch`; loader commands -> `flint.loaders`; mod commands -> `flint.mods`; save commands -> `flint.saves`.
- `features` is lexicographically sorted by `featureId`, and `revision` increases for host/feature lifecycle-state or failure changes. Reasons and `lastFailure` are sanitized and never include provider IDs or secrets.
- A feature command succeeds only when `hostState` is `ready` or `degraded` and the owning feature state is `ready`. Otherwise it returns before I/O with `Feature '<featureId>' is unavailable: <reason>` and performs no fallback or partial work.
- A startup failure leaves all feature commands gated and returns a valid snapshot with `hostState: "startup_failed"`. Normal application exit performs idempotent reverse-order feature/provider shutdown.

## Models

```ts
interface LauncherInfo {
  version: string;
  platform: string;
  dataDir: string;
  defaultGameDir: string;
  settingsPath: string;
}

interface LauncherSettings {
  gameDir: string;
  javaPath: string;
  username: string;
  minMemoryMb: number;
  maxMemoryMb: number;
  jvmArgs: string;
  gameArgs: string;
  width: number;
  height: number;
}

interface InstanceInfo {
  id: string;
  name: string;
  versionType: string;
  loader: string;
  path: string;
  hasClientJar: boolean;
  lastPlayed: string | null;
}

interface JavaInfo {
  path: string;
  version: string;
  major: number;
  architecture: string;
}

interface RemoteVersion {
  id: string;
  versionType: string;
  releaseTime: string;
}

interface InstallProgress {
  stage: string;
  completed: number;
  total: number;
  message: string;
}

interface InstallResult {
  versionId: string;
  filesWritten: number;
  bytesDownloaded: number;
  installPath: string;
}

interface LaunchRequest {
  gameDir: string;
  instanceId: string;
  javaPath: string;
  username: string;
  minMemoryMb: number;
  maxMemoryMb: number;
  jvmArgs: string;
  gameArgs: string;
  width: number;
  height: number;
}

interface LaunchResult {
  pid: number;
  command: string;
  logPath: string;
}
```

## Behavioural rules

- An empty `gameDir` means the platform-default `.minecraft` path.
- Settings are stored at `LauncherInfo.settingsPath` and defaults are created when absent.
- Offline usernames are validated to 3-16 characters matching `[A-Za-z0-9_]`.
- Instance discovery reads `versions/<id>/<id>.json`; inherited version metadata must be resolved before launch.
- Modern platform/OS rules in libraries are honoured, duplicate classpath entries are removed, and native jars are extracted to a per-instance `natives` directory.
- Legacy `launchwrapper`/`minecraftArguments` versions remain launchable where their files exist.
- Install downloads Mojang's version metadata, client jar, declared libraries, asset index and asset objects into the selected game directory. It must use the Mojang `resources.download.minecraft.net` object layout.
- Launch output is redirected to a log file; `logPath` points at that file. Linux and macOS use `:` in classpaths, Windows uses `;`.
- All user-facing backend errors are concise English or Chinese strings suitable for display; no panic on malformed local files.
- Unit tests cover pure helpers and must not access the network.

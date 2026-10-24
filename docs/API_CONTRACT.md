# Flint frontend/backend API contract

Flint is a Tauri v2 launcher. This file documents the base launcher commands; account, loader, mod, and save extensions are documented in [`FEATURE_API_CONTRACT.md`](FEATURE_API_CONTRACT.md). All Rust structs exposed to the frontend use `#[serde(rename_all = "camelCase")]`.

## Commands

| Command | Arguments | Result |
| --- | --- | --- |
| `launcher_info` | none | `LauncherInfo` |
| `load_settings` | none | `LauncherSettings` |
| `save_settings` | `settings: LauncherSettings` | `LauncherSettings` |
| `list_instances` | `gameDir: string` | `InstanceInfo[]` |
| `scan_java` | none | `JavaInfo[]` |
| `fetch_remote_versions` | none | `RemoteVersion[]` |
| `install_version` | `gameDir: string`, `versionId: string` | `InstallResult` |
| `launch_instance` | `request: LaunchRequest` | `LaunchResult` |

`install_version` emits `flint://install-progress` events with an `InstallProgress` payload. A failed install returns an error string; the version directory may be safely retried.

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

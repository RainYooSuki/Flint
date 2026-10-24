import { invoke } from "@tauri-apps/api/core";
import type {
  InstallResult,
  InstanceInfo,
  JavaInfo,
  LauncherInfo,
  LauncherSettings,
  LaunchRequest,
  LaunchResult,
  RemoteVersion
} from "./types";

export const api = {
  launcherInfo(): Promise<LauncherInfo> {
    return invoke<LauncherInfo>("launcher_info");
  },

  loadSettings(): Promise<LauncherSettings> {
    return invoke<LauncherSettings>("load_settings");
  },

  saveSettings(settings: LauncherSettings): Promise<LauncherSettings> {
    return invoke<LauncherSettings>("save_settings", { settings });
  },

  listInstances(gameDir: string): Promise<InstanceInfo[]> {
    return invoke<InstanceInfo[]>("list_instances", { gameDir });
  },

  scanJava(): Promise<JavaInfo[]> {
    return invoke<JavaInfo[]>("scan_java");
  },

  fetchRemoteVersions(): Promise<RemoteVersion[]> {
    return invoke<RemoteVersion[]>("fetch_remote_versions");
  },

  installVersion(gameDir: string, versionId: string): Promise<InstallResult> {
    return invoke<InstallResult>("install_version", { gameDir, versionId });
  },

  launchInstance(request: LaunchRequest): Promise<LaunchResult> {
    return invoke<LaunchResult>("launch_instance", { request });
  }
};
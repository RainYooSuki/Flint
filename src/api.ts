import { invoke } from "@tauri-apps/api/core";
import type {
  AccountInfo,
  HostAvailabilitySnapshot,
  InstallResult,
  InstanceInfo,
  InstalledModInfo,
  JavaInfo,
  LauncherInfo,
  LauncherSettings,
  LaunchRequest,
  LaunchResult,
  LoaderInstallRequest,
  LoaderInstallResult,
  LoaderKind,
  LoaderVersionInfo,
  MicrosoftDeviceCode,
  MicrosoftLoginPoll,
  ModDependencyPlan,
  ModInstallResult,
  ModSearchPage,
  ModSource,
  ModVersionInfo,
  RemoteVersion,
  SaveInfo,
  SaveTransferResult
} from "./types";
import { toErrorMessage } from "./utils";

export type FeatureUnavailableListener = () => void;

const featureUnavailableListeners = new Set<FeatureUnavailableListener>();
const UNAVAILABLE_FEATURE_PATTERN = /\bfeature\b[^\r\n]*\bunavailable\b|\bunavailable\s+feature\b/i;

export function isUnavailableFeatureError(error: unknown): boolean {
  return UNAVAILABLE_FEATURE_PATTERN.test(toErrorMessage(error));
}

/** Shared subscription for refreshing availability after a rejected command. */
export function subscribeFeatureUnavailable(listener: FeatureUnavailableListener): () => void {
  featureUnavailableListeners.add(listener);
  return () => {
    featureUnavailableListeners.delete(listener);
  };
}

async function invokeWithAvailabilityRefresh<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    if (isUnavailableFeatureError(error)) {
      for (const listener of [...featureUnavailableListeners]) {
        try {
          listener();
        } catch (listenerError) {
          console.error("Feature availability listener failed.", listenerError);
        }
      }
    }
    throw error;
  }
}

export const api = {
  launcherInfo(): Promise<LauncherInfo> {
    return invokeWithAvailabilityRefresh<LauncherInfo>("launcher_info");
  },

  getHostAvailability(): Promise<HostAvailabilitySnapshot> {
    return invoke<HostAvailabilitySnapshot>("get_host_availability");
  },

  loadSettings(): Promise<LauncherSettings> {
    return invokeWithAvailabilityRefresh<LauncherSettings>("load_settings");
  },

  saveSettings(settings: LauncherSettings): Promise<LauncherSettings> {
    return invokeWithAvailabilityRefresh<LauncherSettings>("save_settings", { settings });
  },

  listInstances(gameDir: string): Promise<InstanceInfo[]> {
    return invokeWithAvailabilityRefresh<InstanceInfo[]>("list_instances", { gameDir });
  },

  scanJava(): Promise<JavaInfo[]> {
    return invokeWithAvailabilityRefresh<JavaInfo[]>("scan_java");
  },

  fetchRemoteVersions(): Promise<RemoteVersion[]> {
    return invokeWithAvailabilityRefresh<RemoteVersion[]>("fetch_remote_versions");
  },

  installVersion(gameDir: string, versionId: string): Promise<InstallResult> {
    return invokeWithAvailabilityRefresh<InstallResult>("install_version", { gameDir, versionId });
  },

  launchInstance(request: LaunchRequest): Promise<LaunchResult> {
    return invokeWithAvailabilityRefresh<LaunchResult>("launch_instance", { request });
  },

  listAccounts(): Promise<AccountInfo[]> {
    return invokeWithAvailabilityRefresh<AccountInfo[]>("list_accounts");
  },

  beginMicrosoftLogin(): Promise<MicrosoftDeviceCode> {
    return invokeWithAvailabilityRefresh<MicrosoftDeviceCode>("begin_microsoft_login");
  },

  pollMicrosoftLogin(deviceCode: string): Promise<MicrosoftLoginPoll> {
    return invokeWithAvailabilityRefresh<MicrosoftLoginPoll>("poll_microsoft_login", { deviceCode });
  },

  selectAccount(accountId: string): Promise<AccountInfo[]> {
    return invokeWithAvailabilityRefresh<AccountInfo[]>("select_account", { accountId });
  },

  removeAccount(accountId: string): Promise<AccountInfo[]> {
    return invokeWithAvailabilityRefresh<AccountInfo[]>("remove_account", { accountId });
  },

  refreshActiveAccount(): Promise<AccountInfo> {
    return invokeWithAvailabilityRefresh<AccountInfo>("refresh_active_account");
  },

  listLoaderVersions(loader: LoaderKind, gameVersion: string): Promise<LoaderVersionInfo[]> {
    return invokeWithAvailabilityRefresh<LoaderVersionInfo[]>("list_loader_versions", { loader, gameVersion });
  },

  installLoader(request: LoaderInstallRequest): Promise<LoaderInstallResult> {
    return invokeWithAvailabilityRefresh<LoaderInstallResult>("install_loader", { request });
  },

  searchMods(
    source: ModSource,
    query: string,
    gameVersion: string,
    loader: string,
    offset: number,
    limit: number
  ): Promise<ModSearchPage> {
    return invokeWithAvailabilityRefresh<ModSearchPage>("search_mods", { source, query, gameVersion, loader, offset, limit });
  },

  listModVersions(
    source: ModSource,
    projectId: string,
    gameVersion: string,
    loader: string
  ): Promise<ModVersionInfo[]> {
    return invokeWithAvailabilityRefresh<ModVersionInfo[]>("list_mod_versions", { source, projectId, gameVersion, loader });
  },

  resolveModDependencies(
    source: ModSource,
    projectId: string,
    versionId: string,
    gameVersion: string,
    loader: string
  ): Promise<ModDependencyPlan> {
    return invokeWithAvailabilityRefresh<ModDependencyPlan>("resolve_mod_dependencies", { source, projectId, versionId, gameVersion, loader });
  },

  installMod(
    gameDir: string,
    source: ModSource,
    projectId: string,
    versionId: string,
    gameVersion: string,
    loader: string,
    installDependencies: boolean
  ): Promise<ModInstallResult> {
    return invokeWithAvailabilityRefresh<ModInstallResult>("install_mod", {
      gameDir,
      source,
      projectId,
      versionId,
      gameVersion,
      loader,
      installDependencies
    });
  },

  listInstalledMods(gameDir: string): Promise<InstalledModInfo[]> {
    return invokeWithAvailabilityRefresh<InstalledModInfo[]>("list_installed_mods", { gameDir });
  },

  setModEnabled(gameDir: string, fileName: string, enabled: boolean): Promise<InstalledModInfo> {
    return invokeWithAvailabilityRefresh<InstalledModInfo>("set_mod_enabled", { gameDir, fileName, enabled });
  },

  deleteMod(gameDir: string, fileName: string): Promise<void> {
    return invokeWithAvailabilityRefresh<void>("delete_mod", { gameDir, fileName });
  },

  listSaves(gameDir: string): Promise<SaveInfo[]> {
    return invokeWithAvailabilityRefresh<SaveInfo[]>("list_saves", { gameDir });
  },

  backupSave(gameDir: string, folderName: string): Promise<SaveTransferResult> {
    return invokeWithAvailabilityRefresh<SaveTransferResult>("backup_save", { gameDir, folderName });
  },

  exportSave(gameDir: string, folderName: string, destination: string): Promise<SaveTransferResult> {
    return invokeWithAvailabilityRefresh<SaveTransferResult>("export_save", { gameDir, folderName, destination });
  },

  importSave(gameDir: string, archivePath: string): Promise<SaveInfo> {
    return invokeWithAvailabilityRefresh<SaveInfo>("import_save", { gameDir, archivePath });
  },

  renameSave(gameDir: string, folderName: string, newName: string): Promise<SaveInfo> {
    return invokeWithAvailabilityRefresh<SaveInfo>("rename_save", { gameDir, folderName, newName });
  },

  deleteSave(gameDir: string, folderName: string): Promise<void> {
    return invokeWithAvailabilityRefresh<void>("delete_save", { gameDir, folderName });
  }
};

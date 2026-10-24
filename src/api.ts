import { invoke } from "@tauri-apps/api/core";
import type {
  AccountInfo,
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
  },

  listAccounts(): Promise<AccountInfo[]> {
    return invoke<AccountInfo[]>("list_accounts");
  },

  beginMicrosoftLogin(): Promise<MicrosoftDeviceCode> {
    return invoke<MicrosoftDeviceCode>("begin_microsoft_login");
  },

  pollMicrosoftLogin(deviceCode: string): Promise<MicrosoftLoginPoll> {
    return invoke<MicrosoftLoginPoll>("poll_microsoft_login", { deviceCode });
  },

  selectAccount(accountId: string): Promise<AccountInfo[]> {
    return invoke<AccountInfo[]>("select_account", { accountId });
  },

  removeAccount(accountId: string): Promise<AccountInfo[]> {
    return invoke<AccountInfo[]>("remove_account", { accountId });
  },

  refreshActiveAccount(): Promise<AccountInfo> {
    return invoke<AccountInfo>("refresh_active_account");
  },

  listLoaderVersions(loader: LoaderKind, gameVersion: string): Promise<LoaderVersionInfo[]> {
    return invoke<LoaderVersionInfo[]>("list_loader_versions", { loader, gameVersion });
  },

  installLoader(request: LoaderInstallRequest): Promise<LoaderInstallResult> {
    return invoke<LoaderInstallResult>("install_loader", { request });
  },

  searchMods(
    source: ModSource,
    query: string,
    gameVersion: string,
    loader: string,
    offset: number,
    limit: number
  ): Promise<ModSearchPage> {
    return invoke<ModSearchPage>("search_mods", { source, query, gameVersion, loader, offset, limit });
  },

  listModVersions(
    source: ModSource,
    projectId: string,
    gameVersion: string,
    loader: string
  ): Promise<ModVersionInfo[]> {
    return invoke<ModVersionInfo[]>("list_mod_versions", { source, projectId, gameVersion, loader });
  },

  resolveModDependencies(
    source: ModSource,
    projectId: string,
    versionId: string,
    gameVersion: string,
    loader: string
  ): Promise<ModDependencyPlan> {
    return invoke<ModDependencyPlan>("resolve_mod_dependencies", { source, projectId, versionId, gameVersion, loader });
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
    return invoke<ModInstallResult>("install_mod", {
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
    return invoke<InstalledModInfo[]>("list_installed_mods", { gameDir });
  },

  setModEnabled(gameDir: string, fileName: string, enabled: boolean): Promise<InstalledModInfo> {
    return invoke<InstalledModInfo>("set_mod_enabled", { gameDir, fileName, enabled });
  },

  deleteMod(gameDir: string, fileName: string): Promise<void> {
    return invoke<void>("delete_mod", { gameDir, fileName });
  },

  listSaves(gameDir: string): Promise<SaveInfo[]> {
    return invoke<SaveInfo[]>("list_saves", { gameDir });
  },

  backupSave(gameDir: string, folderName: string): Promise<SaveTransferResult> {
    return invoke<SaveTransferResult>("backup_save", { gameDir, folderName });
  },

  exportSave(gameDir: string, folderName: string, destination: string): Promise<SaveTransferResult> {
    return invoke<SaveTransferResult>("export_save", { gameDir, folderName, destination });
  },

  importSave(gameDir: string, archivePath: string): Promise<SaveInfo> {
    return invoke<SaveInfo>("import_save", { gameDir, archivePath });
  },

  renameSave(gameDir: string, folderName: string, newName: string): Promise<SaveInfo> {
    return invoke<SaveInfo>("rename_save", { gameDir, folderName, newName });
  },

  deleteSave(gameDir: string, folderName: string): Promise<void> {
    return invoke<void>("delete_save", { gameDir, folderName });
  }
};
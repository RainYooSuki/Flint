export type SectionId = "launch" | "versions" | "saves" | "settings" | "about";

export interface LauncherInfo {
  version: string;
  platform: string;
  dataDir: string;
  defaultGameDir: string;
  settingsPath: string;
}

export interface LauncherSettings {
  gameDir: string;
  javaPath: string;
  username: string;
  minMemoryMb: number;
  maxMemoryMb: number;
  jvmArgs: string;
  gameArgs: string;
  width: number;
  height: number;
  microsoftClientId: string;
  curseforgeApiKey: string;
}

export interface InstanceInfo {
  id: string;
  name: string;
  versionType: string;
  loader: string;
  path: string;
  hasClientJar: boolean;
  lastPlayed: string | null;
  gameVersion: string;
}

export interface JavaInfo {
  path: string;
  version: string;
  major: number;
  architecture: string;
}

export interface RemoteVersion {
  id: string;
  versionType: string;
  releaseTime: string;
}

export interface InstallProgress {
  stage: string;
  completed: number;
  total: number;
  message: string;
}

export interface InstallResult {
  versionId: string;
  filesWritten: number;
  bytesDownloaded: number;
  installPath: string;
}

export interface LaunchRequest {
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

export interface LaunchResult {
  pid: number;
  command: string;
  logPath: string;
}

export type AccountType = "offline" | "microsoft";
export type LoginStatus = "pending" | "success" | "error";

export interface AccountInfo {
  id: string;
  username: string;
  uuid: string | null;
  accountType: AccountType;
  active: boolean;
  skinUrl: string | null;
  expiresAt: number | null;
}

export interface MicrosoftDeviceCode {
  deviceCode: string;
  userCode: string;
  verificationUri: string;
  expiresIn: number;
  interval: number;
  message: string;
}

export interface MicrosoftLoginPoll {
  status: LoginStatus;
  message: string;
  account: AccountInfo | null;
  slowDown: boolean;
}

export type LoaderKind = "forge" | "neoforge" | "fabric" | "quilt" | "liteloader" | "optifine";

export interface LoaderVersionInfo {
  loader: LoaderKind;
  version: string;
  label: string;
  recommended: boolean;
  gameVersion: string;
  releaseTime: string | null;
}

export interface LoaderInstallRequest {
  gameDir: string;
  gameVersion: string;
  loader: LoaderKind;
  loaderVersion: string;
  javaPath: string;
}

export interface LoaderInstallResult {
  gameDir: string;
  gameVersion: string;
  loader: LoaderKind;
  loaderVersion: string;
  installedVersionId: string;
  installPath: string;
  logPath: string | null;
}

export interface LoaderInstallProgress {
  loader: LoaderKind;
  gameVersion: string;
  loaderVersion: string;
  stage: string;
  completed: number;
  total: number;
  message: string;
}

export type ModSource = "curseforge" | "modrinth";
export type ModDependencyType = "required" | "optional" | "embedded" | "incompatible";

export interface ModProject {
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

export interface ModSearchPage {
  source: ModSource;
  total: number;
  offset: number;
  limit: number;
  hits: ModProject[];
}

export interface ModDependencyRef {
  source: ModSource;
  projectId: string;
  versionId: string | null;
  name: string;
  dependencyType: ModDependencyType;
}

export interface ModVersionInfo {
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

export interface ModDependencyPlan {
  root: ModVersionInfo;
  required: ModVersionInfo[];
  optional: ModDependencyRef[];
  conflicts: string[];
  warnings: string[];
}

export interface InstalledModInfo {
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

export interface ModInstallProgress {
  projectName: string;
  fileName: string;
  stage: string;
  completed: number;
  total: number;
  message: string;
}

export interface ModInstallResult {
  root: InstalledModInfo;
  dependencies: InstalledModInfo[];
  skipped: string[];
  warnings: string[];
}

export interface SaveInfo {
  folderName: string;
  displayName: string;
  path: string;
  iconDataUrl: string | null;
  lastPlayed: number | null;
  sizeBytes: number;
  gameMode: string | null;
  hardcore: boolean;
  versionName: string | null;
}

export interface SaveTransferResult {
  path: string;
  save: SaveInfo | null;
}

export type NoticeKind = "info" | "success" | "error";

export interface Notice {
  id: number;
  kind: NoticeKind;
  text: string;
}

export interface InstallRuntime {
  versionId: string;
  progress: InstallProgress | null;
}

export type HostLifecycleState = "starting" | "ready" | "degraded" | "startup_failed" | "runtime_failed" | "stopping" | "stopped";

export type FeatureLifecycleState = "starting" | "ready" | "failed" | "blocked_dependency" | "stopped";

export interface FeatureAvailability {
  featureId: string;
  state: FeatureLifecycleState;
  reason: string | null;
}

export interface HostAvailabilitySnapshot {
  revision: number;
  hostState: HostLifecycleState;
  lastFailure: string | null;
  features: FeatureAvailability[];
}

export type SectionId = "launch" | "versions" | "settings" | "about";

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
}

export interface InstanceInfo {
  id: string;
  name: string;
  versionType: string;
  loader: string;
  path: string;
  hasClientJar: boolean;
  lastPlayed: string | null;
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
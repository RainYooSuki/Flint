import type {
  AccountInfo,
  FeatureLifecycleState,
  HostLifecycleState,
  InstallResult,
  InstallRuntime,
  InstanceInfo,
  JavaInfo,
  LauncherInfo,
  LauncherSettings,
  LaunchResult,
  NoticeKind,
  RemoteVersion
} from "../types";
import type { InstanceActionContribution, SettingsSectionContribution } from "./contributions";
import type { HostServices } from "./platform/hostServices";

/**
 * The shell-owned state and actions exposed to page contributions.
 *
 * The shell owns shared state, injects typed host services, and exposes only
 * the availability-filtered, frozen contribution lists. Feature pages and
 * sections receive this explicit typed contract instead of importing shell
 * internals or another feature module.
 */
export interface AppPageContext {
  readonly navigateToPage: (targetPageId: string) => void;
  readonly services: HostServices;
  readonly hostState: HostLifecycleState | null;
  readonly featureStates: Readonly<Record<string, FeatureLifecycleState | undefined>>;
  readonly settingsSections: readonly SettingsSectionContribution[];
  readonly instanceActions: readonly InstanceActionContribution[];

  readonly info: LauncherInfo | null;
  readonly infoLoading: boolean;
  readonly infoError: string | null;

  readonly settings: LauncherSettings | null;
  readonly settingsDraft: LauncherSettings | null;
  readonly settingsLoading: boolean;
  readonly settingsError: string | null;
  readonly savingSettings: boolean;
  readonly settingsDirty: boolean;

  readonly javas: JavaInfo[];
  readonly javaLoading: boolean;
  readonly javaError: string | null;

  readonly instances: InstanceInfo[];
  readonly selectedId: string;
  readonly selectedInstance: InstanceInfo | null;
  readonly instancesLoading: boolean;
  readonly instancesError: string | null;

  readonly remoteVersions: RemoteVersion[];
  readonly remoteLoading: boolean;
  readonly remoteError: string | null;
  readonly remoteFetched: boolean;
  readonly installRuntime: InstallRuntime | null;
  readonly installResult: InstallResult | null;

  readonly launching: boolean;
  readonly launchResult: LaunchResult | null;
  readonly launchStartedAt: string | null;
  readonly activeAccount: AccountInfo | null;

  readonly gameDirPath: string;
  readonly gameDirLabel: string;

  readonly accounts: AccountInfo[];
  readonly accountsLoading: boolean;
  readonly accountsError: string | null;
  readonly accountDialogOpen: boolean;

  readonly onLaunch: () => void;
  readonly onSelectInstance: (id: string) => void;
  readonly onRefreshInstances: () => void;
  readonly onFetchRemote: () => void;
  readonly onInstall: (version: RemoteVersion) => void;
  readonly onOpenPath: (path: string, label: string) => void;
  readonly onNotify: (kind: NoticeKind, message: string) => void;

  readonly onChangeSettings: (settings: LauncherSettings) => void;
  readonly onSaveSettings: () => void;
  readonly onReloadSettings: () => void;
  readonly onResetSettings: () => void;
  readonly onPickGameDir: () => void;
  readonly onPickJava: () => void;
  readonly onScanJava: () => void;
  readonly onRetryInfo: () => void;

  readonly onCloseAccountDialog: () => void;
  readonly onAccountsChanged: (accounts: AccountInfo[]) => void;
  readonly onRefreshAccounts: () => Promise<void>;
}

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { ReactNode } from "react";
import { api, subscribeFeatureUnavailable } from "../api";
import { Icon } from "../components/Icon";
import { TitleBar } from "../components/TitleBar";
import type { AppPageContext } from "../core/app-context";
import {
  evaluateAvailability,
  filterContributionRegistry,
  isBackendFeatureReady,
  sanitizeAvailabilityReason,
  unavailableFeatureDiagnostics
} from "../core/availability";
import type { AvailabilityDiagnostic } from "../core/availability";
import { ContributionBoundary } from "../core/ContributionBoundary";
import { useHostEvent } from "../core/events/useHostEvent";
import type { HostServices } from "../core/platform/hostServices";
import { useRequestGeneration } from "../core/requests/useRequestGeneration";
import type {
  AccountInfo,
  HostAvailabilitySnapshot,
  InstallResult,
  InstallRuntime,
  InstanceInfo,
  JavaInfo,
  LauncherInfo,
  LauncherSettings,
  LaunchRequest,
  LaunchResult,
  Notice,
  NoticeKind,
  RemoteVersion
} from "../types";
import {
  formatBytes,
  platformLabel,
  settingsEqual,
  toErrorMessage,
  validateUsername
} from "../utils";
import { AppSidebar } from "./AppSidebar";
import { frontendRegistry } from "./composition";

const STARTING_AVAILABILITY_REFRESH_INTERVAL_MS = 2_000;
const STARTING_AVAILABILITY_REFRESH_LIMIT = 5;

interface AppProps {
  readonly services: HostServices;
}

interface AvailabilitySurfaceProps {
  title: string;
  message: string;
  details?: readonly AvailabilityDiagnostic[];
  lastFailure?: string | null;
  loading?: boolean;
  error?: boolean;
  onRetry: () => void;
}

function AvailabilitySurface({
  title,
  message,
  details = [],
  lastFailure = null,
  loading = false,
  error = false,
  onRetry
}: AvailabilitySurfaceProps) {
  const detailText = details
    .map((detail) => detail.reason ? `${detail.featureId}: ${detail.reason}` : detail.featureId)
    .join("; ");
  const parts = [detailText, lastFailure ? `Last failure: ${lastFailure}` : ""].filter(Boolean);
  const bodyText = parts.length > 0 ? parts.join(" ") : message;

  return (
    <div className="page-shell">
      <header className="page-head">
        <div className="page-title">
          <h1>{title}</h1>
          <span>{message}</span>
        </div>
      </header>
      <div className="page-body">
        <div
          className={`inline-message ${error ? "error" : "warning"}`}
          role={error ? "alert" : "status"}
          aria-live={error ? "assertive" : "polite"}
        >
          {loading ? <span className="spinner spinner-small" /> : <Icon name="alert" />}
          <span>{bodyText}</span>
          <button type="button" className="text-link" onClick={onRetry} disabled={loading}>
            Retry
          </button>
        </div>
      </div>
    </div>
  );
}

export default function App({ services }: AppProps) {
  const [activePageId, setActivePageId] = useState("");
  const [availabilitySnapshot, setAvailabilitySnapshot] = useState<HostAvailabilitySnapshot | null>(null);
  const [availabilityLoading, setAvailabilityLoading] = useState(true);
  const [availabilityError, setAvailabilityError] = useState<string | null>(null);
  const availability = useMemo(() => evaluateAvailability(availabilitySnapshot), [availabilitySnapshot]);
  const availableRegistry = useMemo(
    () => filterContributionRegistry(frontendRegistry, availability),
    [availability]
  );
  const availabilityDiagnostics = useMemo(
    () => unavailableFeatureDiagnostics(availability, frontendRegistry.features),
    [availability]
  );
  const activePage = availableRegistry.pageById[activePageId] ?? availableRegistry.pages[0] ?? null;
  const [info, setInfo] = useState<LauncherInfo | null>(null);
  const [infoLoading, setInfoLoading] = useState(true);
  const [infoError, setInfoError] = useState<string | null>(null);
  const [settings, setSettings] = useState<LauncherSettings | null>(null);
  const [settingsDraft, setSettingsDraft] = useState<LauncherSettings | null>(null);
  const [settingsLoading, setSettingsLoading] = useState(true);
  const [settingsError, setSettingsError] = useState<string | null>(null);
  const [savingSettings, setSavingSettings] = useState(false);
  const [javas, setJavas] = useState<JavaInfo[]>([]);
  const [javaLoading, setJavaLoading] = useState(true);
  const [javaError, setJavaError] = useState<string | null>(null);
  const [instances, setInstances] = useState<InstanceInfo[]>([]);
  const [instancesLoading, setInstancesLoading] = useState(true);
  const [instancesError, setInstancesError] = useState<string | null>(null);
  const [selectedId, setSelectedId] = useState("");
  const [remoteVersions, setRemoteVersions] = useState<RemoteVersion[]>([]);
  const [remoteLoading, setRemoteLoading] = useState(false);
  const [remoteError, setRemoteError] = useState<string | null>(null);
  const [remoteFetched, setRemoteFetched] = useState(false);
  const [installRuntime, setInstallRuntime] = useState<InstallRuntime | null>(null);
  const [installResult, setInstallResult] = useState<InstallResult | null>(null);
  const [progressListenerError, setProgressListenerError] = useState<string | null>(null);
  const [launching, setLaunching] = useState(false);
  const [launchResult, setLaunchResult] = useState<LaunchResult | null>(null);
  const [launchStartedAt, setLaunchStartedAt] = useState<string | null>(null);
  const [notice, setNotice] = useState<Notice | null>(null);
  const [accounts, setAccounts] = useState<AccountInfo[]>([]);
  const [accountsLoading, setAccountsLoading] = useState(true);
  const [accountsError, setAccountsError] = useState<string | null>(null);
  const [accountDialogOpen, setAccountDialogOpen] = useState(false);

  const noticeIdRef = useRef(0);
  const contentRef = useRef<HTMLElement>(null);
  const settingsRef = useRef<LauncherSettings | null>(null);
  const availabilityRequests = useRequestGeneration();
  const instancesRequests = useRequestGeneration();
  const accountsRequests = useRequestGeneration();

  const pushNotice = useCallback((kind: NoticeKind, text: string) => {
    noticeIdRef.current += 1;
    setNotice({ id: noticeIdRef.current, kind, text });
  }, []);

  const refreshAvailability = useCallback(async (): Promise<void> => {
    const requestId = availabilityRequests.next();
    setAvailabilityLoading(true);
    setAvailabilityError(null);
    try {
      const snapshot = await api.getHostAvailability();
      if (!availabilityRequests.isCurrent(requestId)) return;
      setAvailabilitySnapshot(snapshot);
    } catch (error) {
      if (!availabilityRequests.isCurrent(requestId)) return;
      setAvailabilitySnapshot(null);
      setAvailabilityError(sanitizeAvailabilityReason(toErrorMessage(error)));
    } finally {
      if (availabilityRequests.isCurrent(requestId)) setAvailabilityLoading(false);
    }
  }, [availabilityRequests]);

  useEffect(() => {
    void refreshAvailability();
    const handleFocus = () => {
      void refreshAvailability();
    };
    const unsubscribe = subscribeFeatureUnavailable(() => {
      void refreshAvailability();
    });
    window.addEventListener("focus", handleFocus);
    return () => {
      availabilityRequests.invalidate();
      window.removeEventListener("focus", handleFocus);
      unsubscribe();
    };
  }, [refreshAvailability, availabilityRequests]);

  const retryAvailability = useCallback(() => {
    void refreshAvailability();
  }, [refreshAvailability]);

  useEffect(() => {
    if (availability.hostState !== "starting") return;
    let cancelled = false;
    let attempts = 0;
    let timer: number | null = null;
    const scheduleRefresh = () => {
      if (cancelled || attempts >= STARTING_AVAILABILITY_REFRESH_LIMIT) return;
      timer = window.setTimeout(() => {
        timer = null;
        if (cancelled) return;
        attempts += 1;
        void refreshAvailability()
          .catch((error: unknown) => {
            console.error("Availability poll refresh failed.", error);
          })
          .finally(() => {
            if (!cancelled) scheduleRefresh();
          });
      }, STARTING_AVAILABILITY_REFRESH_INTERVAL_MS);
    };
    scheduleRefresh();
    return () => {
      cancelled = true;
      if (timer !== null) window.clearTimeout(timer);
    };
  }, [availability.hostState, refreshAvailability]);

  const refreshInstances = useCallback(async (gameDir: string, announce = false): Promise<InstanceInfo[] | null> => {
    const requestId = instancesRequests.next();
    setInstancesLoading(true);
    setInstancesError(null);
    try {
      const next = await api.listInstances(gameDir);
      if (!instancesRequests.isCurrent(requestId)) return null;
      setInstances(next);
      setSelectedId((current) => (
        next.some((instance) => instance.id === current) ? current : next[0]?.id ?? ""
      ));
      if (announce) pushNotice("success", `本地版本扫描完成，共发现 ${next.length} 个版本。`);
      return next;
    } catch (error) {
      if (!instancesRequests.isCurrent(requestId)) return null;
      const message = toErrorMessage(error);
      setInstancesError(message);
      if (announce) pushNotice("error", `本地版本扫描失败：${message}`);
      return null;
    } finally {
      if (instancesRequests.isCurrent(requestId)) setInstancesLoading(false);
    }
  }, [instancesRequests, pushNotice]);

  const refreshAccounts = useCallback(async (announce = false): Promise<void> => {
    const requestId = accountsRequests.next();
    setAccountsLoading(true);
    setAccountsError(null);
    try {
      const next = await api.listAccounts();
      if (!accountsRequests.isCurrent(requestId)) return;
      setAccounts(next);
      if (announce) {
        const active = next.find((account) => account.active);
        pushNotice("success", active ? `账户已刷新：${active.username}。` : "账户列表已刷新。");
      }
    } catch (error) {
      if (!accountsRequests.isCurrent(requestId)) return;
      const message = toErrorMessage(error);
      setAccountsError(message);
      if (announce) pushNotice("error", `账户列表读取失败：${message}`);
    } finally {
      if (accountsRequests.isCurrent(requestId)) setAccountsLoading(false);
    }
  }, [accountsRequests, pushNotice]);

  const applyAccounts = useCallback((next: AccountInfo[]) => {
    accountsRequests.invalidate();
    setAccounts(next);
    setAccountsError(null);
    setAccountsLoading(false);
  }, [accountsRequests]);

  useEffect(() => {
    void refreshAccounts();
  }, [refreshAccounts]);

  useEffect(() => {
    let cancelled = false;

    async function bootstrap() {
      setSettingsLoading(true);
      setJavaLoading(true);
      setInfoLoading(true);
      setInstancesLoading(true);

      const [infoResult, settingsResult, javaResult] = await Promise.allSettled([
        api.launcherInfo(),
        api.loadSettings(),
        api.scanJava()
      ]);

      if (cancelled) return;

      let gameDir = "";

      if (infoResult.status === "fulfilled") {
        setInfo(infoResult.value);
        setInfoError(null);
      } else {
        setInfoError(toErrorMessage(infoResult.reason));
      }
      setInfoLoading(false);

      if (settingsResult.status === "fulfilled") {
        gameDir = settingsResult.value.gameDir;
        setSettings(settingsResult.value);
        settingsRef.current = settingsResult.value;
        setSettingsDraft(settingsResult.value);
        setSettingsError(null);
      } else {
        setSettingsError(toErrorMessage(settingsResult.reason));
      }
      setSettingsLoading(false);

      if (javaResult.status === "fulfilled") {
        setJavas(javaResult.value);
        setJavaError(null);
      } else {
        setJavaError(toErrorMessage(javaResult.reason));
      }
      setJavaLoading(false);

      const instancesRequestId = instancesRequests.next();
      setInstancesError(null);
      try {
        const next = await api.listInstances(gameDir);
        if (cancelled || !instancesRequests.isCurrent(instancesRequestId)) return;
        setInstances(next);
        setSelectedId(next[0]?.id ?? "");
      } catch (error) {
        if (cancelled || !instancesRequests.isCurrent(instancesRequestId)) return;
        setInstancesError(toErrorMessage(error));
      } finally {
        if (!cancelled && instancesRequests.isCurrent(instancesRequestId)) {
          setInstancesLoading(false);
        }
      }
    }

    void bootstrap().catch((error: unknown) => {
      if (cancelled) return;
      const message = toErrorMessage(error);
      setInfoError(message);
      setSettingsError(message);
      setJavaError(message);
      setInstancesError(message);
      setInfoLoading(false);
      setSettingsLoading(false);
      setJavaLoading(false);
      setInstancesLoading(false);
    });

    return () => {
      cancelled = true;
    };
  }, [instancesRequests]);

  useHostEvent(
    services.events,
    "installProgress",
    (progress) => {
      setInstallRuntime((current) => current ? { ...current, progress } : current);
    },
    (error) => setProgressListenerError(toErrorMessage(error))
  );

  useEffect(() => {
    settingsRef.current = settings;
  }, [settings]);

  useEffect(() => {
    if (!notice) return;
    const timer = window.setTimeout(() => {
      setNotice((current) => current?.id === notice.id ? null : current);
    }, notice.kind === "error" ? 7000 : 4500);
    return () => window.clearTimeout(timer);
  }, [notice]);

  useEffect(() => {
    document.querySelectorAll<HTMLElement>(".page-body, .settings-scroll, .launch-content, .runtime-list, .quick-list").forEach((element) => { element.scrollTop = 0; });
  }, [activePageId]);

  useEffect(() => {
    if (availabilityLoading) return;
    setActivePageId((current) => (
      current && availableRegistry.pageById[current]
        ? current
        : availableRegistry.defaultPageId ?? ""
    ));
  }, [availabilityLoading, availableRegistry]);

  useEffect(() => {
    document.title = `${activePage?.title ?? "页面不可用"} - Flint`;
  }, [activePage]);

  const selectedInstance = useMemo(
    () => instances.find((instance) => instance.id === selectedId) ?? null,
    [instances, selectedId]
  );

  const settingsDirty = useMemo(
    () => !settingsEqual(settings, settingsDraft),
    [settings, settingsDraft]
  );

  const activeAccount = useMemo(
    () => accounts.find((account) => account.active) ?? null,
    [accounts]
  );
  const accountsAvailable = isBackendFeatureReady(availability, "flint.accounts");
  const accountName = activeAccount?.username || settings?.username || "";
  const accountTypeLabel = activeAccount?.accountType === "microsoft" ? "Microsoft 账户" : "离线账户";
  const gameDirPath = settings?.gameDir.trim() || info?.defaultGameDir || "";
  const gameDirLabel = gameDirPath || "平台默认 .minecraft";

  useEffect(() => {
    if (!accountsAvailable) setAccountDialogOpen(false);
  }, [accountsAvailable]);

  const navigate = useCallback((targetPageId: string) => {
    setActivePageId((current) => (
      availableRegistry.pageById[targetPageId] ? targetPageId : current
    ));
  }, [availableRegistry]);

  const openAccountDialog = useCallback(() => {
    if (!accountsAvailable) return;
    setAccountDialogOpen(true);
    void refreshAccounts();
  }, [accountsAvailable, refreshAccounts]);

  const retryInfo = useCallback(async () => {
    setInfoLoading(true);
    try {
      const value = await api.launcherInfo();
      setInfo(value);
      setInfoError(null);
    } catch (error) {
      setInfoError(toErrorMessage(error));
    } finally {
      setInfoLoading(false);
    }
  }, []);

  const pickGameDir = useCallback(async () => {
    try {
      const selected = await services.dialogs.pickDirectory("选择 Minecraft 游戏目录");
      if (selected !== null) {
        setSettingsDraft((current) => current ? { ...current, gameDir: selected } : current);
        pushNotice("info", "游戏目录已填入设置表单，保存后生效。");
      }
    } catch (error) {
      pushNotice("error", `无法打开目录选择器：${toErrorMessage(error)}`);
    }
  }, [pushNotice, services.dialogs]);

  const pickJava = useCallback(async () => {
    try {
      const selected = await services.dialogs.pickFile({ title: "选择 Java 可执行文件" });
      if (selected !== null) {
        setSettingsDraft((current) => current ? { ...current, javaPath: selected } : current);
        pushNotice("info", "Java 路径已填入设置表单，保存后生效。");
      }
    } catch (error) {
      pushNotice("error", `无法打开 Java 文件选择器：${toErrorMessage(error)}`);
    }
  }, [pushNotice, services.dialogs]);

  const scanJava = useCallback(async () => {
    setJavaLoading(true);
    setJavaError(null);
    try {
      const found = await api.scanJava();
      setJavas(found);
      pushNotice(found.length > 0 ? "success" : "info", found.length > 0 ? `已检测到 ${found.length} 个 Java 运行环境。` : "未检测到 Java，请手动选择可执行文件。");
    } catch (error) {
      const message = toErrorMessage(error);
      setJavaError(message);
      pushNotice("error", `Java 扫描失败：${message}`);
    } finally {
      setJavaLoading(false);
    }
  }, [pushNotice]);

  const reloadSettings = useCallback(async () => {
    setSettingsLoading(true);
    setSettingsError(null);
    try {
      const loaded = await api.loadSettings();
      setSettings(loaded);
      settingsRef.current = loaded;
      setSettingsDraft(loaded);
      await refreshInstances(loaded.gameDir);
      await refreshAccounts();
      pushNotice("success", "已从 Flint 后端重新载入设置。");
    } catch (error) {
      const message = toErrorMessage(error);
      setSettingsError(message);
      pushNotice("error", `设置重新载入失败：${message}`);
    } finally {
      setSettingsLoading(false);
    }
  }, [pushNotice, refreshInstances, refreshAccounts]);

  const saveSettings = useCallback(async () => {
    if (!settingsDraft) return;
    setSavingSettings(true);
    setSettingsError(null);
    try {
      const saved = await api.saveSettings(settingsDraft);
      const gameDirChanged = saved.gameDir !== (settings?.gameDir ?? "");
      setSettings(saved);
      settingsRef.current = saved;
      setSettingsDraft(saved);
      void refreshAccounts();
      pushNotice("success", "设置已保存到 Flint 后端。");
      if (gameDirChanged) await refreshInstances(saved.gameDir);
    } catch (error) {
      const message = toErrorMessage(error);
      setSettingsError(message);
      pushNotice("error", `设置保存失败：${message}`);
    } finally {
      setSavingSettings(false);
    }
  }, [settingsDraft, settings?.gameDir, pushNotice, refreshInstances, refreshAccounts]);

  const resetSettingsDraft = useCallback(() => {
    if (!settings) return;
    setSettingsDraft(settings);
    setSettingsError(null);
  }, [settings]);

  const refreshLocalVersions = useCallback(() => {
    void refreshInstances(settings?.gameDir ?? "", true);
  }, [refreshInstances, settings?.gameDir]);

  const fetchRemoteVersions = useCallback(async () => {
    setRemoteFetched(true);
    setRemoteLoading(true);
    setRemoteError(null);
    try {
      const versions = await api.fetchRemoteVersions();
      setRemoteVersions(versions);
      pushNotice("success", `已获取 ${versions.length} 个 Mojang 版本。`);
    } catch (error) {
      const message = toErrorMessage(error);
      setRemoteError(message);
      pushNotice("error", `远程版本清单获取失败：${message}`);
    } finally {
      setRemoteLoading(false);
    }
  }, [pushNotice]);

  const installVersion = useCallback(async (version: RemoteVersion) => {
    if (installRuntime) return;
    const gameDir = settings?.gameDir ?? "";
    setInstallRuntime({ versionId: version.id, progress: null });
    setInstallResult(null);
    if (progressListenerError) {
      pushNotice("info", "实时安装进度订阅不可用；安装请求仍会继续执行。");
    }
    try {
      const result = await api.installVersion(gameDir, version.id);
      setInstallResult(result);
      pushNotice("success", `${result.versionId} 安装完成：写入 ${result.filesWritten.toLocaleString()} 个文件，下载 ${formatBytes(result.bytesDownloaded)}。`);
      const currentGameDir = settingsRef.current?.gameDir ?? gameDir;
      const next = await refreshInstances(currentGameDir);
      if (next?.some((instance) => instance.id === result.versionId)) {
        setSelectedId(result.versionId);
      }
    } catch (error) {
      pushNotice("error", `${version.id} 安装失败：${toErrorMessage(error)}`);
    } finally {
      setInstallRuntime(null);
    }
  }, [installRuntime, settings?.gameDir, progressListenerError, pushNotice, refreshInstances]);

  const launchSelected = useCallback(async () => {
    if (!settings) {
      pushNotice("error", "设置尚未载入，无法启动。请先在设置页重新载入。");
      return;
    }
    if (!selectedInstance) {
      pushNotice("error", "请先选择一个本地版本。");
      return;
    }
    if (!settings.javaPath.trim()) {
      pushNotice("error", "尚未选择 Java，请先在设置中指定 Java 可执行文件。");
      return;
    }
    const usernameError = activeAccount?.accountType === "microsoft" ? null : validateUsername(settings.username);
    if (usernameError) {
      pushNotice("error", usernameError);
      return;
    }
    if (settings.maxMemoryMb < settings.minMemoryMb) {
      pushNotice("error", "最大内存不能小于最小内存，请先修正设置。");
      return;
    }
    if (!selectedInstance.hasClientJar) {
      pushNotice("error", "该版本缺少客户端 JAR，无法启动。请尝试重新安装该版本。");
      return;
    }

    const request: LaunchRequest = {
      gameDir: settings.gameDir,
      instanceId: selectedInstance.id,
      javaPath: settings.javaPath,
      username: settings.username,
      minMemoryMb: settings.minMemoryMb,
      maxMemoryMb: settings.maxMemoryMb,
      jvmArgs: settings.jvmArgs,
      gameArgs: settings.gameArgs,
      width: settings.width,
      height: settings.height
    };

    setLaunchResult(null);
    setLaunchStartedAt(null);
    setLaunching(true);
    try {
      const result = await api.launchInstance(request);
      setLaunchResult(result);
      setLaunchStartedAt(new Date().toISOString());
      pushNotice("success", `${selectedInstance.name} 已启动，进程 PID ${result.pid}。`);
    } catch (error) {
      pushNotice("error", `启动失败：${toErrorMessage(error)}`);
    } finally {
      setLaunching(false);
    }
  }, [settings, selectedInstance, activeAccount, pushNotice]);

  const titlebarContext = infoError ? "核心连接异常" : info ? `v${info.version} · ${platformLabel(info.platform)}` : "正在连接核心";

  const openPathSafely = useCallback(async (path: string, label: string) => {
    if (!path.trim()) {
      pushNotice("error", `${label}路径为空。`);
      return;
    }
    try {
      await services.opener.openPath(path);
    } catch (error) {
      pushNotice("error", `无法打开${label}：${toErrorMessage(error)}`);
    }
  }, [pushNotice, services.opener]);

  const pageContext: AppPageContext = {
    navigateToPage: navigate,
    services,
    hostState: availability.hostState,
    featureStates: availability.featureStates,
    settingsSections: availableRegistry.settingsSections,
    instanceActions: availableRegistry.instanceActions,
    info,
    infoLoading,
    infoError,
    settings,
    settingsDraft,
    settingsLoading,
    settingsError,
    savingSettings,
    settingsDirty,
    javas,
    javaLoading,
    javaError,
    instances,
    selectedId,
    selectedInstance,
    instancesLoading,
    instancesError,
    remoteVersions,
    remoteLoading,
    remoteError,
    remoteFetched,
    installRuntime,
    installResult,
    launching,
    launchResult,
    launchStartedAt,
    activeAccount,
    gameDirPath,
    gameDirLabel,
    accounts,
    accountsLoading,
    accountsError,
    accountDialogOpen,
    onLaunch: () => { void launchSelected(); },
    onSelectInstance: (id) => setSelectedId(id),
    onRefreshInstances: refreshLocalVersions,
    onFetchRemote: () => { void fetchRemoteVersions(); },
    onInstall: (version) => { void installVersion(version); },
    onOpenPath: (path, label) => { void openPathSafely(path, label); },
    onNotify: pushNotice,
    onChangeSettings: (next) => setSettingsDraft(next),
    onSaveSettings: () => { void saveSettings(); },
    onReloadSettings: () => { void reloadSettings(); },
    onResetSettings: resetSettingsDraft,
    onPickGameDir: () => { void pickGameDir(); },
    onPickJava: () => { void pickJava(); },
    onScanJava: () => { void scanJava(); },
    onRetryInfo: () => { void retryInfo(); },
    onCloseAccountDialog: () => setAccountDialogOpen(false),
    onAccountsChanged: applyAccounts,
    onRefreshAccounts: refreshAccounts
  };
  const ActivePage = activePage?.component;
  let mainContent: ReactNode;
  if (availabilityLoading && !availabilitySnapshot) {
    mainContent = (
      <AvailabilitySurface
        title="Feature availability"
        message="Loading host availability..."
        loading
        onRetry={retryAvailability}
      />
    );
  } else if (availabilityError) {
    mainContent = (
      <AvailabilitySurface
        title="Feature availability error"
        message={availabilityError}
        error
        onRetry={retryAvailability}
      />
    );
  } else if (!availability.valid) {
    mainContent = (
      <AvailabilitySurface
        title="Feature availability error"
        message="The host availability snapshot is malformed or contains duplicate feature IDs."
        error
        onRetry={retryAvailability}
      />
    );
  } else if (!availability.hostOperational) {
    mainContent = (
      <AvailabilitySurface
        title="Features unavailable"
        message={`Host state is ${availability.hostState ?? "unknown"}.`}
        details={availabilityDiagnostics}
        lastFailure={availability.lastFailure}
        error={availability.hostState !== "starting" && availability.hostState !== "stopping"}
        onRetry={retryAvailability}
      />
    );
  } else if (ActivePage) {
    mainContent = (
      <ContributionBoundary key={activePage?.id ?? "unknown-page"} contributionId={activePage?.id ?? "unknown-page"}>
        <ActivePage context={pageContext} />
      </ContributionBoundary>
    );
  } else {
    mainContent = (
      <AvailabilitySurface
        title="No available page"
        message="No registered page is currently available."
        details={availabilityDiagnostics}
        lastFailure={availability.lastFailure}
        onRetry={retryAvailability}
      />
    );
  }
  const degradedText = availabilityDiagnostics
    .map((detail) => detail.reason ? `${detail.featureId}: ${detail.reason}` : detail.featureId)
    .join("; ")
    || availability.lastFailure
    || "Some optional features are unavailable.";

  return (
    <div className="app-shell">
      <a className="skip-link" href="#main-content">跳到主要内容</a>
      <TitleBar
        context={titlebarContext}
        windowService={services.window}
        onError={(message) => pushNotice("error", message)}
      />

      <div className="shell">
        <AppSidebar
          activePageId={activePage?.id ?? activePageId}
          navigation={availableRegistry.navigation}
          settingsDirty={settingsDirty}
          coreReady={Boolean(info) && !infoError}
          accountAvailable={accountsAvailable}
          username={accountName}
          accountTypeLabel={accountTypeLabel}
          instancesCount={instances.length}
          javaCount={javas.length}
          selectedInstance={selectedInstance}
          onNavigate={navigate}
          onOpenAccounts={openAccountDialog}
        />

        <div className="main-column">
          {availability.hostState === "degraded" || (availability.hostOperational && availabilityDiagnostics.length > 0) ? (
            <div className="global-error" role="status" aria-live="polite" title={degradedText}>
              <Icon name="alert" />
              <span>{availability.hostState === "degraded" ? "Degraded" : "Feature availability"}: {degradedText}</span>
            </div>
          ) : null}

          {infoError ? (
            <div className="global-error" role="alert">
              <Icon name="alert" />
              <span>{infoError}</span>
              <button type="button" className="text-link" onClick={() => void retryInfo()} disabled={infoLoading}>
                {infoLoading ? "重试中" : "重试"}
              </button>
            </div>
          ) : null}

          <main className="app-main" id="main-content" ref={contentRef} tabIndex={-1}>
            {mainContent}
          </main>
        </div>
      </div>

      {availableRegistry.overlays.map((overlay) => {
        if (overlay.visible && !overlay.visible(pageContext)) return null;
        const Overlay = overlay.component;
        return (
          <ContributionBoundary key={overlay.id} contributionId={overlay.id}>
            <Overlay context={pageContext} />
          </ContributionBoundary>
        );
      })}

      {notice ? (
        <div className={`toast toast-${notice.kind}`} role={notice.kind === "error" ? "alert" : "status"}>
          <span className="toast-icon" aria-hidden="true">
            <Icon name={notice.kind === "error" ? "alert" : notice.kind === "success" ? "check" : "info"} />
          </span>
          <div className="toast-copy">
            <strong>{notice.kind === "error" ? "操作失败" : notice.kind === "success" ? "已完成" : "提示"}</strong>
            <p>{notice.text}</p>
          </div>
          <button type="button" className="toast-close" onClick={() => setNotice(null)} aria-label="关闭提示">
            <Icon name="close" />
          </button>
        </div>
      ) : null}
    </div>
  );
}

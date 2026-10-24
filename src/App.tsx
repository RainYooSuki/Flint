import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { openPath as openSystemPath } from "@tauri-apps/plugin-opener";
import { api } from "./api";
import { Icon } from "./components/Icon";
import { TitleBar } from "./components/TitleBar";
import { AboutView } from "./views/AboutView";
import { LaunchView } from "./views/LaunchView";
import { SettingsView } from "./views/SettingsView";
import { VersionsView } from "./views/VersionsView";
import type {
  InstallProgress,
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
  RemoteVersion,
  SectionId
} from "./types";
import {
  formatBytes,
  settingsEqual,
  toErrorMessage,
  validateUsername
} from "./utils";

const pageTitles: Record<SectionId, string> = {
  launch: "启动",
  versions: "下载",
  settings: "设置",
  about: "工具"
};
export default function App() {
  const [activeSection, setActiveSection] = useState<SectionId>("launch");
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

  const noticeIdRef = useRef(0);
  const contentRef = useRef<HTMLElement>(null);
  const instancesRequestIdRef = useRef(0);
  const settingsRef = useRef<LauncherSettings | null>(null);

  const pushNotice = useCallback((kind: NoticeKind, text: string) => {
    noticeIdRef.current += 1;
    setNotice({ id: noticeIdRef.current, kind, text });
  }, []);

  const refreshInstances = useCallback(async (gameDir: string, announce = false): Promise<InstanceInfo[] | null> => {
    const requestId = instancesRequestIdRef.current + 1;
    instancesRequestIdRef.current = requestId;
    setInstancesLoading(true);
    setInstancesError(null);
    try {
      const next = await api.listInstances(gameDir);
      if (requestId !== instancesRequestIdRef.current) return null;
      setInstances(next);
      setSelectedId((current) => (
        next.some((instance) => instance.id === current) ? current : next[0]?.id ?? ""
      ));
      if (announce) pushNotice("success", `本地版本扫描完成，共发现 ${next.length} 个版本。`);
      return next;
    } catch (error) {
      if (requestId !== instancesRequestIdRef.current) return null;
      const message = toErrorMessage(error);
      setInstancesError(message);
      if (announce) pushNotice("error", `本地版本扫描失败：${message}`);
      return null;
    } finally {
      if (requestId === instancesRequestIdRef.current) setInstancesLoading(false);
    }
  }, [pushNotice]);

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

      const instancesRequestId = instancesRequestIdRef.current + 1;
      instancesRequestIdRef.current = instancesRequestId;
      setInstancesError(null);
      try {
        const next = await api.listInstances(gameDir);
        if (cancelled || instancesRequestId !== instancesRequestIdRef.current) return;
        setInstances(next);
        setSelectedId(next[0]?.id ?? "");
      } catch (error) {
        if (cancelled || instancesRequestId !== instancesRequestIdRef.current) return;
        setInstancesError(toErrorMessage(error));
      } finally {
        if (!cancelled && instancesRequestId === instancesRequestIdRef.current) {
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
  }, []);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    void listen<InstallProgress>("flint://install-progress", (event) => {
      if (cancelled) return;
      setInstallRuntime((current) => current ? { ...current, progress: event.payload } : current);
    })
      .then((removeListener) => {
        if (cancelled) {
          removeListener();
        } else {
          unlisten = removeListener;
        }
      })
      .catch((error: unknown) => {
        if (!cancelled) setProgressListenerError(toErrorMessage(error));
      });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

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
    document.querySelectorAll<HTMLElement>(".page-content, .settings-scroll, .launch-dock, .stage-body").forEach((element) => { element.scrollTop = 0; });
  }, [activeSection]);

  useEffect(() => {
    document.title = `${pageTitles[activeSection]} - Flint`;
  }, [activeSection]);

  const selectedInstance = useMemo(
    () => instances.find((instance) => instance.id === selectedId) ?? null,
    [instances, selectedId]
  );

  const settingsDirty = useMemo(
    () => !settingsEqual(settings, settingsDraft),
    [settings, settingsDraft]
  );

  const gameDirLabel = settings?.gameDir.trim() || info?.defaultGameDir || "平台默认 .minecraft";

  const navigate = useCallback((section: SectionId) => {
    setActiveSection(section);
  }, []);

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
      const selected = await open({
        directory: true,
        multiple: false,
        title: "选择 Minecraft 游戏目录"
      });
      if (typeof selected === "string") {
        setSettingsDraft((current) => current ? { ...current, gameDir: selected } : current);
        pushNotice("info", "游戏目录已填入设置表单，保存后生效。");
      }
    } catch (error) {
      pushNotice("error", `无法打开目录选择器：${toErrorMessage(error)}`);
    }
  }, [pushNotice]);

  const pickJava = useCallback(async () => {
    try {
      const selected = await open({
        directory: false,
        multiple: false,
        title: "选择 Java 可执行文件"
      });
      if (typeof selected === "string") {
        setSettingsDraft((current) => current ? { ...current, javaPath: selected } : current);
        pushNotice("info", "Java 路径已填入设置表单，保存后生效。");
      }
    } catch (error) {
      pushNotice("error", `无法打开 Java 文件选择器：${toErrorMessage(error)}`);
    }
  }, [pushNotice]);

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
      pushNotice("success", "已从 Flint 后端重新载入设置。");
    } catch (error) {
      const message = toErrorMessage(error);
      setSettingsError(message);
      pushNotice("error", `设置重新载入失败：${message}`);
    } finally {
      setSettingsLoading(false);
    }
  }, [pushNotice, refreshInstances]);

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
      pushNotice("success", "设置已保存到 Flint 后端。");
      if (gameDirChanged) await refreshInstances(saved.gameDir);
    } catch (error) {
      const message = toErrorMessage(error);
      setSettingsError(message);
      pushNotice("error", `设置保存失败：${message}`);
    } finally {
      setSavingSettings(false);
    }
  }, [settingsDraft, settings?.gameDir, pushNotice, refreshInstances]);

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
    const usernameError = validateUsername(settings.username);
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
  }, [settings, selectedInstance, pushNotice]);

  const openPathSafely = useCallback(async (path: string, label: string) => {
    if (!path.trim()) {
      pushNotice("error", `${label}路径为空。`);
      return;
    }
    try {
      await openSystemPath(path);
    } catch (error) {
      pushNotice("error", `无法打开${label}：${toErrorMessage(error)}`);
    }
  }, [pushNotice]);

  return (
    <div className="app-shell">
      <a className="skip-link" href="#main-content">跳到主要内容</a>
      <TitleBar
        active={activeSection}
        settingsDirty={settingsDirty}
        onNavigate={navigate}
        onError={(message) => pushNotice("error", message)}
      />

      <div className="app-body">
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
          {activeSection === "launch" ? (
            <LaunchView
              info={info}
              settings={settings}
              instances={instances}
              selectedInstance={selectedInstance}
              selectedId={selectedId}
              javas={javas}
              instancesLoading={instancesLoading}
              instancesError={instancesError}
              launching={launching}
              launchResult={launchResult}
              launchStartedAt={launchStartedAt}
              onSelectInstance={setSelectedId}
              onLaunch={() => void launchSelected()}
              onGoVersions={() => navigate("versions")}
              onGoSettings={() => navigate("settings")}
              onRefreshInstances={refreshLocalVersions}
              onOpenPath={(path, label) => void openPathSafely(path, label)}
            />
          ) : null}

          {activeSection === "versions" ? (
            <VersionsView
              instances={instances}
              selectedId={selectedId}
              instancesLoading={instancesLoading}
              instancesError={instancesError}
              remoteVersions={remoteVersions}
              remoteLoading={remoteLoading}
              remoteError={remoteError}
              remoteFetched={remoteFetched}
              installRuntime={installRuntime}
              installResult={installResult}
              gameDirLabel={gameDirLabel}
              onRefreshInstances={refreshLocalVersions}
              onSelectInstance={setSelectedId}
              onFetchRemote={() => void fetchRemoteVersions()}
              onInstall={(version) => void installVersion(version)}
              onOpenPath={(path, label) => void openPathSafely(path, label)}
              onGoLaunch={() => navigate("launch")}
            />
          ) : null}

          {activeSection === "settings" ? (
            <SettingsView
              draft={settingsDraft}
              saved={settings}
              javas={javas}
              settingsLoading={settingsLoading}
              javaLoading={javaLoading}
              settingsError={settingsError}
              javaError={javaError}
              saving={savingSettings}
              dirty={settingsDirty}
              onChange={(next) => setSettingsDraft(next)}
              onSave={() => void saveSettings()}
              onReload={() => void reloadSettings()}
              onReset={resetSettingsDraft}
              onPickGameDir={() => void pickGameDir()}
              onPickJava={() => void pickJava()}
              onScanJava={() => void scanJava()}
            />
          ) : null}

          {activeSection === "about" ? (
            <AboutView
              info={info}
              infoLoading={infoLoading}
              infoError={infoError}
              settings={settings}
              instancesCount={instances.length}
              javaCount={javas.length}
              settingsDirty={settingsDirty}
              onOpenPath={(path, label) => void openPathSafely(path, label)}
              onRetryInfo={() => void retryInfo()}
              onGoSettings={() => navigate("settings")}
              onGoVersions={() => navigate("versions")}
            />
          ) : null}
        </main>
      </div>

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
import { useEffect, useMemo, useState } from "react";
import { api } from "../../../api";
import { Icon } from "../../../components/Icon";
import type { HostEvents } from "../../../core/events/hostEvents";
import { useHostEvent } from "../../../core/events/useHostEvent";
import { useRequestGeneration } from "../../../core/requests/useRequestGeneration";
import type {
  InstanceInfo,
  LauncherSettings,
  LoaderInstallProgress,
  LoaderInstallRequest,
  LoaderInstallResult,
  LoaderKind,
  LoaderVersionInfo,
  NoticeKind,
  RemoteVersion
} from "../../../types";
import {
  LOADER_KINDS,
  formatDateTime,
  parseLoaderKind,
  loaderKindLabel,
  progressPercent,
  toErrorMessage
} from "../../../utils";

interface LoaderInstallPanelProps {
  gameDir: string;
  gameDirLabel: string;
  selectedInstance: InstanceInfo | null;
  settings: LauncherSettings | null;
  remoteVersions: RemoteVersion[];
  remoteLoading: boolean;
  remoteError: string | null;
  remoteFetched: boolean;
  onFetchRemote: () => void;
  onRefreshInstances: () => void;
  onGoSettings: () => void;
  onOpenPath: (path: string, label: string) => void;
  onNotify: (kind: NoticeKind, message: string) => void;
  onBusyChange: (busy: boolean) => void;
  events: HostEvents;
}

export function LoaderInstallPanel({
  gameDir,
  gameDirLabel,
  selectedInstance,
  settings,
  remoteVersions,
  remoteLoading,
  remoteError,
  remoteFetched,
  onFetchRemote,
  onRefreshInstances,
  onGoSettings,
  onOpenPath,
  onNotify,
  onBusyChange,
  events
}: LoaderInstallPanelProps) {
  const [loaderKind, setLoaderKind] = useState<LoaderKind>("fabric");
  const [gameVersion, setGameVersion] = useState("");
  const [loaderVersions, setLoaderVersions] = useState<LoaderVersionInfo[]>([]);
  const [loaderVersionsLoading, setLoaderVersionsLoading] = useState(false);
  const [loaderVersionsError, setLoaderVersionsError] = useState<string | null>(null);
  const [selectedLoaderVersion, setSelectedLoaderVersion] = useState("");
  const [installing, setInstalling] = useState(false);
  const [installProgress, setInstallProgress] = useState<LoaderInstallProgress | null>(null);
  const [installResult, setInstallResult] = useState<LoaderInstallResult | null>(null);
  const [listenerError, setListenerError] = useState<string | null>(null);
  const [installError, setInstallError] = useState<string | null>(null);
  const requests = useRequestGeneration();

  const releaseVersions = useMemo(() => {
    const ids = new Set<string>();
    if (selectedInstance?.gameVersion) ids.add(selectedInstance.gameVersion);
    for (const version of remoteVersions) {
      if (version.versionType.trim().toLowerCase() === "release") ids.add(version.id);
    }
    return Array.from(ids).sort((left, right) => right.localeCompare(left, "en", { numeric: true }));
  }, [remoteVersions, selectedInstance?.gameVersion]);

  useEffect(() => {
    const instanceLoader = selectedInstance ? parseLoaderKind(selectedInstance.loader) : null;
    if (instanceLoader) setLoaderKind(instanceLoader);
    if (selectedInstance?.gameVersion) setGameVersion(selectedInstance.gameVersion);
    requests.invalidate();
    setLoaderVersions([]);
    setLoaderVersionsLoading(false);
    setSelectedLoaderVersion("");
    setInstallResult(null);
    setInstallProgress(null);
  }, [selectedInstance?.id, selectedInstance?.gameVersion, selectedInstance?.loader, requests]);

  useEffect(() => {
    if (!gameVersion && releaseVersions.length > 0) setGameVersion(releaseVersions[0]);
  }, [gameVersion, releaseVersions]);

  useHostEvent(
    events,
    "loaderInstallProgress",
    (progress) => setInstallProgress(progress),
    (error) => setListenerError(toErrorMessage(error))
  );

  function resetInstallState() {
    requests.invalidate();
    setLoaderVersions([]);
    setLoaderVersionsLoading(false);
    setSelectedLoaderVersion("");
    setInstallProgress(null);
    setInstallResult(null);
    setLoaderVersionsError(null);
    setInstallError(null);
  }

  async function loadLoaderVersions() {
    if (!gameVersion) {
      setLoaderVersionsError("请先选择 Minecraft 版本。");
      return;
    }
    const requestId = requests.next();
    setLoaderVersionsLoading(true);
    setLoaderVersionsError(null);
    setInstallProgress(null);
    setInstallResult(null);
    try {
      const versions = await api.listLoaderVersions(loaderKind, gameVersion);
      if (!requests.isCurrent(requestId)) return;
      setLoaderVersions(versions);
      const preferred = versions.find((version) => version.recommended) ?? versions[0] ?? null;
      setSelectedLoaderVersion(preferred?.version ?? "");
      if (versions.length === 0) setLoaderVersionsError("该版本没有可用的加载器版本。");
    } catch (error) {
      if (!requests.isCurrent(requestId)) return;
      setLoaderVersions([]);
      setSelectedLoaderVersion("");
      setLoaderVersionsError(toErrorMessage(error));
    } finally {
      if (requests.isCurrent(requestId)) setLoaderVersionsLoading(false);
    }
  }

  async function installLoader() {
    if (installing || !settings || !selectedLoaderVersion || !gameVersion) return;
    const needsJavaInstaller = loaderKind === "forge" || loaderKind === "neoforge" || loaderKind === "optifine";
    if (needsJavaInstaller && !settings.javaPath.trim()) {
      onNotify("error", "请先在设置中选择 Java 可执行文件。");
      return;
    }
    const request: LoaderInstallRequest = {
      gameDir,
      gameVersion,
      loader: loaderKind,
      loaderVersion: selectedLoaderVersion,
      javaPath: settings.javaPath
    };
    setInstalling(true);
    onBusyChange(true);
    setInstallError(null);
    setInstallProgress(null);
    setInstallResult(null);
    try {
      const result = await api.installLoader(request);
      setInstallResult(result);
      onNotify("success", `${loaderKindLabel(result.loader)} ${result.loaderVersion} 安装完成。`);
      onRefreshInstances();
    } catch (error) {
      const message = toErrorMessage(error);
      setInstallProgress(null);
      setInstallError(`加载器安装失败：${message}`);
      onNotify("error", `加载器安装失败：${message}`);
    } finally {
      setInstalling(false);
      onBusyChange(false);
    }
  }

  const selectedVersionInfo = loaderVersions.find((version) => version.version === selectedLoaderVersion) ?? null;
  const percent = installProgress ? progressPercent(installProgress.completed, installProgress.total) : null;
  const javaReady = Boolean(settings?.javaPath.trim());
  const needsJavaInstaller = loaderKind === "forge" || loaderKind === "neoforge" || loaderKind === "optifine";
  const canInstall = Boolean(settings && gameVersion && selectedLoaderVersion && (!needsJavaInstaller || javaReady) && !installing);

  return (
    <div className="download-tab-panel loader-install-panel">
      <div className="loader-context">
        <div><span>当前实例</span><strong>{selectedInstance?.name ?? "未选择"}</strong></div>
        <div><span>游戏目录</span><strong title={gameDirLabel}>{gameDirLabel}</strong></div>
        <div><span>Java</span><strong className={javaReady ? "ok-text" : "warn-text"} title={settings?.javaPath || undefined}>{javaReady ? "已配置" : "未配置"}</strong></div>
      </div>

      <div className="loader-grid">
        <section className="panel" aria-labelledby="loader-config-title">
          <div className="panel-head">
            <span id="loader-config-title">加载器安装</span>
            <span>{loaderKindLabel(loaderKind)}</span>
          </div>
          <div className="loader-form">
            <label className="form-row">
              <span>Minecraft 版本</span>
              <span className="select-field">
                <select
                  value={gameVersion}
                  onChange={(event) => { setGameVersion(event.target.value); resetInstallState(); }}
                  disabled={installing}
                >
                  {releaseVersions.length === 0 ? <option value="">暂无正式版清单</option> : null}
                  {releaseVersions.map((version) => <option key={version} value={version}>{version}</option>)}
                </select>
                <Icon name="chevronDown" />
              </span>
            </label>

            <label className="form-row">
              <span>加载器</span>
              <span className="select-field">
                <select
                  value={loaderKind}
                  onChange={(event) => { setLoaderKind(event.target.value as LoaderKind); resetInstallState(); }}
                  disabled={installing}
                >
                  {LOADER_KINDS.map((kind) => <option key={kind} value={kind}>{loaderKindLabel(kind)}</option>)}
                </select>
                <Icon name="chevronDown" />
              </span>
            </label>

            <div className="form-row">
              <span>可用版本</span>
              <div className="inline-actions">
                <span className="select-field grow">
                  <select
                    value={selectedLoaderVersion}
                    onChange={(event) => { setSelectedLoaderVersion(event.target.value); setInstallProgress(null); setInstallResult(null); }}
                    disabled={loaderVersionsLoading || loaderVersions.length === 0 || installing}
                  >
                    <option value="">{loaderVersionsLoading ? "正在获取..." : loaderVersions.length === 0 ? "请先获取版本" : "选择加载器版本"}</option>
                    {loaderVersions.map((version) => (
                      <option key={version.version} value={version.version}>
                        {version.label || version.version}{version.recommended ? " · 推荐" : ""}
                      </option>
                    ))}
                  </select>
                  <Icon name="chevronDown" />
                </span>
                <button type="button" className="button compact" onClick={() => void loadLoaderVersions()} disabled={loaderVersionsLoading || installing || !gameVersion}>
                  <Icon name="refresh" className={loaderVersionsLoading ? "icon-spin" : undefined} />
                  获取版本
                </button>
              </div>
            </div>

            {loaderVersionsError ? (
              <div className="inline-message error compact" role="alert">
                <Icon name="alert" /><span>{loaderVersionsError}</span>
              </div>
            ) : null}
            {selectedVersionInfo ? (
              <div className="loader-version-meta">
                <span>{selectedVersionInfo.gameVersion}</span>
                <span>{selectedVersionInfo.releaseTime ? formatDateTime(selectedVersionInfo.releaseTime) : "发布时间未知"}</span>
                {selectedVersionInfo.recommended ? <span className="status-chip">推荐</span> : null}
              </div>
            ) : null}

            <div className="form-row">
              <span>Java 环境</span>
              <div className="java-summary">
                <span className={javaReady ? "ok-text" : "warn-text"} title={settings?.javaPath || undefined}>
                  {javaReady ? settings?.javaPath : "尚未选择 Java"}
                </span>
                <button type="button" className="link-button" onClick={onGoSettings}>设置</button>
              </div>
            </div>

            {releaseVersions.length === 0 ? (
              <div className="inline-message" role="status">
                <Icon name="info" /><span>{remoteError ? "远程版本清单暂不可用，请重试。" : remoteLoading ? "正在获取 Mojang 版本清单..." : remoteFetched ? "Mojang 版本清单中没有可用的正式版。" : "需要先获取 Mojang 版本清单才能选择正式版。"}</span>
                <button type="button" className="text-link" onClick={onFetchRemote} disabled={remoteLoading}>
                  {remoteLoading ? "获取中" : "获取清单"}
                </button>
              </div>
            ) : null}
            {remoteError ? (
              <div className="inline-message error compact" role="alert">
                <Icon name="alert" /><span>{remoteError}</span>
                <button type="button" className="text-link" onClick={onFetchRemote}>重试</button>
              </div>
            ) : null}
            {listenerError ? (
              <div className="inline-message error compact" role="alert">
                <Icon name="alert" /><span>无法订阅加载器进度：{listenerError}</span>
              </div>
            ) : null}

            <button type="button" className="button primary loader-install-button" onClick={() => void installLoader()} disabled={!canInstall}>
              {installing ? <span className="spinner spinner-light" /> : <Icon name="wrench" />}
              {installing ? "正在安装加载器" : "安装加载器"}
            </button>
          </div>
        </section>

        <section className="panel" aria-labelledby="loader-progress-title">
          <div className="panel-head">
            <span id="loader-progress-title">安装状态</span>
            <span aria-live="polite">{installing ? "进行中" : installError ? "安装失败" : installResult ? "已完成" : "等待开始"}</span>
          </div>
          <div className="loader-status" aria-busy={installing || undefined}>
            {installError ? (
              <div className="inline-message error compact" role="alert">
                <Icon name="alert" /><span>{installError}</span>
              </div>
            ) : null}
            {installProgress ? (
              <div className="install-progress-panel">
                <strong>{installProgress.message || "正在安装加载器"}</strong>
                <p>{loaderKindLabel(installProgress.loader)} {installProgress.loaderVersion} · {installProgress.gameVersion}</p>
                <div className={`progress-bar${percent === null ? " indeterminate" : ""}`} role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={percent === null ? undefined : Math.round(percent)} aria-label="加载器安装进度">
                  <span style={percent === null ? undefined : { width: `${percent}%` }} />
                </div>
                <div className="progress-meta">
                  <span>{installProgress.stage}</span>
                  <span>{installProgress.total > 0 ? `${installProgress.completed} / ${installProgress.total}` : "准备中"}</span>
                </div>
              </div>
            ) : null}

            {installResult ? (
              <div className="loader-result">
                <Icon name="check" />
                <div>
                  <strong>已安装 {installResult.installedVersionId}</strong>
                  <p>{loaderKindLabel(installResult.loader)} {installResult.loaderVersion} · {installResult.gameVersion}</p>
                  <small title={installResult.installPath}>{installResult.installPath}</small>
                </div>
                <div className="inline-actions">
                  <button type="button" className="button compact" onClick={() => onOpenPath(installResult.installPath, "版本安装目录")}>打开目录</button>
                  {installResult.logPath ? <button type="button" className="button compact" onClick={() => onOpenPath(installResult.logPath as string, "安装日志")}>打开日志</button> : null}
                </div>
              </div>
            ) : null}

            {installing && !installProgress ? (
              <div className="loader-status-empty">
                <span className="spinner state-spinner" />
                <strong>正在准备安装</strong>
                <p>正在创建安装请求并等待进度事件。</p>
              </div>
            ) : !installProgress && !installResult && !installError ? (
              <div className="loader-status-empty">
                <Icon name="wrench" />
                <strong>尚未开始安装</strong>
                <p>选择版本、加载器和 Java 环境后开始安装。已有原版版本会复用本地文件。</p>
              </div>
            ) : null}
          </div>
        </section>
      </div>
    </div>
  );
}

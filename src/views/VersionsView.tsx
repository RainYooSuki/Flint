import { useEffect, useId, useMemo, useRef, useState } from "react";
import type { ReactNode } from "react";
import { Icon } from "../components/Icon";
import { InstalledModsPanel } from "./downloads/InstalledModsPanel";
import { LoaderInstallPanel } from "./downloads/LoaderInstallPanel";
import { ModDiscoveryPanel } from "./downloads/ModDiscoveryPanel";
import type {
  InstallResult,
  InstallRuntime,
  InstanceInfo,
  LauncherSettings,
  NoticeKind,
  RemoteVersion
} from "../types";
import {
  formatBytes,
  formatDate,
  formatDateTime,
  loaderLabel,
  progressPercent,
  versionTypeLabel
} from "../utils";

type VersionMode = "local" | "remote" | "loaders" | "mods" | "installed";
type LocalTypeFilter = "all" | "release" | "snapshot" | "other";
type RemoteTypeFilter = "all" | "release" | "snapshot";

interface VersionsViewProps {
  instances: InstanceInfo[];
  selectedId: string;
  instancesLoading: boolean;
  instancesError: string | null;
  remoteVersions: RemoteVersion[];
  remoteLoading: boolean;
  remoteError: string | null;
  remoteFetched: boolean;
  installRuntime: InstallRuntime | null;
  installResult: InstallResult | null;
  gameDirLabel: string;
  gameDirPath: string;
  settings: LauncherSettings | null;
  onRefreshInstances: () => void;
  onSelectInstance: (id: string) => void;
  onFetchRemote: () => void;
  onInstall: (version: RemoteVersion) => void;
  onOpenPath: (path: string, label: string) => void;
  onGoLaunch: () => void;
  onGoSettings: () => void;
  onNotify: (kind: NoticeKind, message: string) => void;
}

function localTypeGroup(value: string): Exclude<LocalTypeFilter, "all"> {
  const normalized = value.trim().toLowerCase();
  if (normalized === "release") return "release";
  if (normalized === "snapshot") return "snapshot";
  return "other";
}

export function VersionsView({
  instances,
  selectedId,
  instancesLoading,
  instancesError,
  remoteVersions,
  remoteLoading,
  remoteError,
  remoteFetched,
  installRuntime,
  installResult,
  gameDirLabel,
  gameDirPath,
  settings,
  onRefreshInstances,
  onSelectInstance,
  onFetchRemote,
  onInstall,
  onOpenPath,
  onGoLaunch,
  onGoSettings,
  onNotify
}: VersionsViewProps) {
  const [mode, setMode] = useState<VersionMode>("local");
  const [featureBusy, setFeatureBusy] = useState(false);
  const autoFetchAttemptedRef = useRef(false);
  const [localSearch, setLocalSearch] = useState("");
  const [localType, setLocalType] = useState<LocalTypeFilter>("all");
  const [loaderFilter, setLoaderFilter] = useState("all");
  const [remoteSearch, setRemoteSearch] = useState("");
  const [remoteType, setRemoteType] = useState<RemoteTypeFilter>("release");
  const localSearchId = useId();
  const localTypeId = useId();
  const loaderFilterId = useId();
  const remoteSearchId = useId();

  useEffect(() => {
    if ((mode === "remote" || mode === "loaders") && !remoteFetched && !remoteLoading && !autoFetchAttemptedRef.current) {
      autoFetchAttemptedRef.current = true;
      onFetchRemote();
    }
  }, [mode, remoteFetched, remoteLoading, onFetchRemote]);

  const loaderOptions = useMemo(() => {
    const values = new Map<string, string>();
    for (const instance of instances) {
      const key = instance.loader.trim().toLowerCase() || "vanilla";
      if (!values.has(key)) values.set(key, loaderLabel(instance.loader));
    }
    return Array.from(values.entries()).sort((left, right) => left[1].localeCompare(right[1], "zh-CN"));
  }, [instances]);

  const filteredInstances = useMemo(() => {
    const query = localSearch.trim().toLowerCase();
    return instances.filter((instance) => {
      const matchesQuery =
        !query ||
        instance.name.toLowerCase().includes(query) ||
        instance.id.toLowerCase().includes(query) ||
        instance.loader.toLowerCase().includes(query);
      const matchesType = localType === "all" || localTypeGroup(instance.versionType) === localType;
      const loaderKey = instance.loader.trim().toLowerCase() || "vanilla";
      const matchesLoader = loaderFilter === "all" || loaderKey === loaderFilter;
      return matchesQuery && matchesType && matchesLoader;
    });
  }, [instances, localSearch, localType, loaderFilter]);

  const filteredRemoteVersions = useMemo(() => {
    const query = remoteSearch.trim().toLowerCase();
    return remoteVersions.filter((version) => {
      const matchesQuery = !query || version.id.toLowerCase().includes(query);
      const matchesType = remoteType === "all" || version.versionType.trim().toLowerCase() === remoteType;
      return matchesQuery && matchesType;
    });
  }, [remoteVersions, remoteSearch, remoteType]);

  const installedIds = useMemo(() => new Set(instances.map((instance) => instance.id)), [instances]);
  const selectedInstance = instances.find((instance) => instance.id === selectedId) ?? null;
  const percent = installRuntime?.progress
    ? progressPercent(installRuntime.progress.completed, installRuntime.progress.total)
    : null;
  const modeContext = mode === "local"
    ? `${instances.length} 个本地版本`
    : mode === "remote"
      ? remoteFetched ? `${filteredRemoteVersions.length} 个远程版本` : "Mojang 版本清单"
      : mode === "loaders"
        ? "安装 Minecraft 模组加载器"
        : mode === "mods"
          ? selectedInstance?.name ? `${selectedInstance.name} · 模组发现` : "模组发现"
          : "管理已安装模组";

  return (
    <div className="page-shell">
      <header className="page-head">
        <div className="page-title">
          <h1>下载</h1>
          <span>{modeContext}</span>
        </div>
        <div className="page-head-actions">
          {mode === "local" && selectedInstance ? (
            <button type="button" className="button primary" onClick={onGoLaunch}>
              <Icon name="play" /> 启动所选版本
            </button>
          ) : null}
        </div>
      </header>

      <div className="page-body downloads-content">
        <div className="section-tabs" role="group" aria-label="下载功能">
          <button
            type="button"
            className={mode === "local" ? "active" : ""}
            aria-pressed={mode === "local"}
            onClick={() => setMode("local")}
            disabled={featureBusy}
          >
            本地版本 <span className="tab-count">{instances.length}</span>
          </button>
          <button
            type="button"
            className={mode === "remote" ? "active" : ""}
            aria-pressed={mode === "remote"}
            onClick={() => setMode("remote")}
            disabled={featureBusy}
          >
            Mojang 版本
          </button>
          <button
            type="button"
            className={mode === "loaders" ? "active" : ""}
            aria-pressed={mode === "loaders"}
            onClick={() => setMode("loaders")}
            disabled={featureBusy}
          >
            加载器
          </button>
          <button
            type="button"
            className={mode === "mods" ? "active" : ""}
            aria-pressed={mode === "mods"}
            onClick={() => setMode("mods")}
            disabled={featureBusy}
          >
            模组发现
          </button>
          <button
            type="button"
            className={mode === "installed" ? "active" : ""}
            aria-pressed={mode === "installed"}
            onClick={() => setMode("installed")}
            disabled={featureBusy}
          >
            已安装模组
          </button>
          <div className="tab-meta" title={gameDirLabel}>
            <Icon name="folder" />
            <span>{gameDirLabel}</span>
          </div>
        </div>

        {mode === "local" ? (
          <section className="download-tab-panel" aria-label="本地版本">
            <div className="list-toolbar">
              <div className="search-box">
                <label className="sr-only" htmlFor={localSearchId}>搜索本地版本</label>
                <Icon name="search" />
                <input
                  id={localSearchId}
                  type="search"
                  value={localSearch}
                  onChange={(event) => setLocalSearch(event.target.value)}
                  placeholder="搜索名称、版本 ID 或加载器"
                  autoComplete="off"
                />
                {localSearch ? (
                  <button type="button" className="search-clear" onClick={() => setLocalSearch("")} aria-label="清空搜索">
                    <Icon name="close" />
                  </button>
                ) : null}
              </div>
              <label className="compact-field" htmlFor={localTypeId}>
                <span className="sr-only">版本类型</span>
                <select id={localTypeId} value={localType} onChange={(event) => setLocalType(event.target.value as LocalTypeFilter)}>
                  <option value="all">全部类型</option>
                  <option value="release">正式版</option>
                  <option value="snapshot">快照</option>
                  <option value="other">其他</option>
                </select>
                <Icon name="chevronDown" />
              </label>
              <label className="compact-field" htmlFor={loaderFilterId}>
                <span className="sr-only">加载器</span>
                <select id={loaderFilterId} value={loaderFilter} onChange={(event) => setLoaderFilter(event.target.value)}>
                  <option value="all">全部加载器</option>
                  {loaderOptions.map(([value, label]) => <option key={value} value={value}>{label}</option>)}
                </select>
                <Icon name="chevronDown" />
              </label>
              <button type="button" className="button secondary compact" onClick={onRefreshInstances} disabled={instancesLoading}>
                <Icon name="refresh" className={instancesLoading ? "icon-spin" : undefined} />
                {instancesLoading ? "扫描中" : "刷新"}
              </button>
            </div>

            <div className="list-summary">
              <span>共 {instances.length} 个本地版本，显示 {filteredInstances.length} 个</span>
              {selectedInstance ? <span>当前选择：<strong>{selectedInstance.name}</strong></span> : null}
            </div>

            {instancesError ? (
              <div className="inline-message error" role="alert">
                <Icon name="alert" />
                <span>{instancesError}</span>
                <button type="button" className="text-link" onClick={onRefreshInstances}>重试</button>
              </div>
            ) : null}

            {instancesLoading && instances.length === 0 ? (
              <StatePanel title="正在读取本地版本" message="正在检查版本元数据和客户端文件。" loading />
            ) : instances.length === 0 ? (
              <StatePanel
                title="没有本地版本"
                message="当前游戏目录中未发现可用的版本。可以切换到远程下载安装原版版本。"
                action={<button type="button" className="button primary" onClick={() => setMode("remote")}>远程下载</button>}
              />
            ) : filteredInstances.length === 0 ? (
              <StatePanel
                title="没有匹配的版本"
                message="请调整搜索关键词或筛选条件。"
                action={<button type="button" className="button secondary" onClick={() => { setLocalSearch(""); setLocalType("all"); setLoaderFilter("all"); }}>清除筛选</button>}
              />
            ) : (
              <div className="data-table">
                <div className="data-head local-cols" aria-hidden="true">
                  <span>版本</span>
                  <span>类型</span>
                  <span>加载器</span>
                  <span>客户端</span>
                  <span>上次启动</span>
                </div>
                {filteredInstances.map((instance) => {
                  const selected = instance.id === selectedId;
                  return (
                    <button
                      key={instance.id}
                      type="button"
                      className={`data-row version-row local-cols${selected ? " selected" : ""}`}
                      aria-pressed={selected}
                      onClick={() => onSelectInstance(instance.id)}
                    >
                      <span className="name-cell">
                        <span className="name-copy">
                          <span className="name-line">
                            <strong>{instance.name}</strong>
                            {selected ? <span className="current-tag">当前</span> : null}
                          </span>
                          <small title={instance.path}>{instance.path}</small>
                        </span>
                      </span>
                      <span><span className={`type-badge type-${instance.versionType.toLowerCase()}`}>{versionTypeLabel(instance.versionType)}</span></span>
                      <span className="cell-muted">{loaderLabel(instance.loader)}</span>
                      <span className={instance.hasClientJar ? "state-text ok" : "state-text warn"}>
                        <span className="inline-dot" />{instance.hasClientJar ? "完整" : "缺失 JAR"}
                      </span>
                      <span className="cell-muted">{formatDate(instance.lastPlayed)}</span>
                    </button>
                  );
                })}
              </div>
            )}
          </section>
        ) : mode === "remote" ? (
          <section className="download-tab-panel" aria-label="远程版本">
            <div className="list-toolbar">
              <div className="search-box">
                <label className="sr-only" htmlFor={remoteSearchId}>搜索远程版本</label>
                <Icon name="search" />
                <input
                  id={remoteSearchId}
                  type="search"
                  value={remoteSearch}
                  onChange={(event) => setRemoteSearch(event.target.value)}
                  placeholder="搜索 Mojang 版本 ID"
                  autoComplete="off"
                />
                {remoteSearch ? (
                  <button type="button" className="search-clear" onClick={() => setRemoteSearch("")} aria-label="清空搜索">
                    <Icon name="close" />
                  </button>
                ) : null}
              </div>
              <div className="filter-group" role="group" aria-label="版本类型">
                {([
                  ["all", "全部"],
                  ["release", "正式版"],
                  ["snapshot", "快照"]
                ] as Array<[RemoteTypeFilter, string]>).map(([value, label]) => (
                  <button
                    key={value}
                    type="button"
                    className={remoteType === value ? "active" : ""}
                    aria-pressed={remoteType === value}
                    onClick={() => setRemoteType(value)}
                  >
                    {label}
                  </button>
                ))}
              </div>
              <button type="button" className="button secondary compact" onClick={onFetchRemote} disabled={remoteLoading || Boolean(installRuntime)}>
                <Icon name="refresh" className={remoteLoading ? "icon-spin" : undefined} />
                {remoteLoading ? "获取中" : remoteFetched ? "刷新清单" : "获取清单"}
              </button>
            </div>

            {installRuntime ? (
              <div className="install-progress" aria-live="polite">
                <div className="install-progress-head">
                  <span className="spinner" />
                  <div>
                    <strong>正在安装 {installRuntime.versionId}</strong>
                    <span>{installRuntime.progress?.message || "正在准备版本文件。"}</span>
                  </div>
                  <span className="progress-count">
                    {installRuntime.progress && installRuntime.progress.total > 0
                      ? `${installRuntime.progress.completed.toLocaleString()} / ${installRuntime.progress.total.toLocaleString()}`
                      : "准备中"}
                  </span>
                </div>
                <div
                  className={`progress-bar${percent === null ? " indeterminate" : ""}`}
                  role="progressbar"
                  aria-valuemin={0}
                  aria-valuemax={100}
                  aria-valuenow={percent === null ? undefined : Math.round(percent)}
                  aria-label={`安装 ${installRuntime.versionId}`}
                >
                  <span style={percent === null ? undefined : { width: `${percent}%` }} />
                </div>
                <div className="progress-foot">
                  <span>{installRuntime.progress?.stage || "initializing"}</span>
                  <span>安装期间请保持 Flint 开启</span>
                </div>
              </div>
            ) : null}

            {installResult ? (
              <div className="install-result">
                <Icon name="check" />
                <div>
                  <strong>{installResult.versionId} 安装完成</strong>
                  <span>{installResult.filesWritten.toLocaleString()} 个文件，下载 {formatBytes(installResult.bytesDownloaded)}</span>
                  <small title={installResult.installPath}>{installResult.installPath}</small>
                </div>
                <button type="button" className="button secondary compact" onClick={() => onOpenPath(installResult.installPath, "版本目录")}>打开目录</button>
              </div>
            ) : null}

            <div className="list-summary">
              <span>{remoteFetched ? `共 ${filteredRemoteVersions.length} 个版本` : "尚未获取远程清单"}</span>
              {remoteFetched && !remoteLoading ? <span>来源：Mojang 版本清单</span> : null}
            </div>

            {remoteLoading && remoteVersions.length === 0 ? (
              <StatePanel title="正在获取版本清单" message="正在连接 Mojang 版本服务。" loading />
            ) : remoteError ? (
              <StatePanel title="版本清单获取失败" message={remoteError} error action={<button type="button" className="button secondary" onClick={onFetchRemote}>重试</button>} />
            ) : !remoteFetched ? (
              <StatePanel title="远程版本" message="获取 Mojang 版本清单后，可以选择正式版或快照进行安装。" action={<button type="button" className="button primary" onClick={onFetchRemote}><Icon name="download" /> 获取清单</button>} />
            ) : filteredRemoteVersions.length === 0 ? (
              <StatePanel title="没有匹配的版本" message="请调整搜索关键词或版本类型。" action={<button type="button" className="button secondary" onClick={() => { setRemoteSearch(""); setRemoteType("all"); }}>清除筛选</button>} />
            ) : (
              <div className="data-table">
                <div className="data-head remote-cols" aria-hidden="true">
                  <span>版本</span>
                  <span>类型</span>
                  <span>发布时间</span>
                  <span>操作</span>
                </div>
                {filteredRemoteVersions.map((version) => {
                  const installed = installedIds.has(version.id);
                  const thisInstall = installRuntime?.versionId === version.id;
                  return (
                    <div key={version.id} className="data-row remote-row remote-cols">
                      <span className="name-cell">
                        <span className="name-copy">
                          <span className="name-line"><strong>{version.id}</strong>{installed ? <span className="installed-tag">本地已有</span> : null}</span>
                          <small>Mojang 原版版本</small>
                        </span>
                      </span>
                      <span><span className={`type-badge type-${version.versionType.toLowerCase()}`}>{versionTypeLabel(version.versionType)}</span></span>
                      <span className="cell-muted">{formatDateTime(version.releaseTime)}</span>
                      <span className="row-action">
                        <button
                          type="button"
                          className={`install-button${thisInstall ? " active" : ""}`}
                          onClick={() => onInstall(version)}
                          disabled={Boolean(installRuntime) || remoteLoading}
                          aria-label={`${installed ? "重新安装" : "安装"} ${version.id}`}
                        >
                          {thisInstall ? <span className="spinner spinner-small" /> : <Icon name="download" />}
                          {thisInstall ? "安装中" : installed ? "重新安装" : "安装"}
                        </button>
                      </span>
                    </div>
                  );
                })}
              </div>
            )}

            <p className="scope-note">安装内容来自 Mojang 版本清单，写入当前游戏目录。不会自动安装模组或整合包。</p>
          </section>
        ) : mode === "loaders" ? (
          <LoaderInstallPanel
            gameDir={settings?.gameDir ?? ""}
            gameDirLabel={gameDirLabel}
            selectedInstance={selectedInstance}
            settings={settings}
            remoteVersions={remoteVersions}
            remoteLoading={remoteLoading}
            remoteError={remoteError}
            remoteFetched={remoteFetched}
            onFetchRemote={onFetchRemote}
            onRefreshInstances={onRefreshInstances}
            onGoSettings={onGoSettings}
            onOpenPath={onOpenPath}
            onNotify={onNotify}
            onBusyChange={setFeatureBusy}
          />
        ) : mode === "mods" ? (
          <ModDiscoveryPanel
            gameDir={settings?.gameDir ?? ""}
            gameDirLabel={gameDirLabel}
            selectedInstance={selectedInstance}
            curseforgeApiKey={settings?.curseforgeApiKey ?? ""}
            onGoSettings={onGoSettings}
            onGoLocal={() => setMode("local")}
            onNotify={onNotify}
            onBusyChange={setFeatureBusy}
          />
        ) : (
          <InstalledModsPanel
            gameDir={settings?.gameDir ?? ""}
            gameDirPath={gameDirPath}
            gameDirLabel={gameDirLabel}
            selectedInstance={selectedInstance}
            onOpenPath={onOpenPath}
            onNotify={onNotify}
          />
        )}
      </div>
    </div>
  );
}

function StatePanel({ title, message, action, loading = false, error = false }: { title: string; message: string; action?: ReactNode; loading?: boolean; error?: boolean }) {
  return (
    <div className="state-panel" role={error ? "alert" : loading ? "status" : undefined}>
      {loading ? <span className="spinner state-spinner" /> : <Icon name="layers" />}
      <strong>{title}</strong>
      <p>{message}</p>
      {action}
    </div>
  );
}
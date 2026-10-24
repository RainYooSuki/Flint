import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api } from "../../../api";
import { Icon } from "../../../components/Icon";
import type {
  InstanceInfo,
  ModDependencyPlan,
  ModInstallProgress,
  ModInstallResult,
  ModProject,
  ModSource,
  ModVersionInfo,
  NoticeKind
} from "../../../types";
import {
  loaderKindFromValue,
  loaderKindLabel,
  progressPercent,
  toErrorMessage
} from "../../../utils";

const PAGE_SIZE = 12;

function dependencyTypeLabel(value: string): string {
  if (value === "optional") return "可选";
  if (value === "embedded") return "内嵌";
  if (value === "incompatible") return "不兼容";
  return value;
}

interface ModDiscoveryPanelProps {
  gameDir: string;
  gameDirLabel: string;
  selectedInstance: InstanceInfo | null;
  curseforgeApiKey: string;
  onGoSettings: () => void;
  onGoLocal: () => void;
  onNotify: (kind: NoticeKind, message: string) => void;
  onBusyChange: (busy: boolean) => void;
}

export function ModDiscoveryPanel({
  gameDir,
  gameDirLabel,
  selectedInstance,
  curseforgeApiKey,
  onGoSettings,
  onGoLocal,
  onNotify,
  onBusyChange
}: ModDiscoveryPanelProps) {
  const [source, setSource] = useState<ModSource>("modrinth");
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<ModProject[]>([]);
  const [total, setTotal] = useState(0);
  const [offset, setOffset] = useState(0);
  const [searchLoading, setSearchLoading] = useState(false);
  const [searchError, setSearchError] = useState<string | null>(null);
  const [searched, setSearched] = useState(false);
  const [selectedProject, setSelectedProject] = useState<ModProject | null>(null);
  const [versions, setVersions] = useState<ModVersionInfo[]>([]);
  const [selectedVersion, setSelectedVersion] = useState<ModVersionInfo | null>(null);
  const [versionsLoading, setVersionsLoading] = useState(false);
  const [versionsError, setVersionsError] = useState<string | null>(null);
  const [plan, setPlan] = useState<ModDependencyPlan | null>(null);
  const [planLoading, setPlanLoading] = useState(false);
  const [planError, setPlanError] = useState<string | null>(null);
  const [installDependencies, setInstallDependencies] = useState(true);
  const [installing, setInstalling] = useState(false);
  const [installProgress, setInstallProgress] = useState<ModInstallProgress | null>(null);
  const [installResult, setInstallResult] = useState<ModInstallResult | null>(null);
  const [listenerError, setListenerError] = useState<string | null>(null);
  const [installError, setInstallError] = useState<string | null>(null);
  const projectRequestRef = useRef(0);
  const planRequestRef = useRef(0);
  const searchRequestRef = useRef(0);

  const gameVersion = selectedInstance?.gameVersion?.trim() ?? "";
  const detectedLoader = selectedInstance ? loaderKindFromValue(selectedInstance.loader) : null;
  const modLoader = detectedLoader && ["fabric", "forge", "neoforge", "quilt"].includes(detectedLoader) ? detectedLoader : null;
  const contextReady = Boolean(gameVersion && modLoader);
  const providerReady = source === "modrinth" || Boolean(curseforgeApiKey.trim());

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    void listen<ModInstallProgress>("flint://mod-install-progress", (event) => {
      if (!cancelled) setInstallProgress(event.payload);
    })
      .then((removeListener) => {
        if (cancelled) removeListener();
        else unlisten = removeListener;
      })
      .catch((error: unknown) => {
        if (!cancelled) setListenerError(toErrorMessage(error));
      });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  function resetProjectState() {
    projectRequestRef.current += 1;
    planRequestRef.current += 1;
    setSelectedProject(null);
    setVersions([]);
    setSelectedVersion(null);
    setVersionsLoading(false);
    setVersionsError(null);
    setPlan(null);
    setPlanLoading(false);
    setPlanError(null);
    setInstallProgress(null);
    setInstallResult(null);
    setListenerError(null);
    setInstallError(null);
  }

  async function searchMods(nextOffset: number) {
    if (!contextReady || !modLoader || !providerReady) return;
    const requestId = searchRequestRef.current + 1;
    searchRequestRef.current = requestId;
    setSearchLoading(true);
    setSearchError(null);
    setSearched(true);
    resetProjectState();
    try {
      const page = await api.searchMods(source, query.trim(), gameVersion, modLoader, nextOffset, PAGE_SIZE);
      if (requestId !== searchRequestRef.current) return;
      setResults(page.hits);
      setTotal(page.total);
      setOffset(page.offset);
    } catch (error) {
      if (requestId !== searchRequestRef.current) return;
      setResults([]);
      setTotal(0);
      setOffset(nextOffset);
      setSearchError(toErrorMessage(error));
    } finally {
      if (requestId === searchRequestRef.current) setSearchLoading(false);
    }
  }

  function changeSource(nextSource: ModSource) {
    searchRequestRef.current += 1;
    setSource(nextSource);
    setResults([]);
    setSearched(false);
    setSearchError(null);
    setSearchLoading(false);
    setTotal(0);
    setOffset(0);
    resetProjectState();
  }

  async function resolvePlan(project: ModProject, version: ModVersionInfo) {    if (!modLoader) return;
    const requestId = planRequestRef.current + 1;
    planRequestRef.current = requestId;
    setPlanLoading(true);
    setPlanError(null);
    setPlan(null);
    setInstallError(null);
    setInstallProgress(null);
    setInstallResult(null);
    try {
      const resolved = await api.resolveModDependencies(source, project.id, version.id, gameVersion, modLoader);
      if (requestId !== planRequestRef.current) return;
      setPlan(resolved);
    } catch (error) {
      if (requestId !== planRequestRef.current) return;
      setPlanError(toErrorMessage(error));
    } finally {
      if (requestId === planRequestRef.current) setPlanLoading(false);
    }
  }

  async function openProject(project: ModProject) {
    if (!contextReady || !modLoader) return;
    const requestId = projectRequestRef.current + 1;
    projectRequestRef.current = requestId;
    planRequestRef.current += 1;
    setSelectedProject(project);
    setVersions([]);
    setSelectedVersion(null);
    setVersionsLoading(true);
    setVersionsError(null);
    setPlan(null);
    setPlanError(null);
    setInstallError(null);
    setInstallProgress(null);
    setInstallResult(null);
    try {
      const nextVersions = await api.listModVersions(source, project.id, gameVersion, modLoader);
      if (requestId !== projectRequestRef.current) return;
      setVersions(nextVersions);
      const first = nextVersions[0] ?? null;
      setSelectedVersion(first);
      if (first) await resolvePlan(project, first);
    } catch (error) {
      if (requestId === projectRequestRef.current) setVersionsError(toErrorMessage(error));
    } finally {
      if (requestId === projectRequestRef.current) setVersionsLoading(false);
    }
  }

  async function chooseVersion(version: ModVersionInfo) {
    if (!selectedProject) return;
    setSelectedVersion(version);
    await resolvePlan(selectedProject, version);
  }

  async function installMod() {
    if (installing || !selectedProject || !selectedVersion || !modLoader) return;
    if (installDependencies && !plan) {
      setPlanError("请先完成依赖检查，或取消同时安装必需依赖。");
      return;
    }
    setInstalling(true);
    onBusyChange(true);
    setInstallError(null);
    setInstallProgress(null);
    setInstallResult(null);
    try {
      const result = await api.installMod(
        gameDir,
        source,
        selectedProject.id,
        selectedVersion.id,
        gameVersion,
        modLoader,
        installDependencies
      );
      setInstallResult(result);
      onNotify("success", `${selectedProject.name} 安装完成。`);
    } catch (error) {
      const message = toErrorMessage(error);
      setInstallProgress(null);
      setInstallError(`模组安装失败：${message}`);
      onNotify("error", `模组安装失败：${message}`);
    } finally {
      setInstalling(false);
      onBusyChange(false);
    }
  }

  const percent = installProgress ? progressPercent(installProgress.completed, installProgress.total) : null;
  const canInstall = Boolean(selectedProject && selectedVersion && !installing && (!installDependencies || plan) && selectedVersion.downloadUrl);
  return (
    <div className="download-tab-panel mod-discovery-panel">
      <div className="mod-context">
        <div><span>当前实例</span><strong>{selectedInstance?.name ?? "未选择"}</strong></div>
        <div><span>游戏版本</span><strong>{gameVersion || "未知"}</strong></div>
        <div><span>加载器</span><strong>{detectedLoader ? loaderKindLabel(detectedLoader) : "未识别"}</strong></div>
        <div><span>安装目录</span><strong title={gameDirLabel}>{gameDirLabel}</strong></div>
      </div>

      {!contextReady ? (
        <div className="state-panel compact-state">
          <Icon name="package" />
          <strong>需要选择一个带加载器的本地版本</strong>
          <p>模组搜索使用当前实例的 Minecraft 版本和加载器。请先在本地版本中选择 Fabric、Forge、NeoForge 或 Quilt 实例。</p>
          <button type="button" className="button secondary" onClick={onGoLocal}>前往本地版本</button>
        </div>
      ) : (
        <>
          <div className="mods-toolbar">
            <div className="source-switch" role="group" aria-label="模组来源">
              <button type="button" className={source === "modrinth" ? "active" : ""} aria-pressed={source === "modrinth"} onClick={() => changeSource("modrinth")} disabled={installing}>Modrinth</button>
              <button type="button" className={source === "curseforge" ? "active" : ""} aria-pressed={source === "curseforge"} onClick={() => changeSource("curseforge")} disabled={installing}>CurseForge</button>
            </div>
            <form className="mod-search-form" onSubmit={(event) => { event.preventDefault(); void searchMods(0); }}>
              <label className="search-box">
                <span className="sr-only">搜索模组</span>
                <Icon name="search" />
                <input type="search" value={query} onChange={(event) => setQuery(event.target.value)} placeholder="搜索模组名称或关键词" autoComplete="off" disabled={installing || !providerReady} />
              </label>
              <button type="submit" className="button primary compact" disabled={searchLoading || !providerReady || installing}>
                {searchLoading ? <span className="spinner spinner-light" /> : <Icon name="search" />}
                {searchLoading ? "搜索中" : "搜索"}
              </button>
            </form>
            <span className="mod-context-label">{gameVersion} · {modLoader ? loaderKindLabel(modLoader) : ""}</span>
          </div>

          {!providerReady ? (
            <div className="inline-message warning" role="status">
              <Icon name="info" />
              <span>CurseForge 搜索需要 API Key。密钥仅保存在本地设置中。</span>
              <button type="button" className="text-link" onClick={onGoSettings}>前往设置</button>
            </div>
          ) : null}

          <div className="mods-grid">
            <section className="panel mod-results-panel" aria-labelledby="mod-results-title">
              <div className="panel-head">
                <span id="mod-results-title">搜索结果</span>
                <span>{searched ? `共 ${total} 项` : source === "modrinth" ? "Modrinth" : "CurseForge"}</span>
              </div>

              {searchLoading ? (
                <div className="mod-panel-state"><span className="spinner" /><strong>正在搜索模组</strong></div>
              ) : searchError ? (
                <div className="mod-panel-state error" role="alert">
                  <Icon name="alert" />
                  <strong>搜索失败</strong>
                  <p>{searchError}</p>
                  <button type="button" className="button compact" onClick={() => void searchMods(offset)}>重试</button>
                </div>
              ) : !providerReady ? (
                <div className="mod-panel-state"><Icon name="info" /><strong>需要 API Key</strong><p>配置 CurseForge API Key 后即可搜索。</p></div>
              ) : !searched ? (
                <div className="mod-panel-state"><Icon name="search" /><strong>输入关键词开始搜索</strong><p>结果会按当前实例的游戏版本和加载器过滤。</p></div>
              ) : results.length === 0 ? (
                <div className="mod-panel-state"><Icon name="search" /><strong>没有匹配的模组</strong><p>尝试更换关键词或检查游戏版本和加载器。</p></div>
              ) : (
                <div className="mod-project-list">
                  {results.map((project) => (
                    <button key={`${project.source}:${project.id}`} type="button" className={`mod-project-row${selectedProject?.id === project.id ? " active" : ""}`} onClick={() => void openProject(project)} disabled={installing} aria-pressed={selectedProject?.id === project.id}>
                      <span className="mod-project-icon">
                        <span aria-hidden="true">{project.name.slice(0, 1).toUpperCase()}</span>
                        {project.iconUrl ? <img src={project.iconUrl} alt="" loading="lazy" onError={(event) => { event.currentTarget.style.display = "none"; }} /> : null}
                      </span>
                      <span className="mod-project-copy">
                        <span className="name-line"><strong>{project.name}</strong><span className="source-tag">{project.source === "modrinth" ? "Modrinth" : "CurseForge"}</span></span>
                        <small title={project.summary}>{project.summary}</small>
                        <span className="mod-project-meta">{project.author || "作者未知"} · {project.downloads.toLocaleString()} 次下载</span>
                      </span>
                    </button>
                  ))}
                </div>
              )}

              <div className="mod-pager">
                <button type="button" className="button compact" disabled={searchLoading || installing || offset <= 0} onClick={() => void searchMods(Math.max(0, offset - PAGE_SIZE))}>上一页</button>
                <span>{searched ? `${total === 0 ? 0 : offset + 1}–${Math.min(offset + results.length, total)} / ${total}` : "0 / 0"}</span>
                <button type="button" className="button compact" disabled={searchLoading || installing || offset + PAGE_SIZE >= total} onClick={() => void searchMods(offset + PAGE_SIZE)}>下一页</button>
              </div>
            </section>
            <section className="panel mod-detail-panel" aria-labelledby="mod-detail-title">
              <div className="panel-head">
                <span id="mod-detail-title">{selectedProject?.name ?? "模组详情"}</span>
                <span>{selectedProject ? selectedProject.author || "作者未知" : "选择搜索结果"}</span>
              </div>

              <div className="mod-detail-scroll">
                {!selectedProject ? (
                  <div className="mod-panel-state"><Icon name="package" /><strong>尚未选择模组</strong><p>从搜索结果中选择项目以查看版本、依赖和安装状态。</p></div>
                ) : (
                  <>
                    <div className="mod-detail-summary">
                      <p>{selectedProject.summary}</p>
                      <div className="mod-detail-meta">
                        <span>{selectedProject.categories.slice(0, 4).join(" · ") || "未分类"}</span>
                        <span>{selectedProject.downloads.toLocaleString()} 次下载</span>
                      </div>
                    </div>

                    <div className="mod-subhead">
                      <span>可用版本</span>
                      <span>{versionsLoading ? "读取中" : `${versions.length} 个`}</span>
                    </div>

                    {versionsLoading ? (
                      <div className="inline-loading"><span className="spinner" /> 正在读取模组版本…</div>
                    ) : versionsError ? (
                      <div className="inline-message error compact" role="alert">
                        <Icon name="alert" /><span>{versionsError}</span>
                        <button type="button" className="text-link" onClick={() => void openProject(selectedProject)}>重试</button>
                      </div>
                    ) : versions.length === 0 ? (
                      <div className="empty-line">没有匹配当前游戏版本和加载器的文件。</div>
                    ) : (
                      <div className="mod-version-list">
                        {versions.map((version) => (
                          <button key={version.id} type="button" className={`mod-version-row${selectedVersion?.id === version.id ? " active" : ""}`} onClick={() => void chooseVersion(version)} disabled={installing} aria-pressed={selectedVersion?.id === version.id}>
                            <span className="name-line">
                              <strong>{version.versionNumber}</strong>
                              <span className="source-tag">{version.releaseType || "release"}</span>
                            </span>
                            <small title={version.fileName}>{version.fileName}</small>
                            <span className="mod-version-meta">
                              <span>{version.gameVersions.slice(0, 3).join(", ") || "版本未知"}</span>
                              <span>{version.loaders.join(", ") || "加载器未知"}</span>
                            </span>
                          </button>
                        ))}
                      </div>
                    )}

                    {selectedVersion ? (
                      <>
                        <div className="mod-subhead">
                          <span>依赖检查</span>
                          <span>{planLoading ? "检查中" : plan ? "已完成" : "待检查"}</span>
                        </div>

                        {planLoading ? (
                          <div className="inline-loading"><span className="spinner" /> 正在解析依赖…</div>
                        ) : planError ? (
                          <div className="inline-message error compact" role="alert">
                            <Icon name="alert" /><span>{planError}</span>
                            <button type="button" className="text-link" onClick={() => selectedProject && void resolvePlan(selectedProject, selectedVersion)}>重试</button>
                          </div>
                        ) : plan ? (
                          <div className="dependency-review">
                            <div className="dependency-counts">
                              <span>必需 {plan.required.length}</span>
                              <span>可选 {plan.optional.length}</span>
                              <span>冲突 {plan.conflicts.length}</span>
                            </div>
                            {plan.required.length > 0 ? (
                              <div className="dependency-list">
                                {plan.required.map((dependency, index) => (
                                  <div key={`${dependency.source}:${dependency.id}:${index}`}>
                                    <span>{dependency.name}</span>
                                    <small>{dependency.versionNumber || dependency.id}</small>
                                  </div>
                                ))}
                              </div>
                            ) : <p className="dependency-empty">没有需要额外安装的必需依赖。</p>}
                            {plan.optional.length > 0 ? (
                              <div className="dependency-optional">
                                <strong>可选依赖</strong>
                                {plan.optional.map((dependency, index) => (
                                  <div key={`${dependency.source}:${dependency.projectId}:${index}`}>
                                    <span>{dependency.name}</span>
                                    <small>{dependencyTypeLabel(dependency.dependencyType)}</small>
                                  </div>
                                ))}
                              </div>
                            ) : null}
                            {plan.conflicts.length > 0 ? (
                              <div className="dependency-conflicts">
                                <strong>冲突</strong>
                                {plan.conflicts.map((conflict, index) => <p key={`conflict:${index}`}>{conflict}</p>)}
                              </div>
                            ) : null}
                            {plan.warnings.length > 0 ? (
                              <div className="dependency-warnings">
                                <strong>提示</strong>
                                {plan.warnings.map((warning, index) => <p key={`warning:${index}`}>{warning}</p>)}
                              </div>
                            ) : null}
                          </div>
                        ) : (
                          <div className="empty-line">选择版本后会自动检查必需依赖。</div>
                        )}

                        <div className="mod-install-controls">
                          <label className="checkbox-line">
                            <input type="checkbox" checked={installDependencies} onChange={(event) => setInstallDependencies(event.target.checked)} disabled={installing} />
                            <span>同时安装必需依赖</span>
                          </label>
                          <button type="button" className="button primary" onClick={() => void installMod()} disabled={!canInstall}>
                            {installing ? <span className="spinner spinner-light" /> : <Icon name="download" />}
                            {installing ? "正在安装" : "安装模组"}
                          </button>
                        </div>
                        {!selectedVersion.downloadUrl ? (
                          <div className="inline-message error compact" role="alert">
                            <Icon name="alert" /><span>该版本没有可用的安全下载地址。</span>
                          </div>
                        ) : null}
                      </>
                    ) : null}

                    {installError ? (
                      <div className="inline-message error compact" role="alert">
                        <Icon name="alert" /><span>{installError}</span>
                      </div>
                    ) : null}

                    {listenerError ? (
                      <div className="inline-message error compact" role="alert">
                        <Icon name="alert" /><span>无法订阅模组安装进度：{listenerError}</span>
                      </div>
                    ) : null}

                    {installProgress ? (
                      <div className="install-progress-panel">
                        <strong>{installProgress.message || "正在安装模组"}</strong>
                        <p>{installProgress.projectName} · {installProgress.fileName}</p>
                        <div className={`progress-bar${percent === null ? " indeterminate" : ""}`} role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={percent === null ? undefined : Math.round(percent)} aria-label="模组安装进度">
                          <span style={percent === null ? undefined : { width: `${percent}%` }} />
                        </div>
                        <div className="progress-meta">
                          <span>{installProgress.stage}</span>
                          <span>{installProgress.total > 0 ? `${installProgress.completed} / ${installProgress.total}` : "准备中"}</span>
                        </div>
                      </div>
                    ) : null}

                    {installResult ? (
                      <div className="mod-install-result">
                        <Icon name="check" />
                        <div>
                          <strong>{installResult.root.name} 已安装</strong>
                          <p>依赖 {installResult.dependencies.length} 个 · 跳过 {installResult.skipped.length} 个</p>
                          {installResult.skipped.length > 0 ? (
                            <ul className="install-skipped">
                              {installResult.skipped.map((item, index) => <li key={`skipped:${index}`}>{item}</li>)}
                            </ul>
                          ) : null}
                          {installResult.warnings.map((warning, index) => <small key={`result-warning:${index}`}>{warning}</small>)}
                        </div>
                      </div>
                    ) : null}
                  </>
                )}
              </div>
            </section>
          </div>
        </>
      )}
    </div>
  );
}

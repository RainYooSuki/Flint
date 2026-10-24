import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api } from "../../../api";
import { Icon } from "../../../components/Icon";
import type { InstanceInfo, InstalledModInfo, NoticeKind } from "../../../types";
import { joinPath, loaderLabel, toErrorMessage } from "../../../utils";

type ModFilter = "all" | "enabled" | "disabled";

interface InstalledModsPanelProps {
  gameDir: string;
  gameDirPath: string;
  gameDirLabel: string;
  selectedInstance: InstanceInfo | null;
  onOpenPath: (path: string, label: string) => void;
  onNotify: (kind: NoticeKind, message: string) => void;
}

export function InstalledModsPanel({
  gameDir,
  gameDirPath,
  gameDirLabel,
  selectedInstance,
  onOpenPath,
  onNotify
}: InstalledModsPanelProps) {
  const [mods, setMods] = useState<InstalledModInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<ModFilter>("all");
  const [busyFile, setBusyFile] = useState<string | null>(null);
  const requestIdRef = useRef(0);

  const loadMods = useCallback(async (announce = false) => {
    const requestId = requestIdRef.current + 1;
    requestIdRef.current = requestId;
    setLoading(true);
    setError(null);
    try {
      const next = await api.listInstalledMods(gameDir);
      if (requestId !== requestIdRef.current) return;
      setMods(next);
      if (announce) onNotify("success", `已刷新已安装模组，共 ${next.length} 个。`);
    } catch (error) {
      if (requestId !== requestIdRef.current) return;
      setError(toErrorMessage(error));
    } finally {
      if (requestId === requestIdRef.current) setLoading(false);
    }
  }, [gameDir, onNotify]);

  useEffect(() => {
    void loadMods();
  }, [loadMods]);

  const filteredMods = useMemo(() => {
    const normalizedQuery = query.trim().toLowerCase();
    return mods.filter((mod) => {
      const matchesQuery = !normalizedQuery
        || mod.name.toLowerCase().includes(normalizedQuery)
        || mod.fileName.toLowerCase().includes(normalizedQuery)
        || (mod.modId?.toLowerCase().includes(normalizedQuery) ?? false)
        || (mod.version?.toLowerCase().includes(normalizedQuery) ?? false);
      const matchesFilter = filter === "all" || (filter === "enabled" ? mod.enabled : !mod.enabled);
      return matchesQuery && matchesFilter;
    });
  }, [mods, query, filter]);

  async function toggleMod(mod: InstalledModInfo) {
    if (busyFile) return;
    setBusyFile(mod.fileName);
    setError(null);
    try {
      const updated = await api.setModEnabled(gameDir, mod.fileName, !mod.enabled);
      setMods((current) => current.map((item) => item.fileName === mod.fileName ? updated : item));
      onNotify("success", `${updated.name} 已${updated.enabled ? "启用" : "禁用"}。`);
    } catch (error) {
      setError(`切换模组状态失败：${toErrorMessage(error)}`);
    } finally {
      setBusyFile(null);
    }
  }

  async function deleteMod(mod: InstalledModInfo) {
    if (busyFile) return;
    if (!window.confirm(`确定删除模组 ${mod.fileName} 吗？此操作不可撤销。`)) return;
    setBusyFile(mod.fileName);
    setError(null);
    try {
      await api.deleteMod(gameDir, mod.fileName);
      setMods((current) => current.filter((item) => item.fileName !== mod.fileName));
      onNotify("success", `${mod.name} 已删除。`);
    } catch (error) {
      setError(`删除模组失败：${toErrorMessage(error)}`);
    } finally {
      setBusyFile(null);
    }
  }

  const modsDirectory = gameDirPath ? joinPath(gameDirPath, "mods") : "";
  const loaderName = selectedInstance ? loaderLabel(selectedInstance.loader) : "未选择实例";

  return (
    <div className="download-tab-panel installed-mods-panel">
      <div className="mod-context">
        <div><span>当前实例</span><strong>{selectedInstance?.name ?? "未选择"}</strong></div>
        <div><span>游戏版本</span><strong>{selectedInstance?.gameVersion || "未知"}</strong></div>
        <div><span>加载器</span><strong>{loaderName}</strong></div>
        <div><span>模组目录</span><strong title={gameDirLabel}>{gameDirLabel}</strong></div>
        <div className="mod-context-actions">
          <button type="button" className="button compact" onClick={() => modsDirectory && onOpenPath(modsDirectory, "模组目录")} disabled={!modsDirectory}>
            <Icon name="folder" /> 打开目录
          </button>
          <button type="button" className="button compact" onClick={() => void loadMods(true)} disabled={loading || Boolean(busyFile)}>
            <Icon name="refresh" className={loading ? "icon-spin" : undefined} /> 刷新
          </button>
        </div>
      </div>

      <div className="list-toolbar">
        <div className="search-box">
          <label className="sr-only" htmlFor="installed-mod-search">搜索已安装模组</label>
          <Icon name="search" />
          <input id="installed-mod-search" type="search" value={query} onChange={(event) => setQuery(event.target.value)} placeholder="搜索模组名称、ID 或文件名" autoComplete="off" />
          {query ? <button type="button" className="search-clear" onClick={() => setQuery("")} aria-label="清空搜索"><Icon name="close" /></button> : null}
        </div>
        <div className="filter-group" role="group" aria-label="模组状态筛选">
          <button type="button" className={filter === "all" ? "active" : ""} aria-pressed={filter === "all"} onClick={() => setFilter("all")}>全部</button>
          <button type="button" className={filter === "enabled" ? "active" : ""} aria-pressed={filter === "enabled"} onClick={() => setFilter("enabled")}>已启用</button>
          <button type="button" className={filter === "disabled" ? "active" : ""} aria-pressed={filter === "disabled"} onClick={() => setFilter("disabled")}>已禁用</button>
        </div>
        <span className="mod-context-label">{filteredMods.length} / {mods.length} 个模组</span>
      </div>

      {error ? (
        <div className="inline-message error" role="alert">
          <Icon name="alert" /><span>{error}</span>
          <button type="button" className="text-link" onClick={() => void loadMods()}>重试</button>
        </div>
      ) : null}

      {loading && mods.length === 0 ? (
        <div className="state-panel compact-state"><span className="spinner state-spinner" /><strong>正在读取已安装模组</strong><p>扫描当前游戏目录的 mods 文件夹。</p></div>
      ) : mods.length === 0 ? (
        <div className="state-panel compact-state"><Icon name="package" /><strong>尚未安装模组</strong><p>在模组发现页搜索并安装模组后，会显示在这里。</p></div>
      ) : filteredMods.length === 0 ? (
        <div className="state-panel compact-state"><Icon name="search" /><strong>没有匹配的模组</strong><p>请调整搜索关键词或状态筛选。</p><button type="button" className="button compact" onClick={() => { setQuery(""); setFilter("all"); }}>清除筛选</button></div>
      ) : (
        <div className="data-table installed-mods-table">
          <div className="data-head installed-cols" aria-hidden="true">
            <span>模组</span><span>版本</span><span>加载器</span><span>来源</span><span>状态</span><span>操作</span>
          </div>
          {filteredMods.map((mod) => (
            <div key={mod.fileName} className="data-row installed-mod-row installed-cols" aria-busy={busyFile === mod.fileName || undefined}>
              <span className="name-cell">
                <span className="row-icon"><Icon name="package" /></span>
                <span className="name-copy">
                  <span className="name-line"><strong>{mod.name}</strong></span>
                  <small title={mod.path}>{mod.fileName}</small>
                </span>
              </span>
              <span className="cell-muted" title={mod.version || undefined}>{mod.version || "未知"}</span>
              <span className="cell-muted">{mod.loader || "未知"}</span>
              <span className="cell-muted">{mod.source === "curseforge" ? "CurseForge" : mod.source === "modrinth" ? "Modrinth" : "本地"}</span>
              <span className={busyFile === mod.fileName ? "state-text" : mod.enabled ? "state-text ok" : "state-text warn"}><span className="inline-dot" />{busyFile === mod.fileName ? "处理中" : mod.enabled ? "已启用" : "已禁用"}</span>
              <span className="row-actions">
                <button type="button" className="button compact" onClick={() => void toggleMod(mod)} disabled={loading || Boolean(busyFile)} aria-label={`${mod.enabled ? "禁用" : "启用"} ${mod.name}`}>
                  {mod.enabled ? "禁用" : "启用"}
                </button>
                <button type="button" className="icon-button danger" onClick={() => void deleteMod(mod)} disabled={loading || Boolean(busyFile)} aria-label={`删除模组 ${mod.name}`} title="删除模组">
                  <Icon name="trash" />
                </button>
              </span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

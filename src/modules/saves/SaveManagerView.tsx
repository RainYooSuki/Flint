import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { api } from "../../api";
import { Icon } from "../../components/Icon";
import type { InstanceInfo, NoticeKind, SaveInfo, SaveTransferResult } from "../../types";
import { formatBytes, formatUnixMilliseconds, gameModeLabel, toErrorMessage } from "../../utils";

interface SaveManagerViewProps {
  gameDir: string;
  gameDirPath: string;
  gameDirLabel: string;
  selectedInstance: InstanceInfo | null;
  onOpenPath: (path: string, label: string) => void;
  onNotify: (kind: NoticeKind, message: string) => void;
}

export function SaveManagerView({
  gameDir,
  gameDirPath,
  gameDirLabel,
  selectedInstance,
  onOpenPath,
  onNotify
}: SaveManagerViewProps) {
  const [saves, setSaves] = useState<SaveInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [busyFolder, setBusyFolder] = useState<string | null>(null);
  const [importing, setImporting] = useState(false);
  const [transferResult, setTransferResult] = useState<SaveTransferResult | null>(null);
  const [renameTarget, setRenameTarget] = useState<SaveInfo | null>(null);
  const [renameValue, setRenameValue] = useState("");
  const [renameError, setRenameError] = useState<string | null>(null);
  const [renaming, setRenaming] = useState(false);
  const requestIdRef = useRef(0);
  const renameInputRef = useRef<HTMLInputElement>(null);
  const renameDialogRef = useRef<HTMLElement>(null);

  const loadSaves = useCallback(async (announce = false) => {
    const requestId = requestIdRef.current + 1;
    requestIdRef.current = requestId;
    setLoading(true);
    setError(null);
    setActionError(null);
    try {
      const next = await api.listSaves(gameDir);
      if (requestId !== requestIdRef.current) return;
      setSaves(next);
      if (announce) onNotify("success", `已刷新存档，共 ${next.length} 个。`);
    } catch (loadError) {
      if (requestId !== requestIdRef.current) return;
      setError(toErrorMessage(loadError));
    } finally {
      if (requestId === requestIdRef.current) setLoading(false);
    }
  }, [gameDir, onNotify]);

  useEffect(() => {
    void loadSaves();
  }, [loadSaves]);

  useEffect(() => {
    if (!renameTarget) return;
    renameInputRef.current?.focus();
    renameInputRef.current?.select();
    function onKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape" && !renaming) {
        setRenameTarget(null);
        return;
      }
      if (event.key !== "Tab" || !renameDialogRef.current) return;
      const focusable = Array.from(renameDialogRef.current.querySelectorAll<HTMLElement>("button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), a[href]")).filter((element) => element.offsetParent !== null);
      if (focusable.length === 0) return;
      const first = focusable[0];
      const last = focusable[focusable.length - 1];
      if (event.shiftKey && (document.activeElement === first || !renameDialogRef.current.contains(document.activeElement))) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [renameTarget, renaming]);

  const filteredSaves = useMemo(() => {
    const normalizedQuery = query.trim().toLowerCase();
    if (!normalizedQuery) return saves;
    return saves.filter((save) => (
      save.displayName.toLowerCase().includes(normalizedQuery)
      || save.folderName.toLowerCase().includes(normalizedQuery)
      || (save.versionName?.toLowerCase().includes(normalizedQuery) ?? false)
    ));
  }, [saves, query]);

  async function backupSave(save: SaveInfo) {
    if (busyFolder) return;
    setBusyFolder(`backup:${save.folderName}`);
    setActionError(null);
    try {
      const result = await api.backupSave(gameDir, save.folderName);
      setTransferResult(result);
      onNotify("success", `存档 ${save.displayName} 已备份。`);
    } catch (backupError) {
      setActionError(`备份存档失败：${toErrorMessage(backupError)}`);
    } finally {
      setBusyFolder(null);
    }
  }

  async function exportSave(save: SaveInfo) {
    if (busyFolder) return;
    try {
      const destination = await saveDialog({
        title: "导出 Minecraft 存档",
        defaultPath: `${save.displayName || save.folderName}.zip`,
        filters: [{ name: "ZIP 存档", extensions: ["zip"] }]
      });
      if (typeof destination !== "string") return;
      setBusyFolder(`export:${save.folderName}`);
      setActionError(null);
      const result = await api.exportSave(gameDir, save.folderName, destination);
      setTransferResult(result);
      onNotify("success", `存档 ${save.displayName} 已导出。`);
    } catch (exportError) {
      setActionError(`导出存档失败：${toErrorMessage(exportError)}`);
    } finally {
      setBusyFolder(null);
    }
  }

  async function importSave() {
    if (importing) return;
    try {
      const archivePath = await openDialog({
        title: "导入 Minecraft 存档 ZIP",
        multiple: false,
        directory: false,
        filters: [{ name: "ZIP 存档", extensions: ["zip"] }]
      });
      if (typeof archivePath !== "string") return;
      if (!window.confirm("导入到当前游戏目录？导入会创建独立副本，不会覆盖现有存档。")) return;
      setImporting(true);
      setActionError(null);
      const imported = await api.importSave(gameDir, archivePath);
      setTransferResult({ path: imported.path, save: imported });
      await loadSaves();
      onNotify("success", `存档 ${imported.displayName} 已导入。`);
    } catch (importError) {
      setActionError(`导入存档失败：${toErrorMessage(importError)}`);
    } finally {
      setImporting(false);
    }
  }

  function beginRename(save: SaveInfo) {
    setRenameTarget(save);
    setRenameValue(save.folderName);
    setRenameError(null);
  }

  async function confirmRename() {
    if (!renameTarget || renaming) return;
    const nextName = renameValue.trim();
    if (!nextName) {
      setRenameError("请输入新的存档名称。");
      return;
    }
    setRenaming(true);
    setRenameError(null);
    try {
      const updated = await api.renameSave(gameDir, renameTarget.folderName, nextName);
      setSaves((current) => current.map((save) => save.folderName === renameTarget.folderName ? updated : save));
      setRenameTarget(null);
      onNotify("success", `存档文件夹已重命名为 ${updated.folderName}。`);
    } catch (renameFailure) {
      setRenameError(`重命名失败：${toErrorMessage(renameFailure)}`);
    } finally {
      setRenaming(false);
    }
  }

  async function deleteSave(save: SaveInfo) {
    if (busyFolder) return;
    if (!window.confirm(`确定删除存档 ${save.displayName} 吗？此操作不可撤销。`)) return;
    setBusyFolder(`delete:${save.folderName}`);
    setActionError(null);
    try {
      await api.deleteSave(gameDir, save.folderName);
      setSaves((current) => current.filter((item) => item.folderName !== save.folderName));
      onNotify("success", `存档 ${save.displayName} 已删除。`);
    } catch (deleteError) {
      setActionError(`删除存档失败：${toErrorMessage(deleteError)}`);
    } finally {
      setBusyFolder(null);
    }
  }

  return (    <div className="page-shell saves-page">
      <header className="page-head">
        <div className="page-title">
          <h1>存档</h1>
          <span>{selectedInstance ? `${selectedInstance.name} · ${selectedInstance.gameVersion || "版本未知"}` : "未选择本地实例"}</span>
        </div>
        <div className="page-head-actions">
          <button type="button" className="button primary compact" onClick={() => void importSave()} disabled={importing || loading || Boolean(busyFolder)}>
            {importing ? <span className="spinner spinner-light" /> : <Icon name="download" />}
            {importing ? "导入中" : "导入存档"}
          </button>
          <button type="button" className="button compact" onClick={() => void loadSaves(true)} disabled={loading || importing || Boolean(busyFolder)}>
            <Icon name="refresh" className={loading ? "icon-spin" : undefined} /> 刷新
          </button>
        </div>
      </header>

      <div className="page-body saves-content">
        <div className="save-context">
          <div><span>当前实例</span><strong>{selectedInstance?.name ?? "未选择"}</strong></div>
          <div><span>游戏版本</span><strong>{selectedInstance?.gameVersion || "未知"}</strong></div>
          <div className="save-context-path"><span>游戏目录</span><strong title={gameDirLabel}>{gameDirLabel}</strong></div>
          <button type="button" className="button compact" onClick={() => gameDirPath && onOpenPath(gameDirPath, "游戏目录")} disabled={!gameDirPath}>
            <Icon name="folder" /> 打开目录
          </button>
        </div>

        <div className="list-toolbar">
          <div className="search-box">
            <label className="sr-only" htmlFor="save-search">搜索存档</label>
            <Icon name="search" />
            <input id="save-search" type="search" value={query} onChange={(event) => setQuery(event.target.value)} placeholder="搜索存档名称、文件夹或版本" autoComplete="off" />
            {query ? <button type="button" className="search-clear" onClick={() => setQuery("")} aria-label="清空搜索"><Icon name="close" /></button> : null}
          </div>
          <span className="mod-context-label">{filteredSaves.length} / {saves.length} 个存档</span>
        </div>

        {actionError ? (
          <div className="inline-message error" role="alert">
            <Icon name="alert" /><span>{actionError}</span>
          </div>
        ) : null}

        {transferResult ? (
          <div className="save-transfer-result">
            <Icon name="check" />
            <div>
              <strong>{transferResult.save ? `${transferResult.save.displayName} 已写入` : "存档操作已完成"}</strong>
              <code title={transferResult.path}>{transferResult.path}</code>
            </div>
            <button type="button" className="button compact" onClick={() => onOpenPath(transferResult.path, "存档位置")}>打开位置</button>
          </div>
        ) : null}

        {loading && saves.length === 0 ? (
          <div className="state-panel compact-state">
            <span className="spinner state-spinner" />
            <strong>正在读取存档</strong>
            <p>扫描当前游戏目录的 saves 文件夹。</p>
          </div>
        ) : error ? (
          <div className="state-panel compact-state" role="alert">
            <Icon name="alert" />
            <strong>存档读取失败</strong>
            <p>{error}</p>
            <button type="button" className="button compact" onClick={() => void loadSaves()}>重试</button>
          </div>
        ) : saves.length === 0 ? (
          <div className="state-panel compact-state">
            <Icon name="save" />
            <strong>没有本地存档</strong>
            <p>启动游戏并创建世界后，存档会显示在这里；也可以导入 ZIP 存档。</p>
            <button type="button" className="button compact" onClick={() => void importSave()} disabled={importing}>导入存档</button>
          </div>
        ) : filteredSaves.length === 0 ? (
          <div className="state-panel compact-state">
            <Icon name="search" />
            <strong>没有匹配的存档</strong>
            <p>请调整搜索关键词。</p>
            <button type="button" className="button compact" onClick={() => setQuery("")}>清除搜索</button>
          </div>
        ) : (
          <div className="save-list">
            {filteredSaves.map((save) => {
              const actionDisabled = loading || Boolean(busyFolder) || importing || renaming;
              return (
                <article key={save.folderName} className="save-row" aria-busy={busyFolder?.endsWith(`:${save.folderName}`) || undefined}>
                  <span className="save-icon" aria-hidden="true">
                    <Icon name="save" />
                    {save.iconDataUrl ? <img src={save.iconDataUrl} alt="" onError={(event) => { event.currentTarget.style.display = "none"; }} /> : null}
                  </span>
                  <div className="save-main">
                    <div className="save-title-line">
                      <strong>{save.displayName}</strong>
                      {save.hardcore ? <span className="hardcore-tag">极限</span> : null}
                    </div>
                    <small className="save-folder" title={save.folderName}>{save.folderName}</small>
                    <div className="save-meta">
                      <span>上次游玩 {formatUnixMilliseconds(save.lastPlayed)}</span>
                      <span>{gameModeLabel(save.gameMode)}</span>
                      <span>{formatBytes(save.sizeBytes)}</span>
                      <span>{save.versionName || "版本未知"}</span>
                    </div>
                  </div>
                  <div className="save-actions">
                    <button type="button" className="button compact" onClick={() => onOpenPath(save.path, "存档目录")} aria-label={`打开存档 ${save.displayName}`}>打开</button>
                    <button type="button" className="button compact" onClick={() => void backupSave(save)} disabled={actionDisabled} aria-label={`备份存档 ${save.displayName}`}>
                      {busyFolder === `backup:${save.folderName}` ? <span className="spinner spinner-small" /> : null}
                      备份
                    </button>
                    <button type="button" className="button compact" onClick={() => void exportSave(save)} disabled={actionDisabled} aria-label={`导出存档 ${save.displayName}`}>
                      {busyFolder === `export:${save.folderName}` ? <span className="spinner spinner-small" /> : null}
                      导出
                    </button>
                    <button type="button" className="button compact" onClick={() => beginRename(save)} disabled={actionDisabled} aria-label={`重命名存档 ${save.displayName}`}>重命名</button>
                    <button type="button" className="icon-button danger" onClick={() => void deleteSave(save)} disabled={actionDisabled} aria-label={`删除存档 ${save.displayName}`} title="删除存档">
                      {busyFolder === `delete:${save.folderName}` ? <span className="spinner spinner-small" aria-hidden="true" /> : <Icon name="trash" />}
                    </button>
                  </div>
                </article>
              );
            })}
          </div>
        )}
      </div>

      {renameTarget ? (
        <div className="modal-backdrop" role="presentation" onMouseDown={(event) => { if (event.target === event.currentTarget && !renaming) setRenameTarget(null); }}>
          <section ref={renameDialogRef} className="modal small-modal" role="dialog" aria-modal="true" aria-labelledby="save-rename-title">
            <header className="modal-head">
              <div>
                <h2 id="save-rename-title">重命名存档</h2>
                <span>{renameTarget.folderName}</span>
              </div>
              <button type="button" className="modal-close" onClick={() => setRenameTarget(null)} disabled={renaming} aria-label="关闭重命名窗口"><Icon name="close" /></button>
            </header>
            <form className="modal-body rename-form" onSubmit={(event) => { event.preventDefault(); void confirmRename(); }}>
              <label htmlFor="save-rename-input">新的文件夹名称</label>
              <input
                ref={renameInputRef}
                id="save-rename-input"
                type="text"
                value={renameValue}
                onChange={(event) => setRenameValue(event.target.value)}
                maxLength={160}
                disabled={renaming}
                autoComplete="off"
              />
              {renameError ? <div className="inline-message error compact" role="alert"><Icon name="alert" /><span>{renameError}</span></div> : null}
              <div className="rename-note">仅重命名存档文件夹；游戏内世界名称（LevelName）不会改变。</div>
            </form>
            <footer className="modal-foot">
              <span>删除操作需要确认。</span>
              <div className="inline-actions">
                <button type="button" className="button compact" onClick={() => setRenameTarget(null)} disabled={renaming}>取消</button>
                <button type="button" className="button primary compact" onClick={() => void confirmRename()} disabled={renaming || !renameValue.trim()}>
                  {renaming ? <span className="spinner spinner-light" /> : <Icon name="edit" />}
                  {renaming ? "保存中" : "保存名称"}
                </button>
              </div>
            </footer>
          </section>
        </div>
      ) : null}
    </div>
  );
}

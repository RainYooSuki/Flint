import { useMemo } from "react";
import { Icon } from "../components/Icon";
import type { InstanceInfo, JavaInfo, LauncherInfo, LauncherSettings, LaunchResult } from "../types";
import {
  formatDate,
  formatDateTime,
  loaderLabel,
  platformLabel,
  validateUsername,
  versionTypeLabel
} from "../utils";

interface LaunchViewProps {
  info: LauncherInfo | null;
  settings: LauncherSettings | null;
  instances: InstanceInfo[];
  selectedInstance: InstanceInfo | null;
  selectedId: string;
  javas: JavaInfo[];
  instancesLoading: boolean;
  instancesError: string | null;
  launching: boolean;
  launchResult: LaunchResult | null;
  launchStartedAt: string | null;
  onSelectInstance: (id: string) => void;
  onLaunch: () => void;
  onGoVersions: () => void;
  onGoSettings: () => void;
  onRefreshInstances: () => void;
  onOpenPath: (path: string, label: string) => void;
}

export function LaunchView({
  info,
  settings,
  instances,
  selectedInstance,
  selectedId,
  javas,
  instancesLoading,
  instancesError,
  launching,
  launchResult,
  launchStartedAt,
  onSelectInstance,
  onLaunch,
  onGoVersions,
  onGoSettings,
  onRefreshInstances,
  onOpenPath
}: LaunchViewProps) {
  const selectedJava = useMemo(
    () => javas.find((java) => java.path === settings?.javaPath) ?? null,
    [javas, settings?.javaPath]
  );

  const usernameError = settings ? validateUsername(settings.username) : "设置尚未载入";
  const missingClient = selectedInstance ? !selectedInstance.hasClientJar : false;
  const gameDir = settings?.gameDir.trim() || info?.defaultGameDir || "";
  const javaReady = Boolean(settings?.javaPath.trim());
  const memoryReady = Boolean(settings && settings.minMemoryMb > 0 && settings.maxMemoryMb >= settings.minMemoryMb && settings.width > 0 && settings.height > 0);
  const canLaunch = Boolean(settings && selectedInstance && javaReady && memoryReady && !usernameError && !missingClient);

  let blocker = "";
  if (!settings) {
    blocker = "设置尚未载入，请先打开设置页。";
  } else if (instancesLoading && !selectedInstance) {
    blocker = "正在扫描本地版本。";
  } else if (!selectedInstance) {
    blocker = instancesError ? "本地版本读取失败。" : "尚未选择启动版本。";
  } else if (missingClient) {
    blocker = "所选版本缺少客户端 JAR。";
  } else if (!javaReady) {
    blocker = "尚未选择 Java 运行环境。";
  } else if (usernameError) {
    blocker = usernameError;
  } else if (!memoryReady) {
    blocker = "内存或窗口设置无效。";
  } else if (launching) {
    blocker = "正在启动游戏，请稍候。";
  } else {
    blocker = "检查完成，可以启动。";
  }

  const javaTitle = selectedJava
    ? `Java ${selectedJava.major}${selectedJava.architecture ? ` / ${selectedJava.architecture}` : ""}`
    : javaReady
      ? "自定义 Java"
      : "未选择";

  return (
    <div className="launch-layout">
      <aside className="launch-dock" aria-label="启动控制栏">
        <div className="dock-tip">
          <Icon name="info" />
          <span>{selectedInstance ? `已选择：${selectedInstance.name}` : "选择本地版本并检查运行环境"}</span>
        </div>

        <section className="offline-account" aria-label="离线账户">
          <span className="offline-account-icon"><Icon name="user" /></span>
          <span className="offline-account-copy">
            <strong>{settings?.username || "未设置用户名"}</strong>
            <small>离线账户</small>
          </span>
          <button type="button" className="small-icon-button" onClick={onGoSettings} title="编辑账户" aria-label="编辑账户">
            <Icon name="settings" />
          </button>
        </section>

        <section className="dock-section">
          <div className="dock-section-head">
            <span>当前版本</span>
            <button type="button" className="dock-link" onClick={onGoVersions}>版本列表</button>
          </div>

          {instancesLoading ? (
            <div className="dock-loading"><span className="spinner" /> 正在扫描版本…</div>
          ) : instancesError ? (
            <div className="dock-error">
              <span>{instancesError}</span>
              <button type="button" className="text-link" onClick={onRefreshInstances}>重试</button>
            </div>
          ) : instances.length === 0 ? (
            <div className="dock-empty">
              <span>未找到本地版本</span>
              <button type="button" className="text-link" onClick={onGoVersions}>前往下载</button>
            </div>
          ) : (
            <>
              <div className="compact-select">
                <select
                  aria-label="选择启动版本"
                  value={selectedId}
                  onChange={(event) => onSelectInstance(event.target.value)}
                >
                  {instances.map((instance) => (
                    <option key={instance.id} value={instance.id}>
                      {instance.name} · {loaderLabel(instance.loader)}
                    </option>
                  ))}
                </select>
                <Icon name="chevronDown" />
              </div>

              {selectedInstance ? (
                <dl className="dock-facts">
                  <div><dt>版本 ID</dt><dd title={selectedInstance.id}>{selectedInstance.id}</dd></div>
                  <div><dt>类型</dt><dd>{versionTypeLabel(selectedInstance.versionType)}</dd></div>
                  <div><dt>加载器</dt><dd>{loaderLabel(selectedInstance.loader)}</dd></div>
                  <div>
                    <dt>文件</dt>
                    <dd className={selectedInstance.hasClientJar ? "ok-text" : "warn-text"}>
                      {selectedInstance.hasClientJar ? "客户端完整" : "缺少客户端 JAR"}
                    </dd>
                  </div>
                </dl>
              ) : null}
            </>
          )}
        </section>

        <section className="dock-section">
          <div className="dock-section-head">
            <span>运行环境</span>
            <button type="button" className="dock-link" onClick={onGoSettings}>修改</button>
          </div>
          <dl className="dock-facts">
            <div><dt>Java</dt><dd title={settings?.javaPath || undefined}>{javaTitle}</dd></div>
            <div><dt>内存</dt><dd>{settings ? `${settings.minMemoryMb}–${settings.maxMemoryMb} MB` : "未设置"}</dd></div>
            <div><dt>游戏目录</dt><dd title={gameDir}>{gameDir || "平台默认目录"}</dd></div>
          </dl>
        </section>

        <div className="dock-spacer" />

        <div className="dock-actions">
          <button
            type="button"
            className="dock-button primary"
            onClick={onLaunch}
            disabled={!canLaunch || launching}
            title={canLaunch ? "启动当前版本" : blocker}
          >
            {launching ? <span className="spinner spinner-light" aria-hidden="true" /> : <Icon name="play" />}
            {launching ? "正在启动…" : "启动游戏"}
          </button>
          <button type="button" className="dock-button secondary" onClick={onGoVersions}>
            版本选择
          </button>
        </div>
      </aside>

      <section className="launch-stage" aria-label="启动状态">
        <header className="stage-head">
          <div className="stage-title">
            <span className={`stage-dot${canLaunch ? " ready" : ""}`} aria-hidden="true" />
            <h1>启动</h1>
          </div>
          <div className="stage-head-meta">
            <span>{info ? platformLabel(info.platform) : "核心连接中"}</span>
            <span className="mode-label">离线模式</span>
          </div>
        </header>

        <div className="stage-body">
          <div className="stage-center">
            {instancesError ? (
              <div className="inline-message error" role="alert">
                <Icon name="alert" />
                <span>{instancesError}</span>
                <button type="button" className="text-link" onClick={onRefreshInstances}>重试</button>
              </div>
            ) : null}

            {selectedInstance ? (
              <section className="instance-summary" aria-labelledby="current-instance-name">
                <header className="instance-summary-head">
                  <div className="instance-summary-title">
                    <span className="instance-icon"><Icon name="layers" /></span>
                    <div>
                      <h2 id="current-instance-name">{selectedInstance.name}</h2>
                      <span>{selectedInstance.id}</span>
                    </div>
                  </div>
                  <button type="button" className="plain-button" onClick={onGoVersions}>版本管理</button>
                </header>

                <dl className="instance-facts">
                  <div>
                    <dt>版本类型</dt>
                    <dd>{versionTypeLabel(selectedInstance.versionType)}</dd>
                  </div>
                  <div>
                    <dt>加载器</dt>
                    <dd>{loaderLabel(selectedInstance.loader)}</dd>
                  </div>
                  <div>
                    <dt>上次启动</dt>
                    <dd>{formatDate(selectedInstance.lastPlayed)}</dd>
                  </div>
                  <div>
                    <dt>客户端文件</dt>
                    <dd className={selectedInstance.hasClientJar ? "ok-text" : "warn-text"}>
                      {selectedInstance.hasClientJar ? "已就绪" : "不完整"}
                    </dd>
                  </div>
                  <div className="full-row">
                    <dt>游戏目录</dt>
                    <dd className="path-text" title={gameDir}>{gameDir || "平台默认目录"}</dd>
                  </div>
                </dl>
              </section>
            ) : (
              <section className="stage-empty">
                <Icon name="layers" />
                <h2>尚未选择启动版本</h2>
                <p>从下载页选择一个本地版本，或安装 Mojang 原版版本。</p>
                <button type="button" className="button secondary" onClick={onGoVersions}>打开下载页</button>
              </section>
            )}

            {launchResult ? (
              <section className="launch-result-panel" aria-labelledby="launch-result-title">
                <header>
                  <div className="result-title">
                    <Icon name="check" />
                    <h2 id="launch-result-title">游戏已启动</h2>
                  </div>
                  <span className="pid-label">PID {launchResult.pid}</span>
                </header>
                <p>{launchStartedAt ? `启动时间：${formatDateTime(launchStartedAt)}` : "已收到后端启动结果。"}</p>
                <div className="result-log-path" title={launchResult.logPath}>{launchResult.logPath || "后端未返回日志路径"}</div>
                <div className="result-actions">
                  <button type="button" className="button secondary" onClick={() => onOpenPath(launchResult.logPath, "启动日志")} disabled={!launchResult.logPath}>打开日志</button>
                  <button type="button" className="button secondary" onClick={() => onOpenPath(gameDir, "游戏目录")} disabled={!gameDir}>打开游戏目录</button>
                </div>
                <details>
                  <summary>启动命令</summary>
                  <code>{launchResult.command || "后端未返回启动命令"}</code>
                </details>
              </section>
            ) : null}
          </div>
        </div>

        <footer className="stage-footer">
          <div className={`stage-status${canLaunch ? " ready" : ""}`} aria-live="polite">
            <span className="status-dot" aria-hidden="true" />
            <span>{blocker}</span>
          </div>
          <div className="stage-actions">
            <button type="button" className="plain-button" onClick={onRefreshInstances} disabled={instancesLoading}>
              <Icon name="refresh" className={instancesLoading ? "icon-spin" : undefined} /> 刷新
            </button>
            <button type="button" className="plain-button" onClick={() => onOpenPath(gameDir, "游戏目录")} disabled={!gameDir}>
              <Icon name="folder" /> 游戏目录
            </button>
          </div>
        </footer>
      </section>
    </div>
  );
}
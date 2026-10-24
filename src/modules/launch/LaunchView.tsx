import { useMemo } from "react";
import { Icon } from "../../components/Icon";
import type { AccountInfo, InstanceInfo, JavaInfo, LauncherInfo, LauncherSettings, LaunchResult } from "../../types";
import {
  formatDate,
  formatDateTime,
  loaderLabel,
  platformLabel,
  validateUsername,
  versionTypeLabel
} from "../../utils";

interface LaunchViewProps {
  info: LauncherInfo | null;
  settings: LauncherSettings | null;
  selectedInstance: InstanceInfo | null;
  javas: JavaInfo[];
  activeAccount: AccountInfo | null;
  instancesLoading: boolean;
  instancesError: string | null;
  launching: boolean;
  launchResult: LaunchResult | null;
  launchStartedAt: string | null;
  onLaunch: () => void;
  onGoVersions: () => void;
  onGoSettings: () => void;
  onGoTools: () => void;
  onRefreshInstances: () => void;
  onOpenPath: (path: string, label: string) => void;
}

export function LaunchView({
  info,
  settings,
  selectedInstance,
  javas,
  activeAccount,
  instancesLoading,
  instancesError,
  launching,
  launchResult,
  launchStartedAt,
  onLaunch,
  onGoVersions,
  onGoSettings,
  onGoTools,
  onRefreshInstances,
  onOpenPath
}: LaunchViewProps) {
  const selectedJava = useMemo(
    () => javas.find((java) => java.path === settings?.javaPath) ?? null,
    [javas, settings?.javaPath]
  );

  const usernameError = activeAccount?.accountType === "microsoft" ? null : settings ? validateUsername(settings.username) : "设置尚未载入";
  const missingClient = selectedInstance ? !selectedInstance.hasClientJar : false;
  const gameDir = settings?.gameDir.trim() || info?.defaultGameDir || "";
  const javaReady = Boolean(settings?.javaPath.trim());
  const memoryReady = Boolean(settings && settings.minMemoryMb > 0 && settings.maxMemoryMb >= settings.minMemoryMb && settings.width > 0 && settings.height > 0);
  const canLaunch = Boolean(settings && selectedInstance && javaReady && memoryReady && !usernameError && !missingClient);

  let blocker = "";
  if (!settings) {
    blocker = "设置尚未载入";
  } else if (instancesLoading && !selectedInstance) {
    blocker = "正在扫描本地版本";
  } else if (!selectedInstance) {
    blocker = instancesError ? "本地版本读取失败" : "尚未选择启动版本";
  } else if (missingClient) {
    blocker = "所选版本缺少客户端 JAR";
  } else if (!javaReady) {
    blocker = "尚未选择 Java 运行环境";
  } else if (usernameError) {
    blocker = usernameError;
  } else if (!memoryReady) {
    blocker = "内存或窗口设置无效";
  } else if (launching) {
    blocker = "正在启动游戏";
  } else {
    blocker = "启动前检查已通过";
  }

  const javaTitle = selectedJava
    ? `Java ${selectedJava.major}${selectedJava.architecture ? ` · ${selectedJava.architecture}` : ""}`
    : javaReady
      ? "自定义 Java"
      : "未选择";

  const accountName = activeAccount?.username || settings?.username || "未设置用户名";
  const accountType = activeAccount?.accountType === "microsoft" ? "Microsoft 账户" : "离线账户";

  return (
    <div className="page-shell launch-page">
      <header className="page-head">
        <div className="page-title">
          <h1>启动</h1>
          <span>
            {activeAccount?.accountType === "microsoft" ? "Microsoft 账户" : settings ? "离线模式" : "正在读取设置"}
            {" · "}
            {info ? platformLabel(info.platform) : "核心连接中"}
            {selectedInstance ? ` · ${selectedInstance.name}` : ""}
          </span>
        </div>
        <div className={`ready${canLaunch ? " ok" : instancesError ? " error" : " pending"}`} aria-live="polite">
          <span className="ready-dot" />
          <span title={blocker}>{canLaunch ? "环境就绪" : blocker}</span>
        </div>
      </header>

      <div className="launch-content">
        <section className="instance-summary" aria-labelledby="launch-instance-title">
          <div className="summary-head">
            <div className="version-lockup">
              <strong id="launch-instance-title">{selectedInstance?.name ?? "未选择版本"}</strong>
              <span>
                {selectedInstance
                  ? `${loaderLabel(selectedInstance.loader)} · ${selectedInstance.id}`
                  : "从下载页选择一个本地版本"}
              </span>
            </div>
            <div className="summary-actions">
              <span className={`client-ready${selectedInstance ? (selectedInstance.hasClientJar ? " ok" : " warn") : ""}`}>
                {selectedInstance ? selectedInstance.hasClientJar ? "客户端完整" : "客户端文件缺失" : "未选择版本"}
              </span>
              <button type="button" className="link-button" onClick={onGoVersions}>选择版本</button>
            </div>
          </div>

          <dl className="summary-grid">
            <div>
              <dt>版本类型</dt>
              <dd>{selectedInstance ? versionTypeLabel(selectedInstance.versionType) : "—"}</dd>
            </div>
            <div>
              <dt>上次启动</dt>
              <dd>{selectedInstance ? formatDate(selectedInstance.lastPlayed) : "—"}</dd>
            </div>
            <div>
              <dt>游戏目录</dt>
              <dd className="path" title={gameDir}>{gameDir || "平台默认目录"}</dd>
            </div>
            <div>
              <dt>加载器</dt>
              <dd>{selectedInstance ? loaderLabel(selectedInstance.loader) : "—"}</dd>
            </div>
          </dl>
        </section>

        {instancesError ? (
          <div className="inline-notice error" role="alert">
            <Icon name="alert" />
            <span>{instancesError}</span>
            <button type="button" className="text-link" onClick={onRefreshInstances}>重试</button>
          </div>
        ) : null}

        <div className="lower-grid">
          <section className="panel" aria-labelledby="launch-runtime-title">
            <div className="panel-head">
              <span id="launch-runtime-title">运行环境</span>
              <span>{selectedJava ? "已检测" : javaReady ? "自定义" : "待配置"}</span>
            </div>
            <dl className="runtime-list">
              <div className="runtime-row">
                <dt>Java</dt>
                <dd title={settings?.javaPath || undefined}>{javaTitle}</dd>
              </div>
              <div className="runtime-row">
                <dt>内存</dt>
                <dd>{settings ? `${settings.minMemoryMb}–${settings.maxMemoryMb} MB` : "—"}</dd>
              </div>
              <div className="runtime-row">
                <dt>窗口</dt>
                <dd>{settings ? `${settings.width} × ${settings.height}` : "—"}</dd>
              </div>
              <div className="runtime-row runtime-log-row">
                <dt>日志</dt>
                <dd>
                  {launchResult ? (
                    <div className="runtime-result">
                      <span className="ok-text">PID {launchResult.pid}</span>
                      <button type="button" className="link-button" onClick={() => onOpenPath(launchResult.logPath, "启动日志")} disabled={!launchResult.logPath}>打开日志</button>
                      <details className="command-details">
                        <summary>命令</summary>
                        <code>{launchResult.command || "后端未返回启动命令"}</code>
                      </details>
                    </div>
                  ) : (
                    "启动后生成"
                  )}
                </dd>
              </div>
            </dl>
          </section>

          <section className="panel" aria-labelledby="launch-quick-title">
            <div className="panel-head">
              <span id="launch-quick-title">快捷操作</span>
              <span>常用</span>
            </div>
            <div className="quick-list">
              <button type="button" className="quick-button" onClick={() => onOpenPath(gameDir, "游戏目录")} disabled={!gameDir}>
                <span>打开游戏目录</span><span aria-hidden="true">›</span>
              </button>
              <button type="button" className="quick-button" onClick={onGoVersions}>
                <span>下载与版本管理</span><span aria-hidden="true">›</span>
              </button>
              <button type="button" className="quick-button" onClick={onGoSettings}>
                <span>编辑启动设置</span><span aria-hidden="true">›</span>
              </button>
              <button type="button" className="quick-button" onClick={onGoTools}>
                <span>工具与运行信息</span><span aria-hidden="true">›</span>
              </button>
            </div>
          </section>
        </div>
      </div>

      <footer className="actionbar">
        <div className="action-copy" aria-live="polite">
          <strong>{launchResult ? `游戏已启动 · PID ${launchResult.pid}` : `${accountName} · ${accountType}`}</strong>
          <span title={blocker}>
            {launchResult && launchStartedAt ? `启动时间 ${formatDateTime(launchStartedAt)}` : blocker}
          </span>
        </div>
        <div className="action-buttons">
          {launchResult ? (
            <button type="button" className="button" onClick={() => onOpenPath(launchResult.logPath, "启动日志")} disabled={!launchResult.logPath}>
              <Icon name="terminal" /> 打开日志
            </button>
          ) : null}
          <button type="button" className="button" onClick={onGoVersions}>
            <Icon name="download" /> 下载版本
          </button>
          <button type="button" className="button" onClick={() => onOpenPath(gameDir, "游戏目录")} disabled={!gameDir}>
            <Icon name="folder" /> 打开目录
          </button>
          <button type="button" className="button primary" onClick={onLaunch} disabled={!canLaunch || launching}>
            {launching ? <span className="spinner spinner-light" aria-hidden="true" /> : <Icon name="play" />}
            {launching ? "正在启动" : "启动游戏"}
          </button>
        </div>
      </footer>
    </div>
  );
}

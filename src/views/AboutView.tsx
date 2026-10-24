import { Icon } from "../components/Icon";
import type { LauncherInfo, LauncherSettings } from "../types";
import { platformLabel } from "../utils";

interface AboutViewProps {
  info: LauncherInfo | null;
  infoLoading: boolean;
  infoError: string | null;
  settings: LauncherSettings | null;
  instancesCount: number;
  javaCount: number;
  settingsDirty: boolean;
  onOpenPath: (path: string, label: string) => void;
  onRetryInfo: () => void;
  onGoSettings: () => void;
  onGoVersions: () => void;
}

export function AboutView({
  info,
  infoLoading,
  infoError,
  settings,
  instancesCount,
  javaCount,
  settingsDirty,
  onOpenPath,
  onRetryInfo,
  onGoSettings,
  onGoVersions
}: AboutViewProps) {
  const currentGameDir = settings?.gameDir.trim() || info?.defaultGameDir || "";
  const coreHealthy = Boolean(info && !infoError && !infoLoading);
  const coreStatus = infoLoading ? "连接中" : infoError ? "异常" : info ? "正常" : "不可用";
  const settingStatus = !settings ? "不可用" : settingsDirty ? "未保存" : "已同步";

  return (
    <div className="page-shell">
      <header className="page-head">
        <div className="page-title">
          <h1>工具</h1>
          <span>运行信息与诊断</span>
        </div>
      </header>

      <div className="page-body tools-content">
        <div className="tools-panel">
          <div className="tools-columns">
            <section className="tools-section" aria-labelledby="tools-runtime-title">
              <div className="panel-head">
                <span id="tools-runtime-title">运行状态</span>
                <span>本机</span>
              </div>
              <dl className="info-list">
                <div>
                  <dt>Flint 核心</dt>
                  <dd className={coreHealthy ? "ok-text" : "warn-text"}>{coreStatus}</dd>
                </div>
                <div>
                  <dt>本地版本</dt>
                  <dd>{instancesCount}</dd>
                </div>
                <div>
                  <dt>Java 环境</dt>
                  <dd>{javaCount}</dd>
                </div>
                <div>
                  <dt>设置状态</dt>
                  <dd className={!settings || settingsDirty ? "warn-text" : "ok-text"}>{settingStatus}</dd>
                </div>
              </dl>
              {infoError ? (
                <div className="inline-message error compact" role="alert">
                  <Icon name="alert" />
                  <span>{infoError}</span>
                  <button type="button" className="text-link" onClick={onRetryInfo}>重试</button>
                </div>
              ) : null}
            </section>

            <section className="tools-section" aria-labelledby="tools-version-title">
              <div className="panel-head">
                <span id="tools-version-title">版本与平台</span>
                <span>{info ? platformLabel(info.platform) : "检测中"}</span>
              </div>
              <dl className="info-list">
                <div><dt>Flint 版本</dt><dd>{info ? `v${info.version}` : "—"}</dd></div>
                <div><dt>运行平台</dt><dd>{info ? platformLabel(info.platform) : "—"}</dd></div>
                <div><dt>当前游戏目录</dt><dd className="path-value" title={currentGameDir}>{currentGameDir || "平台默认目录"}</dd></div>
                <div><dt>配置状态</dt><dd>{settings ? "已载入" : "不可用"}</dd></div>
              </dl>
            </section>
          </div>

          <section className="tools-section" aria-labelledby="tools-paths-title">
            <div className="panel-head">
              <span id="tools-paths-title">目录与文件</span>
              <span>点击右侧按钮打开</span>
            </div>
            <div className="path-rows">
              <div className="path-row">
                <div><span>数据目录</span><code title={info?.dataDir || undefined}>{info?.dataDir || "—"}</code></div>
                <button type="button" className="button" onClick={() => info?.dataDir && onOpenPath(info.dataDir, "数据目录")} disabled={!info?.dataDir}>
                  <Icon name="open" /> 打开
                </button>
              </div>
              <div className="path-row">
                <div><span>设置文件</span><code title={info?.settingsPath || undefined}>{info?.settingsPath || "—"}</code></div>
                <button type="button" className="button" onClick={() => info?.settingsPath && onOpenPath(info.settingsPath, "设置文件")} disabled={!info?.settingsPath}>
                  <Icon name="open" /> 打开
                </button>
              </div>
              <div className="path-row">
                <div><span>默认游戏目录</span><code title={info?.defaultGameDir || undefined}>{info?.defaultGameDir || "—"}</code></div>
                <button type="button" className="button" onClick={() => info?.defaultGameDir && onOpenPath(info.defaultGameDir, "默认游戏目录")} disabled={!info?.defaultGameDir}>
                  <Icon name="folder" /> 打开
                </button>
              </div>
            </div>
          </section>

          <section className="tools-section" aria-labelledby="tools-actions-title">
            <div className="panel-head">
              <span id="tools-actions-title">快捷操作</span>
              <span>常用</span>
            </div>
            <div className="quick-actions">
              <button type="button" className="quick-action" onClick={() => onOpenPath(currentGameDir, "游戏目录")} disabled={!currentGameDir}>
                <Icon name="folder" /><span>打开游戏目录</span>
              </button>
              <button type="button" className="quick-action" onClick={onGoSettings}>
                <Icon name="settings" /><span>打开启动设置</span>
              </button>
              <button type="button" className="quick-action" onClick={onGoVersions}>
                <Icon name="download" /><span>管理与下载版本</span>
              </button>
              <button type="button" className="quick-action" onClick={onRetryInfo} disabled={infoLoading}>
                <Icon name="refresh" className={infoLoading ? "icon-spin" : undefined} /><span>刷新核心信息</span>
              </button>
            </div>
          </section>
        </div>
      </div>
    </div>
  );
}

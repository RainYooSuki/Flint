import { useMemo, useState } from "react";
import { Icon } from "../components/Icon";
import type { JavaInfo, LauncherSettings } from "../types";
import { settingsEqual, validateSettings, type SettingsErrors } from "../utils";

interface SettingsViewProps {
  draft: LauncherSettings | null;
  saved: LauncherSettings | null;
  javas: JavaInfo[];
  settingsLoading: boolean;
  javaLoading: boolean;
  settingsError: string | null;
  javaError: string | null;
  saving: boolean;
  dirty: boolean;
  onChange: (settings: LauncherSettings) => void;
  onSave: () => void;
  onReload: () => void;
  onReset: () => void;
  onPickGameDir: () => void;
  onPickJava: () => void;
  onScanJava: () => void;
}

type TouchableField = "username" | "minMemoryMb" | "maxMemoryMb" | "width" | "height";

function numericInputValue(value: string): number {
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : 0;
}

export function SettingsView({
  draft,
  saved,
  javas,
  settingsLoading,
  javaLoading,
  settingsError,
  javaError,
  saving,
  dirty,
  onChange,
  onSave,
  onReload,
  onReset,
  onPickGameDir,
  onPickJava,
  onScanJava
}: SettingsViewProps) {
  const [touched, setTouched] = useState<Partial<Record<TouchableField, boolean>>>({});
  const errors = useMemo<SettingsErrors>(() => (draft ? validateSettings(draft) : {}), [draft]);
  const errorCount = Object.keys(errors).length;
  const selectedJava = draft ? javas.find((java) => java.path === draft.javaPath) ?? null : null;
  const busy = saving || settingsLoading;
  const maxSliderValue = draft
    ? Math.min(32768, Math.max(512, Math.max(draft.minMemoryMb || 512, draft.maxMemoryMb || 2048)))
    : 2048;

  function touch(field: TouchableField) {
    setTouched((current) => ({ ...current, [field]: true }));
  }

  function update(patch: Partial<LauncherSettings>) {
    if (!draft) return;
    onChange({ ...draft, ...patch });
  }

  if (!draft) {
    return (
      <div className="page-shell settings-page">
        <header className="page-head">
          <div className="page-title"><h1>设置</h1><span>启动配置与运行环境</span></div>
        </header>
        <div className="page-body settings-state-body">
          <div className="state-panel" role={settingsLoading ? "status" : undefined}>
            {settingsLoading ? <span className="spinner state-spinner" /> : <Icon name="alert" />}
            <strong>{settingsLoading ? "正在载入设置" : "设置当前不可用"}</strong>
            <p>{settingsLoading ? "正在从 Flint 核心读取持久化配置。" : settingsError || "后端没有返回设置数据。"}</p>
            {!settingsLoading ? <button type="button" className="button" onClick={onReload}>重新载入</button> : null}
          </div>
        </div>
      </div>
    );
  }

  const saveStateText = saving
    ? "正在保存设置"
    : dirty && errorCount > 0
      ? `有 ${errorCount} 项设置需要修正`
      : dirty
        ? "有未保存的更改"
        : "设置已同步";

  return (
    <div className="page-shell settings-page">
      <header className="page-head">
        <div className="page-title">
          <h1>设置</h1>
          <span>启动配置、Java 与运行参数</span>
        </div>
      </header>

      <div className="settings-scroll">
        {settingsError ? (
          <div className="inline-message error" role="alert">
            <Icon name="alert" />
            <span>{settingsError}</span>
            <button type="button" className="text-link" onClick={onReload} disabled={busy}>重新载入</button>
          </div>
        ) : null}

        <fieldset className="settings-form" disabled={busy}>
          <legend className="sr-only">启动设置</legend>

          <section className="settings-section" aria-labelledby="settings-account-title">
            <div className="settings-section-head">
              <h2 id="settings-account-title">游戏与账户</h2>
              <span>版本目录与离线身份</span>
            </div>

            <div className="setting-row">
              <div className="setting-label">
                <label htmlFor="settings-game-dir">游戏目录</label>
                <small>留空使用平台默认目录；版本扫描与安装都以此为准。</small>
              </div>
              <div className="setting-control">
                <div className="input-with-button">
                  <input
                    id="settings-game-dir"
                    type="text"
                    value={draft.gameDir}
                    onChange={(event) => update({ gameDir: event.target.value })}
                    placeholder="平台默认 .minecraft"
                    spellCheck={false}
                    autoComplete="off"
                  />
                  <button type="button" className="button" onClick={onPickGameDir}>浏览</button>
                </div>
              </div>
            </div>

            <div className="setting-row">
              <div className="setting-label">
                <label htmlFor="settings-username">离线用户名</label>
                <small>3–16 个字符，仅允许字母、数字和下划线。</small>
              </div>
              <div className="setting-control">
                <input
                  id="settings-username"
                  type="text"
                  value={draft.username}
                  onChange={(event) => { update({ username: event.target.value }); touch("username"); }}
                  onBlur={() => touch("username")}
                  maxLength={16}
                  placeholder="例如 Flint_Player"
                  spellCheck={false}
                  autoComplete="off"
                  aria-invalid={Boolean(errors.username)}
                  aria-describedby="settings-username-help"
                />
                {errors.username && touched.username ? (
                  <small className="field-error" id="settings-username-help" role="alert">{errors.username}</small>
                ) : (
                  <small className="field-help" id="settings-username-help">该用户名仅用于离线启动。</small>
                )}
              </div>
            </div>
          </section>

          <section className="settings-section" aria-labelledby="settings-integrations-title">
            <div className="settings-section-head">
              <h2 id="settings-integrations-title">集成服务</h2>
              <span>仅保存在本机设置中</span>
            </div>

            <p className="integration-note">两项配置均为可选，只在对应服务中使用；留空会禁用相应功能。</p>

            <div className="setting-row">
              <div className="setting-label">
                <label htmlFor="settings-microsoft-client-id">Microsoft 应用程序 ID</label>
                <small>Azure 公共客户端应用 ID，用于设备代码登录。无需客户端密钥。</small>
              </div>
              <div className="setting-control">
                <input
                  id="settings-microsoft-client-id"
                  type="text"
                  value={draft.microsoftClientId ?? ""}
                  onChange={(event) => update({ microsoftClientId: event.target.value })}
                  placeholder="Microsoft Entra 应用程序（客户端）ID"
                  spellCheck={false}
                  autoComplete="off"
                />
                <small className="field-help">留空时无法开始 Microsoft 账户登录。</small>
              </div>
            </div>

            <div className="setting-row">
              <div className="setting-label">
                <label htmlFor="settings-curseforge-key">CurseForge API Key</label>
                <small>仅用于 CurseForge 模组搜索和版本查询。</small>
              </div>
              <div className="setting-control">
                <input
                  id="settings-curseforge-key"
                  type="password"
                  value={draft.curseforgeApiKey ?? ""}
                  onChange={(event) => update({ curseforgeApiKey: event.target.value })}
                  placeholder="输入 CurseForge API Key"
                  spellCheck={false}
                  autoComplete="off"
                />
                <small className="field-help">密钥保存在本机设置中，界面不会显示后端日志或请求详情。</small>
              </div>
            </div>
          </section>

          <section className="settings-section" aria-labelledby="settings-java-title">
            <div className="settings-section-head">
              <h2 id="settings-java-title">Java 环境</h2>
              <button type="button" className="link-button" onClick={onScanJava} disabled={javaLoading || busy}>
                <Icon name="refresh" className={javaLoading ? "icon-spin" : undefined} />
                {javaLoading ? "扫描中" : "重新扫描"}
              </button>
            </div>

            <div className="setting-row">
              <div className="setting-label">
                <label htmlFor="settings-java-path">Java 路径</label>
                <small>启动时必须指定可执行的 Java 文件。</small>
              </div>
              <div className="setting-control">
                <div className="input-with-button">
                  <input
                    id="settings-java-path"
                    type="text"
                    value={draft.javaPath}
                    onChange={(event) => update({ javaPath: event.target.value })}
                    placeholder="选择检测到的 Java 或手动填写路径"
                    spellCheck={false}
                    autoComplete="off"
                  />
                  <button type="button" className="button" onClick={onPickJava}>浏览</button>
                </div>
              </div>
            </div>

            <div className="setting-row align-top">
              <div className="setting-label">
                <span>已检测的 Java</span>
                <small>{javas.length} 个运行环境</small>
              </div>
              <div className="setting-control">
                {javaError ? (
                  <div className="inline-message error compact" role="alert">
                    <Icon name="alert" /><span>{javaError}</span>
                  </div>
                ) : null}
                {javaLoading && javas.length === 0 ? (
                  <div className="inline-loading"><span className="spinner" /> 正在扫描 Java…</div>
                ) : javas.length === 0 ? (
                  <div className="empty-line">未检测到 Java，请手动选择可执行文件。</div>
                ) : (
                  <div className="java-list">
                    {javas.map((java) => {
                      const selected = java.path === draft.javaPath;
                      return (
                        <button
                          key={java.path}
                          type="button"
                          className={`java-row${selected ? " selected" : ""}`}
                          aria-pressed={selected}
                          onClick={() => update({ javaPath: java.path })}
                        >
                          <span className="java-major">{java.major || "?"}</span>
                          <span className="java-copy">
                            <strong>{java.version || `Java ${java.major}`}</strong>
                            <small title={java.path}>{java.path}</small>
                          </span>
                          <span className="java-arch">{java.architecture || "unknown"}</span>
                          <span className="java-check">{selected ? <Icon name="check" /> : null}</span>
                        </button>
                      );
                    })}
                  </div>
                )}
                {draft.javaPath && !selectedJava ? <small className="field-help">当前使用自定义 Java 路径。</small> : null}
              </div>
            </div>
          </section>

          <section className="settings-section" aria-labelledby="settings-performance-title">
            <div className="settings-section-head">
              <h2 id="settings-performance-title">内存与窗口</h2>
              <span>当前 {draft.minMemoryMb}–{draft.maxMemoryMb} MB</span>
            </div>

            <div className="setting-row align-top">
              <div className="setting-label">
                <span>内存范围</span>
                <small>最大内存不得小于最小内存。</small>
              </div>
              <div className="setting-control">
                <div className="memory-inputs">
                  <label>
                    <span>最小内存</span>
                    <span className="number-with-unit">
                      <input
                        type="number"
                        min={256}
                        step={256}
                        value={draft.minMemoryMb || ""}
                        onChange={(event) => { update({ minMemoryMb: numericInputValue(event.target.value) }); touch("minMemoryMb"); }}
                        onBlur={() => touch("minMemoryMb")}
                        aria-invalid={Boolean(errors.minMemoryMb)}
                        aria-describedby="settings-memory-help"
                      />
                      <em>MB</em>
                    </span>
                  </label>
                  <label>
                    <span>最大内存</span>
                    <span className="number-with-unit">
                      <input
                        type="number"
                        min={draft.minMemoryMb || 256}
                        step={256}
                        value={draft.maxMemoryMb || ""}
                        onChange={(event) => { update({ maxMemoryMb: numericInputValue(event.target.value) }); touch("maxMemoryMb"); }}
                        onBlur={() => touch("maxMemoryMb")}
                        aria-invalid={Boolean(errors.maxMemoryMb)}
                        aria-describedby="settings-memory-help"
                      />
                      <em>MB</em>
                    </span>
                  </label>
                </div>
                <input
                  className="memory-range"
                  type="range"
                  min={512}
                  max={32768}
                  step={256}
                  value={maxSliderValue}
                  onChange={(event) => {
                    update({ maxMemoryMb: Math.max(Number(event.target.value), draft.minMemoryMb || 512) });
                    touch("maxMemoryMb");
                  }}
                  aria-label="调整最大内存"
                  aria-valuetext={`${draft.maxMemoryMb} MB`}
                />
                <div className="range-scale" aria-hidden="true"><span>512 MB</span><span>建议 2–4 GB</span><span>32 GB</span></div>
                {errors.minMemoryMb && touched.minMemoryMb ? <small className="field-error" role="alert">{errors.minMemoryMb}</small> : null}
                {errors.maxMemoryMb && touched.maxMemoryMb ? <small className="field-error" role="alert">{errors.maxMemoryMb}</small> : null}
                <small className="field-help" id="settings-memory-help">内存过高时可能受系统内存或 Java 架构限制。</small>
              </div>
            </div>

            <div className="setting-row">
              <div className="setting-label">
                <span>窗口尺寸</span>
                <small>游戏窗口的初始宽高。</small>
              </div>
              <div className="setting-control">
                <div className="window-inputs">
                  <label>
                    <span>宽度</span>
                    <span className="number-with-unit">
                      <input
                        type="number"
                        min={1}
                        value={draft.width || ""}
                        onChange={(event) => { update({ width: numericInputValue(event.target.value) }); touch("width"); }}
                        onBlur={() => touch("width")}
                        aria-invalid={Boolean(errors.width)}
                      />
                      <em>px</em>
                    </span>
                  </label>
                  <span className="pair-times">×</span>
                  <label>
                    <span>高度</span>
                    <span className="number-with-unit">
                      <input
                        type="number"
                        min={1}
                        value={draft.height || ""}
                        onChange={(event) => { update({ height: numericInputValue(event.target.value) }); touch("height"); }}
                        onBlur={() => touch("height")}
                        aria-invalid={Boolean(errors.height)}
                      />
                      <em>px</em>
                    </span>
                  </label>
                </div>
                {errors.width && touched.width ? <small className="field-error" role="alert">{errors.width}</small> : null}
                {errors.height && touched.height ? <small className="field-error" role="alert">{errors.height}</small> : null}
              </div>
            </div>
          </section>

          <section className="settings-section" aria-labelledby="settings-arguments-title">
            <div className="settings-section-head">
              <h2 id="settings-arguments-title">启动参数</h2>
              <span>不确定时请保持留空</span>
            </div>

            <div className="setting-row align-top">
              <div className="setting-label">
                <label htmlFor="settings-jvm-args">JVM 参数</label>
                <small>附加到 Java 启动命令，无需重复填写内存参数。</small>
              </div>
              <div className="setting-control">
                <textarea
                  id="settings-jvm-args"
                  rows={3}
                  value={draft.jvmArgs}
                  onChange={(event) => update({ jvmArgs: event.target.value })}
                  placeholder="-XX:+UseG1GC"
                  spellCheck={false}
                />
              </div>
            </div>

            <div className="setting-row align-top">
              <div className="setting-label">
                <label htmlFor="settings-game-args">游戏参数</label>
                <small>传递给游戏进程的可选参数。</small>
              </div>
              <div className="setting-control">
                <textarea
                  id="settings-game-args"
                  rows={3}
                  value={draft.gameArgs}
                  onChange={(event) => update({ gameArgs: event.target.value })}
                  placeholder="留空使用默认参数"
                  spellCheck={false}
                />
              </div>
            </div>
          </section>
        </fieldset>
      </div>

      <footer className="actionbar settings-actionbar">
        <div className="action-copy" aria-live="polite">
          <strong>{saveStateText}</strong>
          <span>{settingsEqual(draft, saved) ? "配置已从 Flint 核心载入" : "保存后写入 Flint 核心"}</span>
        </div>
        <div className="action-buttons">
          <button type="button" className="button" onClick={onReload} disabled={busy}>
            <Icon name="refresh" /> 重载
          </button>
          <button type="button" className="button" onClick={onReset} disabled={!dirty || busy}>撤销</button>
          <button
            type="button"
            className="button primary"
            onClick={() => {
              setTouched({ username: true, minMemoryMb: true, maxMemoryMb: true, width: true, height: true });
              if (errorCount === 0) onSave();
            }}
            disabled={!dirty || busy}
          >
            {saving ? <span className="spinner spinner-light" /> : <Icon name="check" />}
            {saving ? "保存中" : "保存设置"}
          </button>
        </div>
      </footer>
    </div>
  );
}
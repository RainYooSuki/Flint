import { Icon } from "../../../components/Icon";
import type { PageComponentProps } from "../../../core/contributions";
import { SettingsSectionValidationErrors } from "./SettingsSectionValidation";
import { useSettingsSectionDraft } from "./useSettingsSectionDraft";

export function JavaSettingsSection({ context }: PageComponentProps) {
  const { draft, update } = useSettingsSectionDraft(context);
  const javas = context.javas;
  const javaLoading = context.javaLoading;
  const javaError = context.javaError;
  const busy = context.savingSettings || context.settingsLoading;
  const selectedJava = draft ? javas.find((java) => java.path === draft.javaPath) ?? null : null;

  if (!draft) return null;

  return (
    <section className="settings-section" aria-labelledby="settings-java-title">
      <div className="settings-section-head">
        <h2 id="settings-java-title">Java 环境</h2>
        <button type="button" className="link-button" onClick={context.onScanJava} disabled={javaLoading || busy}>
          <Icon name="refresh" className={javaLoading ? "icon-spin" : undefined} />
          {javaLoading ? "扫描中" : "重新扫描"}
        </button>
      </div>

      <SettingsSectionValidationErrors />

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
            <button type="button" className="button" onClick={context.onPickJava}>浏览</button>
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
  );
}

import { useMemo, useState } from "react";
import { Icon } from "../../components/Icon";
import type { PageComponentProps } from "../../core/contributions";
import { ContributionBoundary } from "../../core/ContributionBoundary";
import { settingsEqual, toErrorMessage, validateSettings, type SettingsErrors } from "../../utils";
import { SettingsSectionValidationProvider } from "./sections/SettingsSectionValidation";

export function SettingsView({ context }: PageComponentProps) {
  const {
    settingsDraft: draft,
    settings: saved,
    settingsLoading,
    settingsError,
    savingSettings: saving,
    settingsDirty: dirty,
    onSaveSettings: onSave,
    onReloadSettings: onReload,
    onResetSettings: onReset
  } = context;
  const [showAllErrors, setShowAllErrors] = useState(false);
  const errors = useMemo<SettingsErrors>(() => (draft ? validateSettings(draft) : {}), [draft]);
  const sectionValidationErrors = useMemo(() => {
    const result = new Map<string, readonly string[]>();
    if (!draft) return result;

    const centralMessages = new Set(
      Object.values(errors).filter((message): message is string => Boolean(message))
    );
    for (const section of context.settingsSections) {
      const validate = section.validate;
      if (!validate) continue;
      try {
        // Contribution hooks expose messages only; scope leftovers by section ID and
        // suppress exact central messages that are already rendered beside fields.
        const messages = Array.from(new Set(
          validate(draft)
            .map((message) => message.trim())
            .filter((message) => message.length > 0 && !centralMessages.has(message))
        ));
        if (messages.length > 0) result.set(section.id, messages);
      } catch (error) {
        console.error(`Settings section ${section.id} validation failed.`, error);
        result.set(section.id, [`验证失败：${toErrorMessage(error)}`]);
      }
    }
    return result;
  }, [draft, errors, context.settingsSections]);
  const sectionErrorCount = Array.from(sectionValidationErrors.values())
    .reduce((total, messages) => total + messages.length, 0);
  const errorCount = Object.keys(errors).length + sectionErrorCount;
  const busy = saving || settingsLoading;

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

          {context.settingsSections.map((section) => {
            const Section = section.component;
            const sectionErrors = sectionValidationErrors.get(section.id) ?? [];
            return (
              <ContributionBoundary key={section.id} contributionId={section.id}>
                <SettingsSectionValidationProvider showAllErrors={showAllErrors} errors={sectionErrors}>
                  <Section context={context} />
                </SettingsSectionValidationProvider>
              </ContributionBoundary>
            );
          })}
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
              setShowAllErrors(true);
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

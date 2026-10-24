import { useMemo, useState } from "react";
import type { PageComponentProps } from "../../../core/contributions";
import { validateSettings, type SettingsErrors } from "../../../utils";
import { SettingsSectionValidationErrors, useSettingsSectionValidation } from "./SettingsSectionValidation";
import { useSettingsSectionDraft } from "./useSettingsSectionDraft";

type TouchableField = "username";

export function AccountSettingsSection({ context }: PageComponentProps) {
  const { draft, update } = useSettingsSectionDraft(context);
  const [touched, setTouched] = useState<Partial<Record<TouchableField, boolean>>>({});
  const showAllErrors = useSettingsSectionValidation();
  const errors = useMemo<SettingsErrors>(() => (draft ? validateSettings(draft) : {}), [draft]);

  function touch(field: TouchableField) {
    setTouched((current) => ({ ...current, [field]: true }));
  }

  if (!draft) return null;

  return (
    <section className="settings-section" aria-labelledby="settings-account-title">
      <div className="settings-section-head">
        <h2 id="settings-account-title">游戏与账户</h2>
        <span>版本目录与离线身份</span>
      </div>

      <SettingsSectionValidationErrors />

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
            <button type="button" className="button" onClick={context.onPickGameDir}>浏览</button>
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
          {errors.username && (touched.username || showAllErrors) ? (
            <small className="field-error" id="settings-username-help" role="alert">{errors.username}</small>
          ) : (
            <small className="field-help" id="settings-username-help">该用户名仅用于离线启动。</small>
          )}
        </div>
      </div>
    </section>
  );
}

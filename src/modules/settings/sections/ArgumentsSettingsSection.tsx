import type { PageComponentProps } from "../../../core/contributions";
import { SettingsSectionValidationErrors } from "./SettingsSectionValidation";
import { useSettingsSectionDraft } from "./useSettingsSectionDraft";

export function ArgumentsSettingsSection({ context }: PageComponentProps) {
  const { draft, update } = useSettingsSectionDraft(context);

  if (!draft) return null;

  return (
    <section className="settings-section" aria-labelledby="settings-arguments-title">
      <div className="settings-section-head">
        <h2 id="settings-arguments-title">启动参数</h2>
        <span>不确定时请保持留空</span>
      </div>

      <SettingsSectionValidationErrors />

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
  );
}

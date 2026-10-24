import { useMemo, useState } from "react";
import type { PageComponentProps } from "../../../core/contributions";
import { validateSettings, type SettingsErrors } from "../../../utils";
import { SettingsSectionValidationErrors, useSettingsSectionValidation } from "./SettingsSectionValidation";
import { useSettingsSectionDraft } from "./useSettingsSectionDraft";

type TouchableField = "minMemoryMb" | "maxMemoryMb" | "width" | "height";

function numericInputValue(value: string): number {
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : 0;
}

export function PerformanceSettingsSection({ context }: PageComponentProps) {
  const { draft, update } = useSettingsSectionDraft(context);
  const [touched, setTouched] = useState<Partial<Record<TouchableField, boolean>>>({});
  const showAllErrors = useSettingsSectionValidation();
  const errors = useMemo<SettingsErrors>(() => (draft ? validateSettings(draft) : {}), [draft]);
  const maxSliderValue = draft
    ? Math.min(32768, Math.max(512, Math.max(draft.minMemoryMb || 512, draft.maxMemoryMb || 2048)))
    : 2048;

  function touch(field: TouchableField) {
    setTouched((current) => ({ ...current, [field]: true }));
  }

  if (!draft) return null;

  return (
    <section className="settings-section" aria-labelledby="settings-performance-title">
      <div className="settings-section-head">
        <h2 id="settings-performance-title">内存与窗口</h2>
        <span>当前 {draft.minMemoryMb}–{draft.maxMemoryMb} MB</span>
      </div>

      <SettingsSectionValidationErrors />

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
          {errors.minMemoryMb && (touched.minMemoryMb || showAllErrors) ? <small className="field-error" role="alert">{errors.minMemoryMb}</small> : null}
          {errors.maxMemoryMb && (touched.maxMemoryMb || showAllErrors) ? <small className="field-error" role="alert">{errors.maxMemoryMb}</small> : null}
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
          {errors.width && (touched.width || showAllErrors) ? <small className="field-error" role="alert">{errors.width}</small> : null}
          {errors.height && (touched.height || showAllErrors) ? <small className="field-error" role="alert">{errors.height}</small> : null}
        </div>
      </div>
    </section>
  );
}

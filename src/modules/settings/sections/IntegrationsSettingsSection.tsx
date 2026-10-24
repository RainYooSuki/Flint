import type { PageComponentProps } from "../../../core/contributions";
import { SettingsSectionValidationErrors } from "./SettingsSectionValidation";
import { useSettingsSectionDraft } from "./useSettingsSectionDraft";

export function IntegrationsSettingsSection({ context }: PageComponentProps) {
  const { draft, update } = useSettingsSectionDraft(context);

  if (!draft) return null;

  return (
    <section className="settings-section" aria-labelledby="settings-integrations-title">
      <div className="settings-section-head">
        <h2 id="settings-integrations-title">集成服务</h2>
        <span>仅保存在本机设置中</span>
      </div>

      <SettingsSectionValidationErrors />

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
  );
}

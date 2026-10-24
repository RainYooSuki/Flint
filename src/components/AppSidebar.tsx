import type { InstanceInfo, SectionId } from "../types";
import { Icon, type IconName } from "./Icon";

interface AppSidebarProps {
  active: SectionId;
  settingsDirty: boolean;
  coreReady: boolean;
  username: string;
  accountTypeLabel: string;
  instancesCount: number;
  javaCount: number;
  selectedInstance: InstanceInfo | null;
  onNavigate: (section: SectionId) => void;
  onOpenAccounts: () => void;
}

const navItems: Array<{ id: SectionId; label: string; icon: IconName }> = [
  { id: "launch", label: "启动", icon: "play" },
  { id: "versions", label: "下载", icon: "download" },
  { id: "saves", label: "存档", icon: "save" },
  { id: "settings", label: "设置", icon: "settings" },
  { id: "about", label: "工具", icon: "wrench" }
];

export function AppSidebar({
  active,
  settingsDirty,
  coreReady,
  username,
  accountTypeLabel,
  instancesCount,
  javaCount,
  selectedInstance,
  onNavigate,
  onOpenAccounts
}: AppSidebarProps) {
  const clientState = !coreReady
    ? "未知"
    : !selectedInstance
      ? "未选择"
      : !selectedInstance.hasClientJar
        ? "缺失"
        : "完整";
  const avatarText = username.trim().slice(0, 1).toUpperCase() || "?";

  return (
    <aside className="sidebar">
      <div className="sidebar-label">导航</div>
      <nav className="nav-list" aria-label="主导航">
        {navItems.map((item) => (
          <button
            key={item.id}
            type="button"
            className={`nav-item${active === item.id ? " active" : ""}`}
            aria-current={active === item.id ? "page" : undefined}
            onClick={() => onNavigate(item.id)}
          >
            <Icon name={item.icon} />
            <span>{item.label}</span>
            {item.id === "settings" && settingsDirty ? (
              <>
                <span className="nav-dirty" title="设置有未保存更改" aria-hidden="true" />
                <span className="sr-only">设置有未保存更改</span>
              </>
            ) : null}
          </button>
        ))}
      </nav>

      <div className="nav-divider" />
      <div className="sidebar-section-label">当前账户</div>
      <button type="button" className="sidebar-account" onClick={onOpenAccounts} title="账户管理">
        <span className="avatar" aria-hidden="true">{avatarText}</span>
        <span className="account-copy">
          <strong>{username || "未设置用户名"}</strong>
          <small>{accountTypeLabel}</small>
        </span>
      </button>

      <div className="sidebar-spacer" />
      <div className="sidebar-foot">
        <div><span>本地版本</span><strong>{instancesCount}</strong></div>
        <div><span>Java 环境</span><strong>{javaCount}</strong></div>
        <div title={selectedInstance ? selectedInstance.path : "尚未选择本地版本"}>
          <span>客户端状态</span>
          <strong className={clientState === "完整" ? "status-ok" : clientState === "未知" || clientState === "缺失" ? "status-warn" : ""}>
            {clientState}
          </strong>
        </div>
      </div>
    </aside>
  );
}
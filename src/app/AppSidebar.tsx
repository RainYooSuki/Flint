import type { NavigationContribution, NavigationContext } from "../core/contributions";
import type { InstanceInfo } from "../types";
import { Icon } from "../components/Icon";

interface AppSidebarProps {
  activePageId: string;
  navigation: readonly NavigationContribution[];
  settingsDirty: boolean;
  coreReady: boolean;
  username: string;
  accountTypeLabel: string;
  instancesCount: number;
  javaCount: number;
  selectedInstance: InstanceInfo | null;
  onNavigate: (targetPageId: string) => void;
  onOpenAccounts: () => void;
}

export function AppSidebar({
  activePageId,
  navigation,
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
  const navigationContext: NavigationContext = { settingsDirty, coreReady, selectedInstance };

  return (
    <aside className="sidebar">
      <div className="sidebar-label">导航</div>
      <nav className="nav-list" aria-label="主导航">
        {navigation.map((item) => {
          if (item.visible && !item.visible(navigationContext)) return null;
          const indicator = item.indicator?.(navigationContext) ?? null;
          return (
            <button
              key={item.id}
              type="button"
              className={`nav-item${activePageId === item.targetPageId ? " active" : ""}`}
              aria-current={activePageId === item.targetPageId ? "page" : undefined}
              onClick={() => onNavigate(item.targetPageId)}
            >
              <Icon name={item.icon} />
              <span>{item.label}</span>
              {indicator ? (
                <>
                  <span className={indicator.className} title={indicator.title} aria-hidden="true" />
                  <span className="sr-only">{indicator.screenReaderLabel}</span>
                </>
              ) : null}
            </button>
          );
        })}
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

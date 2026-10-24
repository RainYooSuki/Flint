import type { MouseEvent as ReactMouseEvent } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { SectionId } from "../types";
import { toErrorMessage } from "../utils";
import { FlintMark, Icon, type IconName } from "./Icon";

interface TitleBarProps {
  active: SectionId;
  settingsDirty: boolean;
  onNavigate: (section: SectionId) => void;
  onError: (message: string) => void;
}

const tabs: Array<{ id: SectionId; label: string; icon: IconName }> = [
  { id: "launch", label: "启动", icon: "play" },
  { id: "versions", label: "下载", icon: "download" },
  { id: "settings", label: "设置", icon: "settings" },
  { id: "about", label: "工具", icon: "wrench" }
];

function isInteractiveTarget(target: EventTarget | null): boolean {
  return target instanceof Element && Boolean(target.closest("button, a, input, select, textarea"));
}

export function TitleBar({ active, settingsDirty, onNavigate, onError }: TitleBarProps) {
  async function runWindowAction(action: () => Promise<void>, label: string) {
    try {
      await action();
    } catch (error) {
      onError(`${label}窗口失败：${toErrorMessage(error)}`);
    }
  }

  async function beginDrag(event: ReactMouseEvent<HTMLElement>) {
    if (event.button !== 0 || event.detail > 1 || isInteractiveTarget(event.target)) return;
    try {
      await getCurrentWindow().startDragging();
    } catch (error) {
      onError(`拖动窗口失败：${toErrorMessage(error)}`);
    }
  }

  async function toggleMaximize(event: ReactMouseEvent<HTMLElement>) {
    if (isInteractiveTarget(event.target)) return;
    await runWindowAction(() => getCurrentWindow().toggleMaximize(), "切换最大化");
  }

  return (
    <header
      className="titlebar"
      onMouseDown={(event) => void beginDrag(event)}
      onDoubleClick={(event) => void toggleMaximize(event)}
    >
      <div className="titlebar-brand" aria-label="Flint">
        <FlintMark className="titlebar-mark" />
        <span className="titlebar-wordmark">FLINT</span>
      </div>

      <nav className="titlebar-nav" aria-label="主导航">
        {tabs.map((tab) => (
          <button
            key={tab.id}
            type="button"
            className={`titlebar-tab${active === tab.id ? " active" : ""}`}
            aria-current={active === tab.id ? "page" : undefined}
            onClick={() => onNavigate(tab.id)}
          >
            <Icon name={tab.icon} />
            <span>{tab.label}</span>
            {tab.id === "settings" && settingsDirty ? (
              <span className="tab-dirty" title="设置尚未保存" aria-label="设置尚未保存" />
            ) : null}
          </button>
        ))}
      </nav>

      <div className="window-controls" role="group" aria-label="窗口控制">
        <button
          type="button"
          className="window-control"
          aria-label="最小化"
          title="最小化"
          onClick={() => void runWindowAction(() => getCurrentWindow().minimize(), "最小化")}
        >
          <Icon name="minus" />
        </button>
        <button
          type="button"
          className="window-control"
          aria-label="最大化或还原"
          title="最大化或还原"
          onClick={() => void runWindowAction(() => getCurrentWindow().toggleMaximize(), "切换最大化")}
        >
          <Icon name="square" />
        </button>
        <button
          type="button"
          className="window-control close"
          aria-label="关闭"
          title="关闭"
          onClick={() => void runWindowAction(() => getCurrentWindow().close(), "关闭")}
        >
          <Icon name="close" />
        </button>
      </div>
    </header>
  );
}
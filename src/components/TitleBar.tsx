import type { MouseEvent as ReactMouseEvent } from "react";
import type { HostWindow } from "../core/platform/hostServices";
import { toErrorMessage } from "../utils";
import { Icon } from "./Icon";

interface TitleBarProps {
  context: string;
  windowService: HostWindow;
  onError: (message: string) => void;
}

function isInteractiveTarget(target: EventTarget | null): boolean {
  return target instanceof Element && Boolean(target.closest("button, a, input, select, textarea, summary"));
}

export function TitleBar({ context, windowService, onError }: TitleBarProps) {
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
      await windowService.startDragging();
    } catch (error) {
      onError(`拖动窗口失败：${toErrorMessage(error)}`);
    }
  }

  async function toggleMaximize(event: ReactMouseEvent<HTMLElement>) {
    if (isInteractiveTarget(event.target)) return;
    await runWindowAction(() => windowService.toggleMaximize(), "切换最大化");
  }

  return (
    <header
      className="titlebar"
      onMouseDown={(event) => void beginDrag(event)}
      onDoubleClick={(event) => void toggleMaximize(event)}
    >
      <div className="brand" aria-label="Flint">
        <span className="brand-symbol" aria-hidden="true">F</span>
        <span className="brand-name">Flint</span>
      </div>
      <span className="titlebar-context" title={context}>{context}</span>
      <div className="window-controls" role="group" aria-label="窗口控制">
        <button
          type="button"
          className="window-button"
          aria-label="最小化"
          title="最小化"
          onClick={() => void runWindowAction(() => windowService.minimize(), "最小化")}
        >
          <Icon name="minus" />
        </button>
        <button
          type="button"
          className="window-button"
          aria-label="最大化或还原"
          title="最大化或还原"
          onClick={() => void runWindowAction(() => windowService.toggleMaximize(), "切换最大化")}
        >
          <Icon name="square" />
        </button>
        <button
          type="button"
          className="window-button close"
          aria-label="关闭"
          title="关闭"
          onClick={() => void runWindowAction(() => windowService.close(), "关闭")}
        >
          <Icon name="close" />
        </button>
      </div>
    </header>
  );
}

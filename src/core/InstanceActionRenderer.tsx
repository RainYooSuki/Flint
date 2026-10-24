import { useMemo } from "react";
import { api } from "../api";
import { Icon } from "../components/Icon";
import type { InstanceInfo, NoticeKind } from "../types";
import { toErrorMessage } from "../utils";
import type { InstanceActionContext, InstanceActionContribution } from "./contributions";

export interface InstanceActionRendererProps {
  readonly actions: readonly InstanceActionContribution[];
  readonly selectedInstance: InstanceInfo | null;
  readonly gameDir: string;
  readonly navigateToPage: (targetPageId: string) => void;
  readonly notify: (kind: NoticeKind, message: string) => void;
  readonly buttonClassName?: string;
}

export function InstanceActionRenderer({
  actions,
  selectedInstance,
  gameDir,
  navigateToPage,
  notify,
  buttonClassName = "button"
}: InstanceActionRendererProps) {
  const actionContext = useMemo<InstanceActionContext>(() => ({
    selectedInstance,
    gameDir,
    navigateToPage,
    notify,
    api
  }), [selectedInstance, gameDir, navigateToPage, notify]);

  async function runAction(action: InstanceActionContribution): Promise<void> {
    try {
      await action.handler(actionContext);
    } catch (error) {
      console.error(`Instance action ${action.id} failed.`, error);
      notify("error", `${action.label}失败：${toErrorMessage(error)}`);
    }
  }

  function startAction(action: InstanceActionContribution): void {
    void runAction(action).catch((error: unknown) => {
      console.error(`Instance action ${action.id} could not report its failure.`, error);
    });
  }

  return (
    <>
      {actions.map((action) => {
        if (action.visible && !action.visible(actionContext)) return null;
        const enabled = action.enabled ? action.enabled(actionContext) : true;
        return (
          <button
            key={action.id}
            type="button"
            className={buttonClassName}
            onClick={() => startAction(action)}
            disabled={!enabled}
          >
            <Icon name={action.icon} />
            {action.label}
          </button>
        );
      })}
    </>
  );
}

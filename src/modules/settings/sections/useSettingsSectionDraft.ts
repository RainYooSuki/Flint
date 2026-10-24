import { useCallback } from "react";
import type { AppPageContext } from "../../../core/app-context";
import type { LauncherSettings } from "../../../types";

export function useSettingsSectionDraft(context: AppPageContext) {
  const { settingsDraft: draft, onChangeSettings } = context;
  const update = useCallback((patch: Partial<LauncherSettings>) => {
    if (!draft) return;
    onChangeSettings({ ...draft, ...patch });
  }, [draft, onChangeSettings]);

  return { draft, update };
}

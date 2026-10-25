import { openPath, openUrl } from "@tauri-apps/plugin-opener";
import type { HostOpener } from "./hostServices";

export function createTauriOpener(): HostOpener {
  return {
    openPath(path: string): Promise<void> {
      return openPath(path);
    },
    openUrl(url: string): Promise<void> {
      return openUrl(url);
    }
  };
}

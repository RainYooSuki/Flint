import { getCurrentWindow } from "@tauri-apps/api/window";
import type { HostWindow } from "./hostServices";

export function createTauriWindow(): HostWindow {
  return {
    startDragging(): Promise<void> {
      return getCurrentWindow().startDragging();
    },
    minimize(): Promise<void> {
      return getCurrentWindow().minimize();
    },
    toggleMaximize(): Promise<void> {
      return getCurrentWindow().toggleMaximize();
    },
    close(): Promise<void> {
      return getCurrentWindow().close();
    }
  };
}

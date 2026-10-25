import { createTauriEvents } from "../events/tauriEvents";
import type { HostServices } from "./hostServices";
import { createTauriDialogs } from "./tauriDialogs";
import { createTauriOpener } from "./tauriOpener";
import { createTauriWindow } from "./tauriWindow";

export function createTauriHostServices(): HostServices {
  return Object.freeze({
    events: createTauriEvents(),
    dialogs: createTauriDialogs(),
    opener: createTauriOpener(),
    window: createTauriWindow()
  });
}

import { listen } from "@tauri-apps/api/event";
import {
  createHostEventSubscriber,
  type HostEventHandler,
  type HostEventName,
  type HostEvents,
  type HostListenFn
} from "./hostEvents";

export const HOST_EVENT_NAMES = {
  installProgress: "flint://install-progress",
  loaderInstallProgress: "flint://loader-install-progress",
  modInstallProgress: "flint://mod-install-progress"
} as const satisfies Record<HostEventName, string>;

// Keep the typed event contract available alongside its concrete Tauri adapter.
export * from "./hostEvents";

const tauriListen: HostListenFn = <T>(
  eventName: string,
  handler: HostEventHandler<T>
) => listen<T>(eventName, (event) => handler(event.payload));

export function createTauriEvents(): HostEvents {
  return createHostEventSubscriber(HOST_EVENT_NAMES, tauriListen);
}

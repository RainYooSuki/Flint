import type { InstallProgress, LoaderInstallProgress, ModInstallProgress } from "../../types";

export interface HostEventMap {
  readonly installProgress: InstallProgress;
  readonly loaderInstallProgress: LoaderInstallProgress;
  readonly modInstallProgress: ModInstallProgress;
}

export type HostEventName = keyof HostEventMap;
export type HostEventHandler<T> = (payload: T) => void;
export type HostUnsubscribe = () => void;

export interface HostEvents {
  subscribe<K extends HostEventName>(
    name: K,
    handler: HostEventHandler<HostEventMap[K]>,
    onError?: (error: unknown) => void
  ): HostUnsubscribe;
}

export type HostEventNames = Readonly<Record<HostEventName, string>>;

export type HostListenFn = <T>(
  eventName: string,
  handler: HostEventHandler<T>
) => Promise<HostUnsubscribe>;

export function createHostEventSubscriber(
  eventNames: HostEventNames,
  listen: HostListenFn
): HostEvents {
  return {
    subscribe<K extends HostEventName>(
      name: K,
      handler: HostEventHandler<HostEventMap[K]>,
      onError?: (error: unknown) => void
    ): HostUnsubscribe {
      let closed = false;
      let unlisten: HostUnsubscribe | undefined;

      try {
        void listen<HostEventMap[K]>(eventNames[name], (payload) => {
          if (!closed) handler(payload);
        })
          .then((removeListener) => {
            // Release the listener immediately when cleanup raced its registration.
            if (closed) removeListener();
            else unlisten = removeListener;
          })
          .catch((error: unknown) => {
            if (!closed) onError?.(error);
          });
      } catch (error) {
        if (!closed) onError?.(error);
      }

      return () => {
        if (closed) return;
        closed = true;
        unlisten?.();
        unlisten = undefined;
      };
    }
  };
}

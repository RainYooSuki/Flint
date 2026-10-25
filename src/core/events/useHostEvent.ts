import { useEffect, useRef } from "react";
import type {
  HostEventHandler,
  HostEventMap,
  HostEventName,
  HostEvents
} from "./hostEvents";

export function useHostEvent<K extends HostEventName>(
  events: HostEvents,
  name: K,
  handler: HostEventHandler<HostEventMap[K]>,
  onError?: (error: unknown) => void
): void {
  const handlerRef = useRef(handler);
  const onErrorRef = useRef(onError);

  useEffect(() => {
    handlerRef.current = handler;
  }, [handler]);

  useEffect(() => {
    onErrorRef.current = onError;
  }, [onError]);

  useEffect(
    () => events.subscribe(
      name,
      (payload) => handlerRef.current(payload),
      (error) => onErrorRef.current?.(error)
    ),
    [events, name]
  );
}

import { invoke } from "@tauri-apps/api/core";

export interface InvokeFn {
  <T>(command: string, args?: Record<string, unknown>): Promise<T>;
}

export const tauriInvoke: InvokeFn = <T>(command: string, args?: Record<string, unknown>) =>
  invoke<T>(command, args);

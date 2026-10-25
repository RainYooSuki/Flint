import type { HostEvents } from "../events/hostEvents";

export interface HostDialogFilter {
  readonly name: string;
  readonly extensions: readonly string[];
}

export interface HostFileDialogOptions {
  readonly title: string;
  readonly filters?: readonly HostDialogFilter[];
}

export interface HostSaveDialogOptions extends HostFileDialogOptions {
  readonly defaultPath?: string;
}

export interface HostDialogs {
  pickDirectory(title: string): Promise<string | null>;
  pickFile(options: HostFileDialogOptions): Promise<string | null>;
  saveFile(options: HostSaveDialogOptions): Promise<string | null>;
}

export interface HostOpener {
  openPath(path: string): Promise<void>;
  openUrl(url: string): Promise<void>;
}

export interface HostWindow {
  startDragging(): Promise<void>;
  minimize(): Promise<void>;
  toggleMaximize(): Promise<void>;
  close(): Promise<void>;
}

export interface HostServices {
  readonly events: HostEvents;
  readonly dialogs: HostDialogs;
  readonly opener: HostOpener;
  readonly window: HostWindow;
}

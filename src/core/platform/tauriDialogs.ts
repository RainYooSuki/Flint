import { open, save } from "@tauri-apps/plugin-dialog";
import type {
  HostDialogFilter,
  HostDialogs,
  HostFileDialogOptions,
  HostSaveDialogOptions
} from "./hostServices";

function selectedPath(value: string | string[] | null): string | null {
  return typeof value === "string" ? value : null;
}

function dialogFilters(filters: readonly HostDialogFilter[]): { name: string; extensions: string[] }[] {
  return filters.map((filter) => ({
    name: filter.name,
    extensions: [...filter.extensions]
  }));
}

export function createTauriDialogs(): HostDialogs {
  return {
    async pickDirectory(title: string): Promise<string | null> {
      const selected = await open({
        directory: true,
        multiple: false,
        title
      });
      return selectedPath(selected);
    },

    async pickFile(options: HostFileDialogOptions): Promise<string | null> {
      const selected = await open({
        directory: false,
        multiple: false,
        title: options.title,
        ...(options.filters ? { filters: dialogFilters(options.filters) } : {})
      });
      return selectedPath(selected);
    },

    async saveFile(options: HostSaveDialogOptions): Promise<string | null> {
      const selected = await save({
        title: options.title,
        ...(options.defaultPath !== undefined ? { defaultPath: options.defaultPath } : {}),
        ...(options.filters ? { filters: dialogFilters(options.filters) } : {})
      });
      return selectedPath(selected);
    }
  };
}

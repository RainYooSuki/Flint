import type { LauncherSettings, LoaderKind } from "./types";

export const USERNAME_PATTERN = /^[A-Za-z0-9_]{3,16}$/;

const MIN_MEMORY_MB = 128;
const MAX_MEMORY_MB = 131_072;
const MIN_WINDOW_WIDTH = 320;
const MIN_WINDOW_HEIGHT = 240;
const MAX_WINDOW_DIMENSION = 16_384;

export type SettingsErrors = Partial<
  Record<"username" | "minMemoryMb" | "maxMemoryMb" | "width" | "height", string>
>;

export function toErrorMessage(error: unknown): string {
  if (error instanceof Error && error.message.trim()) {
    return error.message;
  }
  if (typeof error === "string" && error.trim()) {
    return error;
  }
  if (error && typeof error === "object" && "message" in error) {
    const message = (error as { message?: unknown }).message;
    if (typeof message === "string" && message.trim()) {
      return message;
    }
  }
  return "发生未知错误，请稍后重试。";
}

export function validateUsername(value: string): string | null {
  if (!value.trim()) {
    return "请输入离线用户名。";
  }
  if (!USERNAME_PATTERN.test(value)) {
    return "用户名需为 3–16 个字符，仅可使用英文字母、数字和下划线。";
  }
  return null;
}

function positiveIntegerError(value: number, label: string): string | null {
  if (!Number.isFinite(value) || !Number.isInteger(value) || value < 1) {
    return `${label}必须为正整数。`;
  }
  return null;
}

function rangeIntegerError(value: number, label: string, min: number, max: number): string | null {
  const integerError = positiveIntegerError(value, label);
  if (integerError) return integerError;
  if (value < min || value > max) {
    return `${label}必须在 ${min} 到 ${max} 之间。`;
  }
  return null;
}

export function validateSettings(settings: LauncherSettings): SettingsErrors {
  const errors: SettingsErrors = {};

  const username = validateUsername(settings.username);
  if (username) errors.username = username;

  const minMemory = rangeIntegerError(settings.minMemoryMb, "最小内存", MIN_MEMORY_MB, MAX_MEMORY_MB);
  if (minMemory) errors.minMemoryMb = minMemory;

  const maxMemory = rangeIntegerError(settings.maxMemoryMb, "最大内存", MIN_MEMORY_MB, MAX_MEMORY_MB);
  if (maxMemory) {
    errors.maxMemoryMb = maxMemory;
  } else if (settings.maxMemoryMb < settings.minMemoryMb) {
    errors.maxMemoryMb = "最大内存不能小于最小内存。";
  }

  const width = rangeIntegerError(settings.width, "窗口宽度", MIN_WINDOW_WIDTH, MAX_WINDOW_DIMENSION);
  if (width) errors.width = width;

  const height = rangeIntegerError(settings.height, "窗口高度", MIN_WINDOW_HEIGHT, MAX_WINDOW_DIMENSION);
  if (height) errors.height = height;

  return errors;
}

export function settingsEqual(left: LauncherSettings | null, right: LauncherSettings | null): boolean {
  if (!left || !right) return left === right;
  return (
    left.gameDir === right.gameDir &&
    left.javaPath === right.javaPath &&
    left.username === right.username &&
    left.minMemoryMb === right.minMemoryMb &&
    left.maxMemoryMb === right.maxMemoryMb &&
    left.jvmArgs === right.jvmArgs &&
    left.gameArgs === right.gameArgs &&
    left.width === right.width &&
    left.height === right.height &&
    left.microsoftClientId === right.microsoftClientId &&
    left.curseforgeApiKey === right.curseforgeApiKey
  );
}

export function versionTypeLabel(value: string): string {
  const normalized = value.trim().toLowerCase();
  if (normalized === "release" || normalized === "正式版") return "正式版";
  if (normalized === "snapshot" || normalized === "快照") return "快照";
  if (normalized === "old_beta") return "远古 Beta";
  if (normalized === "old_alpha") return "远古 Alpha";
  return value.trim() || "未知类型";
}

export function loaderLabel(value: string): string {
  const normalized = value.trim();
  if (!normalized || normalized.toLowerCase() === "vanilla" || normalized === "原版") {
    return "原版";
  }
  const known: Record<string, string> = {
    fabric: "Fabric",
    forge: "Forge",
    neoforge: "NeoForge",
    quilt: "Quilt",
    liteloader: "LiteLoader",
    optifine: "OptiFine"
  };
  return known[normalized.toLowerCase()] ?? normalized;
}

export function formatDate(value: string | null): string {
  if (!value) return "从未启动";
  const timestamp = Date.parse(value);
  if (!Number.isFinite(timestamp)) return value;
  return new Intl.DateTimeFormat("zh-CN", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit"
  }).format(timestamp);
}

export function formatDateTime(value: string | null): string {
  if (!value) return "—";
  const timestamp = Date.parse(value);
  if (!Number.isFinite(timestamp)) return value;
  return new Intl.DateTimeFormat("zh-CN", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit"
  }).format(timestamp);
}

export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB"];
  const exponent = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  const amount = bytes / 1024 ** exponent;
  return `${amount >= 100 || exponent === 0 ? amount.toFixed(0) : amount.toFixed(1)} ${units[exponent]}`;
}

export function progressPercent(completed: number, total: number): number | null {
  if (!Number.isFinite(total) || total <= 0) return null;
  const percent = (Math.max(0, completed) / total) * 100;
  return Math.max(0, Math.min(100, percent));
}

export function platformLabel(platform: string): string {
  const normalized = platform.trim().toLowerCase();
  const known: Record<string, string> = {
    windows: "Windows",
    win32: "Windows",
    macos: "macOS",
    darwin: "macOS",
    linux: "Linux"
  };
  const family = normalized.split("-")[0];
  return known[family] ?? (platform || "未知平台");
}
export const LOADER_KINDS: LoaderKind[] = ["fabric", "forge", "neoforge", "quilt", "liteloader", "optifine"];

export function isLoaderKind(value: string): value is LoaderKind {
  return (LOADER_KINDS as string[]).includes(value.trim().toLowerCase());
}

export function loaderKindFromValue(value: string): LoaderKind | null {
  const normalized = value.trim().toLowerCase();
  if (!normalized || normalized === "vanilla" || normalized === "原版") return null;
  const exact = LOADER_KINDS.find((kind) => normalized === kind);
  if (exact) return exact;
  const ordered = [...LOADER_KINDS].sort((left, right) => right.length - left.length);
  return ordered.find((kind) => normalized.includes(kind)) ?? null;
}

export function loaderKindLabel(value: LoaderKind): string {
  const labels: Record<LoaderKind, string> = {
    fabric: "Fabric",
    forge: "Forge",
    neoforge: "NeoForge",
    quilt: "Quilt",
    liteloader: "LiteLoader",
    optifine: "OptiFine"
  };
  return labels[value];
}

export function formatUnixMilliseconds(value: number | null): string {
  if (value === null || !Number.isFinite(value) || value <= 0) return "—";
  return new Intl.DateTimeFormat("zh-CN", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit"
  }).format(value);
}

export function formatUnixSeconds(value: number | null): string {
  if (value === null || !Number.isFinite(value) || value <= 0) return "—";
  return formatUnixMilliseconds(value * 1000);
}

export function gameModeLabel(value: string | null): string {
  if (!value) return "未知";
  const normalized = value.trim().toLowerCase();
  const labels: Record<string, string> = {
    survival: "生存",
    creative: "创造",
    adventure: "冒险",
    spectator: "旁观",
    hardcore: "极限"
  };
  return labels[normalized] ?? value;
}

export function joinPath(base: string, child: string): string {
  const trimmed = base.trim().replace(/[\\/]+$/, "");
  if (!trimmed) return child;
  const separator = trimmed.includes("\\") ? "\\" : "/";
  return `${trimmed}${separator}${child}`;
}
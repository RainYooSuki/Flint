import type { SVGProps } from "react";

export type IconName =
  | "play"
  | "download"
  | "settings"
  | "wrench"
  | "folder"
  | "refresh"
  | "search"
  | "check"
  | "close"
  | "chevronDown"
  | "arrowRight"
  | "open"
  | "user"
  | "cpu"
  | "memory"
  | "monitor"
  | "terminal"
  | "package"
  | "alert"
  | "clock"
  | "wifiOff"
  | "info"
  | "layers"
  | "minus"
  | "square";

const paths: Record<IconName, string> = {
  play: "M8 5.4v13.2L19 12 8 5.4Z",
  download: "M12 3v12m0 0 4.5-4.5M12 15l-4.5-4.5M4 20h16",
  settings: "M12 15.5A3.5 3.5 0 1 0 12 8.5a3.5 3.5 0 0 0 0 7ZM19.4 15a1.7 1.7 0 0 0 .34 1.88l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.7 1.7 0 0 0-1.88-.34 1.7 1.7 0 0 0-1 1.55V21a2 2 0 1 1-4 0v-.08a1.7 1.7 0 0 0-1-1.55 1.7 1.7 0 0 0-1.88.34l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06A1.7 1.7 0 0 0 4.6 15a1.7 1.7 0 0 0-1.55-1H3a2 2 0 1 1 0-4h.08a1.7 1.7 0 0 0 1.55-1 1.7 1.7 0 0 0-.34-1.88l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06A1.7 1.7 0 0 0 9 4.6h.1a1.7 1.7 0 0 0 1-1.55V3a2 2 0 1 1 4 0v.08a1.7 1.7 0 0 0 1 1.55 1.7 1.7 0 0 0 1.88-.34l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06A1.7 1.7 0 0 0 19.4 9v.1a1.7 1.7 0 0 0 1.55 1H21a2 2 0 1 1 0 4h-.08a1.7 1.7 0 0 0-1.55 1Z",
  wrench: "m14.8 6.1-2.3 2.3-2.9-.8-.8-2.9 2.3-2.3a4.2 4.2 0 0 0-4.9 5.5L3 14.2 5.8 17l6.3-6.3a4.2 4.2 0 0 0 5.5-4.9l-2.8 2.9Z",
  folder: "M3 7.5h6l1.7 2H21v9.5H3V7.5Z",
  refresh: "M19.5 10.5A7.7 7.7 0 0 0 5.3 6.8M4.5 4v4h4M4.5 13.5a7.7 7.7 0 0 0 14.2 3.7M19.5 20v-4h-4",
  search: "m20 20-4.2-4.2M18 11a7 7 0 1 1-14 0 7 7 0 0 1 14 0Z",
  check: "m5 12 4.5 4.5L19 6.5",
  close: "m6 6 12 12M18 6 6 18",
  chevronDown: "m7 9.5 5 5 5-5",
  arrowRight: "M4 12h16m-6-6 6 6-6 6",
  open: "M14 4h6v6M20 4l-9 9M19 13v5a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2h5",
  user: "M20 21a8 8 0 0 0-16 0M12 13a5 5 0 1 0 0-10 5 5 0 0 0 0 10Z",
  cpu: "M9 3v2M15 3v2M9 19v2M15 19v2M3 9h2M3 15h2M19 9h2M19 15h2M7 5h10a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2ZM9 9h6v6H9V9Z",
  memory: "M5 7h14v10H5zM8 7V4M12 7V4M16 7V4M8 20v-3M12 20v-3M16 20v-3M9 10v4M12 10v4M15 10v4",
  monitor: "M4 4h16v11H4zM8 20h8M12 15v5",
  terminal: "m5 7 4 4-4 4M11 17h7",
  package: "m12 3 8 4.5v9L12 21l-8-4.5v-9L12 3ZM4 7.5l8 4.5 8-4.5M12 12v9",
  alert: "M12 3 2.8 20h18.4L12 3ZM12 9v5M12 17h.01",
  clock: "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18ZM12 7v5l3 2",
  wifiOff: "m2 2 20 20M8.5 8.5a11 11 0 0 1 11 0M5 5a16 16 0 0 1 14 0M8.5 12.5a6 6 0 0 1 7 0M12 17h.01",
  info: "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18ZM12 10v6M12 7h.01",
  layers: "m12 3-9 5 9 5 9-5-9-5ZM3 12l9 5 9-5M3 16l9 5 9-5",
  minus: "M5 12h14",
  square: "M6 6h12v12H6z"
};

interface IconProps extends Omit<SVGProps<SVGSVGElement>, "name"> {
  name: IconName;
}

export function Icon({ name, className, ...props }: IconProps) {
  const filled = name === "play";
  return (
    <svg
      viewBox="0 0 24 24"
      className={className ? `icon ${className}` : "icon"}
      fill={filled ? "currentColor" : "none"}
      stroke={filled ? "none" : "currentColor"}
      strokeWidth="1.7"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      focusable="false"
      {...props}
    >
      <path d={paths[name]} />
    </svg>
  );
}

export function FlintMark({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      <path
        d="M12 2.4 20 7v10l-8 4.6L4 17V7l8-4.6Z"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.35"
        strokeLinejoin="round"
      />
      <path
        d="M9.2 7.2h5.8v1.9h-3.5v2.1h2.9v1.9h-2.9v4H9.2V7.2Z"
        fill="currentColor"
      />
    </svg>
  );
}
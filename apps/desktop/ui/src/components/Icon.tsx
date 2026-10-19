import type { ReactElement } from "react";

export type IconName =
  | "activity"
  | "alert"
  | "arrow-up"
  | "branch"
  | "check"
  | "chip"
  | "circle"
  | "close"
  | "cloud-off"
  | "code"
  | "edit"
  | "eye"
  | "eye-off"
  | "file"
  | "folder"
  | "home"
  | "maximize"
  | "message"
  | "minimize"
  | "panel-left"
  | "plus"
  | "refresh"
  | "search"
  | "settings"
  | "shield"
  | "stop"
  | "terminal"
  | "trash"
  | "user"
  | "x";

const ICONS: Record<IconName, ReactElement> = {
  activity: (
    <>
      <polyline points="22 12 18 12 15 21 9 3 6 12 2 12" />
    </>
  ),
  alert: (
    <>
      <path d="M12 8v5" />
      <path d="M12 16.5v.01" />
      <circle cx="12" cy="12" r="9" />
    </>
  ),
  "arrow-up": (
    <>
      <path d="M12 19V5" />
      <path d="m6.5 10.5 5.5-5.5 5.5 5.5" />
    </>
  ),
  close: (
    <>
      <path d="m7 7 10 10" />
      <path d="m17 7-10 10" />
    </>
  ),
  "cloud-off": (
    <>
      <path d="M4 4l16 16" />
      <path d="M8.5 6.2A6.5 6.5 0 0 1 18 12.5h.5a3.5 3.5 0 0 1 2.2 6.2" />
      <path d="M6.2 8.3A6.5 6.5 0 0 0 6 18h9" />
    </>
  ),
  maximize: <rect x="6.5" y="6.5" width="11" height="11" rx="1.2" />,
  minimize: <path d="M6.5 12h11" />,
  home: (
    <>
      <path d="M4 11.2 12 4.5l8 6.7" />
      <path d="M6.3 10v9.5h11.4V10" />
      <path d="M10 19.5v-5h4v5" />
    </>
  ),
  message: (
    <>
      <path d="M4.5 6.2A2.2 2.2 0 0 1 6.7 4h10.6a2.2 2.2 0 0 1 2.2 2.2v7.6a2.2 2.2 0 0 1-2.2 2.2H10l-4 3.4v-3.4h-.7a2.2 2.2 0 0 1-2.2-2.2z" />
      <path d="M8.5 9.5h7M8.5 12.5h4.5" />
    </>
  ),
  code: (
    <>
      <path d="m8.5 8-4 4 4 4" />
      <path d="m15.5 8 4 4-4 4" />
      <path d="m13 6.5-2 11" />
    </>
  ),
  chip: (
    <>
      <rect x="7" y="7" width="10" height="10" rx="1.8" />
      <path d="M10 3.5v3.5M14 3.5v3.5M10 17v3.5M14 17v3.5M3.5 10H7M3.5 14H7M17 10h3.5M17 14h3.5" />
    </>
  ),
  settings: (
    <>
      <circle cx="12" cy="12" r="3" />
      <path d="M12 3.5v2.2M12 18.3v2.2M3.5 12h2.2M18.3 12h2.2M6 6l1.6 1.6M16.4 16.4 18 18M18 6l-1.6 1.6M7.6 16.4 6 18" />
    </>
  ),
  "panel-left": (
    <>
      <rect x="3.5" y="4.5" width="17" height="15" rx="2.5" />
      <path d="M9.5 4.5v15" />
    </>
  ),
  plus: (
    <>
      <path d="M12 5.5v13" />
      <path d="M5.5 12h13" />
    </>
  ),
  stop: <rect x="7" y="7" width="10" height="10" rx="1.6" fill="currentColor" stroke="none" />,
  check: (
    <>
      <path d="M6 12.5l4 4 8-8.5" />
    </>
  ),
  edit: (
    <>
      <path d="M4 20l5-1 10-10a1.8 1.8 0 0 0-3-3L6 16l-1 5z" />
      <path d="M13.5 6.5l3 3" />
    </>
  ),
  eye: (
    <>
      <path d="M2.5 12s3.6-6 9.5-6 9.5 6 9.5 6-3.6 6-9.5 6-9.5-6-9.5-6z" />
      <circle cx="12" cy="12" r="2.6" />
    </>
  ),
  "eye-off": (
    <>
      <path d="M4 4l16 16" />
      <path d="M9.6 6.4A9.9 9.9 0 0 1 12 6c5.9 0 9.5 6 9.5 6a17 17 0 0 1-2.6 3.3" />
      <path d="M6.4 8.1A16.6 16.6 0 0 0 2.5 12s3.6 6 9.5 6a9.7 9.7 0 0 0 3.4-.6" />
      <path d="M10.2 10.4a2.6 2.6 0 0 0 3.5 3.5" />
    </>
  ),
  trash: (
    <>
      <path d="M4 7h16" />
      <path d="M9 7V5h6v2" />
      <path d="M6 7l1 12h10l1-12" />
      <path d="M10 11v6M14 11v6" />
    </>
  ),
  terminal: (
    <>
      <polyline points="4 17 10 11 4 5" />
      <line x1="12" y1="19" x2="20" y2="19" />
    </>
  ),
  user: (
    <>
      <circle cx="12" cy="7" r="4" />
      <path d="M5.5 20a6.5 6.5 0 0 1 13 0" />
    </>
  ),
  file: (
    <>
      <path d="M7 3.5h7l4 4v13H7z" />
      <path d="M14 3.5v4h4" />
    </>
  ),
  folder: (
    <>
      <path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7z" />
    </>
  ),
  search: (
    <>
      <circle cx="11" cy="11" r="6" />
      <path d="M15.5 15.5L20 20" />
    </>
  ),
  shield: (
    <>
      <path d="M12 3.5l6 2.2v5.6a7 7 0 0 1-6 6.9 7 7 0 0 1-6-6.9V5.7l6-2.2z" />
      <path d="M9.5 12l1.7 1.7 3.3-3.8" />
    </>
  ),
  branch: (
    <>
      <circle cx="7.5" cy="6.5" r="2.2" />
      <circle cx="7.5" cy="17.5" r="2.2" />
      <circle cx="16.5" cy="12" r="2.2" />
      <path d="M7.5 8.7v6.6" />
      <path d="M7.5 8.7c2.5 0 5 1.5 9 3.3" />
    </>
  ),
  circle: <circle cx="12" cy="12" r="8" />,
  refresh: (
    <>
      <path d="M20 12a8 8 0 1 1-2.34-5.66" />
      <path d="M20 4v6h-6" />
    </>
  ),
  x: (
    <>
      <path d="M7 7l10 10" />
      <path d="M17 7L7 17" />
    </>
  ),
};

interface IconProps {
  name: IconName;
  size?: number;
  className?: string;
}

export function Icon({ name, size = 16, className }: IconProps) {
  return (
    <svg
      aria-hidden="true"
      className={className}
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.6"
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      {ICONS[name]}
    </svg>
  );
}

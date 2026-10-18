import type { ReactElement } from "react";

export type IconName =
  | "alert"
  | "arrow-up"
  | "branch"
  | "check"
  | "circle"
  | "close"
  | "cloud-off"
  | "edit"
  | "file"
  | "folder"
  | "maximize"
  | "minimize"
  | "panel-left"
  | "plus"
  | "refresh"
  | "search"
  | "shield"
  | "stop"
  | "trash"
  | "x";

const ICONS: Record<IconName, ReactElement> = {
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
  trash: (
    <>
      <path d="M4 7h16" />
      <path d="M9 7V5h6v2" />
      <path d="M6 7l1 12h10l1-12" />
      <path d="M10 11v6M14 11v6" />
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

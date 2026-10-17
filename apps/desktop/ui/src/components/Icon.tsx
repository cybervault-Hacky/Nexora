import type { ReactElement } from "react";

export type IconName =
  | "alert"
  | "arrow-up"
  | "close"
  | "cloud-off"
  | "maximize"
  | "minimize"
  | "panel-left"
  | "plus"
  | "stop";

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

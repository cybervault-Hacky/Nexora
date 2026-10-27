import type { ReactNode } from "react";

import "./Eyebrow.css";

export interface EyebrowProps {
  children: ReactNode;
  className?: string;
}

/**
 * Small uppercase label used above a heading or as section metadata.
 *
 * Purely typographic — it carries no colour of its own beyond the muted text
 * token, so it never competes with the surrounding heading.
 */
export default function Eyebrow({ children, className }: EyebrowProps) {
  return (
    <p className={["eyebrow", className].filter(Boolean).join(" ")}>{children}</p>
  );
}

import type { ReactNode } from "react";

import "./Surface.css";

export type SurfaceElevation = "flat" | "raised" | "floating";
export type SurfacePadding = "none" | "sm" | "md" | "lg";

export interface SurfaceProps {
  children: ReactNode;
  /** Border and shadow depth. */
  elevation?: SurfaceElevation;
  /** Internal padding preset. */
  padding?: SurfacePadding;
  className?: string;
}

/**
 * Neutral surface primitive for grouping related content.
 *
 * Deliberately rectangular with a modest radius: surfaces are used sparingly,
 * not wrapped around every block. Elevation is subtle enough that stacked
 * surfaces stay readable.
 */
export default function Surface({
  children,
  elevation = "flat",
  padding = "md",
  className,
}: SurfaceProps) {
  const classes = [
    "surface",
    `surface--${elevation}`,
    `surface--pad-${padding}`,
    className,
  ]
    .filter(Boolean)
    .join(" ");

  return <div className={classes}>{children}</div>;
}

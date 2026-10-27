import type { ReactNode } from "react";

export type SectionSpacing = "default" | "tight" | "flush";

export interface SectionProps {
  children: ReactNode;
  /** Vertical rhythm preset. */
  spacing?: SectionSpacing;
  /** id of the heading that labels this section, for assistive technology. */
  labelledBy?: string;
  className?: string;
}

/**
 * Semantic page section with consistent vertical rhythm.
 *
 * Spacing is owned here so that later phases compose sections without
 * introducing ad-hoc margins.
 */
export default function Section({
  children,
  spacing = "default",
  labelledBy,
  className,
}: SectionProps) {
  const classes = [
    "section",
    spacing === "tight" ? "section--tight" : "",
    spacing === "flush" ? "section--flush" : "",
    className,
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <section className={classes} aria-labelledby={labelledBy}>
      {children}
    </section>
  );
}

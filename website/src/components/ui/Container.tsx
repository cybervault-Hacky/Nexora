import type { ReactNode } from "react";

export type ContainerWidth = "default" | "narrow" | "wide";
export type ContainerElement = "div" | "header" | "main" | "footer" | "section" | "article";

export interface ContainerProps {
  children: ReactNode;
  /** Content width preset. `narrow` is for reading-focused content. */
  width?: ContainerWidth;
  /** Rendered element. Defaults to `div`. */
  as?: ContainerElement;
  className?: string;
}

/**
 * Length-constrained wrapper that applies the responsive page gutters.
 *
 * Use one Container per page region rather than nesting them; the gutters are
 * not designed to stack.
 */
export default function Container({
  children,
  width = "default",
  as: Element = "div",
  className,
}: ContainerProps) {
  return (
    <Element
      className={["container", width !== "default" ? `container--${width}` : "", className]
        .filter(Boolean)
        .join(" ")}
    >
      {children}
    </Element>
  );
}

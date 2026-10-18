import type { ReactNode } from "react";

interface PageHeaderProps {
  eyebrow: string;
  title: string;
  copy: string;
  /** Optional real status/action row rendered under the copy. */
  children?: ReactNode;
}

/**
 * Shared page heading so every route has the same title hierarchy:
 * eyebrow → h1 → supporting copy. Presentation only — no glass or
 * animation work (those are Phase 29/30).
 */
export function PageHeader({ eyebrow, title, copy, children }: PageHeaderProps) {
  return (
    <header className="page-header">
      <p className="eyebrow">{eyebrow}</p>
      <h1 className="page-header__title">{title}</h1>
      <p className="page-header__copy">{copy}</p>
      {children}
    </header>
  );
}

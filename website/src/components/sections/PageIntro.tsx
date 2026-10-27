import type { ReactNode } from "react";

import { Eyebrow } from "../ui";

import "./PageIntro.css";

export interface PageIntroProps {
  /** Small uppercase context label above the heading. */
  eyebrow: string;
  /** Page-level heading. Rendered as the page's single `h1`. */
  title: string;
  /** One or two sentences describing what the page is for. */
  lead: ReactNode;
}

/**
 * Shared page header block: eyebrow, `h1`, and lead paragraph.
 *
 * Every page uses exactly one of these so heading level, measure, and spacing
 * stay identical across the site. Sections below the intro use `h2`.
 */
export default function PageIntro({ eyebrow, title, lead }: PageIntroProps) {
  return (
    <div className="page-intro">
      <Eyebrow>{eyebrow}</Eyebrow>
      <h1 className="page-intro__title">{title}</h1>
      <p className="page-intro__lead">{lead}</p>
    </div>
  );
}

import type { ReactNode } from "react";

import "./TrustSection.css";

export interface TrustSectionProps {
  /** Anchor id, also used to label the section for assistive technology. */
  id: string;
  /** Section heading (`h2`). */
  title: string;
  /** One or two sentences framing the section. */
  lead?: ReactNode;
  children?: ReactNode;
}

/**
 * Editorial block for the Privacy and Security pages.
 *
 * Deliberately not a card: these pages are long-form reading, so sections are
 * separated by a hairline and vertical rhythm instead of being boxed. Renders a
 * plain `div` — the page supplies the surrounding layout, and the `h2` carries
 * the structure, so no extra landmark is needed.
 */
export default function TrustSection({
  id,
  title,
  lead,
  children,
}: TrustSectionProps) {
  return (
    <div className="trust-section">
      <h2 className="trust-section__title" id={id}>
        {title}
      </h2>
      {lead ? <p className="trust-section__lead">{lead}</p> : null}
      {children ? <div className="trust-section__body">{children}</div> : null}
    </div>
  );
}

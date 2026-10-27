import type { ReactNode } from "react";

import "./PageNotice.css";

export interface PageNoticeProps {
  /** Heading for the note. */
  title: string;
  children: ReactNode;
}

/**
 * A restrained qualifying note: the sentence a page owes the reader before its
 * detail begins (what this page is not, what has not been verified yet).
 *
 * Rendered as a rule-marked block rather than a card. A page of disclosure
 * content already carries a lot of structure, and boxing every qualification
 * turns the page into a stack of panels that all compete for the same
 * attention. One hairline of accent plus a small label is enough to mark it as
 * an aside.
 */
export default function PageNotice({ title, children }: PageNoticeProps) {
  return (
    <aside className="page-notice">
      <p className="page-notice__title">{title}</p>
      <div className="page-notice__body">{children}</div>
    </aside>
  );
}

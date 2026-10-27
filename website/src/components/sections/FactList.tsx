import type { TrustFact } from "../../content/trust";

import TrustLabel from "./TrustLabel";

import "./FactList.css";

export interface FactListProps {
  /** Statements to render, in reading order. */
  facts: readonly TrustFact[];
}

/**
 * Labelled two-column list of statements.
 *
 * A description list rather than a stack of cards: these are term/definition
 * pairs, and a `<dl>` says so to assistive technology. The term column carries
 * an evidence label where the statement is a claim, so a reader can see how far
 * each row goes without reading the whole page.
 */
export default function FactList({ facts }: FactListProps) {
  return (
    <dl className="fact-list">
      {facts.map((fact) => (
        <div className="fact-list__row" key={fact.term}>
          <dt className="fact-list__term">
            <span className="fact-list__term-text">{fact.term}</span>
            {fact.status ? <TrustLabel status={fact.status} /> : null}
          </dt>
          <dd className="fact-list__detail">{fact.detail}</dd>
        </div>
      ))}
    </dl>
  );
}

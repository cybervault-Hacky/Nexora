import { TRUST_LEGEND } from "../../content/trust";

import TrustLabel from "./TrustLabel";

import "./TrustLegend.css";

/**
 * Explains the three evidence labels once, near the top of a trust page.
 *
 * Rendered without its own heading: the legend is a reading aid for the labels
 * below, not a section of the page, and promoting it to `h2` would put it ahead
 * of the first real section in the heading outline.
 */
export default function TrustLegend() {
  return (
    <div className="trust-legend">
      <p className="trust-legend__title">How to read the labels on this page</p>
      <dl className="trust-legend__list">
        {TRUST_LEGEND.map((entry) => (
          <div className="trust-legend__row" key={entry.status}>
            <dt className="trust-legend__term">
              <TrustLabel status={entry.status} />
            </dt>
            <dd className="trust-legend__meaning">{entry.meaning}</dd>
          </div>
        ))}
      </dl>
    </div>
  );
}

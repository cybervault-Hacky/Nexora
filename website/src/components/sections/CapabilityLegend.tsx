import type { CapabilityStatus } from "../../content/capabilities";

import CapabilityBadge from "./CapabilityBadge";

import "./CapabilityLegend.css";

/**
 * Explains what each status label on the page means.
 *
 * This exists because "a feature exists" and "a feature works for you" are
 * different claims, and the difference is the most important thing on the page.
 */

const TIERS: ReadonlyArray<{
  status: CapabilityStatus;
  meaning: string;
}> = [
  {
    status: "implemented",
    meaning:
      "The code exists and is covered by the project's automated UI tests, which use simulated responses. The application itself has not been run in a verified environment, so no capability here is claimed as working end to end.",
  },
  {
    status: "unverified",
    meaning:
      "The code exists, but the capability depends on something that has not yet been verified — loading a real model, or running on real Windows hardware.",
  },
  {
    status: "planned",
    meaning:
      "Designed or configured but not demonstrated. It may not be usable yet.",
  },
];

export default function CapabilityLegend() {
  return (
    <div className="cap-legend">
      <h2 className="cap-legend__title">How to read the labels</h2>
      <dl className="cap-legend__list">
        {TIERS.map((tier) => (
          <div className="cap-legend__row" key={tier.status}>
            <dt className="cap-legend__term">
              <CapabilityBadge status={tier.status} />
            </dt>
            <dd className="cap-legend__meaning">{tier.meaning}</dd>
          </div>
        ))}
      </dl>
    </div>
  );
}

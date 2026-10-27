import type { CapabilityStatus } from "../../content/capabilities";

import "./CapabilityBadge.css";

/**
 * Status label for a capability.
 *
 * The wording of the label — not the colour — carries the meaning, so the
 * distinction survives greyscale, colour-blindness, and forced-colours mode.
 * The dot is decorative and hidden from assistive technology.
 */

const LABELS: Record<CapabilityStatus, string> = {
  implemented: "In source",
  unverified: "Not yet verified",
  planned: "Planned",
};

const DESCRIPTIONS: Record<CapabilityStatus, string> = {
  implemented:
    "Implemented in the source code. Not verified by running the application.",
  unverified:
    "Implemented, but the required runtime or platform verification has not happened.",
  planned: "Designed or intended. Not demonstrated.",
};

export interface CapabilityBadgeProps {
  status: CapabilityStatus;
}

export default function CapabilityBadge({ status }: CapabilityBadgeProps) {
  return (
    <span className={`cap-badge cap-badge--${status}`}>
      <span className="cap-badge__dot" aria-hidden="true" />
      <span>{LABELS[status]}</span>
      <span className="visually-hidden"> — {DESCRIPTIONS[status]}</span>
    </span>
  );
}

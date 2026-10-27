import type { TrustStatus } from "../../content/trust";
import { TRUST_LABELS } from "../../content/trust";

import "./TrustLabel.css";

export interface TrustLabelProps {
  status: TrustStatus;
}

/**
 * Evidence label for a trust statement.
 *
 * Deliberately typographic rather than a coloured badge: the three states are
 * distinguished by their words alone, so nothing here can be mistaken for a
 * certification mark or read as "approved" at a glance.
 *
 * The label carries no visually-hidden clarification. An earlier version did,
 * and because these labels appear on every row of several long lists, a screen
 * reader would announce the same sentence more than a dozen times per page —
 * noise, not context. `TrustLegend` explains the three labels once in visible
 * text near the top of each page, which serves sighted and assistive-technology
 * users identically.
 */
export default function TrustLabel({ status }: TrustLabelProps) {
  return (
    <span className={`trust-label trust-label--${status}`}>
      {TRUST_LABELS[status]}
    </span>
  );
}

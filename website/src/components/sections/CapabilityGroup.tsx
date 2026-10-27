import type { CapabilityGroup as CapabilityGroupData } from "../../content/capabilities";

import CapabilityBadge from "./CapabilityBadge";

import "./CapabilityGroup.css";

export interface CapabilityGroupProps {
  group: CapabilityGroupData;
}

/**
 * One capability group: numbered heading, framing sentence, and a definition
 * list of capabilities.
 *
 * An editorial list rather than a card grid — each entry is separated by a
 * hairline, so the page reads as documentation instead of a marketing wall of
 * tiles.
 */
export default function CapabilityGroup({ group }: CapabilityGroupProps) {
  return (
    <section className="cap-group" aria-labelledby={`${group.id}-title`}>
      <div className="cap-group__head">
        <h2 className="cap-group__title" id={`${group.id}-title`}>
          {group.title}
        </h2>
        <p className="cap-group__intro">{group.intro}</p>
      </div>

      <dl className="cap-group__list">
        {group.capabilities.map((capability) => (
          <div className="cap-group__item" key={capability.name}>
            <dt className="cap-group__item-name">
              <span className="cap-group__item-title">{capability.name}</span>
              <CapabilityBadge status={capability.status} />
            </dt>
            <dd className="cap-group__item-detail">{capability.detail}</dd>
          </div>
        ))}
      </dl>
    </section>
  );
}

import { Link } from "react-router-dom";

import {
  CapabilityGroup,
  CapabilityLegend,
  PageIntro,
} from "../components/sections";
import { Container, Section } from "../components/ui";
import { CAPABILITY_GROUPS } from "../content/capabilities";
import { ROUTES } from "../config/routes";
import { usePageMetadata } from "../hooks/usePageMetadata";

import "./FeaturesPage.css";


/**
 * Features page.
 *
 * Documents what exists in the Nexora codebase and states, for every
 * capability, how far the evidence actually goes. The status labels are the
 * point of the page: nothing here is presented as working software, because the
 * desktop application has not been run in a verified environment and no system
 * has been tested on real Windows hardware.
 */
export default function FeaturesPage() {
  usePageMetadata(ROUTES.features);

  return (
    <>
      <Section>
        <Container>
          <PageIntro
            eyebrow="Capabilities"
            title="Built around a more local coding workflow."
            lead="Nexora is a Windows desktop AI coding assistant built around local models. This page describes what exists in the current implementation, and is explicit about how much has actually been verified."
          />

          <CapabilityLegend />

          <p className="features__summary">
            What follows covers the model workflow, coding assistance, model
            management, privacy, and the Windows desktop experience. Available
            capabilities depend on the current implementation and the model you
            select — and, at this stage, on verification that has not yet been
            completed.
          </p>
        </Container>
      </Section>

      <Section spacing="tight">
        <Container>
          <div className="features__groups">
            {CAPABILITY_GROUPS.map((group) => (
              <CapabilityGroup key={group.id} group={group} />
            ))}
          </div>
        </Container>
      </Section>

      <Section spacing="tight">
        <Container>
          <div className="features__limits">
            <h2 className="features__limits-title" id="not-claimed">
              What this page does not claim
            </h2>
            <ul className="features__limits-list">
              <li>
                That local inference has been verified. Both model profiles
                remain flagged as not locally verified, and no generation has
                been recorded against a real model.
              </li>
              <li>
                That the Windows application has been tested. No Windows build or
                installer has been produced, installed, or launched.
              </li>
              <li>
                That Nexora guarantees privacy or security. There is no
                certification, external audit, or guarantee behind that claim.
              </li>
              <li>
                That optional integrations work offline. They are separate from
                core use and are not described here at all.
              </li>
              <li>
                That any benchmark, model-quality result, customer number, or
                review exists. None are published.
              </li>
            </ul>
            <p className="features__limits-note">
              The Windows download is not available yet. When an installer
              exists, it will be published on the{" "}
              <Link to={ROUTES.download}>download page</Link> with its version,
              size, and checksum.
            </p>
          </div>
        </Container>
      </Section>
    </>
  );
}

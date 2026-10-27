import { Link } from "react-router-dom";

import {
  CapabilityBadge,
  PageIntro,
  WorkflowDiagram,
} from "../components/sections";
import { Container, Section } from "../components/ui";
import { READINESS_NOTE, WORKFLOW_STEPS } from "../content/workflow";
import { ROUTES } from "../config/routes";
import { usePageMetadata } from "../hooks/usePageMetadata";

import "./HowItWorksPage.css";


/**
 * How It Works page.
 *
 * Walks through the intended setup and usage sequence, with each step labelled
 * by how far its evidence actually goes. No step is presented as working
 * software: the Windows build, a real model load, and end-to-end local
 * inference have none of them been verified.
 */
export default function HowItWorksPage() {
  usePageMetadata(ROUTES.howItWorks);

  return (
    <>
      <Section>
        <Container>
          <PageIntro
            eyebrow="How it works"
            title="Understand the workflow, before you begin."
            lead="Nexora is designed as a Windows desktop client with locally managed coding models. The sequence below is the intended one, and it is honest about where verification stops."
          />

          <div className="how__readiness">
            <h2 className="how__readiness-title" id="readiness">
              {READINESS_NOTE.title}
            </h2>
            <ul className="how__readiness-list">
              {READINESS_NOTE.items.map((item) => (
                <li key={item}>{item}</li>
              ))}
            </ul>
          </div>

          <WorkflowDiagram />
        </Container>
      </Section>

      <Section spacing="tight">
        <Container>
          <h2 className="how__steps-title" id="the-sequence">
            The sequence
          </h2>

          <ol className="how__steps">
            {WORKFLOW_STEPS.map((step) => (
              <li className="how__step" key={step.id}>
                <div className="how__step-head">
                  <h3 className="how__step-title" id={`${step.id}-title`}>
                    {step.title}
                  </h3>
                  <CapabilityBadge status={step.status} />
                </div>

                <div className="how__step-body">
                  {step.body.map((paragraph) => (
                    <p key={paragraph}>{paragraph}</p>
                  ))}
                  <p className="how__step-evidence">{step.evidence}</p>
                </div>
              </li>
            ))}
          </ol>
        </Container>
      </Section>

      <Section spacing="tight">
        <Container>
          <div className="how__closing">
            <h2 className="how__closing-title" id="what-stays-local">
              What stays on your machine
            </h2>
            <p className="how__closing-body">
              For core generation, the design keeps the work local. Nexora
              launches the local runtime itself and points it at{" "}
              <code>127.0.0.1</code>, and it passes the runtime session
              credential through a file rather than on the command line. No
              external AI provider endpoint is referenced in the inference code.
            </p>
            <p className="how__closing-body">
              That is an architectural statement about the current source, not a
              measured result: the runtime has not been observed here. Optional
              integrations are separate from core generation, and none of them
              is claimed to work offline.
            </p>

            <p className="how__closing-links">
              Per-capability status is documented on the{" "}
              <Link to={ROUTES.features}>features page</Link>, and current
              release status is on the{" "}
              <Link to={ROUTES.download}>download page</Link>.
            </p>
          </div>
        </Container>
      </Section>
    </>
  );
}

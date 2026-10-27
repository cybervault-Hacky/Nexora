import { Link } from "react-router-dom";

import {
  FactList,
  PageIntro,
  PageNotice,
  TrustSection,
} from "../components/sections";
import { Container, Section } from "../components/ui";
import {
  ABOUT_NOT_CLAIMED,
  ABOUT_PIECES,
  ABOUT_PRINCIPLES,
  ABOUT_STATUS,
} from "../content/about";
import { ROUTES } from "../config/routes";
import { usePageMetadata } from "../hooks/usePageMetadata";


/**
 * About page.
 *
 * Explains what the product is, why it is built around local models, and how far
 * the implementation actually goes. No company history, team, funding, or
 * adoption figure appears here, because none has been established for release.
 */
export default function AboutPage() {
  usePageMetadata(ROUTES.about);

  return (
    <>
      <Section>
        <Container>
          <PageIntro
            eyebrow="About"
            title="About the Nexora project."
            lead="Nexora is a Windows desktop AI coding assistant designed around local models. This page explains what that means in practice, how the parts fit together, and how much of it has actually been verified."
          />

          <PageNotice title="About the project, not a company">
            <p>
              There is no published founding story, founding date, team
              biography, investor, customer count, award, or testimonial on this
              site, because none of it has been established for release. What
              follows describes the software in this repository and the
              intentions recorded in it.
            </p>
          </PageNotice>
        </Container>
      </Section>

      <Section spacing="tight">
        <Container>
          <TrustSection id="what-nexora-is" title="What Nexora is">
            <p>
              Nexora is a single-user desktop application for working on code
              with an AI model that answers on your own machine. You supply the
              inference engine, the application helps you acquire a model and
              verifies it, and generation runs against a local process rather
              than a remote service.
            </p>
            <p>
              It is not a hosted product with a browser interface, and it is not
              a coding agent that runs unattended. It is a window you work in,
              with conversation history, project context, and proposed edits kept
              on the same machine.
            </p>
          </TrustSection>

          <TrustSection
            id="why"
            title="Why it is built this way"
            lead="Running the model locally is a trade, and it is worth naming both sides of it."
          >
            <p>
              The reason to do this locally is control. Your source code does not
              become another company's data-handling problem, generation keeps
              working when a backend is unavailable, and you can see what the
              application stores because it is stored on your disk.
            </p>
            <p>
              The cost is real. Local models are several gigabytes, they are
              slower than hosted ones, and what they can do depends on the
              hardware you have. Nexora is an attempt to make that trade workable
              for everyday coding — not a claim that it is strictly better, and
              not a claim that any of it has been measured on Windows yet.
            </p>
          </TrustSection>

          <TrustSection
            id="pieces"
            title="How the pieces fit together"
            lead="Three parts, and only one of them is software this project ships."
          >
            <FactList facts={ABOUT_PIECES} />
          </TrustSection>

          <TrustSection
            id="principles"
            title="Design principles"
            lead="These are the rules the source follows. They are also the reason several sections of this site say no."
          >
            <FactList facts={ABOUT_PRINCIPLES} />
          </TrustSection>

          <TrustSection
            id="status"
            title="Current development status"
            lead="Source-level implementation is not the same thing as behaviour a user can rely on."
          >
            <FactList facts={ABOUT_STATUS} />
            <p>
              The practical consequence is that this is a codebase with a
              documented direction, not a product you can evaluate. The{" "}
              <Link to={ROUTES.download}>download page</Link> states the release
              position in full, and the{" "}
              <Link to={ROUTES.features}>features page</Link> labels every
              capability by how far its evidence goes.
            </p>
          </TrustSection>

          <TrustSection
            id="not-claimed"
            title="What Nexora does not claim"
            lead="A short list, kept short on purpose."
          >
            <ul>
              {ABOUT_NOT_CLAIMED.map((item) => (
                <li key={item}>{item}</li>
              ))}
            </ul>
          </TrustSection>

          <TrustSection
            id="next"
            title="Where to go next"
            lead="Each of these goes into one part of the same picture in more detail."
          >
            <p>
              The <Link to={ROUTES.features}>features page</Link> labels every
              capability by how far its evidence goes, and the{" "}
              <Link to={ROUTES.howItWorks}>how it works page</Link> follows the
              setup sequence end to end. What stays on your device, and the
              controls around it, are on the{" "}
              <Link to={ROUTES.privacy}>privacy</Link> and{" "}
              <Link to={ROUTES.security}>security</Link> pages.
            </p>
            <p className="trust-links">
              Questions about compatibility, hardware, or support are answered on
              the <Link to={ROUTES.faq}>FAQ page</Link>, and practical guidance is
              on the <Link to={ROUTES.support}>support page</Link>.
            </p>
          </TrustSection>
        </Container>
      </Section>
    </>
  );
}

import { Link } from "react-router-dom";

import {
  FactList,
  PageIntro,
  PageNotice,
  TrustLegend,
  TrustSection,
} from "../components/sections";
import { Container, Section } from "../components/ui";
import {
  SAFE_USAGE,
  SECURITY_CONTROLS,
  SECURITY_LAYERS,
  SECURITY_LIMITS,
} from "../content/trust";
import { ROUTES } from "../config/routes";
import { usePageMetadata } from "../hooks/usePageMetadata";


/**
 * Security page.
 *
 * Describes the controls that exist in the desktop application's source, then
 * states their limits in the same voice. No certification, audit result, or
 * absolute assurance is asserted, and no security contact address is invented.
 */
export default function SecurityPage() {
  usePageMetadata(ROUTES.security);

  return (
    <>
      <Section>
        <Container>
          <PageIntro
            eyebrow="Security"
            title="What Nexora does to protect your machine."
            lead="Security claims are easy to make. This page separates what the source code actually enforces from what has merely been designed, and says plainly what nobody has tested."
          />

          <PageNotice title="A source review, not a security assessment">
            <p>
              Every control described here was read in the project's source code
              or configuration. No penetration test, code audit, fuzzing campaign,
              or certification has been performed on this application, its
              account service, or this website — and no Windows build exists to
              test.
            </p>
            <p>
              Local-first architecture is not, by itself, a security guarantee. It
              moves where the work happens; it does not remove the need for care.
            </p>
          </PageNotice>

          <TrustLegend />
        </Container>
      </Section>

      <Section spacing="tight">
        <Container>
          <TrustSection id="approach" title="The security approach">
            <p>
              Nexora keeps three things apart: a window that can reach only the
              application itself, a desktop core that performs network and file
              operations, and a separate engine process that generates text.
              Nothing listens on a public network interface, and no remote
              service is needed to produce a reply.
            </p>
            <p>
              What that buys you is narrower than the phrase "local-first"
              suggests. Your prompts and code stop being someone else's
              data-processing problem, and generation keeps working when a backend
              is unavailable. What it does not buy you is protection from a
              malicious project you open, from a compromised Windows account, from
              an inference engine you installed from an untrusted source, or from
              ordinary bugs in this code.
            </p>
          </TrustSection>

          <TrustSection
            id="how-it-fits-together"
            title="How the application is put together"
            lead="Three parts, with different amounts of trust and different failure modes."
          >
            <FactList facts={SECURITY_LAYERS} />
          </TrustSection>

          <TrustSection
            id="controls"
            title="Controls present in the source"
            lead="Each of these was read in the code or configuration. None has been observed running on Windows."
          >
            <FactList facts={SECURITY_CONTROLS} />
          </TrustSection>

          <TrustSection
            id="network-boundary"
            title="The network boundary, and what it does not cover"
          >
            <p>
              The engine binds to 127.0.0.1 on a port chosen at launch, and it
              requires a key that is generated for that launch alone. This is why
              another computer on your network cannot reach it, and why a request
              that arrives without the key is refused.
            </p>
            <p>
              It is not a complete boundary. Anything running as your Windows user
              can connect to that port, and can read the key file if it knows where
              to look — the file's protection is the access control on the
              application's data folder, not encryption. The key separates your
              session from other users and from the network; it does not separate
              you from a program already running with your permissions.
            </p>
            <p>
              The window is more tightly sealed. Its content security policy allows
              connections only to the application's own IPC channel, and it holds
              no filesystem or network plugin permissions, so compromised page code
              has very little to reach with. That closes one common attack path; it
              does not make the core underneath it invulnerable.
            </p>
          </TrustSection>

          <TrustSection id="model-integrity" title="Model integrity">
            <p>
              A model download must match both a recorded byte count and a recorded
              SHA-256 before it is installed. The transfer lands in a temporary
              file and is published without replacing anything that already exists,
              so a failed or cancelled download never leaves something that looks
              installed.
            </p>
            <p>
              What that proves is that the bytes you received are the bytes this
              project recorded. What it does not prove is that the model is
              trustworthy, free of harmful content, or licensed for redistribution.
              A checksum is evidence of integrity, not of safety, and model files
              carry no signature from anyone.
            </p>
          </TrustSection>

          <TrustSection id="windows-installation" title="Windows installation and signing">
            <p>
              There is no Windows installer. No approved release version, published
              checksum, code-signing identity, or SmartScreen reputation exists
              behind anything you could download today.
            </p>
            <p>
              When a build is eventually published, its checksum will be computed
              from the file that was actually published, and its signing status
              will be stated only after being checked on the artifact itself. Until
              then this project claims neither, and the{" "}
              <Link to={ROUTES.download}>download page</Link> says so in place of a
              button.
            </p>
            <p>
              Do not disable Windows security features or bypass SmartScreen
              warnings to install Nexora. A warning is information, and there is no
              release that would justify working around one.
            </p>
          </TrustSection>

          <TrustSection
            id="not-verified"
            title="What has not been verified"
            lead="The gaps are part of the description. Each of these is a real limitation, not a formality."
          >
            <FactList facts={SECURITY_LIMITS} />
          </TrustSection>

          <TrustSection
            id="reporting"
            title="Reporting a security problem"
            lead="There is no reporting address, portal, response commitment, or bounty programme."
          >
            <p>
              None is published because none exists. Inventing an address would
              send a serious report to a mailbox nobody reads, which is worse than
              saying so. The <Link to={ROUTES.support}>support page</Link> records
              the current state of support, and will list a route for reports once
              one is opened.
            </p>
            <p>
              Until then, please do not publish details that could put other people
              at risk — a working exploit against a build you have, for example.
            </p>
          </TrustSection>

          <TrustSection
            id="safe-usage"
            title="Safe usage"
            lead="Nothing here substitutes for your own judgement about what you run."
          >
            <ul>
              {SAFE_USAGE.map((item) => (
                <li key={item}>{item}</li>
              ))}
            </ul>
            <p className="trust-links">
              Related reading:{" "}
              <Link to={ROUTES.privacy}>privacy</Link>,{" "}
              <Link to={ROUTES.download}>Windows download</Link>,{" "}
              <Link to={ROUTES.features}>features</Link>,{" "}
              <Link to={ROUTES.support}>support</Link>.
            </p>
          </TrustSection>
        </Container>
      </Section>
    </>
  );
}

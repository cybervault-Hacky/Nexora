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
  CREDENTIAL_FACTS,
  DATA_STORAGE_FACTS,
  DIAGNOSTICS_FACTS,
  LOCAL_AI_FACTS,
  NETWORK_DESTINATIONS,
  PRIVACY_GLANCE,
  PRIVACY_PRECAUTIONS,
  PRIVACY_UNKNOWNS,
} from "../content/trust";
import { ROUTES } from "../config/routes";
import { usePageMetadata } from "../hooks/usePageMetadata";


/**
 * Privacy page.
 *
 * Every statement here was checked against the desktop application's source
 * before it was written, and each is labelled with how far that evidence goes.
 * The page carries no legal entity, address, contact, effective date, or
 * jurisdiction statement, because none exists to state.
 */
export default function PrivacyPage() {
  usePageMetadata(ROUTES.privacy);

  return (
    <>
      <Section>
        <Container>
          <PageIntro
            eyebrow="Privacy"
            title="Where your code and conversations go."
            lead="Nexora is designed so that coding assistance runs on your own machine. This page describes what the current source code does with your data, what it does not do, and — just as importantly — what has not been verified."
          />

          <PageNotice title="An informational overview, not a legal policy">
            <p>
              This page explains behaviour found in the project's source code. It
              is not a published privacy policy: there is no legal entity,
              registered address, data-protection contact, effective date, or
              jurisdiction statement behind it, and it has not been reviewed by a
              lawyer or a regulator. It must be replaced by a reviewed policy
              before the application is distributed.
            </p>
            <p>
              Nothing here asserts a certification, an independent audit, or a
              guarantee of privacy.
            </p>
          </PageNotice>

          <TrustLegend />
        </Container>
      </Section>

      <Section spacing="tight">
        <Container>
          <TrustSection
            id="privacy-at-a-glance"
            title="Privacy at a glance"
            lead="Six statements cover most of the picture. The label on each one says how much of it has actually been checked."
          >
            <FactList facts={PRIVACY_GLANCE} />
          </TrustSection>

          <TrustSection
            id="local-ai-and-models"
            title="Local AI and model downloads"
            lead="Downloading a model and using one are separate events, and only one of them involves the network."
          >
            <p>
              Nexora does not ship a model. You download one deliberately, once,
              and from then on the file sits on your disk until you choose
              another. The download is verified against a recorded size and
              SHA-256 digest before it is installed, so a corrupted or swapped
              transfer is rejected rather than half-installed.
            </p>
            <p>
              Using a model is a different path. Your prompt, the file context you
              include, and the reply all travel between the application and a
              local process on the loopback interface. To produce an answer, no
              part of a generation is sent to a remote server.
            </p>
            <FactList facts={LOCAL_AI_FACTS} />
            <p>
              The setup sequence this describes — engine, then model, then a
              first conversation — is set out on the{" "}
              <Link to={ROUTES.howItWorks}>how it works page</Link>.
            </p>
          </TrustSection>

          <TrustSection
            id="data-and-conversations"
            title="Data and conversations"
            lead="What the application writes about your work is stored on this device, and most of it is readable without special tools."
          >
            <p>
              Conversation history is the largest thing Nexora keeps. It is a
              single JSON file, written atomically so an interrupted save cannot
              leave a half-written record. That file is plain text: the
              application adds no encryption, so its protection is whatever your
              Windows account and disk protection provide.
            </p>
            <FactList facts={DATA_STORAGE_FACTS} />
            <p>
              Files inside your project are read when you scan a workspace or ask
              for context, and their content is placed in the prompt for the local
              model. Secret-bearing file names are skipped before they are read.
              That is a name-based filter, not a guarantee about file contents,
              and it cannot protect anything you type into the prompt yourself.
            </p>
          </TrustSection>

          <TrustSection
            id="network-connections"
            title="Network connections and external services"
            lead="The application can reach five destinations. Four are optional or start only when you act; one is this machine."
          >
            <FactList facts={NETWORK_DESTINATIONS} />
            <p>
              The account service entry deserves its own sentence, because it is
              the only place where anything about your work leaves the device. A
              per-generation request to a configured service carries a model
              identifier and a request identifier. It does not carry your prompt,
              your files, or your conversation. If no service address was built
              in, or you are signed out, that request is not made at all.
            </p>
          </TrustSection>

          <TrustSection
            id="credentials"
            title="Credentials and sensitive information"
            lead="What is stored, where it lives, and what that does not protect you from."
          >
            <FactList facts={CREDENTIAL_FACTS} />
            <p>
              None of this makes sensitive data impossible to expose. A secret
              typed into a prompt is written to a plain text file, a compromised
              Windows account can read the application's folders, and no
              credential store is a substitute for the machine being yours.
            </p>
          </TrustSection>

          <TrustSection
            id="analytics"
            title="Analytics, telemetry, and diagnostics"
            lead="There is no analytics service, no crash reporter, and no log file. The only record kept is a counter."
          >
            <FactList facts={DIAGNOSTICS_FACTS} />
            <p>
              This is an absence-of-code finding rather than a policy promise: the
              dependencies and call sites were inspected and contain no
              analytics, session-recording, or crash-upload client. Signing in to
              an account service is the one exception where data is recorded
              outside the device, and that service is described by its schema
              rather than by observed behaviour.
            </p>
          </TrustSection>

          <TrustSection
            id="your-controls"
            title="Your controls and practical precautions"
            lead="You do not need an account to generate locally, and what is stored can be deleted. A few precautions are still worth taking."
          >
            <ul>
              {PRIVACY_PRECAUTIONS.map((item) => (
                <li key={item}>{item}</li>
              ))}
            </ul>
            <p>
              The account surface also includes export and deletion commands, and
              the account service exposes matching endpoints. That path has not
              been exercised against a running service, so it is described here
              rather than offered as a working feature. Local files, exported
              archives, and model downloads sit outside it entirely: deleting an
              account would not remove them from your disk.
            </p>
          </TrustSection>

          <TrustSection
            id="limitations"
            title="Current limitations and verification status"
            lead="These are the gaps. A privacy page that only lists strengths is advertising, not information."
          >
            <ul>
              {PRIVACY_UNKNOWNS.map((item) => (
                <li key={item}>{item}</li>
              ))}
            </ul>
            <p>
              The security side of the same picture — what protects the
              application's processes, files, and network behaviour, and what
              remains unproven — is on the{" "}
              <Link to={ROUTES.security}>security page</Link>.
            </p>
          </TrustSection>

          <TrustSection
            id="questions"
            title="Questions and support"
            lead="If something here is unclear or looks wrong, the support page is the route."
          >
            <p>
              There is no privacy contact address, no data-protection officer, and
              no support inbox published, because none exists yet. The{" "}
              <Link to={ROUTES.support}>support page</Link> records what is and is
              not available, and will list a channel once one is opened.
            </p>
            <p className="trust-links">
              Related reading:{" "}
              <Link to={ROUTES.security}>security</Link>,{" "}
              <Link to={ROUTES.features}>features</Link>,{" "}
              <Link to={ROUTES.howItWorks}>how it works</Link>,{" "}
              <Link to={ROUTES.download}>Windows download</Link>.
            </p>
          </TrustSection>
        </Container>
      </Section>
    </>
  );
}

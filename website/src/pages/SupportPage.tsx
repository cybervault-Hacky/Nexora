import { Link } from "react-router-dom";

import {
  FactList,
  PageIntro,
  PageNotice,
  TrustSection,
} from "../components/sections";
import { Container, Section } from "../components/ui";
import type { TrustFact } from "../content/trust";
import { ROUTES } from "../config/routes";
import { usePageMetadata } from "../hooks/usePageMetadata";


/**
 * Failure modes the source anticipates, in the order a user would meet them.
 *
 * These describe intended behaviour and the messages the code produces. None has
 * been reproduced on Windows, because no Windows build exists.
 */
const CHECKS: readonly TrustFact[] = [
  {
    term: "Confirm what you are running",
    detail:
      "There is no public installer. If someone sent you an .exe described as Nexora, it is not an official release and should not be run. A build you produced yourself from this repository is the only thing that exists today.",
  },
  {
    term: "An engine that will not start",
    detail:
      "Nexora does not ship the inference engine. Check the path you configured, that the binary is built for your CPU architecture, and that the version it reports matches the build this integration targets. The application reports a mismatch rather than working around it.",
  },
  {
    term: "A model that will not load",
    detail:
      "Run the application's verification step against the file, then confirm the path and the free disk space. A model that fails its digest check should be downloaded again, not used anyway.",
  },
  {
    term: "Generation that fails or returns nothing",
    detail:
      "The engine has to be running and reachable on its loopback port. The application surfaces a short tail of the engine's error output, which is usually the fastest way to see why it stopped. A credential-store failure is coded not to block local generation.",
  },
  {
    term: "Account or sign-in errors",
    detail:
      "Expected when no account service address is configured in the build. Local generation is designed not to depend on the account service, so a sign-in failure should not stop you from working.",
  },
  {
    term: "A Windows security warning",
    detail:
      "Do not bypass it. A SmartScreen warning about an unsigned, unpublished build is accurate information rather than an obstacle to work around.",
  },
];

/** The minimum useful report, drawn from the project's diagnostics guidance. */
const RECORD: readonly string[] = [
  "Your Windows version and build, and the CPU architecture the build targets.",
  "The version string the build reports. It identifies the build; it is not a support commitment.",
  "The screen you were on, the exact error code or message, and the approximate time.",
  "A short numbered sequence of what you did immediately before the failure.",
  "The model profile identifier, and whether the model file was supplied by you or acquired through the application.",
  "The version line the engine reports, and whether the model passed verification.",
  "Whether you were signed in — yes or no, without account identifiers.",
];

/** Material that must never appear in a public report. */
const DO_NOT_SHARE: readonly string[] = [
  "Prompts, source files, workspace scans, or conversation history.",
  "Passwords, session tokens, OAuth codes, API keys, or signing material.",
  "Database connection strings, or model weights.",
  "The account-service address configured in the build, which can reveal a private deployment. Redact it before quoting any diagnostic output.",
];

const CAN_CHECK_NOW: readonly string[] = [
  "Whether a binary you were given is an official release. It is not, if it did not come from this project.",
  "The engine path, its architecture, and the version line it prints.",
  "A model file's digest, using the application's own verification step.",
  "Free disk space, which the application also checks before starting a download.",
  "The engine's error output, which the application exposes as a short tail.",
  "Everything on this site, which is the only channel currently maintained.",
];

const NEEDS_RELEASE: readonly string[] = [
  "Installing, launching, upgrading, and uninstalling the application on Windows 10 or 11.",
  "Code signing, SmartScreen reputation, and how the installer behaves under Windows security policy.",
  "The first-run WebView2 setup, which the installer is configured to download.",
  "Performance, memory use, and any minimum hardware specification.",
  "Account, plan, and billing behaviour, which has no deployed service to test against.",
  "The quality or accuracy of model output, which has no published evaluation.",
];

/**
 * Support page.
 *
 * No support email, ticket portal, community, telephone number, office address,
 * response commitment, or bug bounty is published, because none exists — the
 * repository's own `SECURITY.md` and `docs/SUPPORT_AND_DIAGNOSTICS.md` both mark
 * the support channel as a release-blocking placeholder. What is offered instead
 * is guidance that is useful without a channel.
 */
export default function SupportPage() {
  usePageMetadata(ROUTES.support);

  return (
    <>
      <Section>
        <Container>
          <PageIntro
            eyebrow="Support"
            title="Getting help with Nexora."
            lead="There is no public support channel yet. This page still covers what you can check yourself, what to record if something goes wrong, and what has to be true before a real support route exists."
          />

          <PageNotice title="No support channel exists yet">
            <p>
              There is no support email address, ticket portal, community server,
              telephone number, office address, response-time commitment, or bug
              bounty. None is published because none exists, and inventing one
              would send you somewhere that never answers.
            </p>
            <p>
              The repository records the support channel as a release-blocking
              placeholder. This page will list a route once one exists and is
              actually monitored.
            </p>
          </PageNotice>
        </Container>
      </Section>

      <Section spacing="tight">
        <Container>
          <TrustSection
            id="check-first"
            title="Check these first"
            lead="These are the failure modes the source anticipates. None has been reproduced on a Windows build, because none exists yet."
          >
            <FactList facts={CHECKS} />
          </TrustSection>

          <TrustSection
            id="what-to-record"
            title="What to record if you report an issue"
            lead="Record only what is needed to reproduce the problem, and nothing that identifies you or your work."
          >
            <ul>
              {RECORD.map((item) => (
                <li key={item}>{item}</li>
              ))}
            </ul>
            <p>
              Do not include any of the following in a report. This list is drawn
              from the project's own diagnostics guidance, not invented for this
              page.
            </p>
            <ul>
              {DO_NOT_SHARE.map((item) => (
                <li key={item}>{item}</li>
              ))}
            </ul>
            <p>
              The application can report its platform, architecture, app version,
              credential-vault label, and a few configuration flags. Those flags
              describe what the source was built with; they are not independent
              proof of runtime behaviour.
            </p>
          </TrustSection>

          <TrustSection
            id="now-or-later"
            title="What you can check now, and what needs a release"
            lead="This distinction decides whether a problem is yours to investigate or simply not testable yet."
          >
            <p>
              <strong>You can investigate now:</strong>
            </p>
            <ul>
              {CAN_CHECK_NOW.map((item) => (
                <li key={item}>{item}</li>
              ))}
            </ul>
            <p>
              <strong>These need a verified release or real Windows testing:</strong>
            </p>
            <ul>
              {NEEDS_RELEASE.map((item) => (
                <li key={item}>{item}</li>
              ))}
            </ul>
            <p>
              A problem in the second list is not something you can resolve by
              changing a setting. If Windows never ran the build, there is no
              runtime behaviour to debug yet.
            </p>
          </TrustSection>

          <TrustSection
            id="read-next"
            title="Where to read next"
            lead="Most questions are already answered on a documentation page."
          >
            <ul>
              <li>
                <Link to={ROUTES.faq}>FAQ</Link> — compatibility, models,
                privacy, downloads, and support, answered directly.
              </li>
              <li>
                <Link to={ROUTES.features}>Features</Link> — what exists in
                source, and how far each claim has been verified.
              </li>
              <li>
                <Link to={ROUTES.howItWorks}>How it works</Link> — the intended
                setup sequence, including the engine you supply.
              </li>
              <li>
                <Link to={ROUTES.privacy}>Privacy</Link> — what stays on your
                device, and what leaves it.
              </li>
              <li>
                <Link to={ROUTES.security}>Security</Link> — the controls that
                exist, their limits, and how to report a security problem.
              </li>
              <li>
                <Link to={ROUTES.download}>Windows download</Link> — why there is
                nothing to download, and what has to happen first.
              </li>
            </ul>
          </TrustSection>

          <TrustSection
            id="when-a-channel-exists"
            title="When a support channel exists"
            lead="What will change on this page, and what will not be promised in advance."
          >
            <p>
              A route will appear here only once it is real: a monitored inbox or
              portal, a named owner, and a documented process for handling a
              report. Response expectations will be stated at the same time,
              because a channel without them is worse than none.
            </p>
            <p>
              No date is given for that, and none will be given until the release
              blockers this project documents are closed. The{" "}
              <Link to={ROUTES.download}>download page</Link> tracks those
              blockers in the same plain terms.
            </p>
          </TrustSection>
        </Container>
      </Section>
    </>
  );
}

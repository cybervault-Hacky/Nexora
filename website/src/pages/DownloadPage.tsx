import { Link } from "react-router-dom";

import {
  PageIntro,
  ReleaseInfo,
  ReleaseStatus,
} from "../components/sections";
import { Container, Section } from "../components/ui";
import { CURRENT_RELEASE, RELEASE_PREREQUISITES } from "../content/release";
import { ROUTES } from "../config/routes";
import { usePageMetadata } from "../hooks/usePageMetadata";

import "./DownloadPage.css";


const CHECKLIST: readonly { title: string; detail: string }[] = [
  {
    title: "Start from this page",
    detail:
      "Only a link published here is official. A download offered anywhere else is not something this project can vouch for.",
  },
  {
    title: "Match the checksum",
    detail:
      "When a release is published, compare the SHA-256 shown here with a hash you compute from the file you downloaded. If they differ, do not run it.",
  },
  {
    title: "Expect a first-run download",
    detail:
      "The installer's packaging relies on Microsoft's WebView2 runtime, which the installer fetches if it is not already present. The model files are also separate, several-gigabyte downloads, so the first setup needs an internet connection.",
  },
  {
    title: "Plan for local setup",
    detail:
      "Nexora does not bundle the local inference engine. Expect to point it at an engine executable and to choose a model profile before generating anything.",
  },
  {
    title: "Check the requirements when they exist",
    detail:
      "No minimum hardware specification has been published. When one is, it will be based on measured behaviour rather than on the model file sizes alone.",
  },
];

/**
 * Windows Download page.
 *
 * No installer exists, so the page is built around that fact: a plain-text
 * "not available" status, no download control of any kind, no release metadata
 * block, and no invented version, size, checksum, or date. `ReleaseInfo`
 * renders nothing while `CURRENT_RELEASE` is `null`.
 */
export default function DownloadPage() {
  usePageMetadata(ROUTES.download);

  return (
    <>
      <Section>
        <Container>
          <PageIntro
            eyebrow="Windows release"
            title="Nexora for Windows."
            lead="The Windows release is being prepared. A download will be published here only once a genuine installer exists and its release information can be published alongside it."
          />

          <div className="download__status">
            <h2 className="download__section-title" id="release-status">
              Release status
            </h2>
            <ReleaseStatus available={CURRENT_RELEASE !== null} />
          </div>

          <ReleaseInfo release={CURRENT_RELEASE} />

          <p className="download__note">
            No download link, version, file size, checksum, or release date
            appears on this page, because no such release exists yet.
          </p>
        </Container>
      </Section>

      <Section spacing="tight">
        <Container>
          <h2 className="download__section-title" id="what-must-be-ready">
            What has to be ready first
          </h2>
          <p className="download__lead">
            A download link is the last step, not the first. These are the
            conditions this project has set for publishing one.
          </p>

          <ul className="download__prereqs">
            {RELEASE_PREREQUISITES.map((item) => (
              <li className="download__prereq" key={item.title}>
                <h3 className="download__prereq-title">{item.title}</h3>
                <p className="download__prereq-detail">{item.detail}</p>
              </li>
            ))}
          </ul>
        </Container>
      </Section>

      <Section spacing="tight">
        <Container>
          <h2 className="download__section-title" id="before-you-download">
            Before you download
          </h2>
          <p className="download__lead">
            This checklist applies to the eventual release. It is written now so
            the expectations are set before a file exists to click.
          </p>

          <dl className="download__checklist">
            {CHECKLIST.map((item) => (
              <div className="download__check" key={item.title}>
                <dt className="download__check-title">{item.title}</dt>
                <dd className="download__check-detail">{item.detail}</dd>
              </div>
            ))}
          </dl>
        </Container>
      </Section>

      <Section spacing="tight">
        <Container>
          <div className="download__trust">
            <h2 className="download__trust-title" id="verification">
              How the download will be verifiable
            </h2>
            <p className="download__trust-body">
              Every release intended for distribution carries a checksum
              computed from the real installer file. The value is published here
              next to the download so you can confirm the file you received is
              the file that was published.
            </p>
            <p className="download__trust-body">
              No digital signature is claimed. If a signed build is published
              later, the signing status will be stated only after it has been
              verified on the artifact itself. This project also makes no claim
              of malware scanning, security certification, or third-party audit.
            </p>
            <p className="download__trust-body">
              Do not disable Windows security features or bypass SmartScreen
              warnings to install Nexora. If Windows blocks a download, treat
              that as information, not as an obstacle to work around.
            </p>

            <p className="download__trust-links">
              What the product is intended to do is described on the{" "}
              <Link to={ROUTES.features}>features page</Link>, and the setup
              sequence is on the{" "}
              <Link to={ROUTES.howItWorks}>how it works page</Link>.
            </p>
          </div>
        </Container>
      </Section>
    </>
  );
}

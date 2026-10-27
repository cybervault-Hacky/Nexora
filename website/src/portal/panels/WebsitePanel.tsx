import { Link } from "react-router-dom";

import { ROUTES } from "../../config/routes";
import { portalApi } from "../portal-api";
import {
  PortalFacts,
  PortalPanel,
  PortalState,
  formatBytes,
  formatDateTime,
  usePortalResource,
  yesNo,
  type PortalFact,
} from "../PortalPrimitives";

/** Public destinations only — the portal routes are private and excluded. */
const PUBLIC_ROUTES = [
  ROUTES.home,
  ROUTES.features,
  ROUTES.howItWorks,
  ROUTES.download,
  ROUTES.privacy,
  ROUTES.security,
  ROUTES.about,
  ROUTES.faq,
  ROUTES.support,
] as const;

/**
 * Website status.
 *
 * Three questions, kept separate on purpose because conflating them is how a
 * status page starts lying:
 *
 * 1. **Is there a build?** — a fact about this server's filesystem.
 * 2. **Is indexing configured?** — a fact about the build's SEO output.
 * 3. **Is it deployed?** — a fact this portal cannot verify, so it says so
 *    instead of inferring a launch from a running process.
 */
export default function WebsitePanel() {
  const { state, reload } = usePortalResource(portalApi.website);

  if (state.status === "loading") {
    return (
      <PortalPanel title="Website status">
        <PortalState tone="loading" title="Reading the website build…" />
      </PortalPanel>
    );
  }

  if (state.status === "failed") {
    return (
      <PortalPanel title="Website status">
        <PortalState tone="error" title="Website status could not be loaded" onRetry={reload}>
          <p>{state.failure.message}</p>
        </PortalState>
      </PortalPanel>
    );
  }

  const { build, indexing, release, deployment } = state.data;

  const facts: PortalFact[] = [
    {
      term: "Build output",
      detail: build.present
        ? `Present (index.html ${formatBytes(build.bytes)})`
        : "Missing — run a production build",
    },
    { term: "Build timestamp", detail: formatDateTime(build.builtAt) },
    { term: "robots.txt", detail: yesNo(indexing.robotsPresent, "Emitted by the build", "Not emitted") },
    {
      term: "sitemap.xml",
      detail: indexing.sitemapEmitted
        ? "Emitted (a production origin was configured at build time)"
        : "Not emitted — no confirmed production domain is configured",
    },
    {
      term: "Production origin",
      detail: indexing.productionOriginConfigured
        ? "Configured on this server"
        : "Not configured. The build is noindex and publishes no absolute URLs.",
    },
    {
      term: "Deployment",
      detail: deployment.verified
        ? "Verified by a deployment integration"
        : "Not verified — no deployment integration is configured, and a running local server is not a deployment",
    },
    {
      term: "Windows installer",
      detail: release.installerHosted
        ? "An installer file is present on this server"
        : "None. /download still explains that no installer exists.",
    },
  ];

  return (
    <PortalPanel title="Website status">
      <PortalFacts facts={facts} />

      <h2 className="portal-panel__subhead">Public route inventory</h2>
      <p className="portal-panel__note">
        Nine public routes plus a not-found fallback, read from the website's own route
        configuration. Portal routes are deliberately absent and are never listed in a sitemap.
      </p>
      <ul className="portal-routes">
        {PUBLIC_ROUTES.map((path) => (
          <li className="portal-routes__item" key={path}>
            <code className="portal-routes__path">{path}</code>
            <Link className="portal-routes__link" to={path}>
              Open
            </Link>
          </li>
        ))}
        <li className="portal-routes__item">
          <code className="portal-routes__path">*</code>
          <span className="portal-routes__meta">Not-found fallback</span>
        </li>
      </ul>
    </PortalPanel>
  );
}

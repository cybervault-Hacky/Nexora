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

/**
 * Overview.
 *
 * Every value comes from the authenticated API and is a *configuration fact*
 * (does this exist?) rather than a measurement. Nothing here counts visitors,
 * downloads, or revenue, because nothing measures them — and the panel says that
 * rather than showing a zero that would read as a real figure.
 */
export default function OverviewPanel() {
  const { state, reload } = usePortalResource(portalApi.overview);

  if (state.status === "loading") {
    return (
      <PortalPanel title="Overview">
        <PortalState tone="loading" title="Loading portal status…" />
      </PortalPanel>
    );
  }

  if (state.status === "failed") {
    return (
      <PortalPanel title="Overview">
        <PortalState tone="error" title="Portal status could not be loaded" onRetry={reload}>
          <p>{state.failure.message}</p>
        </PortalState>
      </PortalPanel>
    );
  }

  const { portal, account, readiness } = state.data;

  const facts: PortalFact[] = [
    { term: "Website identity", detail: "Nexora — official website for the Windows desktop AI coding assistant." },
    { term: "Environment", detail: portal.environment },
    { term: "Authentication", detail: "Signed in with a verified server-side session stored in an HttpOnly cookie." },
    { term: "Signed in at", detail: formatDateTime(portal.signedInAt) },
    { term: "Session expires", detail: formatDateTime(portal.sessionExpiresAt) },
    { term: "Owner account", detail: yesNo(readiness.ownerProvisioned, "Provisioned", "Not provisioned") },
    {
      term: "Portal origin",
      detail: readiness.portalOriginConfigured
        ? "Configured explicitly (used for the CSRF origin check)."
        : "Not configured: development only. Production requires it.",
    },
    { term: "Session cookie", detail: readiness.secureCookies ? "Secure + HttpOnly" : "HttpOnly (not Secure — development)" },
    {
      term: "Website build",
      detail: readiness.staticBuildPresent
        ? `Present (${formatBytes(readiness.staticBuildBytes)} index.html)`
        : "No built website found on this server",
    },
    {
      term: "Analytics",
      detail: "Not configured. No visitor, download, or usage figures exist and none are shown.",
    },
    { term: "Account", detail: `${account.displayName} — ${account.email} (${account.roleLabel})` },
  ];

  return (
    <PortalPanel title="Overview">
      <PortalFacts facts={facts} />

      <h2 className="portal-panel__subhead">Recent portal activity</h2>
      {portal.recentEvents.length === 0 ? (
        <PortalState tone="empty" title="No portal activity has been recorded yet.">
          <p>Sign-in and sign-out events appear here once they happen.</p>
        </PortalState>
      ) : (
        <ul className="portal-events">
          {portal.recentEvents.map((event) => (
            <li className="portal-events__item" key={`${event.at}-${event.event}`}>
              <span className="portal-events__event">{describeEvent(event.event)}</span>
              <span className="portal-events__time">{formatDateTime(event.at)}</span>
            </li>
          ))}
        </ul>
      )}

      <p className="portal-panel__note">
        Portal readiness is reported as configuration presence. This page does not claim the website
        is deployed, indexed, or reachable from the public internet.
      </p>
    </PortalPanel>
  );
}

function describeEvent(event: string): string {
  switch (event) {
    case "sign_in_success":
      return "Signed in";
    case "sign_in_failure":
      return "Failed sign-in attempt";
    case "sign_in_rate_limited":
      return "Sign-in blocked by rate limiting";
    case "sign_out":
      return "Signed out";
    case "password_rotated":
      return "Owner password rotated on the server";
    default:
      return event.replace(/_/g, " ");
  }
}

import { useState } from "react";

import { portalApi } from "../portal-api";
import { usePortalSession } from "../PortalSession";
import {
  PortalFacts,
  PortalPanel,
  PortalState,
  formatDateTime,
  usePortalResource,
  yesNo,
  type PortalFact,
} from "../PortalPrimitives";

/**
 * Developer settings.
 *
 * Only controls that have a real server-side effect appear here. There is no
 * password-change form: the account is provisioned by an offline CLI, and a
 * browser form that "changed" a password without an endpoint behind it would be
 * exactly the fake security control this portal is supposed to avoid. The panel
 * explains the real procedure instead.
 */
export default function SettingsPanel() {
  const { state, reload } = usePortalResource(portalApi.overview);
  const { signOut } = usePortalSession();
  const [signingOut, setSigningOut] = useState(false);
  const [signOutError, setSignOutError] = useState<string | null>(null);

  if (state.status === "loading") {
    return (
      <PortalPanel title="Developer settings">
        <PortalState tone="loading" title="Loading account settings…" />
      </PortalPanel>
    );
  }

  if (state.status === "failed") {
    return (
      <PortalPanel title="Developer settings">
        <PortalState tone="error" title="Account settings could not be loaded" onRetry={reload}>
          <p>{state.failure.message}</p>
        </PortalState>
      </PortalPanel>
    );
  }

  const { account, portal, readiness } = state.data;

  const facts: PortalFact[] = [
    { term: "Account", detail: `${account.displayName} (${account.email})` },
    { term: "Role", detail: account.roleLabel },
    { term: "Session started", detail: formatDateTime(portal.signedInAt) },
    { term: "Session expires", detail: formatDateTime(portal.sessionExpiresAt) },
    {
      term: "Session storage",
      detail: "HttpOnly cookie, revocable server-side. No token is stored in the browser's storage.",
    },
    { term: "Secure cookies", detail: yesNo(readiness.secureCookies, "Enabled", "Disabled (development only)") },
    {
      term: "Portal origin",
      detail: readiness.portalOriginConfigured ? "Configured" : "Not configured — development only",
    },
  ];

  async function handleSignOut() {
    if (signingOut) return;
    setSigningOut(true);
    setSignOutError(null);
    try {
      await signOut();
    } catch {
      setSignOutError("The session could not be ended on the server. Reload to re-check your session.");
    } finally {
      setSigningOut(false);
    }
  }

  return (
    <PortalPanel
      title="Developer settings"
      description="Account and session controls. Every action here has a server-side effect."
    >
      <PortalFacts facts={facts} />

      <h2 className="portal-panel__subhead">Session</h2>
      <div className="portal-actions">
        <button type="button" className="portal-button" onClick={handleSignOut} disabled={signingOut}>
          {signingOut ? "Signing out…" : "Sign out"}
        </button>
        <p className="portal-actions__hint">
          Signing out revokes the session on the server, not just in this tab.
        </p>
      </div>
      {signOutError !== null ? (
        <p className="portal-inline-error" role="alert">
          {signOutError}
        </p>
      ) : null}

      <h2 className="portal-panel__subhead">Password</h2>
      <p className="portal-panel__text">
        There is no password form, because password changes are not exposed over HTTP. To rotate the
        owner password, run this on the server — it reads the new password without echoing it and
        revokes every existing session:
      </p>
      <pre className="portal-code">
        <code>node server/provision-owner.mjs --email {account.email} --rotate-password</code>
      </pre>

      <h2 className="portal-panel__subhead">Security guidance</h2>
      <ul className="portal-list">
        <li>Sign in only over HTTPS, and only from a device you control.</li>
        <li>Keep the portal's server-side configuration out of the website build and out of chat.</li>
        <li>Treat a session that outlives a password rotation as a defect, not a convenience.</li>
        <li>Report a suspected compromise by rotating the owner password, which ends all sessions.</li>
      </ul>

      <h2 className="portal-panel__subhead">Configuration status</h2>
      <p className="portal-panel__text">
        Presence only — no secret value is ever sent to this page.
      </p>
      <ul className="portal-list">
        <li>Owner account: {yesNo(readiness.ownerProvisioned, "provisioned", "not provisioned")}</li>
        <li>Portal origin: {yesNo(readiness.portalOriginConfigured, "configured", "not configured")}</li>
        <li>Secure cookies: {yesNo(readiness.secureCookies, "enabled", "disabled")}</li>
        <li>Support inbox: {yesNo(readiness.supportInboxConfigured, "configured", "not configured")}</li>
        <li>Content management: {yesNo(readiness.contentManagementConfigured, "configured", "not configured")}</li>
        <li>Analytics: {yesNo(readiness.analyticsConfigured, "configured", "not configured")}</li>
      </ul>
    </PortalPanel>
  );
}

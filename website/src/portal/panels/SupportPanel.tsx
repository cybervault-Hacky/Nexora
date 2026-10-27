import { Link } from "react-router-dom";

import { ROUTES } from "../../config/routes";
import { portalApi } from "../portal-api";
import { PortalPanel, PortalState, usePortalResource } from "../PortalPrimitives";

/**
 * Support overview.
 *
 * No support inbox, ticketing system, or contact form backend exists, so there
 * are no tickets to show and nothing that could fabricate one. The panel states
 * that plainly and points at the public guidance that does exist.
 */
export default function SupportPanel() {
  const { state, reload } = usePortalResource(portalApi.support);

  if (state.status === "loading") {
    return (
      <PortalPanel title="Support overview">
        <PortalState tone="loading" title="Checking support configuration…" />
      </PortalPanel>
    );
  }

  if (state.status === "failed") {
    return (
      <PortalPanel title="Support overview">
        <PortalState tone="error" title="Support status could not be loaded" onRetry={reload}>
          <p>{state.failure.message}</p>
        </PortalState>
      </PortalPanel>
    );
  }

  return (
    <PortalPanel
      title="Support overview"
      description="Requests are not connected to any backend, so this portal has no request queue to show."
    >
      <PortalState tone="unconfigured" title="Support request management is not configured">
        <p>{state.data.reason}</p>
      </PortalState>

      <h2 className="portal-panel__subhead">What exists today</h2>
      <ul className="portal-list">
        <li>
          The public <Link to={ROUTES.support}>Support page</Link> explains what a user can check
          themselves before reporting a problem.
        </li>
        <li>
          The public <Link to={ROUTES.faq}>FAQ</Link> answers the questions that do not need a
          human.
        </li>
      </ul>

      <p className="portal-panel__note">
        No ticket counts, response times, or support volumes are shown, because none are measured.
      </p>
    </PortalPanel>
  );
}

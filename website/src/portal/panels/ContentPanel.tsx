import { portalApi } from "../portal-api";
import { PortalPanel, PortalState, usePortalResource } from "../PortalPrimitives";

/**
 * Website content.
 *
 * There is no CMS, no content API, and no publish pipeline, so this panel shows
 * an explicit unconfigured state. It deliberately renders **no editing
 * controls**: a disabled-looking text area or a "save" button with nothing
 * behind it would be a fake affordance, and a real one would let a browser user
 * rewrite application source, which is out of scope for this portal.
 */
export default function ContentPanel() {
  const { state, reload } = usePortalResource(portalApi.content);

  if (state.status === "loading") {
    return (
      <PortalPanel title="Website content">
        <PortalState tone="loading" title="Checking content configuration…" />
      </PortalPanel>
    );
  }

  if (state.status === "failed") {
    return (
      <PortalPanel title="Website content">
        <PortalState tone="error" title="Content status could not be loaded" onRetry={reload}>
          <p>{state.failure.message}</p>
        </PortalState>
      </PortalPanel>
    );
  }

  return (
    <PortalPanel
      title="Website content"
      description="Copy on the public site is versioned in the repository and shipped by the build. Editing it needs a review and publish path, not a browser text box."
    >
      <PortalState tone="unconfigured" title="Content management is not configured">
        <p>{state.data.reason}</p>
      </PortalState>

      <h2 className="portal-panel__subhead">What would have to exist first</h2>
      <ul className="portal-list">
        <li>A content store with an audit trail, so a change can be attributed and reverted.</li>
        <li>A publish path that rebuilds the static site and invalidates the CDN cache.</li>
        <li>Approval rules for public claims, which are currently held to a strict evidence bar.</li>
      </ul>

      <p className="portal-panel__note">
        Nothing here writes to the repository, and no file in the website source is reachable from
        the browser.
      </p>
    </PortalPanel>
  );
}

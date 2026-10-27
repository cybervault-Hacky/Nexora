import { Navigate, useLocation } from "react-router-dom";

import { ROUTES } from "../config/routes";
import { usePageMetadata } from "../hooks/usePageMetadata";
import PortalDashboard, { PORTAL_SECTIONS, portalHref, type PortalSection } from "./PortalDashboard";
import PortalSignIn from "./PortalSignIn";
import { PortalSessionProvider, usePortalSession } from "./PortalSession";
import { PortalState } from "./PortalPrimitives";
import ContentPanel from "./panels/ContentPanel";
import OverviewPanel from "./panels/OverviewPanel";
import SettingsPanel from "./panels/SettingsPanel";
import SupportPanel from "./panels/SupportPanel";
import WebsitePanel from "./panels/WebsitePanel";

import "./PortalDashboard.css";

/**
 * `/admin` and every `/admin/*` path.
 *
 * The route guard below is **presentation only**: it decides whether to show a
 * form or a panel. It is not the security boundary. Deleting this file from the
 * bundle would leave an attacker with a list of API paths that all answer `401`
 * without a session cookie and the CSRF header, and no privileged data in the
 * bundle to read.
 *
 * There is no `?next=` or `?redirect=` parameter anywhere in this flow, so there
 * is no user-supplied destination to validate and no open redirect to get wrong.
 */
export default function PortalPage() {
  usePageMetadata(ROUTES.admin);
  return (
    <PortalSessionProvider>
      <PortalShell />
    </PortalSessionProvider>
  );
}

function sectionFor(pathname: string): PortalSection | null {
  if (!pathname.startsWith(ROUTES.admin)) return null;
  const rest = pathname.slice(ROUTES.admin.length).replace(/^\/+/, "").replace(/\/+$/, "");
  if (rest === "") return PORTAL_SECTIONS[0] ?? null;
  return PORTAL_SECTIONS.find((section) => section.slug === rest) ?? null;
}

function PortalShell() {
  const { state, refresh } = usePortalSession();
  const { pathname } = useLocation();

  if (state.status === "loading") {
    return (
      <div className="portal portal--centered">
        <PortalState tone="loading" title="Checking your session…">
          <p>Asking the portal server whether this browser has a live session.</p>
        </PortalState>
      </div>
    );
  }

  if (state.status === "unavailable") {
    return (
      <div className="portal portal--centered">
        <PortalState tone="error" title="The developer portal is unavailable" onRetry={() => void refresh()}>
          <p>{state.message}</p>
          <p>
            The portal needs its Node server running. No sign-in is simulated while it is
            unreachable, and the public website is unaffected.
          </p>
        </PortalState>
      </div>
    );
  }

  if (state.status === "signed-out") {
    // A protected sub-route on a signed-out browser goes to the sign-in page. The
    // destination is a literal, not a user-supplied parameter.
    if (pathname !== ROUTES.admin) return <Navigate to={ROUTES.admin} replace />;
    return (
      <div className="portal portal--auth">
        <PortalSignIn />
      </div>
    );
  }

  // Signed in. Unknown `/admin/...` paths stay inside the portal with an honest
  // message rather than falling through to the public 404 (which would be
  // confusing) or to a redirect loop.
  const section = sectionFor(pathname);
  if (section === null) {
    return (
      <div className="portal">
        <div className="portal__unknown">
          <PortalState tone="empty" title="That portal section does not exist.">
            <p>
              The signed-in portal has five sections:{" "}
              {PORTAL_SECTIONS.map((item, index) => (
                <span key={item.slug || "overview"}>
                  {index > 0 ? ", " : ""}
                  <a href={portalHref(item.slug)}>{item.label}</a>
                </span>
              ))}
              .
            </p>
          </PortalState>
        </div>
      </div>
    );
  }

  return (
    <PortalDashboard section={section}>
      <SectionPanel slug={section.slug} />
    </PortalDashboard>
  );
}

function SectionPanel({ slug }: { slug: string }) {
  switch (slug) {
    case "":
      return <OverviewPanel />;
    case "website":
      return <WebsitePanel />;
    case "content":
      return <ContentPanel />;
    case "support":
      return <SupportPanel />;
    case "settings":
      return <SettingsPanel />;
    default:
      return null;
  }
}

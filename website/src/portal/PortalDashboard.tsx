import { useState, type ReactNode } from "react";
import { NavLink } from "react-router-dom";

import ApertureMark from "../components/brand/ApertureMark";
import { Eyebrow } from "../components/ui";
import { usePortalSession } from "./PortalSession";

import "./PortalDashboard.css";

export interface PortalSection {
  /** URL segment after `/admin`, or empty for the overview. */
  slug: string;
  /** Navigation label. */
  label: string;
  /** Page heading, and the `h1` of the section. */
  title: string;
  /** One line under the heading. */
  description: string;
}

/** The dashboard's five sections, in navigation order. */
export const PORTAL_SECTIONS: readonly PortalSection[] = [
  {
    slug: "",
    label: "Overview",
    title: "Overview",
    description: "Portal and environment facts, read from the running server.",
  },
  {
    slug: "website",
    label: "Website",
    title: "Website status",
    description: "Build, indexing and deployment state — including what cannot be verified.",
  },
  {
    slug: "content",
    label: "Content",
    title: "Website content",
    description: "How public copy is managed today, and what would have to exist to edit it here.",
  },
  {
    slug: "support",
    label: "Support",
    title: "Support overview",
    description: "Support request handling, which is not connected to a backend.",
  },
  {
    slug: "settings",
    label: "Settings",
    title: "Developer settings",
    description: "Account identity, session control, and security guidance.",
  },
];

/** `href` for a section, keeping `/admin` (not `/admin/`) for the overview. */
export function portalHref(slug: string): string {
  return slug === "" ? "/admin" : `/admin/${slug}`;
}

export interface PortalDashboardProps {
  section: PortalSection;
  children: ReactNode;
}

/**
 * The authenticated shell.
 *
 * Structure is intentionally plain: one `h1` per section, a navigation landmark
 * with `aria-current` on the active item, and an account area whose sign-out is a
 * real button. The navigation is a list of links rather than a tab widget, so it
 * works with browser history, refresh, and direct navigation.
 */
export default function PortalDashboard({ section, children }: PortalDashboardProps) {
  const { state, signOut } = usePortalSession();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const user = state.status === "signed-in" ? state.user : null;

  async function handleSignOut() {
    if (busy) return;
    setBusy(true);
    setError(null);
    try {
      await signOut();
    } catch {
      setError("The session could not be ended on the server. Reload to re-check your session.");
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="portal">
      <header className="portal__bar">
        <div className="portal__identity">
          <ApertureMark size={20} className="portal__mark" />
          <span className="portal__wordmark">Nexora</span>
          <span className="portal__divider" aria-hidden="true" />
          <span className="portal__portal-label">Developer Portal</span>
        </div>

        <div className="portal__account">
          <div className="portal__account-text">
            <span className="portal__account-name">{user?.displayName ?? "Signed in"}</span>
            <span className="portal__account-mail">{user?.email ?? ""}</span>
          </div>
          <button type="button" className="portal__signout" onClick={handleSignOut} disabled={busy}>
            {busy ? "Signing out…" : "Sign out"}
          </button>
        </div>
      </header>

      {error !== null ? (
        <p className="portal-inline-error" role="alert">
          {error}
        </p>
      ) : null}

      <div className="portal__layout">
        <nav className="portal__nav" aria-label="Developer portal sections">
          <ul className="portal__nav-list">
            {PORTAL_SECTIONS.map((item) => (
              <li className="portal__nav-item" key={item.slug || "overview"}>
                <NavLink
                  to={portalHref(item.slug)}
                  end={item.slug === ""}
                  className={({ isActive }) => `portal__nav-link${isActive ? " portal__nav-link--active" : ""}`}
                >
                  {item.label}
                </NavLink>
              </li>
            ))}
          </ul>
        </nav>

        <div className="portal__main">
          <Eyebrow>Developer Portal</Eyebrow>
          <h1 className="portal__title">{section.title}</h1>
          <p className="portal__description">{section.description}</p>
          <div className="portal__panel-slot">{children}</div>
        </div>
      </div>
    </div>
  );
}

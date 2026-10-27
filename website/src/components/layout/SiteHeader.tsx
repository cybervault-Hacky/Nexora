import { useEffect, useId, useRef, useState } from "react";
import { Link, NavLink, useLocation } from "react-router-dom";

import ApertureMark from "../brand/ApertureMark";
import { Container } from "../ui";
import { NAV_ITEMS, ROUTES } from "../../config/routes";

import "./SiteHeader.css";

/** Viewport width at which the desktop navigation takes over (1024px). */
const DESKTOP_QUERY = "(min-width: 64rem)";

/**
 * Site header: brand, primary navigation, and the narrow-viewport menu.
 *
 * Links are real routes. `NavLink` supplies `aria-current="page"` for the
 * active destination, which is what tells assistive technology where the user
 * is — no colour-only indication.
 *
 * The menu is an ARIA disclosure, not a modal dialog: the toggle owns
 * `aria-expanded`/`aria-controls`, focus stays in normal document order, and
 * there is no focus trap.
 *
 * Phase 13: the brand and the navigation controls live inside ONE glass
 * capsule. The capsule is the grid (three columns on desktop, two on narrow
 * screens), so the logo and the links are genuinely in the same container
 * rather than two boxes sitting next to each other. The mobile panel is a
 * sibling of the capsule, not a child: it is an overlay card that drops out of
 * the capsule without inheriting its clip or its blur.
 */
export default function SiteHeader() {
  const [menuOpen, setMenuOpen] = useState(false);
  const panelId = useId();
  const toggleRef = useRef<HTMLButtonElement>(null);
  const headerRef = useRef<HTMLElement>(null);
  const { pathname } = useLocation();

  // A completed navigation should never leave the panel open — including
  // navigations triggered by the browser's back/forward buttons.
  useEffect(() => {
    setMenuOpen(false);
  }, [pathname]);

  // Close the menu when the viewport grows past the desktop breakpoint, so a
  // stale open state can never leave a hidden panel open on resize.
  useEffect(() => {
    if (!menuOpen) return;

    const query = window.matchMedia(DESKTOP_QUERY);
    const onChange = (event: MediaQueryListEvent) => {
      if (event.matches) setMenuOpen(false);
    };

    query.addEventListener("change", onChange);
    return () => query.removeEventListener("change", onChange);
  }, [menuOpen]);

  // Escape closes the menu and returns focus to the toggle.
  useEffect(() => {
    if (!menuOpen) return;

    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      setMenuOpen(false);
      toggleRef.current?.focus();
    };

    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, [menuOpen]);

  // Clicking outside the header closes the menu.
  useEffect(() => {
    if (!menuOpen) return;

    const onPointerDown = (event: PointerEvent) => {
      const target = event.target;
      if (target instanceof Node && !headerRef.current?.contains(target)) {
        setMenuOpen(false);
      }
    };

    document.addEventListener("pointerdown", onPointerDown);
    return () => document.removeEventListener("pointerdown", onPointerDown);
  }, [menuOpen]);

  const closeMenu = () => setMenuOpen(false);

  return (
    <header className="site-header" ref={headerRef}>
      <Container className="site-header__shell">
        <div className="site-header__capsule">
          <Link className="site-header__brand" to={ROUTES.home}>
            <ApertureMark size={26} className="site-header__mark" />
            <span className="site-header__wordmark">Nexora</span>
          </Link>

          <nav className="site-header__nav" aria-label="Primary">
          <ul className="site-header__nav-list">
            {NAV_ITEMS.map((item) => (
              <li key={item.path}>
                <NavLink
                  className={({ isActive }) =>
                    isActive
                      ? "site-header__link site-header__link--active"
                      : "site-header__link"
                  }
                  to={item.path}
                >
                  {item.label}
                </NavLink>
              </li>
            ))}
          </ul>

          <button
            type="button"
            ref={toggleRef}
            className="site-header__toggle"
            aria-expanded={menuOpen}
            aria-controls={panelId}
            onClick={() => setMenuOpen((open) => !open)}
          >
            <span className="visually-hidden">{menuOpen ? "Close menu" : "Menu"}</span>
            {/* Three bars that fold into a cross. Drawn as elements rather than
                swapping two SVG paths so the change can be transitioned — a
                path swap can only ever cut. Decorative: the button's label
                above already says what it does, and `aria-expanded` carries
                the state. */}
            <span className="site-header__icon" aria-hidden="true">
              <span className="site-header__bar" />
              <span className="site-header__bar" />
              <span className="site-header__bar" />
            </span>
            </button>
          </nav>
        </div>

        {/* Mounted at all times so opening and closing can both be
            transitioned. While closed it is `inert` and `visibility: hidden`,
            so it is unreachable by pointer, keyboard and assistive technology
            exactly as if it were not rendered. */}
        <div
          className="site-header__panel"
          id={panelId}
          data-open={menuOpen}
          inert={!menuOpen}
        >
          <ul className="site-header__panel-list">
            {NAV_ITEMS.map((item) => (
              <li key={item.path}>
                <NavLink
                  className={({ isActive }) =>
                    isActive
                      ? "site-header__panel-link site-header__panel-link--active"
                      : "site-header__panel-link"
                  }
                  to={item.path}
                  onClick={closeMenu}
                >
                  {item.label}
                </NavLink>
              </li>
            ))}
          </ul>
        </div>
      </Container>
    </header>
  );
}

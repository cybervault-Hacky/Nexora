import { useEffect, useRef } from "react";
import { Outlet, useLocation } from "react-router-dom";

import CustomCursor from "../motion/CustomCursor";
import SiteFooter from "./SiteFooter";
import SiteHeader from "./SiteHeader";

import "./SiteLayout.css";

/**
 * Shared page layout.
 *
 * Owns the persistent chrome (skip link, header, footer) and the single `main`
 * landmark that routed pages render into, so no page duplicates that markup.
 *
 * On a route change it resets scroll and moves focus to `main`. Without this a
 * client-side navigation would leave focus on the old link and the viewport at
 * the previous scroll offset — the two most common SPA accessibility failures.
 * `main` is the focus target rather than the `h1` so screen readers announce
 * the landmark and users land at the top of the new content.
 *
 * Phase 12 adds two things to the shell:
 * - the decorative cursor overlay, mounted outside `main` so that the page
 *   entrance transform can never become its containing block;
 * - a keyed wrapper around the routed content, which restarts a short
 *   fade-and-rise on every navigation. The wrapper is `key`ed on the pathname,
 *   so the animation replays per route without any transition library, and the
 *   new page is in the DOM immediately — the transition is an entrance, never a
 *   held-open exit, so content can never be hidden waiting for an animation.
 */
export default function SiteLayout() {
  const { pathname } = useLocation();
  const mainRef = useRef<HTMLElement>(null);
  const lastPath = useRef<string | null>(null);

  useEffect(() => {
    // The initial page load already has correct scroll and focus; moving focus
    // then would steal it from the address bar or a deep link.
    //
    // This compares the *previous pathname* rather than using a one-shot
    // "is first render" boolean. React StrictMode invokes effects twice on
    // mount, so a one-shot flag flips on the discarded first pass and the
    // second pass then focuses `main` — which moved the sequential focus
    // point past the header, so the very first Tab skipped the skip link.
    // Tracking the last pathname is idempotent under a double invoke.
    if (lastPath.current === null) {
      lastPath.current = pathname;
      return;
    }
    if (lastPath.current === pathname) return;
    lastPath.current = pathname;

    window.scrollTo({ top: 0, left: 0, behavior: "auto" });
    mainRef.current?.focus();
  }, [pathname]);

  return (
    <>
      <a className="skip-link" href="#main">
        Skip to main content
      </a>

      <CustomCursor />

      <div className="site">
        <SiteHeader />

        <main className="site__main" id="main" ref={mainRef} tabIndex={-1}>
          <div className="site__route" key={pathname}>
            <Outlet />
          </div>
        </main>

        <SiteFooter />
      </div>
    </>
  );
}

import type { AnchorHTMLAttributes, ReactNode } from "react";
import { navigateTo, routeHref, type RouteId } from "../lib/router";

interface RouterLinkProps extends Omit<AnchorHTMLAttributes<HTMLAnchorElement>, "href"> {
  to: RouteId;
  children: ReactNode;
}

/**
 * An in-app navigation anchor. It keeps real anchor semantics (href, focus,
 * middle-click) but performs the hash navigation explicitly so every host —
 * Tauri webview, browser preview, jsdom tests — behaves the same. Modifier
 * clicks are left to the browser.
 */
export function RouterLink({ to, children, onClick, ...rest }: RouterLinkProps) {
  return (
    <a
      href={routeHref(to)}
      onClick={(event) => {
        onClick?.(event);
        if (event.defaultPrevented) return;
        if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
        event.preventDefault();
        navigateTo(to);
      }}
      {...rest}
    >
      {children}
    </a>
  );
}

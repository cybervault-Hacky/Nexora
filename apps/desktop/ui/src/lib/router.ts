import { useSyncExternalStore } from "react";

/**
 * Dependency-free hash router for the Nexora shell.
 *
 * Hash routing was chosen deliberately for Phase 19:
 * - it works in the packaged Tauri webview (`tauri.localhost` custom protocol),
 *   the Vite browser preview, and jsdom tests without server rewrite rules;
 * - it needs no new dependency (the app ships only `react`, `react-dom`,
 *   `@tauri-apps/api`, and `diff`);
 * - hash changes are same-document navigations, so switching pages never
 *   reloads the app and the browser's back/forward buttons keep working.
 */

export const ROUTES = [
  { id: "welcome", path: "/welcome", label: "Welcome" },
  { id: "onboarding", path: "/onboarding", label: "Onboarding" },
  { id: "signin", path: "/signin", label: "Sign In" },
  { id: "signup", path: "/signup", label: "Create account" },
  { id: "verify", path: "/verify-email", label: "Email verification" },
  { id: "recover", path: "/recover-password", label: "Password recovery" },
  { id: "reset", path: "/reset-password", label: "Password reset" },
  { id: "home", path: "/home", label: "Home" },
  { id: "chat", path: "/chat", label: "AI Chat" },
  { id: "projects", path: "/projects", label: "Projects" },
  { id: "code", path: "/code", label: "Code Intelligence" },
  { id: "models", path: "/models", label: "Local Models" },
  { id: "settings", path: "/settings", label: "Settings" },
  { id: "account", path: "/account", label: "Account" },
  { id: "founder", path: "/founder", label: "Founder Control" },
  { id: "licensing", path: "/licensing", label: "Licensing & Plans" },
] as const;

export type RouteId = (typeof ROUTES)[number]["id"];

/**
 * Route ids that belong to the pre-workspace onboarding flow. They render
 * outside the workspace sidebar and keep separate from the feature pages.
 * `verify` and `reset` are the future authentication-flow targets; until that
 * backend exists their pages state their non-functional status honestly.
 */
export const FLOW_ROUTES: ReadonlyArray<AnyRouteId> = [
  "welcome",
  "onboarding",
  "signin",
  "signup",
  "verify",
  "recover",
  "reset",
];

/** True for the welcome/onboarding flow (no workspace sidebar). */
export function isFlowRoute(id: AnyRouteId): boolean {
  return FLOW_ROUTES.includes(id);
}

/** Every recognized route plus the graceful fallback. */
export type AnyRouteId = RouteId | "notFound";

export interface RouteMatch {
  id: AnyRouteId;
  /** The path as written in the URL, used by the not-found page. */
  path: string;
}

/** Normalizes a raw hash (e.g. `#/chat`, `#/chat/`, `#`) to a path. */
function normalizePath(hash: string): string {
  let raw = hash.startsWith("#") ? hash.slice(1) : hash;
  // Drop any query/secondary fragment noise before matching.
  const cut = raw.search(/[?#]/);
  if (cut >= 0) raw = raw.slice(0, cut);
  if (raw === "" || raw === "/") return "/";
  const trimmed = raw.replace(/\/+$/, "");
  return trimmed === "" ? "/" : trimmed;
}

/** Maps a location hash to a route id; unknown paths fall back to `notFound`. */
export function parseRoute(hash: string): RouteMatch {
  const path = normalizePath(hash);
  // The empty/root path is the app entry: the welcome screen (Phase 20).
  if (path === "/" || path === "/welcome") return { id: "welcome", path };
  if (path === "/dashboard") return { id: "home", path };
  if (path === "/billing") return { id: "licensing", path };
  if (path === "/intelligence") return { id: "code", path };
  if (path === "/preferences") return { id: "settings", path };
  if (path === "/model-setup" || path === "/engine") return { id: "models", path };
  for (const route of ROUTES) {
    if (route.path === path) return { id: route.id, path };
  }
  return { id: "notFound", path };
}

/** The hash href for a route — usable as a plain anchor `href`. */
export function routeHref(id: RouteId): string {
  const route = ROUTES.find((candidate) => candidate.id === id);
  return `#${route ? route.path : "/"}`;
}

function subscribe(onStoreChange: () => void): () => void {
  window.addEventListener("hashchange", onStoreChange);
  window.addEventListener("popstate", onStoreChange);
  return () => {
    window.removeEventListener("hashchange", onStoreChange);
    window.removeEventListener("popstate", onStoreChange);
  };
}

function getHashSnapshot(): string {
  return window.location.hash;
}

/** The current route, kept in sync with history (including back/forward). */
export function useHashRoute(): RouteMatch {
  const hash = useSyncExternalStore(subscribe, getHashSnapshot, () => "");
  return parseRoute(hash);
}

/**
 * Navigates to a route without reloading the page.
 *
 * The synchronous `hashchange` dispatch keeps subscribers in step with the
 * click that caused the navigation (embedded webviews and jsdom deliver the
 * native event asynchronously). Browsers simply ignore the duplicate.
 */
export function navigateTo(id: RouteId): void {
  const href = routeHref(id);
  const current = window.location.hash;
  // Already here (including the `#/` ↔ `#/welcome` alias)? Do not stack history.
  if (parseRoute(current).id === id) return;
  window.location.hash = href;
  window.dispatchEvent(new Event("hashchange"));
}

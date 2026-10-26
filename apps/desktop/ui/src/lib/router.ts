import { useSyncExternalStore } from "react";

/**
 * Dependency-free hash router for the Nexora desktop workspace.
 *
 * Nexora launches directly into the main AI Chat view (`""` / `"/"` -> `"chat"`).
 * There is no welcome splash screen or separate authentication window; unauthenticated
 * chat attempts open a centred modal directly over the active chat workspace.
 */

export const ROUTES = [
  { id: "chat", path: "/chat", label: "AI Chat" },
  { id: "home", path: "/home", label: "Overview" },
  { id: "projects", path: "/projects", label: "Projects" },
  { id: "code", path: "/code", label: "Code Intelligence" },
  { id: "models", path: "/models", label: "Local Models" },
  { id: "integrations", path: "/integrations", label: "App Integrations" },
  { id: "plans", path: "/plans", label: "Plans & Premium" },
  { id: "account", path: "/account", label: "Account" },
  { id: "profile", path: "/profile", label: "Profile" },
  { id: "settings", path: "/settings", label: "Settings" },
  { id: "founder", path: "/founder", label: "Diagnostics" },
  { id: "licensing", path: "/licensing", label: "Offline Licence" },
  // Legacy flow route aliases kept only so deep-links resolve predictably
  { id: "welcome", path: "/welcome", label: "Welcome" },
  { id: "onboarding", path: "/onboarding", label: "Onboarding" },
  { id: "signin", path: "/signin", label: "Sign In" },
  { id: "signup", path: "/signup", label: "Create account" },
  { id: "verify", path: "/verify-email", label: "Email verification" },
  { id: "recover", path: "/recover-password", label: "Password recovery" },
  { id: "reset", path: "/reset-password", label: "Password reset" },
] as const;

export type RouteId = (typeof ROUTES)[number]["id"];

export const FLOW_ROUTES: ReadonlyArray<AnyRouteId> = [
  "welcome",
  "onboarding",
  "signin",
  "signup",
  "verify",
  "recover",
  "reset",
];

/**
 * Flow routes no longer replace the main desktop workspace shell; legacy deep
 * links open directly inside the workspace with the floating auth modal active.
 */
export function isFlowRoute(_id: AnyRouteId): boolean {
  return false;
}

export type AnyRouteId = RouteId | "notFound";

export interface RouteMatch {
  id: AnyRouteId;
  path: string;
}

function normalizePath(hash: string): string {
  let raw = hash.startsWith("#") ? hash.slice(1) : hash;
  const cut = raw.search(/[?#]/);
  if (cut >= 0) raw = raw.slice(0, cut);
  if (raw === "" || raw === "/") return "/";
  const trimmed = raw.replace(/\/+$/, "");
  return trimmed === "" ? "/" : trimmed;
}

/** Maps a location hash to a route id; root opens directly into AI Chat (`"chat"`). */
export function parseRoute(hash: string): RouteMatch {
  const path = normalizePath(hash);
  if (path === "/" || path === "/search" || path === "/search-chats") return { id: "chat", path };
  if (path === "/dashboard") return { id: "home", path };
  if (path === "/billing" || path === "/pricing" || path === "/upgrade") {
    return { id: "plans", path };
  }
  if (path === "/membership" || path === "/memberships" || path === "/premium") return { id: "plans", path };
  if (path === "/u/me") return { id: "profile", path };
  if (path === "/intelligence") return { id: "code", path };
  if (path === "/preferences") return { id: "settings", path };
  if (path === "/model-setup" || path === "/engine") return { id: "models", path };
  for (const route of ROUTES) {
    if (route.path === path) return { id: route.id, path };
  }
  return { id: "notFound", path };
}

export function routeHref(id: RouteId): string {
  const route = ROUTES.find((candidate) => candidate.id === id);
  return `#${route ? route.path : "/chat"}`;
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

export function useHashRoute(): RouteMatch {
  const hash = useSyncExternalStore(subscribe, getHashSnapshot, () => "");
  return parseRoute(hash);
}

export function navigateTo(id: RouteId): void {
  const href = routeHref(id);
  const current = window.location.hash;
  if (parseRoute(current).id === id) return;
  window.location.hash = href;
  window.dispatchEvent(new Event("hashchange"));
}

/** Replace a flow URL without retaining a one-time token in browser history. */
export function replaceRouteTo(id: RouteId): void {
  const href = routeHref(id);
  if (window.location.hash === href) return;
  window.history.replaceState(window.history.state, "", href);
  window.dispatchEvent(new Event("hashchange"));
}

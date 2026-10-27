/**
 * Developer Portal — session state for the React tree.
 *
 * The browser is told whether a session exists; it never decides it. This
 * provider holds the server's answer in memory, re-asks on mount (so a refresh
 * cannot inherit a stale client assumption), and clears everything on sign-out.
 *
 * Nothing is persisted: no `localStorage`, no `sessionStorage`, no readable
 * cookie. The only durable artefact is the `HttpOnly` session cookie, which
 * JavaScript cannot read and which the server can revoke.
 */

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";

import { PortalApiError, portalApi, setCsrfToken, toFailure, type PortalUser } from "./portal-api";

export type SessionState =
  /** First load: asking the server. */
  | { status: "loading" }
  /** No live session. `ownerProvisioned` explains whether sign-in is even possible. */
  | { status: "signed-out"; ownerProvisioned: boolean | null }
  /** A verified session. */
  | { status: "signed-in"; user: PortalUser; expiresAt: string }
  /** The server could not be reached, or is not configured. */
  | { status: "unavailable"; message: string };

interface PortalSessionContextValue {
  state: SessionState;
  /** Re-asks the server. Used after a `401` from any panel. */
  refresh: () => Promise<void>;
  signIn: (email: string, password: string) => Promise<void>;
  signOut: () => Promise<void>;
}

const PortalSessionContext = createContext<PortalSessionContextValue | null>(null);

export function PortalSessionProvider({ children }: { children: ReactNode }) {
  const [state, setState] = useState<SessionState>({ status: "loading" });
  const mounted = useRef(true);

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);

  const refresh = useCallback(async () => {
    // Keep an established session on screen while re-validating; otherwise show
    // the loading state so the form never flashes for a signed-in user.
    setState((current) => (current.status === "signed-in" ? current : { status: "loading" }));
    try {
      const session = await portalApi.getSession();
      if (!mounted.current) return;
      if (session !== null) {
        setState({ status: "signed-in", user: session.user, expiresAt: session.expiresAt });
        return;
      }

      // Signed out. The public status endpoint says whether an owner account
      // exists at all — that is what lets the page say "not set up yet" instead
      // of inviting credentials that cannot possibly work.
      let ownerProvisioned: boolean | null = null;
      try {
        ownerProvisioned = (await portalApi.status()).ownerProvisioned;
      } catch {
        ownerProvisioned = null;
      }
      if (!mounted.current) return;
      setState({ status: "signed-out", ownerProvisioned });
    } catch (error) {
      if (!mounted.current) return;
      setState({ status: "unavailable", message: toFailure(error).message });
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const signIn = useCallback(async (email: string, password: string) => {
    const session = await portalApi.signIn(email, password);
    setState({ status: "signed-in", user: session.user, expiresAt: session.expiresAt });
  }, []);

  const signOut = useCallback(async () => {
    try {
      await portalApi.signOut();
    } catch (error) {
      // A failed sign-out must not leave the UI claiming a session the server
      // may still consider live. `401` means it was already gone.
      if (!(error instanceof PortalApiError) || error.status !== 401) throw error;
    } finally {
      setCsrfToken(null);
    }
    setState({ status: "signed-out", ownerProvisioned: true });
  }, []);

  const value = useMemo<PortalSessionContextValue>(
    () => ({ state, refresh, signIn, signOut }),
    [state, refresh, signIn, signOut],
  );

  return <PortalSessionContext.Provider value={value}>{children}</PortalSessionContext.Provider>;
}

export function usePortalSession(): PortalSessionContextValue {
  const context = useContext(PortalSessionContext);
  if (context === null) {
    throw new Error("usePortalSession must be used inside <PortalSessionProvider>");
  }
  return context;
}

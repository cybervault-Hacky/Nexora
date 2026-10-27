/**
 * Developer Portal — small shared building blocks.
 *
 * Everything the dashboard renders is built from these, so the five panels do
 * not each invent their own card, definition list, or empty state.
 */

import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";

import { PortalApiError, toFailure, type PortalRequestFailure } from "./portal-api";
import { usePortalSession } from "./PortalSession";

import "./PortalDashboard.css";

/* ───────────────────────────────── panel ─────────────────────────────── */

export interface PortalPanelProps {
  title: string;
  description?: ReactNode;
  children: ReactNode;
}

/**
 * A labelled region of the dashboard.
 *
 * The visible heading is the section `h1` in the shell, so repeating it here
 * would announce the same words twice and produce a page whose only `h2` is a
 * duplicate of its `h1`. The region therefore carries its name as an
 * `aria-label`, and the panel's own sub-sections use `h2`.
 */
export function PortalPanel({ title, description, children }: PortalPanelProps) {
  return (
    <section className="portal-panel" aria-label={title}>
      {description ? <p className="portal-panel__description">{description}</p> : null}
      {children}
    </section>
  );
}

/* ───────────────────────────────── facts ─────────────────────────────── */

export interface PortalFact {
  term: string;
  detail: ReactNode;
}

/** Term/definition list. No badges, no colour-only status. */
export function PortalFacts({ facts }: { facts: readonly PortalFact[] }) {
  return (
    <dl className="portal-facts">
      {facts.map((fact) => (
        <div className="portal-facts__row" key={fact.term}>
          <dt className="portal-facts__term">{fact.term}</dt>
          <dd className="portal-facts__detail">{fact.detail}</dd>
        </div>
      ))}
    </dl>
  );
}

/* ─────────────────────────────── states ──────────────────────────────── */

export type PortalStateTone = "loading" | "empty" | "error" | "unconfigured";

export interface PortalStateProps {
  tone: PortalStateTone;
  title: string;
  children?: ReactNode;
  /** Optional retry affordance for `error`. */
  onRetry?: () => void;
}

/**
 * Loading, empty, error and unconfigured states.
 *
 * `loading` is announced politely rather than assertively: a panel that finishes
 * in a few hundred milliseconds should not interrupt a screen-reader user
 * mid-sentence. Errors are assertive because they need attention.
 */
export function PortalState({ tone, title, children, onRetry }: PortalStateProps) {
  const isError = tone === "error";
  return (
    <div
      className={`portal-state portal-state--${tone}`}
      role={isError ? "alert" : "status"}
      aria-live={isError ? "assertive" : "polite"}
    >
      <p className="portal-state__title">{title}</p>
      {children ? <div className="portal-state__body">{children}</div> : null}
      {onRetry ? (
        <button type="button" className="portal-state__retry" onClick={onRetry}>
          Try again
        </button>
      ) : null}
    </div>
  );
}

/* ───────────────────────────── data loading ──────────────────────────── */

export type ResourceState<T> =
  | { status: "loading" }
  | { status: "ready"; data: T }
  | { status: "failed"; failure: PortalRequestFailure };

/**
 * Loads one protected resource.
 *
 * A `401`/`403` is not rendered as a panel error: it means the session ended, so
 * the provider is asked to re-check and the shell swaps back to the sign-in page.
 * That keeps "expired session" and "the server is down" as distinct experiences,
 * which they are.
 */
export function usePortalResource<T>(loader: () => Promise<T>): {
  state: ResourceState<T>;
  reload: () => void;
} {
  const { refresh } = usePortalSession();
  const [state, setState] = useState<ResourceState<T>>({ status: "loading" });
  const [attempt, setAttempt] = useState(0);
  const loaderRef = useRef(loader);
  loaderRef.current = loader;
  const active = useRef(true);

  useEffect(() => {
    active.current = true;
    return () => {
      active.current = false;
    };
  }, []);

  useEffect(() => {
    let cancelled = false;
    setState({ status: "loading" });
    void (async () => {
      try {
        const data = await loaderRef.current();
        if (cancelled || !active.current) return;
        setState({ status: "ready", data });
      } catch (error) {
        if (cancelled || !active.current) return;
        if (error instanceof PortalApiError && (error.status === 401 || error.status === 403)) {
          void refresh();
          return;
        }
        setState({ status: "failed", failure: toFailure(error) });
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [attempt, refresh]);

  const reload = useCallback(() => setAttempt((current) => current + 1), []);

  return { state, reload };
}

/* ─────────────────────────────── helpers ─────────────────────────────── */

const DATE_TIME = new Intl.DateTimeFormat(undefined, {
  dateStyle: "medium",
  timeStyle: "short",
});

/** Formats an ISO timestamp for display, or returns a plain fallback. */
export function formatDateTime(iso: string | null | undefined): string {
  if (typeof iso !== "string" || iso === "") return "Not recorded";
  const parsed = Date.parse(iso);
  if (!Number.isFinite(parsed)) return "Not recorded";
  return DATE_TIME.format(new Date(parsed));
}

/** "Yes" / "No" with the reason attached — never a bare colour. */
export function yesNo(value: boolean, yes = "Yes", no = "No"): string {
  return value ? yes : no;
}

export function formatBytes(bytes: number | null): string {
  if (bytes === null || !Number.isFinite(bytes)) return "Unknown";
  if (bytes < 1024) return `${bytes} B`;
  return `${(bytes / 1024).toFixed(1)} kB`;
}

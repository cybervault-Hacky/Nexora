/**
 * Developer Portal — API client.
 *
 * Every call is same-origin and cookie-based. Nothing is stored in
 * `localStorage`, `sessionStorage`, or a readable cookie: the session lives in an
 * `HttpOnly` cookie the browser attaches automatically, and the CSRF token lives
 * in memory for the lifetime of the tab.
 *
 * Two behaviours worth stating outright:
 *
 * - A network failure or a `503` is surfaced as `unavailable`, never as a
 *   successful sign-in. The UI has no "demo mode".
 * - `getSession` returning `401` is a normal state, not an error, so a signed-out
 *   visitor sees the sign-in form rather than an error banner.
 */

export type PortalRole = "owner" | "developer";

export interface PortalUser {
  id: string;
  email: string;
  displayName: string;
  role: PortalRole;
  lastSignInAt: string | null;
  createdAt: string;
}

export interface PortalStatus {
  service: string;
  available: boolean;
  ownerProvisioned: boolean;
  environment: string;
}

export interface PortalSession {
  user: PortalUser;
  csrfToken: string;
  expiresAt: string;
}

export interface PortalOverview {
  portal: {
    service: string;
    environment: string;
    sessionExpiresAt: string;
    signedInAt: string;
    recentEvents: { at: string; event: string; userId: string | null }[];
  };
  account: PortalUser & { roleLabel: string };
  readiness: {
    ownerProvisioned: boolean;
    portalOriginConfigured: boolean;
    secureCookies: boolean;
    staticBuildPresent: boolean;
    staticBuildBytes: number | null;
    analyticsConfigured: boolean;
    contentManagementConfigured: boolean;
    supportInboxConfigured: boolean;
  };
}

export interface PortalWebsiteStatus {
  build: { present: boolean; bytes: number | null; builtAt: string | null };
  indexing: { robotsPresent: boolean; sitemapEmitted: boolean; productionOriginConfigured: boolean };
  release: { installerHosted: boolean };
  deployment: { verified: boolean; note: string };
}

export interface PortalUnconfigured {
  configured: false;
  reason: string;
}

export interface PortalContentStatus extends PortalUnconfigured {
  editableSurfaces: string[];
}

export interface PortalSupportStatus extends PortalUnconfigured {
  openRequests: null;
}

/** A failure with a stable code the UI can branch on, plus a safe message. */
export class PortalApiError extends Error {
  readonly code: string;
  readonly status: number;
  readonly retryAfterSeconds: number | null;

  constructor(message: string, code: string, status: number, retryAfterSeconds: number | null = null) {
    super(message);
    this.name = "PortalApiError";
    this.code = code;
    this.status = status;
    this.retryAfterSeconds = retryAfterSeconds;
  }
}

export type PortalRequestFailure =
  | { kind: "unavailable"; message: string }
  | { kind: "denied"; message: string; code: string }
  | { kind: "rate-limited"; message: string; retryAfterSeconds: number }
  | { kind: "invalid"; message: string; code: string };

/**
 * The CSRF token for the current session, kept in memory only. It is cleared on
 * sign-out and whenever the server says the session is gone.
 */
let csrfToken: string | null = null;

export function setCsrfToken(token: string | null): void {
  csrfToken = token;
}

async function readJson(response: Response): Promise<unknown> {
  try {
    return await response.json();
  } catch {
    return null;
  }
}

function errorFrom(response: Response, body: unknown): PortalApiError {
  const detail = (body as { error?: { code?: string; message?: string } } | null)?.error;
  const message = typeof detail?.message === "string" ? detail.message : "The portal could not complete the request.";
  const code = typeof detail?.code === "string" ? detail.code : "request_failed";
  const retryAfter = Number.parseInt(response.headers.get("retry-after") ?? "", 10);
  return new PortalApiError(message, code, response.status, Number.isFinite(retryAfter) ? retryAfter : null);
}

interface RequestOptions {
  method?: "GET" | "POST" | "DELETE";
  body?: unknown;
  /** Send the CSRF header (all authenticated calls do). */
  authenticated?: boolean;
}

async function request(path: string, options: RequestOptions = {}): Promise<unknown> {
  const method = options.method ?? "GET";
  const headers: Record<string, string> = { Accept: "application/json" };
  if (options.body !== undefined) headers["Content-Type"] = "application/json";
  if (options.authenticated && csrfToken !== null) headers["X-Portal-CSRF"] = csrfToken;

  let response: Response;
  try {
    response = await fetch(path, {
      method,
      headers,
      // Same-origin cookies only; no credentials are ever read by script.
      credentials: "same-origin",
      // Our own requests must never be served from a cache.
      cache: "no-store",
      body: options.body === undefined ? undefined : JSON.stringify(options.body),
    });
  } catch {
    // A refused connection, DNS failure, or CORS rejection lands here. This is
    // exactly the case where a fake "success" would be dangerous.
    throw new PortalApiError(
      "The portal server could not be reached. Nothing was changed.",
      "unavailable",
      0,
    );
  }

  const body = await readJson(response);
  if (!response.ok) throw errorFrom(response, body);
  return body;
}

export const portalApi = {
  async status(): Promise<PortalStatus> {
    return (await request("/api/portal/status")) as PortalStatus;
  },

  /** `null` means "no live session" — a normal state, not an error. */
  async getSession(): Promise<PortalSession | null> {
    try {
      const body = (await request("/api/portal/session")) as
        | { authenticated?: boolean; user?: PortalUser; csrfToken?: string; expiresAt?: string }
        | null;
      if (!body?.authenticated || !body.user || !body.csrfToken || !body.expiresAt) {
        setCsrfToken(null);
        return null;
      }
      setCsrfToken(body.csrfToken);
      return { user: body.user, csrfToken: body.csrfToken, expiresAt: body.expiresAt };
    } catch (error) {
      if (error instanceof PortalApiError && error.status === 401) {
        setCsrfToken(null);
        return null;
      }
      throw error;
    }
  },

  async signIn(email: string, password: string): Promise<PortalSession> {
    const body = (await request("/api/portal/session", {
      method: "POST",
      body: { email, password },
    })) as { user?: PortalUser; csrfToken?: string; expiresAt?: string } | null;
    if (!body?.user || !body.csrfToken || !body.expiresAt) {
      throw new PortalApiError("The portal returned an unexpected response.", "unexpected_response", 500);
    }
    setCsrfToken(body.csrfToken);
    return { user: body.user, csrfToken: body.csrfToken, expiresAt: body.expiresAt };
  },

  async signOut(): Promise<void> {
    try {
      await request("/api/portal/session", { method: "DELETE", authenticated: true });
    } finally {
      setCsrfToken(null);
    }
  },

  async overview(): Promise<PortalOverview> {
    return (await request("/api/portal/overview", { authenticated: true })) as PortalOverview;
  },

  async website(): Promise<PortalWebsiteStatus> {
    return (await request("/api/portal/website", { authenticated: true })) as PortalWebsiteStatus;
  },

  async content(): Promise<PortalContentStatus> {
    return (await request("/api/portal/content", { authenticated: true })) as PortalContentStatus;
  },

  async support(): Promise<PortalSupportStatus> {
    return (await request("/api/portal/support", { authenticated: true })) as PortalSupportStatus;
  },
};

/** Maps a thrown error into the UI's failure vocabulary. */
export function toFailure(error: unknown): PortalRequestFailure {
  if (error instanceof PortalApiError) {
    if (error.code === "unavailable" || error.status === 0 || error.status >= 500) {
      return { kind: "unavailable", message: error.message };
    }
    if (error.code === "rate_limited" || error.status === 429) {
      return {
        kind: "rate-limited",
        message: "Too many attempts. Wait a moment and try again.",
        retryAfterSeconds: error.retryAfterSeconds ?? 60,
      };
    }
    if (error.status === 401 || error.status === 403) {
      return { kind: "denied", message: error.message, code: error.code };
    }
    return { kind: "invalid", message: error.message, code: error.code };
  }
  return { kind: "unavailable", message: "The portal could not be reached. Nothing was changed." };
}

/**
 * SEO configuration resolution — pure, environment-agnostic.
 *
 * Two callers share this module so they can never disagree:
 *
 * - `src/config/site.ts`    → the browser bundle (`import.meta.env`);
 * - `vite/seo-plugin.ts`    → the build step (`loadEnv`, i.e. `.env` files).
 *
 * Nothing here touches the DOM, `import.meta.env`, `process`, or the network,
 * which is what makes it safe to import from both a Vite config file and the
 * client bundle.
 *
 * ## Why the production origin is configuration, not a constant
 *
 * No production domain is recorded anywhere in this repository yet. Rather than
 * invent one, the origin is read from `VITE_SITE_URL`. If it is absent or not a
 * usable production origin, the site emits **no** canonical URL, **no**
 * `og:url`, **no** sitemap, and a conservative `noindex` policy — so a preview
 * or development build can never publish an absolute URL pointing at
 * `localhost`, a temporary preview host, or a domain nobody confirmed.
 *
 * Indexing is a second, explicit switch (`VITE_SITE_INDEXABLE`) and is only
 * honoured when the origin is valid. Both are public, non-secret values.
 */

/** The environment keys this module understands. */
export interface SeoEnv {
  readonly VITE_SITE_URL?: string | undefined;
  readonly VITE_SITE_INDEXABLE?: string | undefined;
}

/** The resolved policy used by both the page tags and the build output. */
export interface SeoConfig {
  /** Confirmed production origin, without a trailing slash; `null` if unset. */
  readonly url: string | null;
  /** Whether this build may be indexed by search engines. */
  readonly indexable: boolean;
  /** Human-readable notes for the build log. Never contains secrets. */
  readonly warnings: readonly string[];
}

/**
 * Hosts that are never a production origin. These are development, preview, or
 * machine-local names, so an accidental `VITE_SITE_URL` pointing at one would
 * leak into canonical URLs and the sitemap.
 */
const NON_PRODUCTION_SUFFIXES = [".e2b.app", ".local", ".localhost", ".internal"] as const;

const NON_PRODUCTION_HOSTNAMES = new Set([
  "localhost",
  "127.0.0.1",
  "0.0.0.0",
  "[::1]",
  "::1",
]);

export interface ParsedSiteUrl {
  readonly url: string | null;
  /** Why the value was rejected, or `null` when it was accepted. */
  readonly problem: string | null;
}

/**
 * Validates one `VITE_SITE_URL` value.
 *
 * Accepts only a bare `https://host` origin: no credentials, no path beyond
 * `/`, no query, no fragment. A path would silently change every canonical URL
 * (and break a root-level `sitemap.xml`), so it is rejected rather than
 * half-supported.
 */
export function normalizeSiteUrl(raw: string | undefined): ParsedSiteUrl {
  const value = raw?.trim() ?? "";
  if (value === "") return { url: null, problem: null };

  let parsed: URL;
  try {
    parsed = new URL(value);
  } catch {
    return { url: null, problem: "VITE_SITE_URL is not a valid absolute URL." };
  }

  if (parsed.protocol !== "https:") {
    return { url: null, problem: "VITE_SITE_URL must use https://." };
  }
  if (parsed.username !== "" || parsed.password !== "") {
    return { url: null, problem: "VITE_SITE_URL must not contain credentials." };
  }
  if (parsed.search !== "" || parsed.hash !== "") {
    return { url: null, problem: "VITE_SITE_URL must not contain a query or fragment." };
  }
  if (parsed.pathname !== "/") {
    return {
      url: null,
      problem:
        "VITE_SITE_URL must be a bare origin (no sub-path); the site is served from the domain root.",
    };
  }

  const hostname = parsed.hostname.toLowerCase();
  if (NON_PRODUCTION_HOSTNAMES.has(hostname) || hostname.startsWith("[")) {
    return { url: null, problem: "VITE_SITE_URL points at a loopback host, not a production domain." };
  }
  if (/^\d+\.\d+\.\d+\.\d+$/.test(hostname)) {
    return { url: null, problem: "VITE_SITE_URL is an IP address, not a production domain." };
  }
  for (const suffix of NON_PRODUCTION_SUFFIXES) {
    if (hostname.endsWith(suffix)) {
      return {
        url: null,
        problem: `VITE_SITE_URL points at "${suffix}", which is a development or preview host.`,
      };
    }
  }

  // `parsed.origin` drops a trailing slash and any default port, so callers can
  // concatenate a route path safely.
  return { url: parsed.origin, problem: null };
}

/** `true` for the usual affirmative spellings; anything else is `false`. */
export function isEnabledFlag(raw: string | undefined): boolean {
  const value = raw?.trim().toLowerCase() ?? "";
  return value === "true" || value === "1" || value === "yes" || value === "on";
}

/**
 * Resolves the full policy for one build.
 *
 * The default is deliberately the closed one: no origin, indexing off. Enabling
 * indexing requires **both** an explicit opt-in and a valid origin, so a half
 * configured deployment stays unindexed rather than publishing a sitemap full
 * of URLs it cannot vouch for.
 */
export function resolveSeoConfig(env: SeoEnv): SeoConfig {
  const warnings: string[] = [];
  const parsed = normalizeSiteUrl(env.VITE_SITE_URL);
  if (parsed.problem) warnings.push(`${parsed.problem} Canonical URLs and the sitemap are disabled.`);

  const requested = isEnabledFlag(env.VITE_SITE_INDEXABLE);
  const indexable = requested && parsed.url !== null;

  if (requested && parsed.url === null) {
    warnings.push(
      "VITE_SITE_INDEXABLE is enabled but no valid VITE_SITE_URL was supplied. " +
        "The build stays noindex: indexing needs a confirmed production origin.",
    );
  }

  return { url: parsed.url, indexable, warnings };
}

/** Absolute URL for a route path, or `null` when no origin is configured. */
export function absoluteUrl(url: string | null, path: string): string | null {
  if (url === null) return null;
  return path === "/" ? `${url}/` : `${url}${path}`;
}

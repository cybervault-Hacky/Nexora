/**
 * Page metadata registry — one entry per route.
 *
 * Titles and descriptions live here rather than inside each page component so
 * that they can be reviewed together (an SEO audit should not require opening
 * ten files), and so a missing route is a **type error** rather than a page
 * with the browser's default title. `PAGE_METADATA` is keyed by `RoutePath`,
 * which comes from `config/routes.ts`, so adding a route without metadata fails
 * `pnpm typecheck`.
 *
 * Writing rules, enforced by review rather than by code:
 *
 * - describe Nexora as a Windows desktop AI coding assistant built around local
 *   models — that is what the product is;
 * - one sentence, under ~160 characters, so a search result does not truncate
 *   it mid-clause;
 * - no keyword lists, no superlatives, no capability the site does not claim
 *   elsewhere, and no "download now" on a release that does not exist;
 * - `indexable: false` only where a route must never appear in an index.
 */

import { ROUTES, type RoutePath } from "./routes";

export interface PageMetadata {
  /** Full document title, including the brand. */
  readonly title: string;
  /** Meta description and the Open Graph description. */
  readonly description: string;
  /** Whether this route may be indexed when the site is configured to be. */
  readonly indexable: boolean;
  /**
   * A route that is not part of the public site at all (the developer portal).
   * Always `noindex, nofollow`, even in an indexable build, and never in a
   * sitemap.
   */
  readonly private?: boolean;
}

export const PAGE_METADATA: Record<RoutePath, PageMetadata> = {
  [ROUTES.home]: {
    title: "Nexora — AI Coding Assistant",
    description:
      "Nexora is a Windows desktop AI coding assistant built around local models, designed for a more private, locally run coding workflow. In active development.",
    indexable: true,
  },
  [ROUTES.features]: {
    title: "Features — Nexora",
    description:
      "What exists in the current Nexora implementation: a local-model coding workflow with workspace controls, described with an evidence label on every claim.",
    indexable: true,
  },
  [ROUTES.howItWorks]: {
    title: "How It Works — Nexora",
    description:
      "The intended Nexora workflow, from preparing a local model to a coding session in the desktop client, including what has not been verified yet.",
    indexable: true,
  },
  [ROUTES.download]: {
    title: "Windows Download — Nexora",
    description:
      "The Windows release of Nexora is being prepared. No installer is available yet; this page explains what will be published and what has to be ready first.",
    indexable: true,
  },
  [ROUTES.privacy]: {
    title: "Privacy — Nexora",
    description:
      "An informational overview, not a legal policy: what the Nexora source code does with your code and conversations, what it does not do, and what is unverified.",
    indexable: true,
  },
  [ROUTES.security]: {
    title: "Security — Nexora",
    description:
      "What the Nexora source code enforces to protect your machine, what is only designed, and what nobody has tested. Not an audit, not a guarantee.",
    indexable: true,
  },
  [ROUTES.about]: {
    title: "About — Nexora",
    description:
      "About the Nexora project: a Windows desktop AI coding assistant designed around local models, how the pieces fit together, and what has been verified.",
    indexable: true,
  },
  [ROUTES.faq]: {
    title: "Frequently Asked Questions — Nexora",
    description:
      "Answers about Nexora, checked against the project source: local models, privacy, security, downloads and support, including what is unknown or not built.",
    indexable: true,
  },
  [ROUTES.support]: {
    title: "Support — Nexora",
    description:
      "What you can check yourself when Nexora misbehaves, what to record if something goes wrong, and why there is no public support channel yet.",
    indexable: true,
  },
  [ROUTES.admin]: {
    title: "Developer Portal — Nexora",
    description:
      "Private sign-in for authorised Nexora developers. Not a public page, not part of the website's navigation, and never indexed.",
    indexable: false,
    private: true,
  },
};

/**
 * The fallback route.
 *
 * Never indexable: a catch-all can be reached by any typo or stale inbound
 * link, and none of those URLs are content. It is deliberately absent from
 * `PAGE_METADATA` because it has no path in `ROUTES` — the router's `*` route
 * owns it.
 */
export const NOT_FOUND_METADATA: PageMetadata = {
  title: "Page not found — Nexora",
  description:
    "The address you requested is not part of the Nexora website. It may have been mistyped, or the page may not have been built yet.",
  indexable: false,
};

/**
 * Routes that belong in a sitemap, in navigation order with home first.
 *
 * A sitemap lists destinations a crawler should treat as content, so the list
 * is derived from the registry's own `indexable` flag: a route marked
 * `indexable: false` cannot leak into it.
 */
export const SITEMAP_PATHS: readonly RoutePath[] = [
  ROUTES.home,
  ...(Object.keys(PAGE_METADATA) as RoutePath[]).filter(
    (path) => path !== ROUTES.home && PAGE_METADATA[path].indexable,
  ),
];

/**
 * Route and navigation configuration — single source of truth.
 *
 * Every approved destination is its own page with a stable URL. Paths are
 * declared here once and imported by the router, the header, and the footer,
 * so a URL can never drift between them.
 *
 * The site uses the History API (react-router `BrowserRouter`), so the
 * development server and any future static host must serve `index.html` for
 * unknown paths. See README → "Routing" for the hosting requirement.
 */

/** Canonical paths. Change a URL here and it changes everywhere. */
export const ROUTES = {
  home: "/",
  features: "/features",
  howItWorks: "/how-it-works",
  download: "/download",
  privacy: "/privacy",
  security: "/security",
  about: "/about",
  faq: "/faq",
  support: "/support",
  /**
   * Private developer portal. Deliberately **not** in `NAV_ITEMS`: it is not a
   * destination for the public, and nothing links to it from the site chrome.
   * It is `indexable: false` in `config/metadata.ts`, so it can never enter
   * `SITEMAP_PATHS` either.
   */
  admin: "/admin",
} as const;

export type RoutePath = (typeof ROUTES)[keyof typeof ROUTES];

export interface NavItem {
  /** Canonical path this item links to. */
  path: RoutePath;
  /** Label shown in the header and footer. */
  label: string;
}

/**
 * Primary navigation, in the approved order.
 *
 * Home is intentionally absent: the brand mark links there, and repeating it
 * in the link list would duplicate that destination. Every other required page
 * appears here, so all destinations remain discoverable.
 */
export const NAV_ITEMS: readonly NavItem[] = [
  {
    path: ROUTES.features,
    label: "Features",
  },
  {
    path: ROUTES.howItWorks,
    label: "How It Works",
  },
  {
    path: ROUTES.download,
    label: "Download",
  },
  {
    path: ROUTES.privacy,
    label: "Privacy",
  },
  {
    path: ROUTES.security,
    label: "Security",
  },
  {
    path: ROUTES.about,
    label: "About",
  },
  {
    path: ROUTES.faq,
    label: "FAQ",
  },
  {
    path: ROUTES.support,
    label: "Support",
  },
];

/*
 * Document titles and meta descriptions are not here: they live in
 * `config/metadata.ts`, keyed by these same paths, so one file holds every
 * page's search and social metadata.
 */

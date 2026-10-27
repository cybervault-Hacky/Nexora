/**
 * Applies the current route's metadata to the document head.
 *
 * The site is a client-rendered single-page app, so the head has to be managed
 * per navigation: `index.html` can only describe one page. This hook is the
 * single place that writes it, which is what keeps the tags from drifting out
 * of sync with React Router.
 *
 * Every write is an **upsert keyed on the tag's own identity** (`meta[name]`,
 * `meta[property]`), never an append. Two consequences:
 *
 * - navigating Features → About updates the existing tags instead of stacking a
 *   second `<meta name="description">` (the classic SPA duplicate-tag bug);
 * - React StrictMode's double-invoked effect is harmless, because applying the
 *   same values twice produces the same document.
 *
 * What is deliberately *not* emitted:
 *
 * - `<link rel="canonical">` when no confirmed production origin is configured,
 *   and never on the not-found route (a 404 must not canonicalise to a product
 *   page);
 * - `og:url` for the same reason;
 * - `og:image` / `twitter:image` — the repository has no social preview asset,
 *   and a screenshot of the site is not one. The card type stays `summary`.
 */

import { useEffect } from "react";

import { NOT_FOUND_METADATA, PAGE_METADATA, type PageMetadata } from "../config/metadata";
import type { RoutePath } from "../config/routes";
import { SITE_INDEXABLE, SITE_NAME, SITE_URL } from "../config/site";
import { absoluteUrl } from "../config/site-config";

function upsertMeta(attribute: "name" | "property", key: string, content: string): void {
  let element = document.head.querySelector<HTMLMetaElement>(`meta[${attribute}="${key}"]`);
  if (!element) {
    element = document.createElement("meta");
    element.setAttribute(attribute, key);
    document.head.append(element);
  }
  if (element.getAttribute("content") !== content) element.setAttribute("content", content);
}

function removeMeta(attribute: "name" | "property", key: string): void {
  document.head.querySelector<HTMLMetaElement>(`meta[${attribute}="${key}"]`)?.remove();
}

function upsertCanonical(href: string | null): void {
  const existing = document.head.querySelector<HTMLLinkElement>('link[rel="canonical"]');
  if (href === null) {
    existing?.remove();
    return;
  }
  if (!existing) {
    const link = document.createElement("link");
    link.rel = "canonical";
    link.href = href;
    document.head.append(link);
    return;
  }
  if (existing.href !== href) existing.href = href;
}

/**
 * The indexing policy for one page.
 *
 * The build-level switch wins: an unconfigured or preview build is `noindex`
 * on every route, including the not-found page. Once indexing is enabled, only
 * a route marked `indexable` is allowed in — the catch-all stays `noindex` while
 * keeping `follow`, so its outbound links are still crawled.
 */
function robotsDirective(meta: PageMetadata): string {
  // A private route (the developer portal) is never followed: it is not content,
  // and its links lead only to more private pages.
  if (meta.private === true) return "noindex, nofollow";
  if (!SITE_INDEXABLE) return "noindex, nofollow";
  return meta.indexable ? "index, follow" : "noindex, follow";
}

/** The route metadata for a path, or the not-found entry for `null`. */
function metadataFor(path: RoutePath | null): PageMetadata {
  if (path === null) return NOT_FOUND_METADATA;
  return PAGE_METADATA[path];
}

/**
 * @param path The route's canonical path, or `null` for the fallback route.
 */
export function usePageMetadata(path: RoutePath | null): void {
  useEffect(() => {
    const meta = metadataFor(path);
    const canonical = path === null ? null : absoluteUrl(SITE_URL, path);

    document.title = meta.title;

    upsertMeta("name", "description", meta.description);
    upsertMeta("name", "robots", robotsDirective(meta));

    upsertMeta("property", "og:title", meta.title);
    upsertMeta("property", "og:description", meta.description);
    upsertMeta("property", "og:type", "website");
    upsertMeta("property", "og:site_name", SITE_NAME);
    if (canonical === null) removeMeta("property", "og:url");
    else upsertMeta("property", "og:url", canonical);

    // `summary` is the correct card without an image; `summary_large_image`
    // would need an asset this repository does not have.
    upsertMeta("name", "twitter:card", "summary");
    upsertMeta("name", "twitter:title", meta.title);
    upsertMeta("name", "twitter:description", meta.description);

    upsertCanonical(canonical);

    return () => {
      // Nothing is torn down on unmount: the next route's effect overwrites the
      // same tags in place, and removing them first would briefly leave the
      // document with no description during the transition.
    };
  }, [path]);
}

/**
 * Runtime SEO configuration for the browser bundle.
 *
 * The resolution rules live in `site-config.ts` (shared with the build step) so
 * that the tags this module exposes and the `robots.txt` / `sitemap.xml` the
 * build emits are always derived from the same decision.
 *
 * Both values are public: they are baked into the client bundle at build time.
 */

import { resolveSeoConfig } from "./site-config";

export const SITE_NAME = "Nexora";

const resolved = resolveSeoConfig({
  VITE_SITE_URL: import.meta.env.VITE_SITE_URL,
  VITE_SITE_INDEXABLE: import.meta.env.VITE_SITE_INDEXABLE,
});

/** Confirmed production origin without a trailing slash, or `null`. */
export const SITE_URL: string | null = resolved.url;

/** Whether this build is allowed to be indexed. `false` unless configured. */
export const SITE_INDEXABLE: boolean = resolved.indexable;

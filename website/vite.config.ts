import { defineConfig, loadEnv } from "vite";
import react from "@vitejs/plugin-react";

import { seoPlugin } from "./vite/seo-plugin";

/**
 * Website-only Vite configuration.
 *
 * `host: "0.0.0.0"` and the preview-host allowlist support the container
 * preview used during development. This affects the development server only;
 * the production build emits static files and calls no network endpoints.
 *
 * The `/api` proxy forwards the developer portal's API to the Node process in
 * `server/` during development, so the browser only ever talks to one origin
 * (no CORS, no second host in the public site's configuration). The target is a
 * server-to-server address and is never exposed to the bundle; override it with
 * `NEXORA_PORTAL_PROXY_TARGET` if the portal runs elsewhere. When the portal is
 * not running, `/api/*` fails and the portal UI shows its unavailable state —
 * it never fabricates a session.
 *
 * Phase 14 adds `seoPlugin`, which reads the public `VITE_SITE_URL` /
 * `VITE_SITE_INDEXABLE` values (from `.env`, or the shell environment — shell
 * wins) and emits `robots.txt`, an optional `sitemap.xml`, and the indexing
 * directive in `index.html`. With neither variable set, which is the committed
 * default, the build is `noindex` and no absolute production URL is written
 * anywhere. See README → "Phase 14 — SEO and discoverability".
 */
export default defineConfig(({ mode }) => {
  // "." is the website directory: these scripts run from it, and it is the Vite
  // root. The third argument ("") loads every variable, not only
  // client-exposed ones, so `VITE_SITE_URL=… pnpm build` and a `.env` file
  // behave identically.
  const env = loadEnv(mode, ".", "");

  // Server-to-server only: this URL is used by the dev/preview proxy and is not
  // inlined into the client bundle.
  const portalTarget = env.NEXORA_PORTAL_PROXY_TARGET?.trim() || "http://127.0.0.1:4174";
  const proxy = {
    "/api": {
      target: portalTarget,
      // Keep the browser's `Host` so the portal's origin check sees the same
      // host the browser used; a rewritten Host would make every sign-in look
      // cross-origin.
      changeOrigin: false,
    },
  };

  return {
    plugins: [react(), seoPlugin(env)],
    server: {
      host: "0.0.0.0",
      port: 5173,
      allowedHosts: [".e2b.app", "localhost", "127.0.0.1", "[::1]"],
      proxy,
    },
    preview: {
      host: "0.0.0.0",
      port: 4173,
      allowedHosts: [".e2b.app", "localhost", "127.0.0.1", "[::1]"],
      proxy,
    },
    build: {
      outDir: "dist",
      emptyOutDir: true,
      sourcemap: false,
    },
  };
});

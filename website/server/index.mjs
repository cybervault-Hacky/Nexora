/**
 * Nexora Developer Portal — server entry point.
 *
 * One Node process serves the public site (`dist/`) and the private portal API
 * from the same origin. Run it with:
 *
 *     node server/index.mjs            # or: pnpm portal
 *
 * Configuration is environment-only (`server/config.mjs`). Nothing here reads a
 * `VITE_*` variable, so no server secret can reach the browser bundle.
 *
 * Request order:
 *   1. `/api/portal/*`            → JSON API (cookie session, no CORS)
 *   2. `/robots.txt`, `/sitemap.xml` → real file, or an honest 404 — never the
 *      SPA shell, matching the Phase 16 crawl-file behaviour
 *   3. existing file in `dist/`    → served
 *   4. extensionless path          → `index.html` (client routing)
 *   5. anything else               → 404
 */

import fs from "node:fs";
import http from "node:http";
import { fileURLToPath } from "node:url";
import path from "node:path";

import { handleApi } from "./api.mjs";
import { describeConfig, resolveConfig } from "./config.mjs";
import { envFilePermissionWarning, loadEnvFile } from "./env-file.mjs";
import { describeOwnerBootstrap, ensureOwnerFromEnvironment, readOwnerEnvironment } from "./owner-config.mjs";
import {
  BASE_HEADERS,
  NOINDEX_HEADERS,
  NO_STORE_HEADERS,
  productionSecurityHeaders,
  resolveStaticPath,
  serveFile,
  sendText,
} from "./http.mjs";
import { createLoginLimiter } from "./rate-limit.mjs";
import { StoreUnavailableError, openStore } from "./store.mjs";

const NO_SITEMAP_BODY = "No sitemap: no confirmed production domain is configured.\n";

/**
 * @param {ReturnType<typeof resolveConfig>} config
 * @param {{ store?: import("./store.mjs").Store | null, storeProblem?: string | null }} [deps]
 */
export async function createPortalServer(config, deps = {}) {
  let store = deps.store ?? null;
  let storeProblem = deps.storeProblem ?? null;

  if (store === null && storeProblem === null) {
    try {
      store = await openStore(config.dataDir);
    } catch (error) {
      storeProblem = error instanceof StoreUnavailableError ? error.message : "store: unknown failure";
      console.error(`[portal] storage unavailable: ${storeProblem}`);
    }
  }

  const context = {
    config,
    store,
    loginLimiter: createLoginLimiter(),
    storeProblem,
  };

  // Production-only headers, applied once per request so every response carries
  // them (static, API, error) without each handler repeating the rule.
  const deploymentHeaders = Object.entries(productionSecurityHeaders(config));

  const server = http.createServer((req, res) => {
    void (async () => {
      try {
        // `setHeader` values persist and are merged into the later `writeHead`
        // call, so one loop covers static files, API responses and errors.
        for (const [name, value] of deploymentHeaders) res.setHeader(name, value);

        const url = new URL(req.url ?? "/", "http://portal.local");
        const pathname = url.pathname;

        // Only the portal's own pages are noindex. The public site's indexing is
        // decided by the build (robots.txt + the page-level directive), and
        // Phase 17 removed a header that was overriding it on every response.
        if (pathname === "/admin" || pathname.startsWith("/admin/")) {
          res.setHeader("X-Robots-Tag", NOINDEX_HEADERS["X-Robots-Tag"]);
        }

        if (pathname === "/api" || pathname.startsWith("/api/")) {
          await handleApi(req, res, context);
          return;
        }

        if (req.method !== "GET" && req.method !== "HEAD") {
          res.writeHead(405, { ...BASE_HEADERS, Allow: "GET, HEAD", "Content-Length": 0 });
          res.end();
          return;
        }

        // Crawl files: served from the build when it emitted them. A missing
        // sitemap is a 404 with an explanation rather than the app shell.
        if (pathname === "/sitemap.xml") {
          const served = await serveFile(res, path.resolve(config.staticDir, "sitemap.xml"));
          if (!served) sendText(res, 404, NO_SITEMAP_BODY);
          return;
        }
        if (pathname === "/robots.txt") {
          const served = await serveFile(res, path.resolve(config.staticDir, "robots.txt"));
          if (!served) sendText(res, 404, "No robots.txt was emitted by this build.\n");
          return;
        }

        const filePath = resolveStaticPath(config.staticDir, pathname);
        if (filePath !== null && (await serveFile(res, filePath))) return;

        const looksLikeFile = path.posix.extname(pathname) !== "";
        if (looksLikeFile) {
          sendText(res, 404, "Not found.\n");
          return;
        }

        // Client-side routing: the SPA shell. `X-Robots-Tag` keeps an
        // authenticated-only path out of an index even on a host that ignores
        // the page-level directive.
        const shell = path.resolve(config.staticDir, "index.html");
        if (!(await serveFile(res, shell))) {
          sendText(res, 503, "This portal has no built website to serve. Run `pnpm build` first.\n");
        }
      } catch (error) {
        console.error("[portal] request failed:", error?.name ?? "Error");
        if (!res.headersSent) {
          res.writeHead(500, { ...BASE_HEADERS, ...NO_STORE_HEADERS, "Content-Type": "text/plain; charset=utf-8" });
        }
        res.end("Internal error.\n");
      }
    })();
  });

  return { server, context };
}

/**
 * Loads `website/.env` before anything reads the environment.
 *
 * Server-side only: this module is not reachable from `src/`, and none of the
 * variables it loads is a `VITE_*` name, so nothing here can be inlined into the
 * browser bundle. Existing process-environment values win, and only key *names*
 * are logged — never a value.
 */
function loadLocalEnvironment(cwd) {
  // `NEXORA_PORTAL_ENV_FILE` lets an operator keep the file outside the deploy
  // directory (and lets the tests use a hermetic file). Default: `website/.env`.
  const filePath = process.env.NEXORA_PORTAL_ENV_FILE?.trim() || path.resolve(cwd, ".env");
  const result = loadEnvFile({ env: process.env, filePath });

  if (result.problem !== null) {
    console.warn(`[portal] .env ${result.problem}; continuing with the process environment.`);
    return;
  }
  if (!result.loaded) return;

  console.log(`[portal] loaded ${result.appliedKeys.length} variable(s) from .env`);
  if (result.skippedKeys.length > 0) {
    // Names only: a value must never appear in a log, even for a variable the
    // operator set in both places.
    console.log(
      `[portal] ${result.skippedKeys.length} .env variable(s) ignored because the process environment already sets them: ` +
        result.skippedKeys.join(", "),
    );
  }

  const permissionWarning = envFilePermissionWarning(filePath, fs);
  if (permissionWarning !== null) console.warn(`[portal] ${permissionWarning}`);
}

async function main() {
  loadLocalEnvironment(process.cwd());
  const config = resolveConfig(process.env, { cwd: process.cwd() });

  for (const problem of config.problems) {
    const log = problem.level === "fatal" ? console.error : console.warn;
    log(`[portal] ${problem.level}: ${problem.variable} ${problem.message}`);
  }
  if (!config.usable) {
    console.error("[portal] refusing to start: configuration is incomplete.");
    process.exitCode = 1;
    return;
  }

  const { server, context } = await createPortalServer(config);

  // Environment-based owner provisioning. Runs once, at startup, and only
  // creates an account when none exists. It never rotates a password and never
  // prints one; every message names a variable, not a value.
  if (context.store !== null) {
    const ownerDecision = readOwnerEnvironment(process.env);
    const bootstrap = await ensureOwnerFromEnvironment({ store: context.store, decision: ownerDecision });
    for (const line of describeOwnerBootstrap(ownerDecision, bootstrap)) {
      const log = line.level === "error" ? console.error : line.level === "warn" ? console.warn : console.log;
      log(`[portal] ${line.message}`);
    }
  }

  server.listen(config.port, config.host, () => {
    const described = describeConfig(config);
    console.log(
      `[portal] listening on ${config.host}:${config.port} ` +
        `(env=${described.environment}, origin=${described.originConfigured ? "configured" : "derived"}, ` +
        `secureCookies=${described.secureCookies}, store=${context.store ? "ready" : "unavailable"}, ` +
        `owner=${context.store?.provisioned ? "provisioned" : "not provisioned"})`,
    );
    if (!context.store?.provisioned) {
      console.warn(
        "[portal] no owner account exists yet: run `node server/provision-owner.mjs --email <owner>` " +
          "on the server. Sign-in answers 401 until then and the UI shows an unconfigured state.",
      );
    }
  });

  const shutdown = (signal) => {
    console.log(`[portal] ${signal} received, closing.`);
    server.close(() => process.exit(0));
    setTimeout(() => process.exit(0), 3000).unref();
  };
  process.on("SIGINT", () => shutdown("SIGINT"));
  process.on("SIGTERM", () => shutdown("SIGTERM"));
}

/** True when this file is the process entry point (not imported by a test). */
const isMain = (() => {
  const entry = process.argv[1];
  if (!entry) return false;
  try {
    return path.resolve(entry) === path.resolve(fileURLToPath(import.meta.url));
  } catch {
    return false;
  }
})();

if (isMain) {
  await main();
}

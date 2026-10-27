/**
 * Deployment-contract tests (Phase 17).
 *
 * These check the things a host will actually do to this server, and the things
 * a host will silently break if the server does not handle them itself:
 *
 * - deep links and refresh on any route;
 * - `/api/*` never falling through to the SPA shell;
 * - `/robots.txt` and `/sitemap.xml` under both configured and unconfigured
 *   builds (a real file when the build emitted one, an honest 404 otherwise);
 * - cache headers that let a CDN serve hashed assets immutably;
 * - portal pages staying `noindex` while public pages stay indexable;
 * - production-only security headers appearing in production and never in
 *   development;
 * - path traversal refusal, HEAD support, graceful shutdown;
 * - a production startup that refuses to run with incomplete configuration.
 *
 * Run with `pnpm test:portal`.
 */

import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";

import { productionSecurityHeaders } from "../http.mjs";
import { resolveConfig } from "../config.mjs";
import { createPortalServer } from "../index.mjs";

const WEBSITE_DIR = path.resolve(import.meta.dirname, "../..");
const STATIC_DIR = path.join(WEBSITE_DIR, "dist");
const NETLIFY_CONFIG = path.resolve(WEBSITE_DIR, "../netlify.toml");

/* ─────────────────────────────── harness ─────────────────────────────── */

async function startServer(options = {}) {
  const dataDir = await fs.mkdtemp(path.join(os.tmpdir(), "nexora-deploy-"));
  const base = resolveConfig(
    {
      NEXORA_PORTAL_DATA_DIR: dataDir,
      NEXORA_PORTAL_STATIC_DIR: options.staticDir ?? STATIC_DIR,
      ...(options.env ?? {}),
    },
    { cwd: WEBSITE_DIR },
  );
  const config = { ...base, ...(options.overrides ?? {}) };
  const { server } = await createPortalServer(config);
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const baseUrl = `http://127.0.0.1:${server.address().port}`;
  return {
    baseUrl,
    config,
    server,
    close: async () => {
      await new Promise((resolve) => server.close(resolve));
      await fs.rm(dataDir, { recursive: true, force: true });
    },
  };
}

/** A temporary static directory with a known file layout. */
async function makeStaticFixture() {
  const dir = await fs.mkdtemp(path.join(os.tmpdir(), "nexora-static-"));
  await fs.mkdir(path.join(dir, "assets"), { recursive: true });
  await fs.writeFile(
    path.join(dir, "index.html"),
    '<!doctype html><html><head><meta name="robots" content="index, follow" /></head><body><div id="root"></div></body></html>',
  );
  await fs.writeFile(path.join(dir, "assets", "index-BbfPWENj.js"), "console.log('asset');");
  await fs.writeFile(path.join(dir, "assets", "index-CCuSGdpx.css"), "body{color:#000}");
  await fs.writeFile(path.join(dir, "robots.txt"), "User-agent: *\nAllow: /\n\nSitemap: https://example.com/sitemap.xml\n");
  await fs.writeFile(
    path.join(dir, "sitemap.xml"),
    '<?xml version="1.0" encoding="UTF-8"?><urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9"><url><loc>https://example.com/</loc></url></urlset>',
  );
  return dir;
}

/* ─────────────────────── static site + SPA routing ───────────────────── */

test("Netlify's SPA fallback cannot turn API or missing-file failures into HTML success responses", async () => {
  const source = await fs.readFile(NETLIFY_CONFIG, "utf8");
  const rules = source
    .split(/\r?\n(?=\[\[redirects\]\])/)
    .slice(1)
    .map((section) => {
      const rule = {};
      for (const match of section.matchAll(/^\s{2}(from|to|status)\s*=\s*(?:"([^"]+)"|(\d+))\s*$/gm)) {
        rule[match[1]] = match[1] === "status" ? Number(match[3]) : match[2];
      }
      return rule;
    });

  // Rule order is part of the contract: Netlify uses the first match. Leaving
  // every rule unforced lets real files shadow the fallback, including hashed
  // assets and sitemap.xml when an indexable build actually emits it.
  assert.deepEqual(rules, [
    { from: "/api", to: "/404.txt", status: 404 },
    { from: "/api/*", to: "/404.txt", status: 404 },
    { from: "/sitemap.xml", to: "/404.txt", status: 404 },
    { from: "/assets/*", to: "/404.txt", status: 404 },
    { from: "/*", to: "/index.html", status: 200 },
  ]);
  assert.ok(!source.includes("force = true"), "static files must retain Netlify's default shadowing");

  const notFound = await fs.readFile(path.join(WEBSITE_DIR, "public", "404.txt"), "utf8");
  assert.ok(notFound.length > 0);
  assert.ok(!notFound.includes("<div id=\"root\">"), "the failure body must not be the React app shell");
});

test("deep links and refreshes serve the app shell, and assets are served separately", async () => {
  const app = await startServer();
  try {
    for (const route of ["/", "/features", "/admin", "/admin/website", "/no-such-page", "/a/b/c/d"]) {
      const response = await fetch(app.baseUrl + route);
      assert.equal(response.status, 200, route);
      assert.match(response.headers.get("content-type") ?? "", /text\/html/);
      assert.match(await response.text(), /<div id="root"><\/div>/);
    }

    const file = await fs.readdir(path.join(STATIC_DIR, "assets"));
    const script = file.find((name) => name.endsWith(".js"));
    assert.ok(script, "the build must contain a JS asset");
    const asset = await fetch(`${app.baseUrl}/assets/${script}`);
    assert.equal(asset.status, 200);
    assert.match(asset.headers.get("content-type") ?? "", /javascript/);
  } finally {
    await app.close();
  }
});

test("a missing file with an extension is a 404, not the app shell", async () => {
  const app = await startServer();
  try {
    const missing = await fetch(`${app.baseUrl}/assets/does-not-exist-abc123.js`);
    assert.equal(missing.status, 404);
    assert.match(missing.headers.get("content-type") ?? "", /text\/plain/);
    assert.ok(!(await missing.text()).includes("<div id=\"root\">"));
  } finally {
    await app.close();
  }
});

test("path traversal cannot escape the static directory", async () => {
  const dir = await makeStaticFixture();
  // A file outside the served tree, to prove traversal cannot reach it. It is
  // created beside the fixture, not inside it, and removed in the `finally`.
  const outside = path.join(await fs.realpath(path.dirname(dir)), `nexora-outside-${process.pid}.txt`);
  await fs.writeFile(outside, "do not serve me");
  const app = await startServer({ staticDir: dir });
  try {
    const attempts = [
      `/../${path.basename(outside)}`,
      `/%2e%2e/${path.basename(outside)}`,
      `/..%2f${path.basename(outside)}`,
      `/assets/../../${path.basename(outside)}`,
      "/server/index.mjs",
      "/.env",
      "/.portal-data/portal.json",
    ];
    for (const attempt of attempts) {
      const response = await fetch(app.baseUrl + attempt);
      const body = await response.text();
      assert.ok(!body.includes("do not serve me"), `${attempt} reached a file outside the root`);
      assert.ok(!body.includes("NEXORA_PORTAL_"), `${attempt} leaked server source`);
      assert.ok(!body.includes("passwordHash"), `${attempt} leaked the store`);
    }
  } finally {
    await app.close();
    await fs.rm(dir, { recursive: true, force: true });
    await fs.rm(outside, { force: true });
  }
});

/* ─────────────────────────────── API routing ─────────────────────────── */

test("API paths never fall through to the SPA, and unknown API paths stay JSON", async () => {
  const app = await startServer();
  try {
    const unknown = await fetch(`${app.baseUrl}/api/portal/does-not-exist`);
    assert.equal(unknown.status, 404);
    assert.match(unknown.headers.get("content-type") ?? "", /application\/json/);
    assert.equal((await unknown.json()).error.code, "not_found");

    const bare = await fetch(`${app.baseUrl}/api`);
    assert.equal(bare.status, 404);
    assert.match(bare.headers.get("content-type") ?? "", /application\/json/);

    const status = await fetch(`${app.baseUrl}/api/portal/status`);
    assert.equal(status.status, 200);
    assert.match(status.headers.get("content-type") ?? "", /application\/json/);
    // Private responses must never be cached by a shared cache.
    assert.match(status.headers.get("cache-control") ?? "", /no-store/);
  } finally {
    await app.close();
  }
});

test("HEAD is answered like GET for the API and for pages", async () => {
  const app = await startServer();
  try {
    const status = await fetch(`${app.baseUrl}/api/portal/status`, { method: "HEAD" });
    assert.equal(status.status, 200, "a HEAD health check must not be a 404");

    const page = await fetch(`${app.baseUrl}/features`, { method: "HEAD" });
    assert.equal(page.status, 200);
    assert.equal(await page.text(), "", "HEAD must not return a body");
  } finally {
    await app.close();
  }
});

/* ────────────────────── crawl files, both environments ───────────────── */

test("unconfigured build: robots.txt disallows and /sitemap.xml is an honest 404", async () => {
  const app = await startServer();
  try {
    const robots = await fetch(`${app.baseUrl}/robots.txt`);
    assert.equal(robots.status, 200);
    assert.match(await robots.text(), /Disallow: \//);

    const sitemap = await fetch(`${app.baseUrl}/sitemap.xml`);
    assert.equal(sitemap.status, 404);
    assert.match(sitemap.headers.get("content-type") ?? "", /text\/plain/);
    assert.match(await sitemap.text(), /no confirmed production domain/i);
  } finally {
    await app.close();
  }
});

test("configured build: both crawl files are served as their real types", async () => {
  const dir = await makeStaticFixture();
  const app = await startServer({ staticDir: dir });
  try {
    const robots = await fetch(`${app.baseUrl}/robots.txt`);
    assert.equal(robots.status, 200);
    assert.match(robots.headers.get("content-type") ?? "", /text\/plain/);
    assert.match(await robots.text(), /Sitemap: https:\/\/example\.com\/sitemap\.xml/);

    const sitemap = await fetch(`${app.baseUrl}/sitemap.xml`);
    assert.equal(sitemap.status, 200);
    assert.match(sitemap.headers.get("content-type") ?? "", /application\/xml/);
    assert.match(await sitemap.text(), /<urlset/);
  } finally {
    await app.close();
    await fs.rm(dir, { recursive: true, force: true });
  }
});

/* ─────────────────────────── indexing boundaries ─────────────────────── */

test("public pages stay indexable while portal pages and the API stay noindex", async () => {
  const app = await startServer();
  try {
    // Regression: Phase 17 removed an `X-Robots-Tag: noindex` that was applied to
    // every response, which would have deindexed a launched site.
    for (const route of ["/", "/features", "/download", "/faq"]) {
      const response = await fetch(app.baseUrl + route);
      assert.equal(response.headers.get("x-robots-tag"), null, `${route} must not be noindexed at the edge`);
    }

    for (const route of ["/admin", "/admin/website", "/admin/settings"]) {
      const response = await fetch(app.baseUrl + route);
      assert.match(response.headers.get("x-robots-tag") ?? "", /noindex/, `${route} must be noindex`);
    }

    const api = await fetch(`${app.baseUrl}/api/portal/status`);
    assert.match(api.headers.get("x-robots-tag") ?? "", /noindex/);
  } finally {
    await app.close();
  }
});

/* ───────────────────────────── cache policy ──────────────────────────── */

test("hashed build assets are immutable and the shell is not cached", async () => {
  const dir = await makeStaticFixture();
  const app = await startServer({ staticDir: dir });
  try {
    // Regression: the previous pattern required a dot before the hash, so Vite's
    // `index-HASH.js` was served `no-cache` and revalidated on every request.
    const asset = await fetch(`${app.baseUrl}/assets/index-BbfPWENj.js`);
    assert.equal(asset.status, 200);
    assert.match(asset.headers.get("cache-control") ?? "", /immutable/);
    assert.match(asset.headers.get("cache-control") ?? "", /max-age=31536000/);

    const stylesheet = await fetch(`${app.baseUrl}/assets/index-CCuSGdpx.css`);
    assert.match(stylesheet.headers.get("cache-control") ?? "", /immutable/);

    const shell = await fetch(`${app.baseUrl}/features`);
    assert.equal(shell.headers.get("cache-control"), "no-cache", "the shell must revalidate");
  } finally {
    await app.close();
    await fs.rm(dir, { recursive: true, force: true });
  }
});

/* ──────────────────── production-only security headers ───────────────── */

test("production headers are a configuration decision, not a constant", () => {
  const development = productionSecurityHeaders(resolveConfig({}, { cwd: WEBSITE_DIR }));
  assert.deepEqual(development, {}, "nothing production-only in development");

  const production = productionSecurityHeaders(
    resolveConfig(
      { NEXORA_PORTAL_ENV: "production", NEXORA_PORTAL_ORIGIN: "https://portal.example.com" },
      { cwd: WEBSITE_DIR },
    ),
  );
  assert.equal(production["X-Frame-Options"], "DENY");
  assert.match(production["Content-Security-Policy"], /frame-ancestors 'none'/);
  assert.match(production["Strict-Transport-Security"], /max-age=31536000/);
});

test("a production-configured server sends them, and a development one does not", async () => {
  const dev = await startServer();
  try {
    const page = await fetch(`${dev.baseUrl}/`);
    assert.equal(page.headers.get("strict-transport-security"), null);
    assert.equal(page.headers.get("x-frame-options"), null);
  } finally {
    await dev.close();
  }

  const prod = await startServer({
    env: { NEXORA_PORTAL_ENV: "production", NEXORA_PORTAL_ORIGIN: "https://portal.example.com" },
  });
  try {
    const page = await fetch(`${prod.baseUrl}/`);
    assert.match(page.headers.get("strict-transport-security") ?? "", /max-age=31536000/);
    assert.equal(page.headers.get("x-frame-options"), "DENY");
    const api = await fetch(`${prod.baseUrl}/api/portal/status`);
    assert.equal(api.headers.get("x-frame-options"), "DENY", "API responses carry them too");
  } finally {
    await prod.close();
  }
});

/* ──────────────────────── startup and shutdown ───────────────────────── */

test("a production start without mandatory configuration exits non-zero", async () => {
  const dir = await fs.mkdtemp(path.join(os.tmpdir(), "nexora-prod-fail-"));
  const run = new Promise((resolve) => {
    const child = spawn(process.execPath, ["server/index.mjs"], {
      cwd: WEBSITE_DIR,
      env: {
        ...process.env,
        NEXORA_PORTAL_ENV: "production",
        NEXORA_PORTAL_ENV_FILE: path.join(dir, "absent.env"),
        NEXORA_PORTAL_DATA_DIR: path.join(dir, "data"),
        NEXORA_PORTAL_PORT: "0",
      },
    });
    let stderr = "";
    child.stderr.on("data", (chunk) => {
      stderr += chunk;
    });
    child.on("close", (code) => resolve({ code, stderr }));
  });

  try {
    const result = await run;
    assert.notEqual(result.code, 0, "the process must refuse to serve production unconfigured");
    assert.match(result.stderr, /NEXORA_PORTAL_ORIGIN/, "the operator is told which variable is missing");
  } finally {
    await fs.rm(dir, { recursive: true, force: true });
  }
});

test("SIGTERM shuts the server down cleanly", async () => {
  const dir = await fs.mkdtemp(path.join(os.tmpdir(), "nexora-shutdown-"));
  const port = 25000 + Math.floor(Math.random() * 2000);
  const child = spawn(process.execPath, ["server/index.mjs"], {
    cwd: WEBSITE_DIR,
    env: {
      ...process.env,
      NEXORA_PORTAL_ENV_FILE: path.join(dir, "absent.env"),
      NEXORA_PORTAL_DATA_DIR: path.join(dir, "data"),
      NEXORA_PORTAL_PORT: String(port),
      NEXORA_PORTAL_HOST: "127.0.0.1",
    },
  });

  let stderr = "";
  child.stderr.on("data", (chunk) => {
    stderr += chunk;
  });

  try {
    const deadline = Date.now() + 20_000;
    let ready = false;
    while (Date.now() < deadline && !ready) {
      try {
        ready = (await fetch(`http://127.0.0.1:${port}/api/portal/status`)).ok;
      } catch {
        await new Promise((resolve) => setTimeout(resolve, 150));
      }
    }
    assert.equal(ready, true, `server did not start: ${stderr.slice(0, 200)}`);

    const exited = new Promise((resolve) => child.on("close", (code) => resolve(code)));
    child.kill("SIGTERM");
    const code = await exited;
    assert.equal(code, 0, "a clean shutdown exits 0");

    await assert.rejects(
      async () => {
        const response = await fetch(`http://127.0.0.1:${port}/api/portal/status`, { signal: AbortSignal.timeout(1500) });
        if (!response.ok) throw new Error("down");
      },
      "the port must be closed after shutdown",
    );
  } finally {
    child.kill("SIGKILL");
    await fs.rm(dir, { recursive: true, force: true });
  }
});

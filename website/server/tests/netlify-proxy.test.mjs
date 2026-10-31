import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";

import {
  BACKEND_ORIGIN_ENV,
  prepareNetlifyRedirects,
  readBackendOrigin,
  renderBackendRedirects,
} from "../../scripts/prepare-netlify-redirects.mjs";

const WEBSITE_DIR = path.resolve(import.meta.dirname, "../..");
const SCRIPT = path.join(WEBSITE_DIR, "scripts/prepare-netlify-redirects.mjs");

async function fixture() {
  const distDir = await fs.mkdtemp(path.join(os.tmpdir(), "nexora-netlify-proxy-"));
  await fs.writeFile(path.join(distDir, "index.html"), '<div id="root"></div>');
  return distDir;
}

test("an unset backend origin preserves the static 404 fallback and removes a stale generated proxy", async () => {
  const distDir = await fixture();
  const output = path.join(distDir, "_redirects");
  await fs.writeFile(output, "/api/* https://stale.example.com/api/:splat 200!\n");

  try {
    for (const env of [{}, { [BACKEND_ORIGIN_ENV]: "" }, { [BACKEND_ORIGIN_ENV]: "   " }]) {
      const result = await prepareNetlifyRedirects({ env, distDir });
      assert.equal(result.configured, false);
      await assert.rejects(fs.access(output), "no generated proxy may remain");
    }
  } finally {
    await fs.rm(distDir, { recursive: true, force: true });
  }
});

test("a real HTTPS origin generates only the two forced API proxy rules", async () => {
  const distDir = await fixture();
  try {
    const result = await prepareNetlifyRedirects({
      env: { [BACKEND_ORIGIN_ENV]: "https://portal-backend.example.com/" },
      distDir,
    });
    assert.equal(result.configured, true);
    assert.equal(
      await fs.readFile(result.output, "utf8"),
      renderBackendRedirects("https://portal-backend.example.com"),
    );
    const rules = (await fs.readFile(result.output, "utf8")).split("\n").filter((line) => !line.startsWith("#") && line);
    assert.deepEqual(rules, [
      "/api https://portal-backend.example.com/api 200!",
      "/api/* https://portal-backend.example.com/api/:splat 200!",
    ]);
  } finally {
    await fs.rm(distDir, { recursive: true, force: true });
  }
});

test("unsafe or ambiguous backend origins are rejected without leaking their values", async () => {
  const invalid = [
    "http://portal-backend.example.com",
    "https://user:do-not-print-this-password@example.com",
    "https://portal-backend.example.com/api",
    "https://portal-backend.example.com/?token=do-not-print-this-token",
    "https://portal-backend.example.com/#fragment",
    "not a URL",
  ];

  for (const value of invalid) {
    assert.throws(
      () => readBackendOrigin({ [BACKEND_ORIGIN_ENV]: value }),
      (error) => {
        assert.match(error.message, /bare HTTPS origin/);
        assert.ok(!error.message.includes("do-not-print-this-password"));
        assert.ok(!error.message.includes("do-not-print-this-token"));
        return true;
      },
      value,
    );
  }
});

test("the Netlify build helper exits non-zero for an invalid opt-in", async () => {
  const distDir = await fixture();
  const secretLookingValue = "https://user:never-log-this@example.com";
  const result = await new Promise((resolve) => {
    const child = spawn(process.execPath, [SCRIPT], {
      cwd: WEBSITE_DIR,
      env: { PATH: process.env.PATH, [BACKEND_ORIGIN_ENV]: secretLookingValue },
    });
    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (chunk) => {
      stdout += chunk;
    });
    child.stderr.on("data", (chunk) => {
      stderr += chunk;
    });
    child.on("close", (code) => resolve({ code, stdout, stderr }));
  });

  try {
    assert.notEqual(result.code, 0);
    assert.match(result.stderr, /bare HTTPS origin/);
    assert.ok(!result.stderr.includes("never-log-this"));
    assert.ok(!result.stdout.includes("never-log-this"));
  } finally {
    await fs.rm(distDir, { recursive: true, force: true });
  }
});

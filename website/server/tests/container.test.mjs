import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import fs from "node:fs/promises";
import net from "node:net";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";

const WEBSITE_DIR = path.resolve(import.meta.dirname, "../..");

async function availablePort() {
  const listener = net.createServer();
  await new Promise((resolve) => listener.listen(0, "127.0.0.1", resolve));
  const { port } = listener.address();
  await new Promise((resolve) => listener.close(resolve));
  return port;
}

test("the container definition keeps secrets out and runs as an unprivileged user", async () => {
  const dockerfile = await fs.readFile(path.join(WEBSITE_DIR, "Dockerfile"), "utf8");
  assert.match(dockerfile, /FROM node:22-bookworm-slim AS build/);
  assert.match(dockerfile, /RUN pnpm install --frozen-lockfile/);
  assert.match(dockerfile, /RUN pnpm build/);
  assert.match(dockerfile, /USER node/);
  assert.match(dockerfile, /EXPOSE 4174/);
  assert.match(dockerfile, /CMD \["node", "server\/index\.mjs"\]/);
  assert.ok(!/^\s*ARG\s/m.test(dockerfile), "no build-argument secret channel is needed");
  assert.ok(!/NEXORA_PORTAL_OWNER_(?:EMAIL|NAME|PASSWORD)/.test(dockerfile));

  const ignored = await fs.readFile(path.join(WEBSITE_DIR, ".dockerignore"), "utf8");
  for (const pattern of ["node_modules/", "dist/", ".env", ".env.*", "**/.env", "**/.env.*", ".portal-data/"]) {
    assert.ok(ignored.split(/\r?\n/).includes(pattern), `.dockerignore is missing ${pattern}`);
  }
});

test("the runtime image layout starts without node_modules and keeps production fail-closed behaviour", { timeout: 25_000 }, async () => {
  const runtimeDir = await fs.mkdtemp(path.join(os.tmpdir(), "nexora-container-runtime-"));
  const port = await availablePort();
  let child;

  try {
    // Mirror the final Dockerfile stage rather than testing from the source
    // tree: no installed dependencies are copied into this runtime layout.
    await fs.cp(path.join(WEBSITE_DIR, "server"), path.join(runtimeDir, "server"), {
      recursive: true,
      filter: (source) => !source.includes(`${path.sep}server${path.sep}tests`),
    });
    await fs.cp(path.join(WEBSITE_DIR, "dist"), path.join(runtimeDir, "dist"), { recursive: true });
    await fs.copyFile(path.join(WEBSITE_DIR, "package.json"), path.join(runtimeDir, "package.json"));
    await assert.rejects(fs.access(path.join(runtimeDir, "node_modules")));

    child = spawn(process.execPath, ["server/index.mjs"], {
      cwd: runtimeDir,
      env: {
        PATH: process.env.PATH,
        NODE_ENV: "production",
        NEXORA_PORTAL_ENV: "production",
        NEXORA_PORTAL_ORIGIN: "https://portal.example.com",
        NEXORA_PORTAL_SECURE_COOKIES: "true",
        NEXORA_PORTAL_TRUST_PROXY: "false",
        NEXORA_PORTAL_HOST: "127.0.0.1",
        NEXORA_PORTAL_PORT: String(port),
        NEXORA_PORTAL_DATA_DIR: path.join(runtimeDir, "volume"),
        NEXORA_PORTAL_STATIC_DIR: path.join(runtimeDir, "dist"),
        NEXORA_PORTAL_ENV_FILE: path.join(runtimeDir, "absent.env"),
      },
    });

    let stderr = "";
    child.stderr.on("data", (chunk) => {
      stderr += chunk;
    });

    let ready = false;
    const deadline = Date.now() + 12_000;
    while (Date.now() < deadline && !ready) {
      if (child.exitCode !== null) break;
      try {
        ready = (await fetch(`http://127.0.0.1:${port}/api/portal/status`)).ok;
      } catch {
        await new Promise((resolve) => setTimeout(resolve, 100));
      }
    }
    assert.equal(ready, true, `runtime layout did not start: ${stderr.slice(0, 300)}`);

    const status = await fetch(`http://127.0.0.1:${port}/api/portal/status`);
    assert.equal(status.status, 200);
    assert.match(status.headers.get("strict-transport-security") ?? "", /max-age=31536000/);
    const body = await status.json();
    assert.equal(body.available, true, "the persistent store must open successfully");
    assert.equal(body.environment, "production");
    assert.equal(body.ownerProvisioned, false);

    const signedOut = await fetch(`http://127.0.0.1:${port}/api/portal/session`);
    assert.equal(signedOut.status, 401);
    assert.match(signedOut.headers.get("content-type") ?? "", /application\/json/);

    const exited = new Promise((resolve) => child.once("close", (code) => resolve(code)));
    child.kill("SIGTERM");
    assert.equal(await exited, 0);
    child = undefined;
  } finally {
    if (child && child.exitCode === null) child.kill("SIGKILL");
    await fs.rm(runtimeDir, { recursive: true, force: true });
  }
});

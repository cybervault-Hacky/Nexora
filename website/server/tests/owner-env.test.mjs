/**
 * Tests for environment-based Owner provisioning.
 *
 * Covers the `.env` loader, the Owner configuration rules, creation on first
 * start, the "never overwrite on restart" guarantee, explicit rotation with
 * session revocation, invalid configuration, and — the part that matters most —
 * that no configured value reaches a log, an API response, or the build output.
 *
 * Run with `pnpm test:portal`.
 */

import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";

import { hashPassword, hashToken, verifyPassword } from "../auth.mjs";
import { envFilePermissionWarning, loadEnvFile, parseEnvFile } from "../env-file.mjs";
import {
  describeOwnerBootstrap,
  ensureOwnerFromEnvironment,
  readOwnerEnvironment,
  rotateOwnerFromEnvironment,
} from "../owner-config.mjs";
import { openStore } from "../store.mjs";

const WEBSITE_DIR = path.resolve(import.meta.dirname, "../..");
const ENV_PASSWORD = "environment-password-1234";
const ENV_EMAIL = "owner@example.com";

/* ─────────────────────────────── helpers ─────────────────────────────── */

async function tempDir(prefix = "nexora-portal-env-") {
  return await fs.mkdtemp(path.join(os.tmpdir(), prefix));
}

/** Writes a `.env` file and returns its path. */
async function writeEnvFile(dir, body) {
  const file = path.join(dir, "test.env");
  await fs.writeFile(file, body, { mode: 0o600 });
  return file;
}

function ownerEnv(overrides = {}) {
  return {
    NEXORA_PORTAL_OWNER_EMAIL: ENV_EMAIL,
    NEXORA_PORTAL_OWNER_NAME: "Configured Owner",
    NEXORA_PORTAL_OWNER_PASSWORD: ENV_PASSWORD,
    ...overrides,
  };
}

/** Opens a store in a fresh temp directory. */
async function freshStore() {
  const dir = await tempDir();
  const store = await openStore(dir);
  return { store, dir };
}

/* ──────────────────────────── .env parsing ───────────────────────────── */

test("the .env parser handles comments, quotes, escapes and CRLF", () => {
  const parsed = parseEnvFile(
    [
      "# a comment",
      "",
      "PLAIN=value",
      "TRAILING=value   # inline comment",
      'DOUBLE="quoted value"',
      "ESCAPED=\"line\\nbreak\\tand quote\\\"\"",
      "SINGLE='literal $dollar and \\n stays'",
      "export EXPORTED=yes",
      "  SPACED  =  padded  ",
      "EMPTY=",
      "NOT_A_PAIR",
      "1INVALID=value",
    ].join("\r\n"),
  );

  assert.equal(parsed.PLAIN, "value");
  assert.equal(parsed.TRAILING, "value");
  assert.equal(parsed.DOUBLE, "quoted value");
  assert.equal(parsed.ESCAPED, 'line\nbreak\tand quote"');
  assert.equal(parsed.SINGLE, "literal $dollar and \\n stays");
  assert.equal(parsed.EXPORTED, "yes");
  assert.equal(parsed.SPACED, "padded");
  assert.equal(parsed.EMPTY, "");
  assert.equal(parsed.NOT_A_PAIR, undefined);
  assert.equal(parsed["1INVALID"], undefined);
});

test("loading a .env file never overrides the process environment", async () => {
  const dir = await tempDir();
  const file = await writeEnvFile(dir, "FROM_FILE=file-value\nBOTH=file-value\n");
  const env = { BOTH: "process-value" };

  const result = loadEnvFile({ env, filePath: file });

  assert.equal(result.loaded, true);
  assert.equal(env.FROM_FILE, "file-value");
  assert.equal(env.BOTH, "process-value", "the process environment wins");
  assert.deepEqual(result.appliedKeys, ["FROM_FILE"]);
  assert.deepEqual(result.skippedKeys, ["BOTH"]);
  // Key names only: the result must never carry a value.
  assert.ok(!JSON.stringify(result).includes("file-value"));

  await fs.rm(dir, { recursive: true, force: true });
});

test("a missing .env file is a normal state, not an error", () => {
  const result = loadEnvFile({ env: {}, filePath: "/nonexistent/definitely-not-here.env" });
  assert.equal(result.loaded, false);
  assert.equal(result.problem, null);
  assert.deepEqual(result.appliedKeys, []);
});

test("a world-readable .env file raises a permission warning", async () => {
  const dir = await tempDir();
  const file = path.join(dir, "loose.env");
  await fs.writeFile(file, "SECRET=value\n", { mode: 0o644 });

  const warning = envFilePermissionWarning(file);
  assert.ok(warning !== null, "expected a warning for 644");
  assert.match(warning, /600/, warning);
  assert.ok(!warning.includes("value"));

  await fs.chmod(file, 0o600);
  assert.equal(envFilePermissionWarning(file), null);

  await fs.rm(dir, { recursive: true, force: true });
});

/* ─────────────────────── owner configuration rules ───────────────────── */

test("no owner variables configured is a valid, explicit state", () => {
  const decision = readOwnerEnvironment({});
  assert.equal(decision.status, "absent");
  assert.equal(decision.email, null);
  assert.equal(decision.password, null);
  assert.deepEqual(decision.problems, []);
});

test("a complete owner configuration is accepted and normalised", () => {
  const decision = readOwnerEnvironment({
    NEXORA_PORTAL_OWNER_EMAIL: "  Owner@Example.COM ",
    NEXORA_PORTAL_OWNER_NAME: "Configured Owner",
    NEXORA_PORTAL_OWNER_PASSWORD: ENV_PASSWORD,
  });
  assert.equal(decision.status, "valid");
  assert.equal(decision.email, ENV_EMAIL);
  assert.equal(decision.displayName, "Configured Owner");
  assert.equal(decision.password, ENV_PASSWORD);
});

test("a missing name falls back to the email's local part", () => {
  const decision = readOwnerEnvironment(ownerEnv({ NEXORA_PORTAL_OWNER_NAME: "" }));
  assert.equal(decision.status, "valid");
  assert.equal(decision.displayName, "owner");
});

test("incomplete or weak configuration is rejected without echoing values", () => {
  const cases = [
    { NEXORA_PORTAL_OWNER_EMAIL: ENV_EMAIL },
    { NEXORA_PORTAL_OWNER_PASSWORD: ENV_PASSWORD },
    { ...ownerEnv(), NEXORA_PORTAL_OWNER_PASSWORD: "" },
    { ...ownerEnv(), NEXORA_PORTAL_OWNER_PASSWORD: "short" },
    { ...ownerEnv(), NEXORA_PORTAL_OWNER_EMAIL: "not-an-email" },
    { ...ownerEnv(), NEXORA_PORTAL_OWNER_NAME: "x".repeat(200) },
  ];

  for (const env of cases) {
    const decision = readOwnerEnvironment(env);
    assert.equal(decision.status, "invalid", JSON.stringify(env).slice(0, 40));
    assert.ok(decision.problems.length > 0);
    assert.equal(decision.password, null, "an invalid configuration never carries a password");
    const serialised = decision.problems.join(" ");
    assert.ok(!serialised.includes(ENV_PASSWORD), "no value in a problem message");
    assert.ok(!serialised.includes(ENV_EMAIL), "no value in a problem message");
    assert.ok(!serialised.includes("short"), "not even the rejected password");
  }
});

/* ──────────────────────── provisioning behaviour ─────────────────────── */

test("startup provisioning creates the owner once, with a scrypt hash", async () => {
  const { store, dir } = await freshStore();
  try {
    const decision = readOwnerEnvironment(ownerEnv());
    const first = await ensureOwnerFromEnvironment({ store, decision });
    assert.equal(first.outcome, "created");

    const users = store.listUsers();
    assert.equal(users.length, 1);
    assert.equal(users[0].email, ENV_EMAIL);
    assert.equal(users[0].role, "owner");
    assert.match(users[0].passwordHash, /^scrypt\$32768\$8\$1\$/);
    assert.equal(await verifyPassword(ENV_PASSWORD, users[0].passwordHash), true);

    // Nothing on disk holds the plaintext.
    const file = await fs.readFile(path.join(dir, "portal.json"), "utf8");
    assert.ok(!file.includes(ENV_PASSWORD));
    assert.equal(store.recentAudit(5)[0].event, "owner_provisioned_from_environment");
  } finally {
    await fs.rm(dir, { recursive: true, force: true });
  }
});

test("a restart never overwrites an existing owner's password", async () => {
  const { store, dir } = await freshStore();
  try {
    const created = await ensureOwnerFromEnvironment({ store, decision: readOwnerEnvironment(ownerEnv()) });
    assert.equal(created.outcome, "created");
    const originalHash = store.listUsers()[0].passwordHash;

    // A restart with a *different* password configured must change nothing.
    const restart = await ensureOwnerFromEnvironment({
      store,
      decision: readOwnerEnvironment(ownerEnv({ NEXORA_PORTAL_OWNER_PASSWORD: "a-brand-new-password-9876" })),
    });
    assert.equal(restart.outcome, "exists");
    assert.equal(store.listUsers()[0].passwordHash, originalHash);
    assert.equal(store.listUsers().length, 1);

    // The original credential still works; the configured new one does not.
    assert.equal(await verifyPassword(ENV_PASSWORD, store.listUsers()[0].passwordHash), true);
    assert.equal(await verifyPassword("a-brand-new-password-9876", store.listUsers()[0].passwordHash), false);
  } finally {
    await fs.rm(dir, { recursive: true, force: true });
  }
});

test("a configured email that does not match the existing owner changes nothing", async () => {
  const { store, dir } = await freshStore();
  try {
    await store.createOwner({
      email: "existing@example.com",
      displayName: "Existing",
      passwordHash: await hashPassword("existing-owner-password-1"),
    });
    const before = store.listUsers();

    const result = await ensureOwnerFromEnvironment({
      store,
      decision: readOwnerEnvironment(ownerEnv()),
    });

    assert.equal(result.outcome, "exists-other-email");
    assert.deepEqual(store.listUsers(), before, "no account was added, renamed, or modified");
  } finally {
    await fs.rm(dir, { recursive: true, force: true });
  }
});

test("absent or invalid configuration provisions nothing", async () => {
  const { store, dir } = await freshStore();
  try {
    assert.equal((await ensureOwnerFromEnvironment({ store, decision: readOwnerEnvironment({}) })).outcome, "skipped-absent");
    assert.equal(
      (await ensureOwnerFromEnvironment({
        store,
        decision: readOwnerEnvironment({ NEXORA_PORTAL_OWNER_EMAIL: ENV_EMAIL }),
      })).outcome,
      "skipped-invalid",
    );
    assert.equal(store.listUsers().length, 0);
    assert.equal(store.provisioned, false);
  } finally {
    await fs.rm(dir, { recursive: true, force: true });
  }
});

/* ───────────────────────────── rotation ──────────────────────────────── */

test("explicit rotation replaces the password and revokes every session", async () => {
  const { store, dir } = await freshStore();
  try {
    await ensureOwnerFromEnvironment({ store, decision: readOwnerEnvironment(ownerEnv()) });
    const owner = store.listUsers()[0];

    // Two live sessions, as a stolen cookie would be.
    for (const token of ["a".repeat(64), "b".repeat(64)]) {
      await store.createSession({
        userId: owner.id,
        tokenHash: hashToken(token),
        csrfToken: hashToken(`csrf-${token}`),
        expiresAt: new Date(Date.now() + 3600_000).toISOString(),
      });
    }
    assert.equal(store.getSessionByTokenHash(hashToken("a".repeat(64))) !== null, true);

    const rotated = await rotateOwnerFromEnvironment({
      store,
      decision: readOwnerEnvironment(ownerEnv({ NEXORA_PORTAL_OWNER_PASSWORD: "rotated-password-5678" })),
    });

    assert.equal(rotated.outcome, "rotated");
    const after = store.listUsers()[0];
    assert.equal(await verifyPassword("rotated-password-5678", after.passwordHash), true);
    assert.equal(await verifyPassword(ENV_PASSWORD, after.passwordHash), false);
    assert.equal(store.getSessionByTokenHash(hashToken("a".repeat(64))), null, "session revoked");
    assert.equal(store.getSessionByTokenHash(hashToken("b".repeat(64))), null, "session revoked");
    assert.equal(store.recentAudit(3)[0].event, "password_rotated");
  } finally {
    await fs.rm(dir, { recursive: true, force: true });
  }
});

test("rotation refuses a different email and creates when no owner exists", async () => {
  const { store, dir } = await freshStore();
  try {
    const created = await rotateOwnerFromEnvironment({ store, decision: readOwnerEnvironment(ownerEnv()) });
    assert.equal(created.outcome, "created");

    const conflict = await rotateOwnerFromEnvironment({
      store,
      decision: readOwnerEnvironment(ownerEnv({ NEXORA_PORTAL_OWNER_EMAIL: "other@example.com" })),
    });
    assert.equal(conflict.outcome, "conflict");
    assert.equal(store.listUsers().length, 1);
    assert.equal(store.listUsers()[0].email, ENV_EMAIL);
  } finally {
    await fs.rm(dir, { recursive: true, force: true });
  }
});

/* ──────────────────────────── log hygiene ────────────────────────────── */

test("bootstrap messages name variables, never values", () => {
  const valid = readOwnerEnvironment(ownerEnv());
  const messages = [
    ...describeOwnerBootstrap(valid, { outcome: "created" }),
    ...describeOwnerBootstrap(valid, { outcome: "exists" }),
    ...describeOwnerBootstrap(valid, { outcome: "exists-other-email" }),
    ...describeOwnerBootstrap(readOwnerEnvironment({}), { outcome: "skipped-absent" }),
    ...describeOwnerBootstrap(
      readOwnerEnvironment({ NEXORA_PORTAL_OWNER_EMAIL: "bad", NEXORA_PORTAL_OWNER_PASSWORD: "short" }),
      { outcome: "skipped-invalid" },
    ),
  ];
  const text = messages.map((line) => `${line.level}: ${line.message}`).join("\n");
  assert.ok(text.includes("NEXORA_PORTAL_OWNER_EMAIL"), "the operator is told which variable to fix");
  assert.ok(!text.includes(ENV_PASSWORD));
  assert.ok(!text.includes("short"));
  assert.ok(!text.includes(ENV_EMAIL), "the configured email is not logged either");
});

/* ───────────────────── validation against the API ────────────────────── */

test("an environment-provisioned owner can sign in through the real server", async () => {
  const dir = await tempDir();
  const port = 24000 + Math.floor(Math.random() * 4000);
  const envFile = await writeEnvFile(
    dir,
    [
      `NEXORA_PORTAL_OWNER_EMAIL=${ENV_EMAIL}`,
      "NEXORA_PORTAL_OWNER_NAME=Configured Owner",
      `NEXORA_PORTAL_OWNER_PASSWORD=${ENV_PASSWORD}`,
    ].join("\n"),
  );

  const child = spawn(process.execPath, ["server/index.mjs"], {
    cwd: WEBSITE_DIR,
    env: {
      ...process.env,
      NEXORA_PORTAL_DATA_DIR: path.join(dir, "data"),
      NEXORA_PORTAL_PORT: String(port),
      NEXORA_PORTAL_HOST: "127.0.0.1",
      NEXORA_PORTAL_ENV_FILE: envFile,
    },
  });

  let stdout = "";
  let stderr = "";
  child.stdout.on("data", (chunk) => {
    stdout += chunk;
  });
  child.stderr.on("data", (chunk) => {
    stderr += chunk;
  });

  const base = `http://127.0.0.1:${port}`;
  const deadline = Date.now() + 20_000;
  try {
    let ready = false;
    while (Date.now() < deadline && !ready) {
      try {
        const probe = await fetch(`${base}/api/portal/status`);
        ready = probe.ok;
      } catch {
        await new Promise((resolve) => setTimeout(resolve, 150));
      }
    }
    assert.equal(ready, true, `the server did not start: ${stderr.slice(0, 300)}`);

    const status = await (await fetch(`${base}/api/portal/status`)).json();
    assert.equal(status.ownerProvisioned, true, "startup provisioning created the owner");

    const signIn = await fetch(`${base}/api/portal/session`, {
      method: "POST",
      headers: { "Content-Type": "application/json", Origin: base },
      body: JSON.stringify({ email: ENV_EMAIL, password: ENV_PASSWORD }),
    });
    assert.equal(signIn.status, 200);
    const cookie = (signIn.headers.getSetCookie?.() ?? []).find((value) =>
      value.startsWith("nexora_portal_session="),
    );
    assert.ok(cookie, "sign-in issued a session cookie");
    const body = await signIn.json();
    assert.equal(body.user.email, ENV_EMAIL);
    assert.equal(body.user.role, "owner");

    const overview = await fetch(`${base}/api/portal/overview`, {
      headers: {
        Cookie: cookie.split(";")[0],
        "X-Portal-CSRF": body.csrfToken,
        Origin: base,
      },
    });
    assert.equal(overview.status, 200);

    // The running process must never print the configured values.
    assert.ok(!stdout.includes(ENV_PASSWORD), "password in stdout");
    assert.ok(!stderr.includes(ENV_PASSWORD), "password in stderr");
    assert.ok(!stdout.includes(ENV_EMAIL), "email in stdout");
    assert.ok(/owner account created from the environment configuration/.test(stdout), stdout.slice(0, 400));
  } finally {
    child.kill("SIGTERM");
    await new Promise((resolve) => child.on("close", resolve));
    await fs.rm(dir, { recursive: true, force: true });
  }
});

test("invalid owner configuration starts safely and provisions nothing", async () => {
  const dir = await tempDir();
  const port = 28000 + Math.floor(Math.random() * 3000);
  const envFile = await writeEnvFile(
    dir,
    [`NEXORA_PORTAL_OWNER_EMAIL=${ENV_EMAIL}`, "NEXORA_PORTAL_OWNER_PASSWORD=short"].join("\n"),
  );

  const child = spawn(process.execPath, ["server/index.mjs"], {
    cwd: WEBSITE_DIR,
    env: {
      ...process.env,
      NEXORA_PORTAL_DATA_DIR: path.join(dir, "data"),
      NEXORA_PORTAL_PORT: String(port),
      NEXORA_PORTAL_HOST: "127.0.0.1",
      NEXORA_PORTAL_ENV_FILE: envFile,
    },
  });

  let stdout = "";
  let stderr = "";
  child.stdout.on("data", (chunk) => {
    stdout += chunk;
  });
  child.stderr.on("data", (chunk) => {
    stderr += chunk;
  });

  const base = `http://127.0.0.1:${port}`;
  const deadline = Date.now() + 20_000;
  try {
    let ready = false;
    while (Date.now() < deadline && !ready) {
      try {
        const probe = await fetch(`${base}/api/portal/status`);
        ready = probe.ok;
      } catch {
        await new Promise((resolve) => setTimeout(resolve, 150));
      }
    }
    assert.equal(ready, true, "the server must still serve the public site");

    const status = await (await fetch(`${base}/api/portal/status`)).json();
    assert.equal(status.ownerProvisioned, false, "an invalid configuration provisions nobody");

    const signIn = await fetch(`${base}/api/portal/session`, {
      method: "POST",
      headers: { "Content-Type": "application/json", Origin: base },
      body: JSON.stringify({ email: ENV_EMAIL, password: "short" }),
    });
    assert.equal(signIn.status, 401, "no account exists, so sign-in is refused");

    // The report names the variable and never the value.
    assert.match(stderr, /NEXORA_PORTAL_OWNER_PASSWORD/);
    assert.ok(!/short\b(?!er)/.test(stderr.replace(/at least 12 characters/g, "")), stderr);
    assert.ok(!stderr.includes(ENV_PASSWORD));
  } finally {
    child.kill("SIGTERM");
    await new Promise((resolve) => child.on("close", resolve));
    await fs.rm(dir, { recursive: true, force: true });
  }
});

/* ─────────────────────── build-output leakage ────────────────────────── */

test("the frontend build contains none of the portal's server-side configuration", async () => {
  const assetsDir = path.join(WEBSITE_DIR, "dist", "assets");
  const files = await fs.readdir(assetsDir).catch(() => []);
  assert.ok(files.length > 0, "run `pnpm build` before this suite");

  const needles = [
    "NEXORA_PORTAL_OWNER",
    "NEXORA_PORTAL_DATA_DIR",
    "NEXORA_PORTAL_ORIGIN",
    "NEXORA_PORTAL_SECURE_COOKIES",
    "scrypt$",
    "portal.json",
  ];
  for (const file of files) {
    const text = await fs.readFile(path.join(assetsDir, file), "utf8");
    for (const needle of needles) {
      assert.ok(!text.includes(needle), `${file} must not contain "${needle}"`);
    }
  }

  // The local `.env` (if present) must not appear in any emitted file either.
  const localEnv = await fs.readFile(path.join(WEBSITE_DIR, ".env"), "utf8").catch(() => "");
  const secret = /^NEXORA_PORTAL_OWNER_PASSWORD=(.+)$/m.exec(localEnv)?.[1]?.trim();
  if (secret) {
    for (const file of files) {
      const text = await fs.readFile(path.join(assetsDir, file), "utf8");
      assert.ok(!text.includes(secret), `${file} must not contain the configured password`);
    }
    const indexHtml = await fs.readFile(path.join(WEBSITE_DIR, "dist", "index.html"), "utf8");
    assert.ok(!indexHtml.includes(secret));
  }
});

/* ──────────────────────────── CLI: --from-env ────────────────────────── */

test("the CLI provisions and rotates from the environment file", async () => {
  const dir = await tempDir("nexora-portal-cli-env-");
  const envFile = await writeEnvFile(
    dir,
    [
      `NEXORA_PORTAL_OWNER_EMAIL=${ENV_EMAIL}`,
      "NEXORA_PORTAL_OWNER_NAME=CLI Owner",
      `NEXORA_PORTAL_OWNER_PASSWORD=${ENV_PASSWORD}`,
    ].join("\n"),
  );
  const cli = path.join(WEBSITE_DIR, "server/provision-owner.mjs");

  const run = (args, env = {}) =>
    new Promise((resolve) => {
      const child = spawn(process.execPath, [cli, ...args], {
        cwd: WEBSITE_DIR,
        env: {
          ...process.env,
          NEXORA_PORTAL_DATA_DIR: path.join(dir, "data"),
          NEXORA_PORTAL_ENV_FILE: envFile,
          ...env,
        },
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
      child.stdin.end();
    });

  try {
    const created = await run(["--from-env"]);
    assert.equal(created.code, 0, created.stderr);
    assert.match(created.stdout, /provisioned from the environment/);
    assert.ok(!created.stdout.includes(ENV_PASSWORD));

    const file = JSON.parse(await fs.readFile(path.join(dir, "data", "portal.json"), "utf8"));
    assert.equal(file.users.length, 1);
    assert.equal(await verifyPassword(ENV_PASSWORD, file.users[0].passwordHash), true);

    // A second run without --rotate-password refuses and changes nothing.
    const refused = await run(["--from-env"], {
      NEXORA_PORTAL_OWNER_PASSWORD: "another-password-4321",
    });
    assert.notEqual(refused.code, 0);
    assert.match(refused.stderr, /already exists/);
    const unchanged = JSON.parse(await fs.readFile(path.join(dir, "data", "portal.json"), "utf8"));
    assert.equal(unchanged.users[0].passwordHash, file.users[0].passwordHash);

    // Explicit rotation from the environment.
    const rotated = await run(["--from-env", "--rotate-password"], {
      NEXORA_PORTAL_OWNER_PASSWORD: "rotated-from-env-8765",
    });
    assert.equal(rotated.code, 0, rotated.stderr);
    assert.match(rotated.stdout, /password rotated/i);
    assert.ok(!rotated.stdout.includes("rotated-from-env-8765"));
    assert.ok(!rotated.stderr.includes("rotated-from-env-8765"));
    const after = JSON.parse(await fs.readFile(path.join(dir, "data", "portal.json"), "utf8"));
    assert.equal(await verifyPassword("rotated-from-env-8765", after.users[0].passwordHash), true);
    assert.equal(await verifyPassword(ENV_PASSWORD, after.users[0].passwordHash), false);

    // Absent configuration is a usage error, not a crash.
    const absent = await run(["--from-env"], {
      NEXORA_PORTAL_ENV_FILE: path.join(dir, "missing.env"),
      NEXORA_PORTAL_OWNER_EMAIL: "",
      NEXORA_PORTAL_OWNER_PASSWORD: "",
    });
    assert.equal(absent.code, 2);
    assert.match(absent.stderr, /NEXORA_PORTAL_OWNER_EMAIL/);
  } finally {
    await fs.rm(dir, { recursive: true, force: true });
  }
});

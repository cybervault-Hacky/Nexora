/**
 * Unit tests for the portal's security primitives.
 *
 * Run with: `pnpm test:portal` (Node's built-in test runner, no dependency).
 *
 * These cover the pieces that are easy to get subtly wrong and impossible to
 * eyeball: password encoding and verification, the token/digest rules, the rate
 * limiter's window, and configuration fail-closed behaviour.
 */

import assert from "node:assert/strict";
import { test } from "node:test";

import {
  PASSWORD_MIN_LENGTH,
  generateCsrfToken,
  generateSessionToken,
  hashPassword,
  hashToken,
  isValidEmailShape,
  isValidSessionToken,
  normalizeEmail,
  safeEqualHex,
  validatePasswordPolicy,
  verifyPassword,
} from "../auth.mjs";
import { describeConfig, parseOrigin, resolveConfig } from "../config.mjs";
import { RateLimiter, rule } from "../rate-limit.mjs";

/* ───────────────────────────── passwords ─────────────────────────────── */

test("a password hashes to a self-describing scrypt string with a random salt", async () => {
  const first = await hashPassword("correct horse battery staple");
  const second = await hashPassword("correct horse battery staple");

  const parts = first.split("$");
  assert.equal(parts.length, 6);
  assert.equal(parts[0], "scrypt");
  assert.equal(parts[1], "32768");
  assert.equal(parts[2], "8");
  assert.equal(parts[3], "1");
  // Per-hash salt: the same password must not produce the same digest.
  assert.notEqual(first, second);
  assert.ok(!first.includes("correct horse"), "the hash must not contain the password");
});

test("verification accepts the right password and rejects a wrong one", async () => {
  const hash = await hashPassword("a-long-enough-password");
  assert.equal(await verifyPassword("a-long-enough-password", hash), true);
  assert.equal(await verifyPassword("a-long-enough-passwore", hash), false);
  assert.equal(await verifyPassword("", hash), false);
});

test("verification fails closed on a malformed or tampered hash", async () => {
  const hash = await hashPassword("a-long-enough-password");

  assert.equal(await verifyPassword("a-long-enough-password", ""), false);
  assert.equal(await verifyPassword("a-long-enough-password", "not-a-hash"), false);
  assert.equal(await verifyPassword("a-long-enough-password", hash.replace("scrypt", "bcrypt")), false);
  assert.equal(await verifyPassword("a-long-enough-password", hash.replace("$32768$", "$1024$")), false);
  // A non-power-of-two N is rejected rather than attempted.
  assert.equal(await verifyPassword("a-long-enough-password", hash.replace("$32768$", "$30000$")), false);
  // Absurd cost parameters are rejected instead of letting a record DoS the process.
  assert.equal(await verifyPassword("a-long-enough-password", hash.replace("$32768$", "$2097152$")), false);
  assert.equal(await verifyPassword("a-long-enough-password", hash.replace("$8$", "$999$")), false);
});

test("password policy enforces a minimum length", () => {
  assert.equal(validatePasswordPolicy("short"), `Password must be at least ${PASSWORD_MIN_LENGTH} characters.`);
  assert.equal(validatePasswordPolicy("x".repeat(PASSWORD_MIN_LENGTH)), null);
  assert.equal(validatePasswordPolicy("x".repeat(257)), "Password must be at most 256 characters.");
  assert.equal(validatePasswordPolicy(undefined), "Password is required.");
});

/* ─────────────────────────────── tokens ──────────────────────────────── */

test("session tokens are 32 random bytes, hex encoded", () => {
  const first = generateSessionToken();
  const second = generateSessionToken();
  assert.match(first, /^[0-9a-f]{64}$/);
  assert.notEqual(first, second);
  assert.equal(isValidSessionToken(first), true);
  assert.equal(isValidSessionToken(first.toUpperCase()), false);
  assert.equal(isValidSessionToken("abc"), false);
  assert.equal(isValidSessionToken(undefined), false);
});

test("only the digest of a token is stored", () => {
  const raw = generateSessionToken();
  const digest = hashToken(raw);
  assert.match(digest, /^[0-9a-f]{64}$/);
  assert.notEqual(digest, raw);
  assert.equal(hashToken(raw), digest, "hashing is deterministic");
});

test("CSRF tokens are independent random values and compare in constant time", () => {
  const first = generateCsrfToken();
  const second = generateCsrfToken();
  assert.notEqual(first, second);
  assert.equal(safeEqualHex(first, first), true);
  assert.equal(safeEqualHex(first, second), false);
  assert.equal(safeEqualHex(first, first.slice(0, -1)), false);
  assert.equal(safeEqualHex("", ""), false);
  assert.equal(safeEqualHex(undefined, first), false);
});

test("emails are normalised, and the shape check rejects the obviously wrong", () => {
  assert.equal(normalizeEmail("  Owner@Example.COM "), "owner@example.com");
  assert.equal(isValidEmailShape("owner@example.com"), true);
  assert.equal(isValidEmailShape("owner@example"), false);
  assert.equal(isValidEmailShape("no-at-sign.example.com"), false);
  assert.equal(isValidEmailShape("two@@example.com"), false);
  assert.equal(isValidEmailShape("with space@example.com"), false);
});

/* ───────────────────────────── rate limiting ─────────────────────────── */

test("the limiter blocks once the window is exhausted and reports a retry", () => {
  const limiter = new RateLimiter({ account: rule(3, 60) });
  const now = 1_000_000;

  assert.equal(limiter.check("a", ["account"], now).allowed, true);
  limiter.record("a", ["account"], now);
  limiter.record("a", ["account"], now + 1);
  limiter.record("a", ["account"], now + 2);

  const blocked = limiter.check("a", ["account"], now + 3);
  assert.equal(blocked.allowed, false);
  assert.ok(blocked.retryAfterSeconds > 0);

  // A different key is unaffected.
  assert.equal(limiter.check("b", ["account"], now + 3).allowed, true);

  // The window expires.
  assert.equal(limiter.check("a", ["account"], now + 61_000).allowed, true);
});

test("a successful sign-in can clear the account key without clearing the address key", () => {
  const limiter = new RateLimiter({ account: rule(3, 60), ip: rule(3, 60) });
  const now = 2_000_000;
  limiter.record("acct", ["account"], now);
  limiter.record("addr", ["ip"], now);

  limiter.reset("acct", ["account"]);

  assert.equal(limiter.count("acct", "account", now), 0);
  assert.equal(limiter.count("addr", "ip", now), 1);
});

/* ──────────────────────────── configuration ──────────────────────────── */

test("development defaults are usable without any environment", () => {
  const config = resolveConfig({}, { cwd: "/tmp/website" });
  assert.equal(config.usable, true);
  assert.equal(config.environment, "development");
  assert.equal(config.origin, null);
  assert.equal(config.secureCookies, false);
  assert.deepEqual(config.fatal, []);
});

test("production refuses to start without an origin and secure cookies", () => {
  const config = resolveConfig(
    { NEXORA_PORTAL_ENV: "production" },
    { cwd: "/tmp/website" },
  );
  assert.equal(config.usable, false);
  const variables = config.fatal.map((problem) => problem.variable);
  assert.ok(variables.includes("NEXORA_PORTAL_ORIGIN"));
  // Secure cookies default to on in production, so this one is satisfied.
  assert.ok(!variables.includes("NEXORA_PORTAL_SECURE_COOKIES"));
});

test("production rejects an http origin, since a Secure cookie could never be set", () => {
  const config = resolveConfig(
    { NEXORA_PORTAL_ENV: "production", NEXORA_PORTAL_ORIGIN: "http://portal.example.com" },
    { cwd: "/tmp/website" },
  );
  assert.equal(config.usable, false);
  assert.ok(config.fatal.some((problem) => problem.variable === "NEXORA_PORTAL_ORIGIN"));
});

test("production rejects secure cookies being switched off", () => {
  const config = resolveConfig(
    {
      NEXORA_PORTAL_ENV: "production",
      NEXORA_PORTAL_ORIGIN: "https://portal.example.com",
      NEXORA_PORTAL_SECURE_COOKIES: "false",
    },
    { cwd: "/tmp/website" },
  );
  assert.equal(config.usable, false);
  assert.ok(config.fatal.some((problem) => problem.variable === "NEXORA_PORTAL_SECURE_COOKIES"));
});

test("a valid production configuration is accepted", () => {
  const config = resolveConfig(
    {
      NEXORA_PORTAL_ENV: "production",
      NEXORA_PORTAL_ORIGIN: "https://portal.example.com",
    },
    { cwd: "/tmp/website" },
  );
  assert.equal(config.usable, true);
  assert.equal(config.origin, "https://portal.example.com");
  assert.equal(config.secureCookies, true);
});

test("origin parsing rejects anything ambiguous", () => {
  assert.equal(parseOrigin("https://portal.example.com").origin, "https://portal.example.com");
  assert.equal(parseOrigin("https://portal.example.com/").origin, "https://portal.example.com");
  assert.equal(parseOrigin("https://portal.example.com/path").origin, null);
  assert.equal(parseOrigin("https://user:pass@portal.example.com").origin, null);
  assert.equal(parseOrigin("ftp://portal.example.com").origin, null);
  assert.equal(parseOrigin("").origin, null);
});

test("session lifetimes are bounded", () => {
  assert.ok(
    resolveConfig({ NEXORA_PORTAL_SESSION_TTL_SECONDS: "10" }, { cwd: "/tmp" }).fatal.length > 0,
    "a ten-second absolute lifetime is rejected",
  );
  assert.ok(
    resolveConfig(
      { NEXORA_PORTAL_SESSION_TTL_SECONDS: "600", NEXORA_PORTAL_SESSION_IDLE_SECONDS: "900" },
      { cwd: "/tmp" },
    ).fatal.length > 0,
    "an idle window longer than the absolute lifetime is rejected",
  );
});

test("the configuration description never contains a secret or a path", () => {
  const config = resolveConfig(
    {
      NEXORA_PORTAL_ENV: "production",
      NEXORA_PORTAL_ORIGIN: "https://portal.example.com",
      NEXORA_PORTAL_DATA_DIR: "/var/lib/nexora-portal",
    },
    { cwd: "/tmp" },
  );
  const described = JSON.stringify(describeConfig(config));
  assert.ok(!described.includes("/var/lib"), described);
  assert.ok(!described.toLowerCase().includes("secret"));
  assert.ok(!described.toLowerCase().includes("password"));
});

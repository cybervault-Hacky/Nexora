/**
 * Developer Portal — credentials, tokens, and session lifecycle.
 *
 * Cryptography is **not invented here**: passwords use `node:crypto`'s
 * `scrypt`, which is OpenSSL's scrypt (RFC 7914) with its standard parameters,
 * and session tokens come from `crypto.randomBytes` (the platform CSPRNG).
 * What this module defines is only the *encoding* and the *lifecycle rules*.
 *
 * Password encoding — a self-describing PHC-style string:
 *
 *     scrypt$<N>$<r>$<p>$<salt base64>$<key base64>
 *
 * Storing the parameters with the hash is what allows them to be raised later
 * without invalidating existing accounts.
 *
 * Session encoding: the raw token (32 random bytes, hex) is the cookie value.
 * The store keeps only `sha256(raw)`, so a stolen data file cannot be replayed
 * as a session. Verification is a constant-time comparison of digests.
 */

import {
  createHash,
  randomBytes,
  scrypt as scryptCallback,
  timingSafeEqual,
} from "node:crypto";
import { promisify } from "node:util";

const scrypt = promisify(scryptCallback);

/** Cost parameters. `N=2^15, r=8, p=1` ≈ 32 MiB and ~50–100 ms per hash. */
export const SCRYPT_PARAMS = Object.freeze({ N: 32768, r: 8, p: 1, keyLength: 32, saltLength: 16 });

/** `maxmem` must exceed 128 * N * r (32 MiB here), so allow headroom. */
const SCRYPT_MAXMEM = 96 * 1024 * 1024;

/** Minimum password policy for the single owner account. */
export const PASSWORD_MIN_LENGTH = 12;
export const PASSWORD_MAX_LENGTH = 256;

export const SESSION_TOKEN_BYTES = 32;
export const SESSION_TOKEN_PATTERN = /^[0-9a-f]{64}$/;

/** The session cookie name. Deliberately not a generic `session`/`sid`. */
export const SESSION_COOKIE = "nexora_portal_session";
export const CSRF_HEADER = "x-portal-csrf";

/* ────────────────────────────── passwords ────────────────────────────── */

/**
 * @param {string} password
 * @returns {Promise<string>} encoded hash
 */
export async function hashPassword(password) {
  if (typeof password !== "string") throw new TypeError("password must be a string");
  const salt = randomBytes(SCRYPT_PARAMS.saltLength);
  const key = await scrypt(password.normalize("NFKC"), salt, SCRYPT_PARAMS.keyLength, {
    N: SCRYPT_PARAMS.N,
    r: SCRYPT_PARAMS.r,
    p: SCRYPT_PARAMS.p,
    maxmem: SCRYPT_MAXMEM,
  });
  return [
    "scrypt",
    SCRYPT_PARAMS.N,
    SCRYPT_PARAMS.r,
    SCRYPT_PARAMS.p,
    salt.toString("base64"),
    Buffer.from(key).toString("base64"),
  ].join("$");
}

/**
 * Verifies a password against an encoded hash. Never throws on malformed input
 * — a corrupt record is a failed verification, not a crash.
 *
 * @param {string} password
 * @param {string} encoded
 * @returns {Promise<boolean>}
 */
export async function verifyPassword(password, encoded) {
  if (typeof password !== "string" || typeof encoded !== "string") return false;
  const parts = encoded.split("$");
  if (parts.length !== 6 || parts[0] !== "scrypt") return false;

  const N = Number.parseInt(parts[1], 10);
  const r = Number.parseInt(parts[2], 10);
  const p = Number.parseInt(parts[3], 10);
  if (!Number.isInteger(N) || !Number.isInteger(r) || !Number.isInteger(p)) return false;
  // Refuse absurd parameters from a tampered record: they would either DoS the
  // process or weaken verification.
  if (N < 4096 || N > 1 << 20 || r < 1 || r > 32 || p < 1 || p > 8) return false;
  if (N & (N - 1)) return false; // must be a power of two

  let salt;
  let expected;
  try {
    salt = Buffer.from(parts[4], "base64");
    expected = Buffer.from(parts[5], "base64");
  } catch {
    return false;
  }
  if (salt.length < 8 || expected.length < 16 || expected.length > 128) return false;

  try {
    const key = Buffer.from(
      await scrypt(password.normalize("NFKC"), salt, expected.length, {
        N,
        r,
        p,
        maxmem: SCRYPT_MAXMEM,
      }),
    );
    if (key.length !== expected.length) return false;
    return timingSafeEqual(key, expected);
  } catch {
    return false;
  }
}

/**
 * A verification against a throwaway hash, used when no account matches so that
 * an unknown email takes as long as a wrong password. (Timing is not the only
 * enumeration defence — the response is identical either way — but it removes
 * the cheapest signal.)
 */
export async function dummyVerify(password) {
  await verifyPassword(
    typeof password === "string" ? password : "",
    // A fixed, valid-shaped hash of a random value nobody knows.
    "scrypt$32768$8$1$AAAAAAAAAAAAAAAAAAAAAA==$" +
      "Y2FsaWJyYXRpb24tb25seS1ub3QtYS1wYXNzd29yZA==",
  );
  return false;
}

/** Password policy. Returns an error message, or `null` when acceptable. */
export function validatePasswordPolicy(password) {
  if (typeof password !== "string") return "Password is required.";
  if (password.length < PASSWORD_MIN_LENGTH) {
    return `Password must be at least ${PASSWORD_MIN_LENGTH} characters.`;
  }
  if (password.length > PASSWORD_MAX_LENGTH) {
    return `Password must be at most ${PASSWORD_MAX_LENGTH} characters.`;
  }
  return null;
}

/* ──────────────────────────────── tokens ─────────────────────────────── */

/** A URL-safe session token (64 hex characters). */
export function generateSessionToken() {
  return randomBytes(SESSION_TOKEN_BYTES).toString("hex");
}

/** `sha256(raw)` as hex — what the store keeps. */
export function hashToken(raw) {
  return createHash("sha256").update(String(raw), "utf8").digest("hex");
}

/** A short random token for the CSRF double-submit check. */
export function generateCsrfToken() {
  return randomBytes(24).toString("hex");
}

/** Constant-time comparison of two hex digests of equal length. */
export function safeEqualHex(a, b) {
  if (typeof a !== "string" || typeof b !== "string") return false;
  if (a.length !== b.length || a.length === 0) return false;
  const left = Buffer.from(a, "utf8");
  const right = Buffer.from(b, "utf8");
  if (left.length !== right.length) return false;
  return timingSafeEqual(left, right);
}

export function isValidSessionToken(raw) {
  return typeof raw === "string" && SESSION_TOKEN_PATTERN.test(raw);
}

/** Emails are stored trimmed and lowercased; the CLI and login agree on this. */
export function normalizeEmail(raw) {
  if (typeof raw !== "string") return "";
  return raw.trim().toLowerCase();
}

/**
 * Deliberately permissive: the portal has one account created out of band by
 * the CLI, so this is a sanity check, not an identity proof. It exists so a
 * typo in `--email` is caught rather than becoming an account.
 */
export function isValidEmailShape(raw) {
  if (typeof raw !== "string") return false;
  const value = normalizeEmail(raw);
  if (value.length < 3 || value.length > 254) return false;
  if (/\s/.test(value)) return false;
  return /^[^@]+@[^@.]+(\.[^@.]+)+$/.test(value);
}

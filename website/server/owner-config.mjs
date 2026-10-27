/**
 * Developer Portal — the Owner account as described by the environment.
 *
 * This module is the **only** place that reads the Owner's configured password,
 * and it does three things with it: validates the configured values, keeps them
 * in a local decision object, and hands them to the existing scrypt hasher. It
 * never returns the password to a caller that could log it, never puts it in
 * `resolveConfig()` (whose object is logged and described), and never writes it
 * anywhere.
 *
 * Bootstrapping rules, in order of importance:
 *
 * 1. **Creation only.** On startup an account is created **only** when the store
 *    has no owner at all. An existing owner is never overwritten, so a stale
 *    `.env` cannot silently reset a live credential on restart.
 * 2. **No silent rotation.** Changing the password requires the explicit
 *    `provision-owner --from-env --rotate-password` command, which also revokes
 *    every session.
 * 3. **No account claiming.** This runs in the server process from server-side
 *    configuration. There is still no HTTP endpoint that creates or changes an
 *    account.
 * 4. **Nothing echoed.** Every message this module produces names the variable,
 *    never the value.
 */

import {
  hashPassword,
  isValidEmailShape,
  normalizeEmail,
  validatePasswordPolicy,
} from "./auth.mjs";

/** The environment variables this feature reads. */
export const OWNER_ENV_KEYS = Object.freeze([
  "NEXORA_PORTAL_OWNER_EMAIL",
  "NEXORA_PORTAL_OWNER_NAME",
  "NEXORA_PORTAL_OWNER_PASSWORD",
]);

const MAX_NAME_LENGTH = 128;

/**
 * @typedef {object} OwnerDecision
 * @property {"absent" | "invalid" | "valid"} status
 * @property {string | null} email         Normalised. `null` unless valid.
 * @property {string | null} displayName   `null` unless valid.
 * @property {string | null} password      Raw. `null` unless valid. Never log this.
 * @property {string[]} problems           Safe messages: variable names, never values.
 */

/**
 * Reads and validates the Owner configuration.
 *
 * `absent` means none of the three variables is set — the portal is simply not
 * configured for environment provisioning, which is a valid state. `invalid`
 * means something *is* configured but cannot be used; the caller reports the
 * problems and skips provisioning rather than guessing.
 *
 * @param {Record<string, string | undefined>} env
 * @returns {OwnerDecision}
 */
export function readOwnerEnvironment(env = process.env) {
  /** @type {string[]} */
  const problems = [];
  const rawEmail = typeof env.NEXORA_PORTAL_OWNER_EMAIL === "string" ? env.NEXORA_PORTAL_OWNER_EMAIL.trim() : "";
  const rawName = typeof env.NEXORA_PORTAL_OWNER_NAME === "string" ? env.NEXORA_PORTAL_OWNER_NAME.trim() : "";
  const rawPassword = typeof env.NEXORA_PORTAL_OWNER_PASSWORD === "string" ? env.NEXORA_PORTAL_OWNER_PASSWORD : "";

  if (rawEmail === "" && rawName === "" && rawPassword === "") {
    return { status: "absent", email: null, displayName: null, password: null, problems };
  }

  const email = normalizeEmail(rawEmail);
  if (email === "") {
    problems.push("NEXORA_PORTAL_OWNER_EMAIL is required when any owner variable is set.");
  } else if (!isValidEmailShape(email)) {
    problems.push("NEXORA_PORTAL_OWNER_EMAIL is not a valid email address.");
  }

  if (rawPassword === "") {
    problems.push("NEXORA_PORTAL_OWNER_PASSWORD is required when any owner variable is set.");
  } else {
    const policyProblem = validatePasswordPolicy(rawPassword);
    if (policyProblem !== null) problems.push(`NEXORA_PORTAL_OWNER_PASSWORD: ${policyProblem}`);
  }

  if (rawName !== "" && rawName.length > MAX_NAME_LENGTH) {
    problems.push(`NEXORA_PORTAL_OWNER_NAME must be at most ${MAX_NAME_LENGTH} characters.`);
  }

  if (problems.length > 0) {
    return { status: "invalid", email: null, displayName: null, password: null, problems };
  }

  const displayName = (rawName !== "" ? rawName : email.split("@")[0] || "Owner").slice(0, MAX_NAME_LENGTH);
  return { status: "valid", email, displayName, password: rawPassword, problems };
}

/**
 * Creates the owner account from configuration, if — and only if — the store has
 * no owner yet.
 *
 * @param {object} options
 * @param {import("./store.mjs").Store} options.store
 * @param {OwnerDecision} options.decision
 * @returns {Promise<{ outcome: "skipped-absent" | "skipped-invalid" | "created" | "exists" | "exists-other-email" }>}
 */
export async function ensureOwnerFromEnvironment({ store, decision }) {
  if (decision.status === "absent") return { outcome: "skipped-absent" };
  if (decision.status === "invalid") return { outcome: "skipped-invalid" };
  if (decision.email === null || decision.displayName === null || decision.password === null) {
    return { outcome: "skipped-invalid" };
  }

  const existing = store.listUsers().find((user) => user.role === "owner");
  if (existing) {
    // An owner exists. Never touch its password here: rotation is the explicit
    // command, and a restart must not be able to change a credential.
    return {
      outcome:
        normalizeEmail(existing.email) === decision.email ? "exists" : "exists-other-email",
    };
  }

  const passwordHash = await hashPassword(decision.password);
  await store.createOwner({
    email: decision.email,
    displayName: decision.displayName,
    passwordHash,
  });
  await store.recordAudit("owner_provisioned_from_environment", null);
  return { outcome: "created" };
}

/**
 * Explicit rotation, used by `provision-owner --from-env --rotate-password`.
 *
 * Rotating a password revokes every session: a credential change that leaves a
 * stolen cookie working would not be a credential change.
 *
 * @param {object} options
 * @param {import("./store.mjs").Store} options.store
 * @param {OwnerDecision} options.decision
 * @returns {Promise<{ outcome: "created" | "rotated" | "unchanged" | "conflict" }>}
 */
export async function rotateOwnerFromEnvironment({ store, decision }) {
  if (decision.status !== "valid" || decision.email === null || decision.password === null) {
    throw new Error("owner configuration is not valid");
  }

  const existing = store.listUsers().find((user) => user.role === "owner");
  if (!existing) {
    await ensureOwnerFromEnvironment({ store, decision });
    return { outcome: "created" };
  }
  if (normalizeEmail(existing.email) !== decision.email) {
    // Do not create a second account, and do not rename the existing one.
    return { outcome: "conflict" };
  }

  const passwordHash = await hashPassword(decision.password);
  await store.setPasswordHash(existing.id, passwordHash);
  await store.dropSessionsForUser(existing.id);
  await store.recordAudit("password_rotated", existing.id);
  return { outcome: "rotated" };
}

/**
 * Startup report. The caller logs these strings; none of them contains a value.
 *
 * @param {OwnerDecision} decision
 * @param {{ outcome: string }} result
 * @returns {{ level: "info" | "warn" | "error", message: string }[]}
 */
export function describeOwnerBootstrap(decision, result) {
  if (decision.status === "invalid") {
    return [
      {
        level: "error",
        message:
          "owner configuration is invalid, so no owner account was provisioned. " +
          "Fix these in website/.env (or the service environment) and restart:",
      },
      ...decision.problems.map((problem) => ({ level: /** @type {const} */ ("error"), message: `  - ${problem}` })),
    ];
  }

  switch (result.outcome) {
    case "created":
      return [
        {
          level: "info",
          message:
            "owner account created from the environment configuration (password stored as a scrypt hash).",
        },
      ];
    case "exists":
      return [
        {
          level: "info",
          message:
            "an owner account already exists and matches the configured email; its password was left unchanged. " +
            "Use `pnpm provision-owner --from-env --rotate-password` to change it.",
        },
      ];
    case "exists-other-email":
      return [
        {
          level: "warn",
          message:
            "an owner account already exists with a different email than NEXORA_PORTAL_OWNER_EMAIL. " +
            "Nothing was changed; this server keeps the account it has.",
        },
      ];
    case "skipped-invalid":
      return [];
    default:
      return [
        {
          level: "info",
          message:
            "no owner variables are configured, so the portal cannot provision an owner. " +
            "Create website/.env from website/.env.example, or run `pnpm provision-owner`.",
        },
      ];
  }
}

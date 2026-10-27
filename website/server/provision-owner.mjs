#!/usr/bin/env node
/**
 * Provision the Owner / Super Admin account.
 *
 * This is the **only** way an account can be created. It is an offline command
 * that must be run by someone with shell access to the server; it is not
 * exposed over HTTP, so there is no setup URL for a stranger to claim.
 *
 *   node server/provision-owner.mjs --email owner@example.com
 *
 * The password is read from the terminal with echo disabled (or from stdin when
 * piped, which is how a deployment script supplies it). It is deliberately
 * **not** accepted as a command-line argument: `argv` is visible to every
 * process on the machine via `ps`.
 *
 * What it writes: a scrypt hash, an email, a display name, and a role. Never a
 * plaintext password, never a token.
 */

import process from "node:process";

import { hashPassword, isValidEmailShape, normalizeEmail, validatePasswordPolicy } from "./auth.mjs";
import { resolveConfig } from "./config.mjs";
import { loadEnvFile } from "./env-file.mjs";
import { readOwnerEnvironment, rotateOwnerFromEnvironment } from "./owner-config.mjs";
import { openStore } from "./store.mjs";

const EXIT_USAGE = 2;
const EXIT_FAILED = 1;

/** @param {string} message */
function fail(message, code = EXIT_FAILED) {
  console.error(`provision-owner: ${message}`);
  process.exit(code);
}

/** Minimal argument parsing: no dependency, no surprises. */
function parseArgs(argv) {
  /** @type {{ email?: string, name?: string, dataDir?: string, rotate: boolean, fromEnv: boolean, help: boolean }} */
  const out = { rotate: false, fromEnv: false, help: false };
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--help" || arg === "-h") out.help = true;
    else if (arg === "--rotate-password") out.rotate = true;
    else if (arg === "--from-env") out.fromEnv = true;
    else if (arg === "--email") out.email = argv[++index];
    else if (arg === "--name") out.name = argv[++index];
    else if (arg === "--data-dir") out.dataDir = argv[++index];
    else if (arg.startsWith("--email=")) out.email = arg.slice("--email=".length);
    else if (arg.startsWith("--name=")) out.name = arg.slice("--name=".length);
    else if (arg.startsWith("--data-dir=")) out.dataDir = arg.slice("--data-dir=".length);
    else fail(`unknown argument "${arg}" (try --help)`, EXIT_USAGE);
  }
  return out;
}

const USAGE = `Provision the Nexora Developer Portal owner account.

Usage:
  node server/provision-owner.mjs --email <owner@example.com> [options]
  node server/provision-owner.mjs --from-env [--rotate-password]

Options:
  --email <address>     Owner email. Stored lowercased.
  --name <name>         Display name. Defaults to the local part of the email.
  --data-dir <path>     Override NEXORA_PORTAL_DATA_DIR.
  --from-env            Read the owner from website/.env (or the process
                        environment): NEXORA_PORTAL_OWNER_EMAIL,
                        NEXORA_PORTAL_OWNER_NAME, NEXORA_PORTAL_OWNER_PASSWORD.
  --rotate-password     Replace the password of the existing owner account and
                        revoke every live session.
  -h, --help            Show this message.

Without --from-env the password is read from the terminal (hidden) or from
stdin. It is never accepted as an argument, never echoed, and never written
anywhere except as a scrypt hash. There is no HTTP endpoint that creates or
changes accounts.`;

/** Reads a line from stdin with terminal echo disabled when possible. */
async function readSecret(prompt) {
  if (!process.stdin.isTTY) {
    const chunks = [];
    for await (const chunk of process.stdin) chunks.push(Buffer.from(chunk));
    return Buffer.concat(chunks).toString("utf8").split("\n")[0] ?? "";
  }

  return await new Promise((resolve, reject) => {
    process.stdout.write(prompt);
    const stdin = process.stdin;
    stdin.setRawMode(true);
    stdin.setEncoding("utf8");
    stdin.resume();
    let value = "";

    const cleanup = () => {
      stdin.setRawMode(false);
      stdin.pause();
      stdin.removeListener("data", onData);
      process.stdout.write("\n");
    };

    /** @param {string} chunk */
    const onData = (chunk) => {
      for (const char of chunk) {
        if (char === "\u0003") {
          cleanup();
          reject(new Error("cancelled"));
          return;
        }
        if (char === "\r" || char === "\n") {
          cleanup();
          resolve(value);
          return;
        }
        if (char === "\u007f" || char === "\b") {
          if (value.length > 0) {
            value = value.slice(0, -1);
            process.stdout.write("\b \b");
          }
          continue;
        }
        if (char >= " ") {
          value += char;
          process.stdout.write("*");
        }
      }
    };

    stdin.on("data", onData);
  });
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  if (args.help) {
    console.log(USAGE);
    return;
  }

  // `website/.env` is loaded here too, so the documented rotation procedure is
  // "edit .env, run --from-env --rotate-password" without pasting a password.
  const envFile = process.env.NEXORA_PORTAL_ENV_FILE?.trim() || `${process.cwd()}/.env`;
  const envResult = loadEnvFile({ env: process.env, filePath: envFile });
  if (envResult.problem !== null) {
    console.warn(`provision-owner: .env ${envResult.problem}; using the process environment.`);
  }

  const fromEnv = readOwnerEnvironment(process.env);

  if (args.fromEnv) {
    if (fromEnv.status === "absent") {
      fail(
        "no owner variables are set. Add NEXORA_PORTAL_OWNER_EMAIL and NEXORA_PORTAL_OWNER_PASSWORD to website/.env.",
        EXIT_USAGE,
      );
    }
    if (fromEnv.status === "invalid") {
      for (const problem of fromEnv.problems) console.error(`provision-owner: ${problem}`);
      fail("the owner configuration is invalid", EXIT_FAILED);
    }
    if (args.email !== undefined && normalizeEmail(args.email) !== fromEnv.email) {
      fail("--email does not match NEXORA_PORTAL_OWNER_EMAIL in the environment", EXIT_USAGE);
    }
    if (args.name !== undefined) {
      fail("--name cannot be combined with --from-env; set NEXORA_PORTAL_OWNER_NAME instead", EXIT_USAGE);
    }

    const config = resolveConfig(
      { ...process.env, ...(args.dataDir ? { NEXORA_PORTAL_DATA_DIR: args.dataDir } : {}) },
      { cwd: process.cwd() },
    );
    if (!config.usable) {
      for (const problem of config.fatal) {
        console.error(`provision-owner: ${problem.variable} ${problem.message}`);
      }
      fail("configuration is incomplete", EXIT_FAILED);
    }

    let envStore;
    try {
      envStore = await openStore(config.dataDir);
    } catch (error) {
      fail(`cannot open the data store (${error?.message ?? "unknown error"})`);
    }

    const existing = envStore.listUsers().find((user) => user.role === "owner");
    if (existing && !args.rotate) {
      fail(
        "an owner account already exists. Pass --rotate-password with --from-env to change its password; " +
          "startup provisioning never overwrites it.",
      );
    }

    const result = await rotateOwnerFromEnvironment({ store: envStore, decision: fromEnv });
    if (result.outcome === "conflict") {
      fail(
        "an owner account already exists with a different email than NEXORA_PORTAL_OWNER_EMAIL. " +
          "Nothing was changed.",
      );
    }
    if (result.outcome === "created") {
      console.log(`Owner account provisioned from the environment: ${fromEnv.email} (role: owner).`);
      console.log("Start the portal with `pnpm portal`, then sign in at /admin.");
      return;
    }
    console.log(`Owner password rotated from the environment for ${fromEnv.email}. Existing sessions were revoked.`);
    return;
  }

  const email = normalizeEmail(args.email ?? "");
  if (!isValidEmailShape(email)) {
    fail("--email is required and must look like an address (owner@example.com)", EXIT_USAGE);
  }

  const config = resolveConfig(
    { ...process.env, ...(args.dataDir ? { NEXORA_PORTAL_DATA_DIR: args.dataDir } : {}) },
    { cwd: process.cwd() },
  );
  if (!config.usable) {
    for (const problem of config.fatal) {
      console.error(`provision-owner: ${problem.variable} ${problem.message}`);
    }
    fail("configuration is incomplete", EXIT_FAILED);
  }

  let store;
  try {
    store = await openStore(config.dataDir);
  } catch (error) {
    fail(`cannot open the data store (${error?.message ?? "unknown error"})`);
  }

  const existingOwner = store.listUsers().find((user) => user.role === "owner");
  if (existingOwner && !args.rotate) {
    fail(
      "an owner account already exists. Sign in, or pass --rotate-password to replace its password. " +
        "This command never creates a second account.",
    );
  }

  let password = "";
  if (process.stdin.isTTY) {
    password = await readSecret("Password: ");
    const confirmation = await readSecret("Confirm password: ");
    if (password !== confirmation) fail("passwords did not match");
  } else {
    password = await readSecret("");
  }
  password = password.replace(/\r$/, "");

  const policyProblem = validatePasswordPolicy(password);
  if (policyProblem !== null) fail(policyProblem);

  const passwordHash = await hashPassword(password);
  // The plaintext is a local that goes out of scope here; it is never logged
  // and never persisted.

  if (existingOwner) {
    await store.setPasswordHash(existingOwner.id, passwordHash);
    // Rotating the password must also invalidate live sessions, or a stolen
    // cookie would outlive the credential change.
    await store.dropSessionsForUser(existingOwner.id);
    await store.recordAudit("password_rotated", existingOwner.id);
    console.log(`Owner password rotated for ${existingOwner.email}. Existing sessions were revoked.`);
    return;
  }

  const displayName = (args.name ?? email.split("@")[0] ?? "Owner").trim().slice(0, 128) || "Owner";
  const user = await store.createOwner({ email, displayName, passwordHash });
  console.log(`Owner account provisioned: ${user.email} (role: ${user.role}).`);
  console.log("Start the portal with `pnpm portal`, then sign in at /admin.");
}

try {
  await main();
} catch (error) {
  if (error instanceof Error && error.message === "cancelled") {
    console.error("\nprovision-owner: cancelled");
    process.exit(EXIT_FAILED);
  }
  fail(error instanceof Error ? error.message : "unknown failure");
}

/**
 * Developer Portal — `.env` loading (server side only).
 *
 * The portal reads its configuration from a git-ignored `website/.env` file (or
 * from the real process environment, which always wins). This is the only
 * `.env` reader in the website: it lives under `server/`, is imported by the two
 * server entry points, and is never imported by anything in `src/`, so no value
 * it loads can reach the browser bundle.
 *
 * Why not a dependency: the format the portal needs is a handful of
 * `KEY=value` lines, and a dotenv package would be a new dependency for
 * something that must also be *auditable* — a secret loader is exactly the wrong
 * place for transitive code. The parser below supports what the documented
 * template uses and nothing more:
 *
 *   KEY=value            unquoted, trimmed, ` # comment` allowed after it
 *   KEY="value"          double quotes; \n, \r, \t and \" unescaped
 *   KEY='value'          single quotes; taken literally
 *   export KEY=value     the `export ` prefix is ignored
 *   # comment            whole-line comments, and blank lines
 *
 * Values are never logged by this module, and a variable that is already set in
 * `process.env` is never overwritten: an operator who exports a value in the
 * service's environment controls it, and a stale `.env` cannot quietly replace
 * it.
 */

import fs from "node:fs";

/**
 * Parses dotenv text into a plain object.
 *
 * Duplicate keys: the last one wins, matching every other dotenv
 * implementation. Malformed lines are skipped rather than throwing — a stray
 * line must not stop the server from starting and, more importantly, must not
 * print the file's contents in an error message.
 *
 * @param {string} text
 * @returns {Record<string, string>}
 */
export function parseEnvFile(text) {
  /** @type {Record<string, string>} */
  const out = {};
  if (typeof text !== "string") return out;

  const lines = text.replace(/^\uFEFF/, "").split(/\r?\n/);
  for (const rawLine of lines) {
    const line = rawLine.trim();
    if (line === "" || line.startsWith("#")) continue;

    const withoutExport = line.startsWith("export ") ? line.slice("export ".length).trimStart() : line;
    const separator = withoutExport.indexOf("=");
    if (separator < 1) continue;

    const key = withoutExport.slice(0, separator).trim();
    if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(key)) continue;

    let value = withoutExport.slice(separator + 1).trim();

    if (value.startsWith('"')) {
      const end = findClosingQuote(value, '"');
      if (end === -1) continue;
      value = value
        .slice(1, end)
        .replace(/\\n/g, "\n")
        .replace(/\\r/g, "\r")
        .replace(/\\t/g, "\t")
        .replace(/\\"/g, '"');
    } else if (value.startsWith("'")) {
      const end = findClosingQuote(value, "'");
      if (end === -1) continue;
      value = value.slice(1, end);
    } else {
      // Unquoted: an unescaped ` #` starts a trailing comment.
      const comment = value.search(/\s#/);
      if (comment !== -1) value = value.slice(0, comment);
      value = value.trim();
    }

    out[key] = value;
  }

  return out;
}

/** Index of the closing quote, honouring backslash escapes for `"`. */
function findClosingQuote(value, quote) {
  for (let index = 1; index < value.length; index += 1) {
    if (quote === '"' && value[index] === "\\") {
      index += 1;
      continue;
    }
    if (value[index] === quote) return index;
  }
  return -1;
}

/**
 * Loads a `.env` file into `process.env` without overriding existing variables.
 *
 * @param {{ env?: Record<string, string | undefined>, filePath: string, fsModule?: typeof fs }} options
 * @returns {{ loaded: boolean, appliedKeys: string[], skippedKeys: string[], problem: string | null }}
 *   Key **names** only. No value is ever returned, so a caller cannot log one by
 *   accident.
 */
export function loadEnvFile({ env = process.env, filePath, fsModule = fs }) {
  /** @type {string[]} */
  const appliedKeys = [];
  /** @type {string[]} */
  const skippedKeys = [];

  let text;
  try {
    text = fsModule.readFileSync(filePath, "utf8");
  } catch (error) {
    if (error?.code === "ENOENT") {
      return { loaded: false, appliedKeys, skippedKeys, problem: null };
    }
    // Deliberately code-only: a filesystem error message can contain the path,
    // and the path is not a secret, but keeping this uniform avoids any chance
    // of a value leaking through an exotic error.
    return {
      loaded: false,
      appliedKeys,
      skippedKeys,
      problem: `could not read the environment file (${error?.code ?? "error"})`,
    };
  }

  const parsed = parseEnvFile(text);
  for (const [key, value] of Object.entries(parsed)) {
    if (env[key] !== undefined && env[key] !== "") {
      skippedKeys.push(key);
      continue;
    }
    env[key] = value;
    appliedKeys.push(key);
  }

  return { loaded: true, appliedKeys, skippedKeys, problem: null };
}

/** Filesystem permissions of a file, or `null` when it cannot be read. */
export function fileMode(filePath, fsModule = fs) {
  try {
    return fsModule.statSync(filePath).mode & 0o777;
  } catch {
    return null;
  }
}

/**
 * A `.env` that holds a password should not be world-readable. Returns a
 * warning string (never a value), or `null` when the mode is acceptable.
 */
export function envFilePermissionWarning(filePath, fsModule = fs) {
  const mode = fileMode(filePath, fsModule);
  if (mode === null) return null;
  // 0o077 covers group/other read, write, and execute bits.
  if ((mode & 0o077) === 0) return null;
  return `the environment file is readable by other users (mode ${mode.toString(8)}); expected 600`;
}

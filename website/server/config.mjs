/**
 * Developer Portal server — configuration.
 *
 * Every value here is **server-side only**. Nothing in this file is a `VITE_*`
 * variable, so nothing here can be inlined into the browser bundle. The public
 * site's SEO configuration is a separate concern (`src/config/site-config.ts`)
 * and stays public.
 *
 * Two rules the rest of the server depends on:
 *
 * 1. **Fail closed.** A missing or malformed value produces a `fatal` problem
 *    and the process refuses to start (in production) or the API answers
 *    `503 portal_unavailable` (in development). There is no "unconfigured but
 *    pretend it works" path.
 * 2. **Never log values.** `describe()` returns presence and shape, never a
 *    secret.
 */

import path from "node:path";

/** Problems that stop the server from starting. */
const FATAL = "fatal";
/** Problems worth reporting that do not stop a development server. */
const WARNING = "warning";

const DEFAULTS = {
  host: "127.0.0.1",
  port: 4174,
  sessionTtlSeconds: 8 * 60 * 60,
  sessionIdleSeconds: 30 * 60,
  dataDir: ".portal-data",
  staticDir: "dist",
};

/** Bounds that keep a misconfiguration from becoming a security hole. */
const MIN_SESSION_TTL_SECONDS = 300;
const MAX_SESSION_TTL_SECONDS = 7 * 24 * 60 * 60;
const MIN_IDLE_SECONDS = 60;

function parseInteger(raw, fallback) {
  if (raw === undefined || raw === "") return fallback;
  if (!/^\d+$/.test(raw.trim())) return null;
  const value = Number.parseInt(raw, 10);
  return Number.isFinite(value) ? value : null;
}

function parseBoolean(raw, fallback) {
  if (raw === undefined || raw === "") return fallback;
  const value = raw.trim().toLowerCase();
  if (value === "true" || value === "1") return true;
  if (value === "false" || value === "0") return false;
  return null;
}

/**
 * Parses an origin (`https://host[:port]`) and returns it without a trailing
 * slash. Paths, queries, fragments, credentials and wildcards are rejected:
 * this value is used for an exact string comparison in the CSRF origin check,
 * so anything ambiguous must not be accepted.
 */
export function parseOrigin(raw) {
  if (typeof raw !== "string" || raw.trim() === "") {
    return { origin: null, problem: "is empty" };
  }
  let url;
  try {
    url = new URL(raw.trim());
  } catch {
    return { origin: null, problem: "is not a valid URL" };
  }
  if (url.protocol !== "https:" && url.protocol !== "http:") {
    return { origin: null, problem: "must use http or https" };
  }
  if (url.username !== "" || url.password !== "") {
    return { origin: null, problem: "must not contain credentials" };
  }
  if (url.pathname !== "/" || url.search !== "" || url.hash !== "") {
    return { origin: null, problem: "must be a bare origin with no path, query or fragment" };
  }
  return { origin: url.origin, problem: null, protocol: url.protocol };
}

/**
 * Resolves the portal's configuration.
 *
 * @param {Record<string, string | undefined>} env
 * @param {{ cwd?: string }} [options]
 */
export function resolveConfig(env = process.env, options = {}) {
  const cwd = options.cwd ?? process.cwd();
  /** @type {{ level: "fatal" | "warning", variable: string, message: string }[]} */
  const problems = [];

  const rawEnv = (env.NEXORA_PORTAL_ENV ?? "development").trim().toLowerCase();
  const environment = rawEnv === "production" ? "production" : "development";
  if (rawEnv !== "production" && rawEnv !== "development") {
    problems.push({
      level: WARNING,
      variable: "NEXORA_PORTAL_ENV",
      message: `is "${rawEnv}"; treating it as development`,
    });
  }
  const isProduction = environment === "production";

  const port = parseInteger(env.NEXORA_PORTAL_PORT, DEFAULTS.port);
  if (port === null || port < 1 || port > 65535) {
    problems.push({ level: FATAL, variable: "NEXORA_PORTAL_PORT", message: "must be a port number" });
  }

  const sessionTtlSeconds = parseInteger(env.NEXORA_PORTAL_SESSION_TTL_SECONDS, DEFAULTS.sessionTtlSeconds);
  if (
    sessionTtlSeconds === null ||
    sessionTtlSeconds < MIN_SESSION_TTL_SECONDS ||
    sessionTtlSeconds > MAX_SESSION_TTL_SECONDS
  ) {
    problems.push({
      level: FATAL,
      variable: "NEXORA_PORTAL_SESSION_TTL_SECONDS",
      message: `must be between ${MIN_SESSION_TTL_SECONDS} and ${MAX_SESSION_TTL_SECONDS}`,
    });
  }

  const sessionIdleSeconds = parseInteger(env.NEXORA_PORTAL_SESSION_IDLE_SECONDS, DEFAULTS.sessionIdleSeconds);
  if (sessionIdleSeconds === null || sessionIdleSeconds < MIN_IDLE_SECONDS) {
    problems.push({
      level: FATAL,
      variable: "NEXORA_PORTAL_SESSION_IDLE_SECONDS",
      message: `must be at least ${MIN_IDLE_SECONDS}`,
    });
  }
  if (
    typeof sessionIdleSeconds === "number" &&
    typeof sessionTtlSeconds === "number" &&
    sessionIdleSeconds > sessionTtlSeconds
  ) {
    problems.push({
      level: FATAL,
      variable: "NEXORA_PORTAL_SESSION_IDLE_SECONDS",
      message: "must not exceed the absolute session lifetime",
    });
  }

  const secureCookiesRaw = parseBoolean(
    env.NEXORA_PORTAL_SECURE_COOKIES,
    // Secure-by-default in production; explicitly opt-in otherwise, because a
    // `Secure` cookie is not set over plain http://localhost.
    isProduction,
  );
  if (secureCookiesRaw === null) {
    problems.push({
      level: FATAL,
      variable: "NEXORA_PORTAL_SECURE_COOKIES",
      message: "must be true or false",
    });
  }

  const trustProxy = parseBoolean(env.NEXORA_PORTAL_TRUST_PROXY, false);
  if (trustProxy === null) {
    problems.push({ level: FATAL, variable: "NEXORA_PORTAL_TRUST_PROXY", message: "must be true or false" });
  }

  const originConfig =
    env.NEXORA_PORTAL_ORIGIN === undefined || env.NEXORA_PORTAL_ORIGIN.trim() === ""
      ? null
      : parseOrigin(env.NEXORA_PORTAL_ORIGIN);

  if (originConfig !== null && originConfig.problem !== null) {
    problems.push({
      level: FATAL,
      variable: "NEXORA_PORTAL_ORIGIN",
      message: originConfig.problem,
    });
  }

  if (isProduction && originConfig === null) {
    problems.push({
      level: FATAL,
      variable: "NEXORA_PORTAL_ORIGIN",
      message: "is required in production: the CSRF origin check has nothing to compare against",
    });
  }
  if (isProduction && secureCookiesRaw === false) {
    problems.push({
      level: FATAL,
      variable: "NEXORA_PORTAL_SECURE_COOKIES",
      message: "must be true in production: session cookies must not travel over plain http",
    });
  }
  // A Secure cookie is only ever set over HTTPS, so an http:// production
  // origin would silently produce a portal that cannot sign anyone in. Refuse
  // the combination rather than shipping a broken login.
  if (isProduction && originConfig?.protocol === "http:") {
    problems.push({
      level: FATAL,
      variable: "NEXORA_PORTAL_ORIGIN",
      message: "must be an https:// origin in production",
    });
  }

  const dataDir = path.resolve(cwd, env.NEXORA_PORTAL_DATA_DIR?.trim() || DEFAULTS.dataDir);
  const staticDir = path.resolve(cwd, env.NEXORA_PORTAL_STATIC_DIR?.trim() || DEFAULTS.staticDir);

  const fatal = problems.filter((problem) => problem.level === FATAL);

  return {
    environment,
    isProduction,
    host: env.NEXORA_PORTAL_HOST?.trim() || DEFAULTS.host,
    port: port ?? DEFAULTS.port,
    /** Configured public origin, or `null` in development when not supplied. */
    origin: originConfig?.origin ?? null,
    originProblem: originConfig?.problem ?? null,
    secureCookies: secureCookiesRaw === true,
    sessionTtlSeconds: sessionTtlSeconds ?? DEFAULTS.sessionTtlSeconds,
    sessionIdleSeconds: sessionIdleSeconds ?? DEFAULTS.sessionIdleSeconds,
    trustProxy: trustProxy === true,
    dataDir,
    staticDir,
    problems,
    fatal,
    /** True when the server may start. */
    usable: fatal.length === 0,
  };
}

/**
 * Safe description of the configuration for logs and for the dashboard.
 * Presence only: never a value that could be a credential, and the data
 * directory is reported as a boolean rather than a path.
 */
export function describeConfig(config) {
  return {
    environment: config.environment,
    originConfigured: config.origin !== null,
    secureCookies: config.secureCookies,
    trustProxy: config.trustProxy,
    dataDirConfigured: config.dataDir.length > 0,
    staticDirConfigured: config.staticDir.length > 0,
    sessionTtlSeconds: config.sessionTtlSeconds,
    sessionIdleSeconds: config.sessionIdleSeconds,
    problems: config.problems.map((problem) => ({
      level: problem.level,
      variable: problem.variable,
      message: problem.message,
    })),
  };
}

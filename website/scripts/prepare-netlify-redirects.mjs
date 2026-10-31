/**
 * Emit an optional Netlify API proxy after the normal Vite build.
 *
 * The tracked netlify.toml always retains its static API 404 rules. A real
 * backend is enabled only when NEXORA_PORTAL_BACKEND_ORIGIN contains a bare
 * HTTPS origin. Netlify processes a publish-directory _redirects file before
 * netlify.toml, so this generated file takes precedence only when present.
 */

import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { parseOrigin } from "../server/config.mjs";

export const BACKEND_ORIGIN_ENV = "NEXORA_PORTAL_BACKEND_ORIGIN";
const DEFAULT_DIST_DIR = path.resolve(import.meta.dirname, "../dist");

/**
 * Return a normalised public backend origin, or null when the opt-in is absent.
 * Values are deliberately never included in errors: malformed input must not
 * leak credentials into build logs.
 */
export function readBackendOrigin(env = process.env) {
  const raw = env[BACKEND_ORIGIN_ENV];
  if (raw === undefined || (typeof raw === "string" && raw.trim() === "")) return null;

  const parsed = parseOrigin(raw);
  if (parsed.problem !== null || parsed.protocol !== "https:" || parsed.origin === null) {
    throw new Error(
      `${BACKEND_ORIGIN_ENV} must be a bare HTTPS origin with no credentials, path, query, or fragment.`,
    );
  }
  return parsed.origin;
}

/** Only these two rules are generated; all other routing stays in netlify.toml. */
export function renderBackendRedirects(origin) {
  return (
    "# Generated from NEXORA_PORTAL_BACKEND_ORIGIN during the Netlify build.\n" +
    "# The tracked static API 404 rules remain the fallback when this file is absent.\n" +
    `/api ${origin}/api 200!\n` +
    `/api/* ${origin}/api/:splat 200!\n`
  );
}

export async function prepareNetlifyRedirects({ env = process.env, distDir = DEFAULT_DIST_DIR } = {}) {
  const output = path.join(distDir, "_redirects");
  const origin = readBackendOrigin(env);

  if (origin === null) {
    // Also remove a stale file when this helper is reused without a clean Vite
    // build. Unsetting the variable must restore the tracked API 404 behaviour.
    await fs.rm(output, { force: true });
    return { configured: false, output };
  }

  await fs.writeFile(output, renderBackendRedirects(origin), "utf8");
  return { configured: true, output };
}

const isMain =
  process.argv[1] !== undefined && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);

if (isMain) {
  try {
    const result = await prepareNetlifyRedirects();
    console.log(
      result.configured
        ? "[netlify] generated the opt-in API proxy from NEXORA_PORTAL_BACKEND_ORIGIN."
        : "[netlify] no backend origin configured; preserving the static API 404 rules.",
    );
  } catch (error) {
    console.error(`[netlify] ${error instanceof Error ? error.message : "could not configure the API proxy."}`);
    process.exitCode = 1;
  }
}

/**
 * Strict patch-generation contract — versioned, single-file, validated.
 *
 * Mirrors `src-tauri/src/workspace/patch_contract.rs`.
 * Model output is untrusted; every field is re-validated before a proposal
 * reaches `create_patch_proposal` and the Rust patch boundary.
 */

export const PATCH_CONTRACT_VERSION = 1 as const;
export const MAX_EXPLANATION_CHARS = 1000;
export const MAX_INSTRUCTION_CHARS = 2000;
export const MAX_PROPOSED_CHARS = 200_000;
export const MAX_PREVIEW_BYTES = 262144;
export const MAX_PATH_CHARS = 4096;

const SENSITIVE_BASENAMES = new Set([
  ".env",
  ".env.local",
  ".env.development",
  ".env.production",
  "credentials.json",
  "id_rsa",
  "id_ed25519",
]);

const SENSITIVE_EXTS = new Set([".pem", ".key", ".p12", ".pfx"]);

export interface PatchGenerationResponse {
  version: number;
  targetPath: string;
  proposedContent: string;
  explanation?: string;
}

export interface ValidatedPatchContract {
  version: number;
  targetPath: string;
  proposedContent: string;
  explanation?: string;
}

export interface ContractError {
  code: string;
  message: string;
}

function isSensitivePath(path: string): boolean {
  const base = path.split("/").pop() ?? path;
  if (SENSITIVE_BASENAMES.has(base)) return true;
  const lower = base.toLowerCase();
  for (const ext of SENSITIVE_EXTS) {
    if (lower.endsWith(ext)) return true;
  }
  // Any segment ".env"
  if (path.split("/").some((seg) => seg === ".env")) return true;
  return false;
}

function validateRelativePath(path: string): string | null {
  if (!path || !path.trim()) return "Target path is empty.";
  if (path.length > MAX_PATH_CHARS) return "Target path too long.";
  if (path.startsWith("/") || path.startsWith("\\")) return "Absolute target path not allowed.";
  // Check traversal
  const parts = path.split("/");
  for (const part of parts) {
    if (part === "..") return `Invalid target path '${path}': traversal not allowed.`;
    if (part === "" && path !== "/") {
      // empty segment like "a//b" or leading/trailing slash
      // allow but not critical — strictly reject double slash
      if (path.includes("//")) return `Invalid target path '${path}': empty segment.`;
    }
  }
  if (path.includes("\\")) return `Invalid target path '${path}': backslash not allowed.`;
  if (isSensitivePath(path)) return "Sensitive file cannot be patched.";
  return null;
}

export function validateInstruction(instruction: string): { ok: true; value: string } | { ok: false; error: string } {
  const t = instruction.trim();
  if (!t) return { ok: false, error: "Change request is empty — describe the change you want." };
  if (t.length < 3) return { ok: false, error: "Change request too short." };
  if (t.length > MAX_INSTRUCTION_CHARS) {
    return { ok: false, error: `Change request too long (${t.length} chars, max ${MAX_INSTRUCTION_CHARS}).` };
  }
  return { ok: true, value: t };
}

function validateProposedContent(
  proposed: string,
  original: string,
): string | null {
  if (!proposed || !proposed.trim()) return "Missing 'proposedContent' — patch must contain replacement content.";
  if (new TextEncoder().encode(proposed).length > MAX_PREVIEW_BYTES) {
    return `Proposed content too large (${new TextEncoder().encode(proposed).length} bytes, max ${MAX_PREVIEW_BYTES}).`;
  }
  if (proposed.length > MAX_PROPOSED_CHARS) return "Proposed content exceeds character limit.";
  if (proposed.includes("\0")) return "Proposed content is binary (contains NUL).";
  if (proposed === original) return "No changes detected — proposed content equals original.";
  return null;
}

function extractFencedJson(input: string): string | null {
  const lower = input.toLowerCase();
  const jsonFence = lower.indexOf("```json");
  if (jsonFence !== -1) {
    const after = input.slice(jsonFence + 7);
    const end = after.indexOf("```");
    if (end !== -1) return after.slice(0, end).trim();
  }
  const fence = input.indexOf("```");
  if (fence !== -1) {
    const after = input.slice(fence + 3);
    // skip optional language tag line
    const newline = after.indexOf("\n");
    let candidate = after;
    if (newline !== -1) {
      const firstLine = after.slice(0, newline).trim();
      // if first line is language tag and not JSON, skip it
      if (!firstLine.startsWith("{") && !firstLine.startsWith("[")) {
        candidate = after.slice(newline + 1);
      }
    }
    const end = candidate.indexOf("```");
    if (end !== -1) {
      const c = candidate.slice(0, end).trim();
      if (c.startsWith("{")) return c;
    }
  }
  return null;
}

function extractFirstJsonObject(input: string): string | null {
  const bytes = input;
  let start: number | null = null;
  let depth = 0;
  let inString = false;
  let escape = false;
  for (let i = 0; i < bytes.length; i++) {
    const ch = bytes[i]!;
    if (start === null) {
      if (ch === "{") {
        start = i;
        depth = 1;
        inString = false;
        escape = false;
      }
      continue;
    }
    if (inString) {
      if (escape) {
        escape = false;
      } else if (ch === "\\") {
        escape = true;
      } else if (ch === '"') {
        inString = false;
      }
      continue;
    }
    if (ch === '"') {
      inString = true;
    } else if (ch === "{") {
      depth++;
    } else if (ch === "}") {
      depth--;
      if (depth === 0 && start !== null) {
        return input.slice(start, i + 1);
      }
    }
  }
  return null;
}

function detectMultiFile(value: unknown): string | null {
  if (value && typeof value === "object" && !Array.isArray(value)) {
    const obj = value as Record<string, unknown>;
    if ("files" in obj || "patches" in obj || "edits" in obj) {
      return "Only single-file patches are allowed — model returned multiple files.";
    }
    if (Array.isArray((obj as Record<string, unknown>)["proposedContent"])) {
      return "Only single-file patches are allowed.";
    }
  }
  if (Array.isArray(value) && value.length > 0) {
    return "Only single-file patches are allowed — model returned an array.";
  }
  return null;
}

export function parseAndValidatePatchResponse(
  raw: string,
  expectedTarget: string,
  originalContent: string,
): { ok: true; contract: ValidatedPatchContract } | { ok: false; error: string } {
  const trimmed = raw.trim();
  if (!trimmed) return { ok: false, error: "Model returned empty response." };

  // Helper to try parse and validate one JSON string
  const tryParse = (jsonStr: string): { ok: true; contract: ValidatedPatchContract } | { ok: false; error: string } | null => {
    let parsed: unknown;
    try {
      parsed = JSON.parse(jsonStr);
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      // Check for malformed vs missing field will be handled later
      // Return null to let caller try next extraction
      if (jsonStr === trimmed) {
        // Direct parse failed — will be handled as malformed
        // Try to detect multi-file from raw string
        if (trimmed.includes('"files"') || trimmed.includes('"patches"') || trimmed.includes('"edits"')) {
          return { ok: false, error: "Only single-file patches are allowed — model returned multiple files." };
        }
        // Return malformed error
        return { ok: false, error: `Malformed patch JSON: ${msg}` };
      }
      return null;
    }

    // Detect multi-file before field checks
    const multi = detectMultiFile(parsed);
    if (multi) return { ok: false, error: multi };

    if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) {
      return { ok: false, error: "Patch response must be a JSON object." };
    }
    const obj = parsed as Record<string, unknown>;

    // Check unknown fields (strict)
    const allowed = new Set(["version", "targetPath", "target_path", "proposedContent", "proposed_content", "explanation"]);
    for (const key of Object.keys(obj)) {
      if (!allowed.has(key)) {
        return { ok: false, error: `Unexpected field in patch response: '${key}'` };
      }
    }

    // Normalize fields (support both camelCase and snake_case)
    const version = (obj["version"] as number) ?? (obj as Record<string, unknown>)["version"];
    const targetPath = (obj["targetPath"] as string) ?? (obj["target_path"] as string);
    const proposedContent = (obj["proposedContent"] as string) ?? (obj["proposed_content"] as string);
    const explanation = obj["explanation"] as string | undefined;

    if (version === undefined) return { ok: false, error: "Missing required field: 'version'" };
    if (typeof version !== "number" || version !== PATCH_CONTRACT_VERSION) {
      return { ok: false, error: `Unsupported contract version ${String(version)} (expected ${PATCH_CONTRACT_VERSION}).` };
    }
    if (typeof targetPath !== "string" || !targetPath.trim()) {
      return { ok: false, error: "Missing required field: 'targetPath'" };
    }
    if (typeof proposedContent !== "string" || !proposedContent.trim()) {
      return { ok: false, error: "Missing required field: 'proposedContent'" };
    }

    // Validate targetPath matches expected
    const targetTrim = targetPath.trim();
    if (targetTrim !== expectedTarget) {
      return {
        ok: false,
        error: `Target path '${targetTrim}' does not match requested '${expectedTarget}'. Only single-file patches are allowed.`,
      };
    }
    const relErr = validateRelativePath(targetTrim);
    if (relErr) return { ok: false, error: relErr };

    const propErr = validateProposedContent(proposedContent, originalContent);
    if (propErr) return { ok: false, error: propErr };

    let explanationNorm: string | undefined = undefined;
    if (typeof explanation === "string" && explanation.trim()) {
      const t = explanation.trim();
      if (t.length > MAX_EXPLANATION_CHARS) return { ok: false, error: "Explanation too long (max 1000 chars)." };
      explanationNorm = t;
    } else if (explanation !== undefined && typeof explanation !== "string") {
      return { ok: false, error: "Explanation must be a string." };
    }

    // Check for empty explanation not needed

    return {
      ok: true,
      contract: {
        version,
        targetPath: targetTrim,
        proposedContent,
        explanation: explanationNorm,
      },
    };
  };

  // 1. Try direct parse on trimmed
  const direct = tryParse(trimmed);
  if (direct) {
    // If direct parse succeeded (ok true) or failed with explicit error and not just malformed fallback, return
    // But if direct parse returned ok false with malformed, we still want to try fenced extraction before final fail?
    // For our implementation, direct parse that returns ok false with Malformed will be final unless fenced succeeds
    if (direct.ok === true) return direct;
    // If direct error is not "Malformed patch JSON" generic, return it immediately for missing/unsupported etc.
    // However our tryParse for direct on failure already checks for multi-file and returns, so we should return
    // But we want to try fenced if direct is malformed and fenced might succeed
    if (!direct.error.startsWith("Malformed patch JSON") && !direct.error.startsWith("Model did not")) {
      return direct;
    }
    // else continue to try fenced
  }

  // 2. Try fenced JSON
  const fenced = extractFencedJson(trimmed);
  if (fenced) {
    const res = tryParse(fenced);
    if (res) return res;
  }

  // 3. Try first balanced object
  const firstObj = extractFirstJsonObject(trimmed);
  if (firstObj && firstObj !== trimmed) {
    const res = tryParse(firstObj);
    if (res) return res;
  }

  // If we had direct malformed error, return it
  if (direct && direct.ok === false) return direct;

  return { ok: false, error: "Model did not return valid JSON. Try a more specific instruction." };
}

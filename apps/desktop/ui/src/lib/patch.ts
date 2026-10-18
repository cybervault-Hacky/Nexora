import { invoke } from "@tauri-apps/api/core";
import { isDesktopShell } from "./ipc";

export interface PatchProposal {
  id: string;
  path: string;
  originalContent: string;
  originalHash: string;
  proposedContent: string;
  proposedHash: string;
  createdAt: number;
}

export interface PatchDiffSummary {
  added: number;
  removed: number;
  unchanged: number;
  originalLines: number;
  proposedLines: number;
  truncated: boolean;
}

export interface PatchValidation {
  ok: boolean;
  errors: string[];
  warnings: string[];
  diffSummary: PatchDiffSummary;
}

export interface ApplyResult {
  applied: boolean;
  path: string;
  previousHash: string;
  newHash: string;
  newSize: number;
}

export interface PatchError {
  code: string;
  message: string;
}

function toPatchError(cause: unknown): PatchError {
  if (typeof cause === "object" && cause !== null && "code" in cause && "message" in cause) {
    return cause as PatchError;
  }
  if (typeof cause === "string") return { code: "ipc", message: cause };
  if (cause instanceof Error) return { code: "ipc", message: cause.message };
  return { code: "ipc", message: "The desktop core did not answer." };
}

async function call<T>(command: string, args: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (cause) {
    throw toPatchError(cause);
  }
}

export async function createPatchProposal(root: string, relativePath: string, proposedContent: string): Promise<PatchProposal> {
  if (!isDesktopShell()) throw toPatchError({ code: "desktop_only", message: "Patch proposals are only available in the desktop app." });
  return call<PatchProposal>("create_patch_proposal", { root, relative_path: relativePath, proposed_content: proposedContent });
}

export async function validatePatchProposal(root: string, proposal: PatchProposal): Promise<PatchValidation> {
  if (!isDesktopShell()) throw toPatchError({ code: "desktop_only", message: "Patch validation is only available in the desktop app." });
  return call<PatchValidation>("validate_patch_proposal", { root, proposal });
}

export async function applyPatchProposal(root: string, proposal: PatchProposal): Promise<ApplyResult> {
  if (!isDesktopShell()) throw toPatchError({ code: "desktop_only", message: "Patch apply is only available in the desktop app." });
  return call<ApplyResult>("apply_patch_proposal", { root, proposal });
}

export async function getPatchDiff(original: string, proposed: string): Promise<PatchDiffSummary> {
  if (!isDesktopShell()) {
    // Browser fallback: simple line count
    const o = original.split("\n").length;
    const p = proposed.split("\n").length;
    return { added: Math.max(0, p - o), removed: Math.max(0, o - p), unchanged: Math.min(o, p), originalLines: o, proposedLines: p, truncated: false };
  }
  try {
    return await call<PatchDiffSummary>("get_patch_diff", { original, proposed });
  } catch {
    const o = original.split("\n").length;
    const p = proposed.split("\n").length;
    return { added: Math.max(0, p - o), removed: Math.max(0, o - p), unchanged: Math.min(o, p), originalLines: o, proposedLines: p, truncated: false };
  }
}

// ---------------------------------------------------------------------------
// Multi-file patch batches — atomic all-or-nothing (mirrors workspace/patch.rs)
// ---------------------------------------------------------------------------

export interface BatchEdit {
  relativePath: string;
  proposedContent: string;
}

export interface PatchBatch {
  id: string;
  files: PatchProposal[];
  createdAt: number;
}

export interface BatchFileValidation {
  path: string;
  validation: PatchValidation;
}

export interface BatchValidation {
  ok: boolean;
  errors: string[];
  warnings: string[];
  files: BatchFileValidation[];
}

export interface BatchApplyResult {
  applied: boolean;
  results: ApplyResult[];
}

export async function createPatchBatch(root: string, edits: BatchEdit[]): Promise<PatchBatch> {
  if (!isDesktopShell()) throw toPatchError({ code: "desktop_only", message: "Patch batches are only available in the desktop app." });
  return call<PatchBatch>("create_patch_batch", { root, edits });
}

export async function validatePatchBatch(root: string, batch: PatchBatch): Promise<BatchValidation> {
  if (!isDesktopShell()) throw toPatchError({ code: "desktop_only", message: "Patch batch validation is only available in the desktop app." });
  return call<BatchValidation>("validate_patch_batch", { root, batch });
}

export async function applyPatchBatch(root: string, batch: PatchBatch): Promise<BatchApplyResult> {
  if (!isDesktopShell()) throw toPatchError({ code: "desktop_only", message: "Patch batch apply is only available in the desktop app." });
  return call<BatchApplyResult>("apply_patch_batch", { root, batch });
}

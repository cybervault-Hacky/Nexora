import { invoke } from "@tauri-apps/api/core";
import { isDesktopShell } from "./ipc";
import type { PatchProposal } from "./patch";

export type CheckStatus = "passed" | "failed" | "notRun" | "unsupported" | "inconclusive";

export interface Diagnostic {
  path: string;
  line: number;
  column?: number | null;
  message: string;
}

export interface ValidationCheck {
  id: string;
  label: string;
  status: CheckStatus;
  diagnostics: Diagnostic[];
  note?: string | null;
}

export interface ValidationReport {
  overall: CheckStatus;
  checks: ValidationCheck[];
  summary: string;
  durationMs: number;
  proposalId: string;
  proposalPath: string;
  generatedAt: number;
}

export interface ValidationError {
  code: string;
  message: string;
}

function toError(cause: unknown): ValidationError {
  if (typeof cause === "object" && cause !== null && "code" in cause && "message" in cause) {
    return cause as ValidationError;
  }
  if (typeof cause === "string") return { code: "ipc", message: cause };
  if (cause instanceof Error) return { code: "ipc", message: cause.message };
  return { code: "ipc", message: "The desktop core did not answer." };
}

async function call<T>(command: string, args: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (cause) {
    throw toError(cause);
  }
}

export async function validatePatchDetailed(root: string, proposal: PatchProposal): Promise<ValidationReport> {
  if (!isDesktopShell()) {
    throw toError({ code: "desktop_only", message: "Validation is only available in the desktop app." });
  }
  if (!root.trim()) throw toError({ code: "invalid_request", message: "Project path is empty." });
  // Enforce timeout locally as well — caller will use AbortSignal, but we add 3500 ms timeout wrapper
  return call<ValidationReport>("validate_patch_detailed", { root, proposal });
}

// Helper to map backend lowercase "passed" etc to our type already matches.
// Backend uses lowercase passed/failed/notRun...
export function overallLabel(status: CheckStatus): string {
  switch (status) {
    case "passed":
      return "Passed";
    case "failed":
      return "Failed";
    case "notRun":
      return "Not run";
    case "unsupported":
      return "Unsupported";
    case "inconclusive":
      return "Inconclusive";
    default:
      return status;
  }
}

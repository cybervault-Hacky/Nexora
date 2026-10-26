import { invoke } from "@tauri-apps/api/core";
import { isDesktopShell } from "../../lib/ipc";
import type { InstallationStatus } from "./catalogue";

type NativeInstallationResult = Exclude<InstallationStatus, { state: "unknown" | "checking" }>;

function isStringArray(value: unknown): value is string[] {
  return Array.isArray(value) && value.every((item) => typeof item === "string");
}

function isNonNegativeInteger(value: unknown): value is number {
  return typeof value === "number" && Number.isInteger(value) && value >= 0;
}

/** Validate the native tagged union rather than trusting TypeScript's generic IPC type. */
export function isNativeInstallationResult(value: unknown): value is NativeInstallationResult {
  if (typeof value !== "object" || value === null || !("state" in value)) return false;
  const result = value as Record<string, unknown>;

  switch (result.state) {
    case "found":
      return (
        Array.isArray(result.installations) &&
        result.installations.length > 0 &&
        result.installations.every((item) => {
          if (typeof item !== "object" || item === null) return false;
          const installation = item as Record<string, unknown>;
          return (
            typeof installation.executablePath === "string" && installation.executablePath.length > 0 &&
            (typeof installation.version === "string" || installation.version === null) &&
            installation.method === "standard_directory" &&
            installation.confidence === "candidate_only" &&
            typeof installation.validation === "string" && installation.validation.length > 0
          );
        }) &&
        isStringArray(result.incompleteLocations) &&
        isNonNegativeInteger(result.rejectedCandidates)
      );
    case "not_found":
      return isStringArray(result.checkedLocations) && isNonNegativeInteger(result.rejectedCandidates);
    case "unsupported":
      return typeof result.reason === "string";
    case "access_denied":
      return typeof result.reason === "string" && isStringArray(result.incompleteLocations);
    case "error":
      return (
        typeof result.code === "string" &&
        typeof result.message === "string" &&
        isStringArray(result.incompleteLocations)
      );
    default:
      return false;
  }
}

/**
 * Performs Blender's bounded native installation check only after an explicit
 * user action. This is not a connection attempt and does not launch Blender.
 */
let pendingBlenderInstallationCheck: Promise<InstallationStatus> | null = null;

/** Coalesces rapid/replayed clicks into one native IPC request. */
export function checkBlenderInstallation(): Promise<InstallationStatus> {
  if (pendingBlenderInstallationCheck) return pendingBlenderInstallationCheck;

  const request = readBlenderInstallationStatus();
  pendingBlenderInstallationCheck = request.finally(() => {
    pendingBlenderInstallationCheck = null;
  });
  return pendingBlenderInstallationCheck;
}

async function readBlenderInstallationStatus(): Promise<InstallationStatus> {
  if (!isDesktopShell()) {
    return {
      state: "unsupported",
      reason: "The browser preview cannot perform Windows desktop installation checks; no files were scanned.",
    };
  }

  try {
    const result: unknown = await invoke<unknown>("get_blender_detection_status");
    return isNativeInstallationResult(result)
      ? result
      : {
          state: "error",
          code: "invalid_response",
          message: "The desktop core returned an invalid Blender detection status.",
          incompleteLocations: [],
        };
  } catch {
    return {
      state: "error",
      code: "ipc",
      message: "The desktop core could not report Blender installation detection.",
      incompleteLocations: [],
    };
  }
}

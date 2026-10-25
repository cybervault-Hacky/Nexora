import { invoke } from "@tauri-apps/api/core";
import { isDesktopShell, type UsageStatus } from "./ipc";

/**
 * Device-local generation usage IPC.
 *
 * The browser preview has no engine and no usage record, so it reports nothing
 * rather than inventing a counter. The Rust command is the sole source of local
 * accounting data. An absent policy is represented explicitly as unconfigured.
 */

export interface UsageError {
  code: string;
  message: string;
}

export function toUsageError(cause: unknown): UsageError {
  if (
    typeof cause === "object" &&
    cause !== null &&
    "code" in cause &&
    "message" in cause &&
    typeof (cause as UsageError).code === "string" &&
    typeof (cause as UsageError).message === "string"
  ) {
    return cause as UsageError;
  }

  if (typeof cause === "string" && cause.trim().length > 0) {
    return { code: "ipc", message: cause };
  }

  if (cause instanceof Error && cause.message.trim().length > 0) {
    return { code: "ipc", message: cause.message };
  }

  return { code: "ipc", message: "The desktop core did not answer." };
}

/** Rejects a payload that is not exactly the documented shape. */
export function isUsageStatus(value: unknown): value is UsageStatus {
  if (typeof value !== "object" || value === null) {
    return false;
  }

  const candidate = value as Record<string, unknown>;
  const nullableNumbers = ["allowance", "used", "remaining", "resetAtMs"];
  const validNullableNumber = (entry: unknown) =>
    entry === null || (typeof entry === "number" && Number.isFinite(entry));

  if (!nullableNumbers.every((key) => validNullableNumber(candidate[key]))) {
    return false;
  }

  if (
    typeof candidate.policyConfigured !== "boolean" ||
    typeof candidate.day !== "string" ||
    !(candidate.policyId === null || typeof candidate.policyId === "string") ||
    typeof candidate.exhausted !== "boolean" ||
    !(candidate.notice === null || typeof candidate.notice === "string") ||
    !(candidate.error === null || typeof candidate.error === "string")
  ) {
    return false;
  }

  if (candidate.policyConfigured) {
    return (
      typeof candidate.allowance === "number" &&
      candidate.allowance > 0 &&
      typeof candidate.resetAtMs === "number" &&
      typeof candidate.policyId === "string"
    );
  }

  return (
    candidate.allowance === null &&
    candidate.remaining === null &&
    candidate.resetAtMs === null &&
    candidate.policyId === null &&
    candidate.exhausted === false
  );
}

/**
 * Reads device-local accounting. Returns `null` outside the desktop shell,
 * where there is no record to report.
 */
export async function fetchUsage(): Promise<UsageStatus | null> {
  if (!isDesktopShell()) {
    return null;
  }

  try {
    const value = await invoke<unknown>("get_usage_status");

    if (!isUsageStatus(value)) {
      throw {
        code: "protocol",
        message: "The desktop core returned an unexpected usage record.",
      };
    }

    return value;
  } catch (cause) {
    throw toUsageError(cause);
  }
}

/** Short, human countdown to the next reset. Derived from the clock, not stored. */
export function formatCountdown(resetAtMs: number, now: number): string {
  const remainingMs = resetAtMs - now;

  if (!Number.isFinite(remainingMs)) {
    return "soon";
  }

  if (remainingMs <= 0) {
    return "any moment";
  }

  const minutes = Math.ceil(remainingMs / 60_000);

  if (minutes < 60) {
    return `${minutes}m`;
  }

  const hours = Math.floor(minutes / 60);
  const rest = minutes % 60;

  return rest === 0 ? `${hours}h` : `${hours}h ${rest}m`;
}

/** The reset instant in the user's own clock, so a UTC policy stays readable. */
export function formatResetClock(resetAtMs: number): string {
  const date = new Date(resetAtMs);

  if (Number.isNaN(date.getTime())) {
    return "";
  }

  return date.toLocaleTimeString(undefined, {
    hour: "2-digit",
    minute: "2-digit",
  });
}

/** Share of a configured local allowance already used; unknown stays empty. */
export function usedPercent(status: UsageStatus): number {
  if (
    status.allowance === null ||
    status.allowance <= 0 ||
    status.used === null
  ) {
    return 0;
  }

  const percent = (status.used / status.allowance) * 100;

  return Math.min(100, Math.max(0, Math.round(percent)));
}

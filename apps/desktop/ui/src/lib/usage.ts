import { invoke } from "@tauri-apps/api/core";
import { isDesktopShell, type UsageStatus } from "./ipc";

/**
 * Daily free usage IPC.
 *
 * The browser preview has no engine and no usage record, so it reports nothing
 * rather than inventing a friendly-looking counter. The real numbers only ever
 * come from the Rust command.
 */

export interface UsageError {
  code: string;
  message: string;
}

/**
 * Mirrors `DAILY_ALLOWANCE` in `src-tauri/src/usage/mod.rs`.
 *
 * It is only used for copy (the pricing/plan text) and for the progress bar's
 * maximum; every enforced number comes from the backend. `rustContract.test.ts`
 * fails if the two constants drift apart.
 */
export const PUBLISHED_DAILY_ALLOWANCE = 50;

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
  const numbers = ["allowance", "used", "remaining", "resetAtMs"];

  if (!numbers.every((key) => typeof candidate[key] === "number")) {
    return false;
  }

  return (
    typeof candidate.day === "string" &&
    typeof candidate.policyId === "string" &&
    typeof candidate.provisional === "boolean" &&
    typeof candidate.exhausted === "boolean" &&
    (candidate.notice === null || typeof candidate.notice === "string") &&
    (candidate.error === null || typeof candidate.error === "string")
  );
}

/**
 * Reads the allowance. Returns `null` outside the desktop shell, where there is
 * no record to report and nothing to enforce.
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

/** Share of the allowance already used, clamped for the progress bar. */
export function usedPercent(status: UsageStatus): number {
  if (status.allowance <= 0) {
    return 0;
  }

  const percent = (status.used / status.allowance) * 100;

  return Math.min(100, Math.max(0, Math.round(percent)));
}

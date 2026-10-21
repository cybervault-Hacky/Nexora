import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

import {
  PUBLISHED_DAILY_ALLOWANCE,
  fetchUsage,
  formatCountdown,
  formatResetClock,
  isUsageStatus,
  toUsageError,
  usedPercent,
} from "./usage";
import { markDesktop } from "../test/fixtures";
import { makeUsage } from "../test/fixtures";

const RESET = Date.UTC(2026, 9, 21); // 2026-10-21T00:00:00Z
const NOW = Date.UTC(2026, 9, 20, 18, 48);

beforeEach(() => {
  invokeMock.mockReset();
  markDesktop(true);
});

afterEach(() => {
  markDesktop(false);
});

describe("Phase 7 — usage client", () => {
  it("reads the authoritative record from the core", async () => {
    const status = makeUsage({ used: 12 });
    invokeMock.mockResolvedValue(status);

    await expect(fetchUsage()).resolves.toEqual(status);
    expect(invokeMock).toHaveBeenCalledWith("get_usage_status");
  });

  it("reports nothing in a browser preview instead of inventing numbers", async () => {
    markDesktop(false);

    await expect(fetchUsage()).resolves.toBeNull();
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it("rejects a payload that is not the documented shape", async () => {
    invokeMock.mockResolvedValue({ allowance: "a lot" });

    await expect(fetchUsage()).rejects.toMatchObject({ code: "protocol" });
  });

  it("normalises a core failure", async () => {
    invokeMock.mockRejectedValue({ code: "usage_unavailable", message: "disk is read-only" });

    await expect(fetchUsage()).rejects.toEqual({
      code: "usage_unavailable",
      message: "disk is read-only",
    });
  });

  it("normalises transport failures without leaking a stack", () => {
    expect(toUsageError("boom")).toEqual({ code: "ipc", message: "boom" });
    expect(toUsageError(new Error("offline"))).toEqual({ code: "ipc", message: "offline" });
    expect(toUsageError(undefined)).toEqual({
      code: "ipc",
      message: "The desktop core did not answer.",
    });
  });

  it("validates every field the UI relies on", () => {
    expect(isUsageStatus(makeUsage())).toBe(true);
    expect(isUsageStatus(null)).toBe(false);
    expect(isUsageStatus({ ...makeUsage(), notice: 5 })).toBe(false);
    expect(isUsageStatus({ ...makeUsage(), exhausted: "yes" })).toBe(false);
  });

  it("publishes the same allowance the backend enforces", () => {
    // The cross-language check in rustContract.test.ts pins this to Rust.
    expect(PUBLISHED_DAILY_ALLOWANCE).toBe(50);
  });

  it("derives the countdown from the reset instant", () => {
    expect(formatCountdown(RESET, NOW)).toBe("5h 12m");
    expect(formatCountdown(RESET, RESET - 90_000)).toBe("2m");
    expect(formatCountdown(RESET, RESET - 30_000)).toBe("1m");
    expect(formatCountdown(RESET, RESET)).toBe("any moment");
    expect(formatCountdown(RESET, RESET + 60_000)).toBe("any moment");
    expect(formatCountdown(Number.NaN, NOW)).toBe("soon");
  });

  it("renders the reset instant in the viewer's clock", () => {
    const clock = formatResetClock(RESET);

    expect(clock).toMatch(/\d{1,2}:\d{2}/);
    expect(formatResetClock(Number.NaN)).toBe("");
  });

  it("never reports a percentage outside 0–100", () => {
    expect(usedPercent(makeUsage({ used: 0 }))).toBe(0);
    expect(usedPercent(makeUsage({ used: 25 }))).toBe(50);
    expect(usedPercent(makeUsage({ used: 50 }))).toBe(100);
    expect(usedPercent(makeUsage({ allowance: 0, used: 5 }))).toBe(0);
  });
});

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

import {
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

  it("validates the explicit policy state and every field the UI relies on", () => {
    expect(isUsageStatus(makeUsage())).toBe(true);
    expect(isUsageStatus(null)).toBe(false);
    expect(isUsageStatus({ ...makeUsage(), notice: 5 })).toBe(false);
    expect(isUsageStatus({ ...makeUsage(), exhausted: "yes" })).toBe(false);
    expect(isUsageStatus({ ...makeUsage(), policyConfigured: true })).toBe(false);
    expect(
      isUsageStatus(
        makeUsage({ policyConfigured: true, allowance: 50, resetAtMs: RESET }),
      ),
    ).toBe(true);
  });

  it("represents absent quota policy as unconfigured rather than a number", () => {
    const status = makeUsage();

    expect(status.policyConfigured).toBe(false);
    expect(status.allowance).toBeNull();
    expect(status.remaining).toBeNull();
    expect(status.resetAtMs).toBeNull();
    expect(status.policyId).toBeNull();
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

  it("only computes a percentage when policy and usage are known", () => {
    const configured = (used: number, allowance = 50) =>
      makeUsage({ policyConfigured: true, used, allowance });

    expect(usedPercent(configured(0))).toBe(0);
    expect(usedPercent(configured(25))).toBe(50);
    expect(usedPercent(configured(50))).toBe(100);
    expect(usedPercent(configured(500))).toBe(100);
    expect(usedPercent(makeUsage())).toBe(0);
    expect(
      usedPercent(makeUsage({ policyConfigured: true, allowance: 50, used: null })),
    ).toBe(0);
  });
});

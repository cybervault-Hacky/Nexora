import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const fetchUsageMock = vi.fn();

vi.mock("../../lib/usage", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../lib/usage")>();

  return { ...actual, fetchUsage: (...args: unknown[]) => fetchUsageMock(...args) };
});

import { useUsage } from "./useUsage";
import { makeUsage } from "../../test/fixtures";

beforeEach(() => {
  fetchUsageMock.mockReset();
});

afterEach(() => {
  vi.useRealTimers();
});

describe("Phase 7 — useUsage", () => {
  it("loads the allowance once and exposes it", async () => {
    fetchUsageMock.mockResolvedValue(makeUsage({ used: 7 }));

    const { result } = renderHook(() => useUsage());

    await waitFor(() => expect(result.current.status?.used).toBe(7));
    expect(result.current.error).toBeNull();
    expect(result.current.loading).toBe(false);
    expect(fetchUsageMock).toHaveBeenCalledTimes(1);
  });

  it("renders no status when the core has nothing to report", async () => {
    fetchUsageMock.mockResolvedValue(null);

    const { result } = renderHook(() => useUsage());

    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.status).toBeNull();
    expect(result.current.error).toBeNull();
  });

  it("keeps a failed read as an error rather than a zeroed counter", async () => {
    fetchUsageMock.mockRejectedValue({ code: "ipc", message: "core offline" });

    const { result } = renderHook(() => useUsage());

    await waitFor(() => expect(result.current.error).toBe("core offline"));
    expect(result.current.status).toBeNull();
  });

  it("re-reads the record when a generation settles", async () => {
    fetchUsageMock.mockResolvedValueOnce(makeUsage({ used: 1 }));
    fetchUsageMock.mockResolvedValueOnce(makeUsage({ used: 2 }));

    const { result, rerender } = renderHook(({ key }) => useUsage(key), {
      initialProps: { key: 0 },
    });

    await waitFor(() => expect(result.current.status?.used).toBe(1));

    rerender({ key: 1 });

    await waitFor(() => expect(result.current.status?.used).toBe(2));
    expect(fetchUsageMock).toHaveBeenCalledTimes(2);
  });

  it("re-reads once the reset instant has passed, not before", async () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-10-20T23:50:00Z"));

    fetchUsageMock.mockResolvedValueOnce(
      makeUsage({ used: 50, exhausted: true, resetAtMs: Date.UTC(2026, 9, 21) }),
    );
    fetchUsageMock.mockResolvedValueOnce(makeUsage({ used: 0 }));

    const { result } = renderHook(() => useUsage());

    await act(async () => {
      await Promise.resolve();
    });
    expect(result.current.status?.used).toBe(50);

    // Still before midnight: the countdown ticks, but no reset is claimed.
    act(() => {
      vi.advanceTimersByTime(30_000);
    });
    expect(fetchUsageMock).toHaveBeenCalledTimes(1);
    expect(result.current.status?.used).toBe(50);

    vi.setSystemTime(new Date("2026-10-21T00:00:05Z"));

    await act(async () => {
      vi.advanceTimersByTime(30_000);
      await Promise.resolve();
    });

    expect(fetchUsageMock).toHaveBeenCalledTimes(2);
    expect(result.current.status?.used).toBe(0);
  });

  it("can be reloaded explicitly", async () => {
    fetchUsageMock.mockResolvedValue(makeUsage({ used: 3 }));

    const { result } = renderHook(() => useUsage());

    await waitFor(() => expect(result.current.status?.used).toBe(3));

    result.current.reload();

    await waitFor(() => expect(fetchUsageMock).toHaveBeenCalledTimes(2));
  });
});

import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { markDesktop } from "../../test/fixtures";

vi.mock("../../lib/analysis", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../lib/analysis")>();
  return {
    ...actual,
    getChangeImpact: vi.fn(),
  };
});

import { getChangeImpact } from "../../lib/analysis";
import { useImpact } from "./useImpact";

const impactMock = vi.mocked(getChangeImpact);

function report(overrides: any = {}) {
  return {
    target: "src/base.ts",
    direct: [{ path: "src/mid.ts", reason: "imports src/base.ts", evidence: "src/mid.ts imports src/base.ts at line 1", line: 1, confidence: "verified" }],
    transitive: [{ path: "src/top.ts", reason: "transitively via src/mid.ts", evidence: "src/top.ts (transitively via src/mid.ts)", line: null, confidence: "inferred" }],
    risks: ["1 direct + 1 transitive potential dependents", "Static analysis does not prove runtime"],
    truncated: false,
    notes: ["Confidence: verified = resolved import"],
    ...overrides,
  };
}

beforeEach(() => {
  vi.clearAllMocks();
  impactMock.mockReset();
  markDesktop(true);
  vi.useRealTimers();
});

describe("useImpact", () => {
  it("reports direct and transitive with confidence", async () => {
    const r = report();
    impactMock.mockResolvedValue(r as any);
    const { result } = renderHook(() => useImpact("/tmp/proj"));
    await act(async () => {
      await result.current.analyze("src/base.ts");
    });
    await waitFor(() => expect(result.current.impactState.status).toBe("loaded"));
    const state = result.current.impactState as any;
    expect(state.report.direct[0].confidence).toBe("verified");
    expect(state.report.transitive[0].confidence).toBe("inferred");
    expect(state.report.risks[0]).toContain("potential");
  });

  it("handles no dependents isolated", async () => {
    const r = report({ target: "src/isolated.ts", direct: [], transitive: [], risks: ["No dependents found — change appears isolated"] });
    impactMock.mockResolvedValue(r as any);
    const { result } = renderHook(() => useImpact("/tmp/proj"));
    await act(async () => {
      await result.current.analyze("src/isolated.ts");
    });
    await waitFor(() => expect(result.current.impactState.status).toBe("loaded"));
    const state = result.current.impactState as any;
    expect(state.report.direct.length).toBe(0);
    expect(state.report.risks[0]).toContain("isolated");
  });

  it("ranks verified before inferred and truncates", async () => {
    const r = report({ truncated: true, direct: Array.from({ length: 20 }, (_, i) => ({ path: `src/f${i}.ts`, reason: "imports", evidence: "x", line: 1, confidence: "verified" })) });
    impactMock.mockResolvedValue(r as any);
    const { result } = renderHook(() => useImpact("/tmp/proj"));
    await act(async () => {
      await result.current.analyze("src/base.ts");
    });
    await waitFor(() => expect(result.current.impactState.status).toBe("loaded"));
    const state = result.current.impactState as any;
    expect(state.report.truncated).toBe(true);
    expect(state.report.direct.length).toBe(20);
  });

  it("handles circular dependency risk", async () => {
    const r = report({ target: "a.ts", risks: ["Target participates in circular dependency: a.ts -> b.ts -> a.ts", "Static analysis"] });
    impactMock.mockResolvedValue(r as any);
    const { result } = renderHook(() => useImpact("/tmp/proj"));
    await act(async () => {
      await result.current.analyze("a.ts");
    });
    await waitFor(() => expect(result.current.impactState.status).toBe("loaded"));
    const state = result.current.impactState as any;
    expect(state.report.risks[0]).toContain("circular");
  });

  it("handles sensitive file error", async () => {
    impactMock.mockRejectedValue({ code: "secret_excluded", message: "Sensitive file." });
    const { result } = renderHook(() => useImpact("/tmp/proj"));
    await act(async () => {
      await result.current.analyze(".env");
    });
    await waitFor(() => expect(result.current.impactState.status).toBe("error"));
    expect((result.current.impactState as any).message).toContain("Sensitive");
  });

  it("handles cancellation", async () => {
    impactMock.mockImplementation(() => new Promise(() => {}));
    const { result } = renderHook(() => useImpact("/tmp/proj"));
    act(() => {
      void result.current.analyze("src/base.ts");
    });
    await waitFor(() => expect(result.current.impactState.status).toBe("loading"));
    await act(async () => {
      await result.current.cancel();
    });
    expect(result.current.impactState.status).toBe("cancelled");
  });

  it("handles timeout", async () => {
    vi.useFakeTimers();
    impactMock.mockImplementation(() => new Promise(() => {}));
    const { result } = renderHook(() => useImpact("/tmp/proj"));
    act(() => {
      void result.current.analyze("src/base.ts");
    });
    expect(result.current.impactState.status).toBe("loading");
    act(() => {
      vi.advanceTimersByTime(3600);
    });
    expect(result.current.impactState.status).toBe("error");
    expect((result.current.impactState as any).message).toContain("timed out");
    vi.useRealTimers();
  });

  it("resets on workspace switch", async () => {
    const r = report();
    impactMock.mockResolvedValue(r as any);
    const { result, rerender } = renderHook(({ root }) => useImpact(root), { initialProps: { root: "/tmp/proj" as string | null } });
    await act(async () => {
      await result.current.analyze("src/base.ts");
    });
    await waitFor(() => expect(result.current.impactState.status).toBe("loaded"));
    rerender({ root: "/tmp/other" });
    expect(result.current.impactState.status).toBe("idle");
  });

  it("detects stale result", async () => {
    const r1 = report({ target: "src/a.ts" });
    const r2 = report({ target: "src/b.ts", direct: [] });
    impactMock.mockImplementation(async (_root, path) => {
      if (path === "src/a.ts") {
        await new Promise((r) => setTimeout(r, 50));
        return r1 as any;
      }
      return r2 as any;
    });
    const { result } = renderHook(() => useImpact("/tmp/proj"));
    act(() => {
      void result.current.analyze("src/a.ts");
    });
    await act(async () => {
      await result.current.analyze("src/b.ts");
    });
    await waitFor(() => expect(result.current.impactState.status).toBe("loaded"));
    const state = result.current.impactState as any;
    expect(state.report.target).toBe("src/b.ts");
  });

  it("ensures incorrect evidence not accepted", async () => {
    const r = report({ target: "src/other.ts" }); // mismatch
    impactMock.mockResolvedValue(r as any);
    const { result } = renderHook(() => useImpact("/tmp/proj"));
    await act(async () => {
      await result.current.analyze("src/base.ts");
    });
    await waitFor(() => expect(result.current.impactState.status).toBe("error"));
    expect((result.current.impactState as any).message).toContain("does not match");
  });
});

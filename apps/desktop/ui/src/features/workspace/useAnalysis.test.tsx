import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { markDesktop } from "../../test/fixtures";

vi.mock("../../lib/analysis", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../lib/analysis")>();
  return {
    ...actual,
    getDependencyGraph: vi.fn(),
    getFileRelationships: vi.fn(),
  };
});

import { getDependencyGraph, getFileRelationships } from "../../lib/analysis";
import { useAnalysis, useRelationships } from "./useAnalysis";

const graphMock = vi.mocked(getDependencyGraph);
const relMock = vi.mocked(getFileRelationships);

function graph(overrides: any = {}) {
  return {
    version: 1,
    root: "/tmp/proj",
    nodes: [
      { path: "src/a.ts", imports: [{ from: "src/a.ts", to: "src/b.ts", raw: "./b", line: 1, verified: true, kind: "import", note: null }], importedBy: [] },
      { path: "src/b.ts", imports: [], importedBy: ["src/a.ts"] },
    ],
    edges: [{ from: "src/a.ts", to: "src/b.ts", raw: "./b", line: 1, verified: true, kind: "import", note: null }],
    truncated: false,
    unsupported: 0,
    cycles: [],
    generatedAt: Date.now(),
    ...overrides,
  };
}

function relationships(overrides: any = {}) {
  return {
    path: "src/a.ts",
    imports: [{ from: "src/a.ts", to: "src/b.ts", raw: "./b", line: 1, verified: true, kind: "import", note: null }],
    importedBy: ["src/c.ts"],
    definedSymbols: [{ name: "foo", kind: "function", path: "src/a.ts", startLine: 1, endLine: 1 }],
    referencedBy: [{ path: "src/c.ts", line: 5, symbol: "foo", snippet: "foo()" }],
    unresolved: [],
    notes: [],
    ...overrides,
  };
}

beforeEach(() => {
  vi.clearAllMocks();
  graphMock.mockReset();
  relMock.mockReset();
  markDesktop(true);
  vi.useRealTimers();
});

describe("useAnalysis", () => {
  it("loads graph successfully", async () => {
    const g = graph();
    graphMock.mockResolvedValue(g as any);
    const { result } = renderHook(() => useAnalysis("/tmp/proj"));
    await act(async () => {
      await result.current.refresh();
    });
    await waitFor(() => expect(result.current.analysisState.status).toBe("loaded"));
    const state = result.current.analysisState as any;
    expect(state.graph.nodes.length).toBe(2);
    expect(state.graph.edges[0].verified).toBe(true);
  });

  it("handles truncated and cycles", async () => {
    const g = graph({ truncated: true, unsupported: 2, cycles: [["a.ts", "b.ts", "a.ts"]] });
    graphMock.mockResolvedValue(g as any);
    const { result } = renderHook(() => useAnalysis("/tmp/proj"));
    await act(async () => {
      await result.current.refresh();
    });
    await waitFor(() => expect(result.current.analysisState.status).toBe("loaded"));
    const state = result.current.analysisState as any;
    expect(state.graph.truncated).toBe(true);
    expect(state.graph.cycles.length).toBe(1);
  });

  it("handles unresolved imports", async () => {
    const g = graph({
      nodes: [
        { path: "src/app.ts", imports: [{ from: "src/app.ts", to: "./missing", raw: "./missing", line: 2, verified: false, kind: "import", note: "unresolved — file not found" }], importedBy: [] },
      ],
      edges: [{ from: "src/app.ts", to: "./missing", raw: "./missing", line: 2, verified: false, kind: "import", note: "unresolved — file not found" }],
    });
    graphMock.mockResolvedValue(g as any);
    const { result } = renderHook(() => useAnalysis("/tmp/proj"));
    await act(async () => {
      await result.current.refresh();
    });
    await waitFor(() => expect(result.current.analysisState.status).toBe("loaded"));
    const state = result.current.analysisState as any;
    expect(state.graph.edges[0].verified).toBe(false);
  });

  it("handles sensitive exclusion via error", async () => {
    graphMock.mockRejectedValue({ code: "secret_excluded", message: "Sensitive file." });
    const { result } = renderHook(() => useAnalysis("/tmp/proj"));
    await act(async () => {
      await result.current.refresh();
    });
    await waitFor(() => expect(result.current.analysisState.status).toBe("error"));
    expect((result.current.analysisState as any).message).toContain("Sensitive");
  });

  it("handles cancellation", async () => {
    graphMock.mockImplementation(() => new Promise(() => {}));
    const { result } = renderHook(() => useAnalysis("/tmp/proj"));
    act(() => {
      void result.current.refresh();
    });
    await waitFor(() => expect(result.current.analysisState.status).toBe("loading"));
    await act(async () => {
      await result.current.cancel();
    });
    expect(result.current.analysisState.status).toBe("cancelled");
  });

  it("handles timeout", async () => {
    vi.useFakeTimers();
    graphMock.mockImplementation(() => new Promise(() => {}));
    const { result } = renderHook(() => useAnalysis("/tmp/proj"));
    act(() => {
      void result.current.refresh();
    });
    expect(result.current.analysisState.status).toBe("loading");
    act(() => {
      vi.advanceTimersByTime(4100);
    });
    expect(result.current.analysisState.status).toBe("error");
    expect((result.current.analysisState as any).message).toContain("timed out");
    vi.useRealTimers();
  });

  it("resets on workspace switch", async () => {
    const g = graph();
    graphMock.mockResolvedValue(g as any);
    const { result, rerender } = renderHook(({ root }) => useAnalysis(root), { initialProps: { root: "/tmp/proj" as string | null } });
    await act(async () => {
      await result.current.refresh();
    });
    await waitFor(() => expect(result.current.analysisState.status).toBe("loaded"));
    rerender({ root: "/tmp/other" });
    expect(result.current.analysisState.status).toBe("idle");
  });

  it("ignores stale results", async () => {
    const g1 = graph({ nodes: [{ path: "a.ts", imports: [], importedBy: [] }] });
    const g2 = graph({ nodes: [{ path: "b.ts", imports: [], importedBy: [] }] });
    graphMock.mockImplementation(async () => {
      // first call delays, second fast
      await new Promise((r) => setTimeout(r, 50));
      return g1 as any;
    });
    const { result } = renderHook(() => useAnalysis("/tmp/proj"));
    // Start first
    act(() => {
      void result.current.refresh();
    });
    // Mock second quickly resolves to g2
    graphMock.mockResolvedValue(g2 as any);
    await act(async () => {
      await result.current.refresh();
    });
    await waitFor(() => expect(result.current.analysisState.status).toBe("loaded"));
    const state = result.current.analysisState as any;
    // Should be g2, not g1 (stale)
    expect(state.graph.nodes[0].path).toBe("b.ts");
  });
});

describe("useRelationships", () => {
  it("loads relationships with verified vs inferred", async () => {
    const rel = relationships();
    relMock.mockResolvedValue(rel as any);
    const { result } = renderHook(() => useRelationships("/tmp/proj"));
    await act(async () => {
      await result.current.load("src/a.ts");
    });
    await waitFor(() => expect(result.current.relState.status).toBe("loaded"));
    const state = result.current.relState as any;
    expect(state.data.imports[0].verified).toBe(true);
    expect(state.data.referencedBy[0].symbol).toBe("foo");
  });

  it("handles unsupported language (no symbols)", async () => {
    const rel = relationships({ path: "README.md", definedSymbols: [], notes: ["No symbols indexed"] });
    relMock.mockResolvedValue(rel as any);
    const { result } = renderHook(() => useRelationships("/tmp/proj"));
    await act(async () => {
      await result.current.load("README.md");
    });
    await waitFor(() => expect(result.current.relState.status).toBe("loaded"));
    const state = result.current.relState as any;
    expect(state.data.definedSymbols.length).toBe(0);
    expect(state.data.notes[0]).toContain("No symbols");
  });

  it("handles cancellation", async () => {
    relMock.mockImplementation(() => new Promise(() => {}));
    const { result } = renderHook(() => useRelationships("/tmp/proj"));
    act(() => {
      void result.current.load("src/a.ts");
    });
    await waitFor(() => expect(result.current.relState.status).toBe("loading"));
    await act(async () => {
      await result.current.cancel();
    });
    expect(result.current.relState.status).toBe("cancelled");
  });

  it("resets on workspace switch", async () => {
    const rel = relationships();
    relMock.mockResolvedValue(rel as any);
    const { result, rerender } = renderHook(({ root }) => useRelationships(root), { initialProps: { root: "/tmp/proj" as string | null } });
    await act(async () => {
      await result.current.load("src/a.ts");
    });
    await waitFor(() => expect(result.current.relState.status).toBe("loaded"));
    rerender({ root: "/tmp/other" });
    expect(result.current.relState.status).toBe("idle");
  });

  it("reports incorrect evidence association", async () => {
    const rel = relationships({ path: "src/b.ts" }); // mismatch
    relMock.mockResolvedValue(rel as any);
    const { result } = renderHook(() => useRelationships("/tmp/proj"));
    await act(async () => {
      await result.current.load("src/a.ts");
    });
    await waitFor(() => expect(result.current.relState.status).toBe("error"));
    expect((result.current.relState as any).message).toContain("does not match");
  });
});

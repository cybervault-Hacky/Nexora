import { describe, it, expect } from "vitest";
import { retrieveRelevantFilesSmart } from "./retrieval";
import type { SymbolIndex } from "../../lib/symbolIndex";

function idx(symbols: Array<{ name: string; path: string }>): SymbolIndex {
  return {
    version: 1,
    root: "/tmp/root",
    symbols: symbols.map((s) => ({
      name: s.name,
      kind: "function" as any,
      path: s.path,
      startLine: 1,
      endLine: 1,
    })),
    fileCount: 2,
    truncated: false,
    unsupported: 0,
    parseErrors: 0,
    generatedAt: Date.now(),
  };
}

describe("retrieveRelevantFilesSmart (Phase 16)", () => {
  it("prioritizes dependencies of selected files via graph", () => {
    const r = retrieveRelevantFilesSmart({
      query: "",
      selectedFiles: [{ path: "src/app.ts", content: "x", size: 10 }],
      symbolIndex: idx([]),
      graph: {
        nodes: [
          { path: "src/app.ts", imports: [{ to: "src/utils.ts", verified: true }], importedBy: [] },
          { path: "src/utils.ts", imports: [], importedBy: ["src/app.ts"] },
        ],
        edges: [{ from: "src/app.ts", to: "src/utils.ts", verified: true }],
      },
    });
    expect(r.included.some((f) => f.path === "src/app.ts")).toBe(true);
    expect(r.included.some((f) => f.path === "src/utils.ts")).toBe(true);
    expect(r.included.find((f) => f.path === "src/utils.ts")!.reason).toContain("dependency");
  });

  it("adds dependents of selected files", () => {
    const r = retrieveRelevantFilesSmart({
      query: "",
      selectedFiles: [{ path: "src/base.ts", content: "x", size: 10 }],
      symbolIndex: idx([]),
      graph: {
        nodes: [
          { path: "src/base.ts", imports: [], importedBy: ["src/mid.ts"] },
          { path: "src/mid.ts", imports: [{ to: "src/base.ts", verified: true }], importedBy: [] },
        ],
        edges: [{ from: "src/mid.ts", to: "src/base.ts", verified: true }],
      },
    });
    expect(r.included.some((f) => f.path === "src/mid.ts")).toBe(true);
    expect(r.included.find((f) => f.path === "src/mid.ts")!.reason).toContain("dependent");
  });

  it("respects budget and sensitive exclusion", () => {
    const r = retrieveRelevantFilesSmart({
      query: "",
      selectedFiles: [{ path: "src/app.ts", content: "x", size: 10 }],
      symbolIndex: idx([]),
      graph: {
        nodes: [
          { path: "src/app.ts", imports: [{ to: ".env", verified: true }], importedBy: [] },
          { path: ".env", imports: [], importedBy: ["src/app.ts"] },
        ],
        edges: [{ from: "src/app.ts", to: ".env", verified: true }],
      },
      maxFiles: 8,
      maxBytes: 100000,
    });
    expect(r.included.some((f) => f.path === ".env")).toBe(false);
  });

  it("truncates when budget exceeded", () => {
    const r = retrieveRelevantFilesSmart({
      query: "foo",
      selectedFiles: [],
      symbolIndex: idx([{ name: "foo", path: "src/a.ts" }]),
      graph: {
        nodes: [
          { path: "src/a.ts", imports: [], importedBy: ["src/b.ts", "src/c.ts", "src/d.ts"] },
          { path: "src/b.ts", imports: [{ to: "src/a.ts", verified: true }], importedBy: [] },
          { path: "src/c.ts", imports: [{ to: "src/a.ts", verified: true }], importedBy: [] },
          { path: "src/d.ts", imports: [{ to: "src/a.ts", verified: true }], importedBy: [] },
        ],
        edges: [
          { from: "src/b.ts", to: "src/a.ts", verified: true },
          { from: "src/c.ts", to: "src/a.ts", verified: true },
          { from: "src/d.ts", to: "src/a.ts", verified: true },
        ],
      },
      maxFiles: 2,
      maxBytes: 100000,
    });
    // base includes src/a.ts + graph adds one dependent then omits others
    expect(r.included.length).toBe(2);
    expect(r.omitted.length).toBeGreaterThan(0);
    expect(r.omitted[0]!.reason).toContain("budget");
  });

  it("falls back to base when no graph", () => {
    const r = retrieveRelevantFilesSmart({
      query: "hello",
      selectedFiles: [{ path: "src/app.ts", content: "hello", size: 5 }],
      symbolIndex: idx([{ name: "hello", path: "src/other.ts" }]),
      graph: null,
    });
    expect(r.included.some((f) => f.path === "src/app.ts")).toBe(true);
    expect(r.included.some((f) => f.path === "src/other.ts")).toBe(true);
  });

  it("adds dependents of queried symbol defining file", () => {
    const r = retrieveRelevantFilesSmart({
      query: "greet",
      selectedFiles: [],
      symbolIndex: idx([{ name: "greet", path: "src/foo.ts" }]),
      graph: {
        nodes: [
          { path: "src/foo.ts", imports: [], importedBy: ["src/bar.ts"] },
          { path: "src/bar.ts", imports: [{ to: "src/foo.ts", verified: true }], importedBy: [] },
        ],
        edges: [{ from: "src/bar.ts", to: "src/foo.ts", verified: true }],
      },
    });
    expect(r.included.some((f) => f.path === "src/bar.ts")).toBe(true);
    expect(r.included.find((f) => f.path === "src/bar.ts")!.reason).toContain("imports src/foo.ts");
  });

  it("never duplicates selected", () => {
    const r = retrieveRelevantFilesSmart({
      query: "",
      selectedFiles: [{ path: "src/app.ts", content: "x", size: 10 }],
      symbolIndex: idx([]),
      graph: {
        nodes: [{ path: "src/app.ts", imports: [{ to: "src/app.ts", verified: true }], importedBy: [] }],
        edges: [{ from: "src/app.ts", to: "src/app.ts", verified: true }],
      },
    });
    const count = r.included.filter((f) => f.path === "src/app.ts").length;
    expect(count).toBe(1);
  });
});

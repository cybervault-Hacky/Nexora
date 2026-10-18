import { describe, it, expect } from "vitest";
import { retrieveRelevantFiles, rankSymbols } from "./retrieval";
import type { SymbolIndex } from "../../lib/symbolIndex";

function idx(symbols: Array<{ name: string; path: string; kind?: string; startLine?: number }>): SymbolIndex {
  return {
    version: 1,
    root: "/tmp/root",
    symbols: symbols.map((s) => ({
      name: s.name,
      kind: (s.kind as any) ?? "function",
      path: s.path,
      startLine: s.startLine ?? 1,
      endLine: s.startLine ?? 1,
    })),
    fileCount: 2,
    truncated: false,
    unsupported: 0,
    parseErrors: 0,
    generatedAt: Date.now(),
  };
}

describe("retrieveRelevantFiles", () => {
  it("prioritizes selected files", () => {
    const r = retrieveRelevantFiles({
      query: "hello",
      selectedFiles: [{ path: "src/app.ts", content: "hello", size: 5 }],
      symbolIndex: idx([{ name: "hello", path: "src/other.ts" }]),
    });
    expect(r.included[0]!.path).toBe("src/app.ts");
    expect(r.included[0]!.reason).toBe("selected by you");
    expect(r.included.some((f) => f.path === "src/other.ts")).toBe(true);
  });

  it("respects budget and reports omitted", () => {
    const r = retrieveRelevantFiles({
      query: "foo",
      selectedFiles: [],
      symbolIndex: idx([
        { name: "foo", path: "a.ts" },
        { name: "foo", path: "b.ts" },
        { name: "foo", path: "c.ts" },
      ]),
      maxFiles: 1,
      maxBytes: 100000,
    });
    expect(r.included.length).toBe(1);
    expect(r.omitted.length).toBe(2);
    expect(r.omitted[0]!.reason).toContain("budget");
  });

  it("dedupes by path and explains why", () => {
    const r = retrieveRelevantFiles({
      query: "MyClass",
      selectedFiles: [],
      symbolIndex: idx([
        { name: "MyClass", path: "src/a.ts" },
        { name: "MyClassHelper", path: "src/a.ts" }, // same path dedup
        { name: "MyClass", path: "src/b.ts" },
      ]),
    });
    const paths = r.included.map((f) => f.path);
    expect(new Set(paths).size).toBe(paths.length);
    expect(r.included[0]!.reason).toContain("defines symbol");
  });

  it("warns when no relevant symbols", () => {
    const r = retrieveRelevantFiles({
      query: "unknownSymbolXYZ",
      selectedFiles: [],
      symbolIndex: idx([{ name: "hello", path: "a.ts" }]),
    });
    expect(r.warnings.some((w) => w.includes("No relevant symbols"))).toBe(true);
    expect(r.included.length).toBe(0);
  });

  it("falls back gracefully when index unavailable", () => {
    const r = retrieveRelevantFiles({
      query: "hello",
      selectedFiles: [{ path: "a.ts", content: "hi", size: 2 }],
      symbolIndex: null,
    });
    expect(r.included.length).toBe(1);
    expect(r.warnings.length).toBe(0); // selected present, no warning needed
  });

  it("warns when index unavailable and nothing selected", () => {
    const r = retrieveRelevantFiles({
      query: "hello",
      selectedFiles: [],
      symbolIndex: null,
    });
    expect(r.warnings.some((w) => w.includes("Index unavailable"))).toBe(true);
  });

  it("is deterministic: exact > prefix > substring > path", () => {
    const symbols = idx([
      { name: "foobar", path: "src/x.ts" }, // substring
      { name: "foo", path: "src/y.ts" }, // exact
      { name: "foobarish", path: "src/z.ts" }, // prefix? Actually foobarish starts with foo => prefix
    ]);
    const ranked = rankSymbols(symbols.symbols, ["foo"]);
    expect(ranked[0]!.name).toBe("foo");
    expect(ranked[1]!.name).toBe("foobar"); // prefix alphabetically before foobarish
    expect(ranked[2]!.name).toBe("foobarish"); // prefix but alphabetically later
    // Ensure path match works: query contains file path token
    const pathRanked = rankSymbols(idx([{ name: "other", path: "src/foo.ts" }]).symbols, ["foo"]);
    expect(pathRanked.length).toBe(1);
    expect(pathRanked[0]!.path).toBe("src/foo.ts");
  });

  it("filters sensitive paths", () => {
    const r = retrieveRelevantFiles({
      query: "secret",
      selectedFiles: [{ path: ".env", content: "secret", size: 6 }],
      symbolIndex: idx([{ name: "secret", path: ".env" }]),
    });
    // .env is sensitive, should be filtered from selected
    expect(r.included.find((f) => f.path === ".env")).toBeUndefined();
  });

  it("empty query returns selected only with warning if none", () => {
    const r = retrieveRelevantFiles({
      query: "   ",
      selectedFiles: [],
      symbolIndex: idx([{ name: "hello", path: "a.ts" }]),
    });
    expect(r.included.length).toBe(0);
    expect(r.warnings.length).toBeGreaterThan(0);
  });
});

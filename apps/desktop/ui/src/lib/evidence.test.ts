import { describe, it, expect } from "vitest";
import { buildEvidenceReport, formatEvidenceForPrompt, getPatchMissingEvidence, MAX_EVIDENCE_ENTRIES } from "./evidence";
import type { SymbolIndex } from "./symbolIndex";
import type { AnalysisGraph } from "./analysis";
import type { RetrievalResult } from "../features/workspace/retrieval";
import type { ContextPlan } from "../features/workspace/contextBudget";

function makeSymbolIndex(overrides: Partial<SymbolIndex> = {}): SymbolIndex {
  return {
    version: 1,
    root: "/tmp/ws",
    symbols: [],
    fileCount: 2,
    truncated: false,
    unsupported: 0,
    parseErrors: 0,
    generatedAt: Date.now(),
    ...overrides,
  };
}

function makeAnalysisGraph(overrides: Partial<AnalysisGraph> = {}): AnalysisGraph {
  return {
    version: 1,
    root: "/tmp/ws",
    nodes: [],
    edges: [],
    truncated: false,
    unsupported: 0,
    cycles: [],
    generatedAt: Date.now(),
    ...overrides,
  };
}

function makeRetrieval(overrides: Partial<RetrievalResult> = {}): RetrievalResult {
  return {
    included: [],
    omitted: [],
    warnings: [],
    includedPaths: [],
    omittedPaths: [],
    ...overrides,
  };
}

function makeContextPlan(overrides: Partial<ContextPlan> = {}): ContextPlan {
  return {
    mode: "estimated",
    budgetTokens: 32768,
    reservedTokens: 1224,
    availableTokens: 31544,
    usedTokens: 500,
    remainingTokens: 31044,
    estimated: true,
    warnings: [],
    filePlans: [],
    historyIncluded: 2,
    historyOmitted: 0,
    promptTokens: 10,
    promptIncluded: true,
    ...overrides,
  };
}

describe("evidence — grounded answers", () => {
  it("marks selected file as verified", () => {
    const retrieval = makeRetrieval({
      included: [{ path: "src/a.ts", reason: "selected by you" }],
      includedPaths: ["src/a.ts"],
    });
    const report = buildEvidenceReport({
      symbolIndex: makeSymbolIndex(),
      analysisGraph: makeAnalysisGraph(),
      retrievalResult: retrieval,
      contextPlan: makeContextPlan(),
      selectedPaths: ["src/a.ts"],
    });
    expect(report.entries.some((e) => e.path === "src/a.ts" && e.category === "verified")).toBe(true);
    expect(report.hasVerified).toBe(true);
    expect(report.summary).toContain("verified");
  });

  it("distinguishes verified import vs inferred symbol match", () => {
    const retrieval = makeRetrieval({
      included: [
        { path: "src/a.ts", reason: "dependency of src/main.ts (verified import)" },
        { path: "src/b.ts", reason: "defines symbol `foo` (function)", symbol: { name: "foo", kind: "function", path: "src/b.ts", startLine: 10, endLine: 12 } as any },
      ],
      includedPaths: ["src/a.ts", "src/b.ts"],
    });
    const report = buildEvidenceReport({
      symbolIndex: makeSymbolIndex({ symbols: [{ name: "foo", kind: "function", path: "src/b.ts", startLine: 10, endLine: 12 } as any] }),
      analysisGraph: makeAnalysisGraph({ nodes: [{ path: "src/main.ts", imports: [{ to: "src/a.ts", verified: true } as any], importedBy: [] } as any] }),
      retrievalResult: retrieval,
      contextPlan: makeContextPlan(),
    });
    const verified = report.entries.find((e) => e.path === "src/a.ts");
    const inferred = report.entries.find((e) => e.path === "src/b.ts");
    expect(verified?.category).toBe("verified");
    expect(inferred?.category).toBe("inferred");
    expect(report.hasVerified).toBe(true);
    expect(report.hasInferred).toBe(true);
  });

  it("marks omitted as unavailable and sets partial", () => {
    const retrieval = makeRetrieval({
      included: [{ path: "src/a.ts", reason: "selected by you" }],
      omitted: [{ path: "src/b.ts", reason: "defines symbol `bar` — omitted (budget)", symbol: { name: "bar", kind: "function", path: "src/b.ts", startLine: 5, endLine: 6 } as any }],
      includedPaths: ["src/a.ts"],
      omittedPaths: ["src/b.ts"],
      warnings: ["Budget exceeded"],
    });
    const report = buildEvidenceReport({
      symbolIndex: makeSymbolIndex(),
      analysisGraph: makeAnalysisGraph(),
      retrievalResult: retrieval,
      contextPlan: makeContextPlan({ filePlans: [{ path: "src/c.ts", included: false, tokens: 100, size: 200, omittedReason: "Budget exceeded" }] }),
    });
    expect(report.entries.some((e) => e.path === "src/b.ts" && e.category === "unavailable")).toBe(true);
    expect(report.hasUnavailable).toBe(true);
    expect(report.isPartial).toBe(true);
    expect(report.limitations.retrievalOmitted).toBe(1);
    expect(report.limitations.budgetOmitted).toBe(1);
  });

  it("detects unsupported languages", () => {
    const retrieval = makeRetrieval({
      included: [{ path: "README.md", reason: "selected by you" }],
      includedPaths: ["README.md"],
    });
    const report = buildEvidenceReport({
      symbolIndex: makeSymbolIndex({ unsupported: 2 }),
      analysisGraph: makeAnalysisGraph({ unsupported: 1 }),
      retrievalResult: retrieval,
      contextPlan: makeContextPlan(),
      selectedPaths: ["README.md", "config.toml"],
    });
    expect(report.isUnsupported).toBe(true);
    expect(report.limitations.unsupportedLanguages).toEqual(expect.arrayContaining(["md"]));
    expect(report.limitations.unsupported).toBe(2);
    expect(report.limitations.graphUnsupported).toBe(1);
  });

  it("handles truncated and incomplete gracefully", () => {
    const report = buildEvidenceReport({
      symbolIndex: makeSymbolIndex({ truncated: true, parseErrors: 1 }),
      analysisGraph: makeAnalysisGraph({ truncated: true, cycles: [["a", "b", "a"]] }),
      retrievalResult: makeRetrieval(),
      contextPlan: makeContextPlan({ warnings: ["1 file(s) omitted due to budget."] }),
    });
    expect(report.isPartial).toBe(true);
    expect(report.limitations.truncated).toBe(true);
    expect(report.limitations.graphTruncated).toBe(true);
    expect(report.limitations.parseErrors).toBe(1);
    expect(report.limitations.graphCycles).toBe(1);
    expect(formatEvidenceForPrompt(report)).toContain("context is partial");
  });

  it("handles missing index/graph as unavailable", () => {
    const report = buildEvidenceReport({
      symbolIndex: null,
      analysisGraph: null,
      retrievalResult: null,
      contextPlan: null,
    });
    expect(report.limitations.indexUnavailable).toBe(true);
    expect(report.limitations.graphUnavailable).toBe(true);
    expect(report.isPartial).toBe(true);
    expect(report.entries.length).toBe(0);
    expect(report.summary).toContain("No workspace");
    const manifest = formatEvidenceForPrompt(report);
    expect(manifest).toContain("no workspace files were included");
    expect(manifest).toContain("index unavailable");
    expect(manifest).toContain("graph unavailable");
  });

  it("excludes sensitive files from evidence", () => {
    const retrieval = makeRetrieval({
      included: [
        { path: ".env", reason: "selected by you" },
        { path: "src/a.ts", reason: "selected by you" },
        { path: "secrets/credentials.json", reason: "selected by you" },
      ],
      includedPaths: [".env", "src/a.ts", "secrets/credentials.json"],
    });
    const report = buildEvidenceReport({
      symbolIndex: makeSymbolIndex(),
      analysisGraph: makeAnalysisGraph(),
      retrievalResult: retrieval,
      contextPlan: makeContextPlan(),
      selectedPaths: [".env", "src/a.ts"],
    });
    expect(report.entries.some((e) => e.path === ".env")).toBe(false);
    expect(report.entries.some((e) => e.path === "secrets/credentials.json")).toBe(false);
    expect(report.entries.some((e) => e.path === "src/a.ts")).toBe(true);
  });

  it("bounds entries to MAX_EVIDENCE_ENTRIES", () => {
    const manyIncluded = Array.from({ length: 30 }, (_, i) => ({ path: `src/file${i}.ts`, reason: `defines symbol \`f${i}\``, symbol: { name: `f${i}`, kind: "function", path: `src/file${i}.ts`, startLine: i, endLine: i+1 } as any }));
    const retrieval = makeRetrieval({
      included: manyIncluded,
      includedPaths: manyIncluded.map((r) => r.path),
    });
    const report = buildEvidenceReport({
      symbolIndex: makeSymbolIndex(),
      analysisGraph: makeAnalysisGraph(),
      retrievalResult: retrieval,
      contextPlan: makeContextPlan(),
    });
    expect(report.entries.length).toBeLessThanOrEqual(MAX_EVIDENCE_ENTRIES);
  });

  it("only uses line numbers when symbol provides them, never invents", () => {
    const retrieval = makeRetrieval({
      included: [
        { path: "src/a.ts", reason: "selected by you" },
        { path: "src/b.ts", reason: "defines symbol `foo`", symbol: { name: "foo", kind: "function", path: "src/b.ts", startLine: 42, endLine: 50 } as any },
      ],
      includedPaths: ["src/a.ts", "src/b.ts"],
    });
    const report = buildEvidenceReport({
      symbolIndex: makeSymbolIndex(),
      analysisGraph: makeAnalysisGraph(),
      retrievalResult: retrieval,
      contextPlan: makeContextPlan(),
    });
    const withLine = report.entries.find((e) => e.path === "src/b.ts");
    const withoutLine = report.entries.find((e) => e.path === "src/a.ts");
    expect(withLine?.line).toBe(42);
    expect(withoutLine?.line).toBeUndefined();
    const manifest = formatEvidenceForPrompt(report);
    expect(manifest).toContain("src/b.ts");
    // manifest should not invent line for a.ts
    const lines = manifest.split("\n").filter((l) => l.includes("src/a.ts"));
    expect(lines.join(" ")).not.toMatch(/:\d+/);
  });

  it("instructs not to invent tests/builds/validation and to separate observations/hypotheses", () => {
    const report = buildEvidenceReport({
      symbolIndex: makeSymbolIndex(),
      analysisGraph: makeAnalysisGraph(),
      retrievalResult: makeRetrieval(),
      contextPlan: makeContextPlan(),
      verificationPerformed: false,
    });
    const manifest = formatEvidenceForPrompt(report);
    expect(manifest).toContain("Never invent files/symbols/test results/build/validation");
    expect(manifest).toContain("Separate observations");
    expect(manifest).toContain("Hypothesis:");
    expect(manifest).toContain("Suggestion:");
    expect(manifest).toContain("Static analysis does not prove runtime");
    expect(manifest).toContain("Verification not performed");
    expect(manifest).toContain("Do not claim a test, build, or validation passed");
    expect(manifest).toContain("Not grounded in the provided files");
  });

  it("claims about tests/builds only when verificationPerformed true", () => {
    const notVerified = buildEvidenceReport({
      symbolIndex: makeSymbolIndex(),
      analysisGraph: makeAnalysisGraph(),
      retrievalResult: makeRetrieval({ included: [{ path: "src/a.ts", reason: "selected by you" }], includedPaths: ["src/a.ts"] }),
      contextPlan: makeContextPlan(),
      verificationPerformed: false,
    });
    expect(formatEvidenceForPrompt(notVerified)).toContain("Verification not performed for this query");

    const verified = buildEvidenceReport({
      symbolIndex: makeSymbolIndex(),
      analysisGraph: makeAnalysisGraph(),
      retrievalResult: makeRetrieval({ included: [{ path: "src/a.ts", reason: "selected by you" }], includedPaths: ["src/a.ts"] }),
      contextPlan: makeContextPlan(),
      verificationPerformed: true,
    });
    expect(formatEvidenceForPrompt(verified)).not.toContain("Verification not performed for this query");
  });

  it("handles malformed/incomplete: truncated prompt says uncertainty not fabrication", () => {
    const report = buildEvidenceReport({
      symbolIndex: makeSymbolIndex({ truncated: true }),
      analysisGraph: null,
      retrievalResult: makeRetrieval({ omitted: [{ path: "src/missing.ts", reason: "omitted (budget)" }], omittedPaths: ["src/missing.ts"] }),
      contextPlan: makeContextPlan(),
    });
    const manifest = formatEvidenceForPrompt(report);
    expect(manifest).toContain("Not grounded in the provided files");
    expect(manifest).toContain("ask for clarification");
    expect(manifest).toContain("note uncertainty");
  });

  it("patch missing: target not in retrieved context is flagged", () => {
    const retrieval = makeRetrieval({
      included: [{ path: "src/a.ts", reason: "selected by you" }],
      includedPaths: ["src/a.ts"],
      omitted: [{ path: "src/b.ts", reason: "omitted (budget)" }],
      omittedPaths: ["src/b.ts"],
    });
    const report = buildEvidenceReport({
      symbolIndex: makeSymbolIndex(),
      analysisGraph: makeAnalysisGraph(),
      retrievalResult: retrieval,
      contextPlan: makeContextPlan({ filePlans: [{ path: "src/target.ts", included: false, tokens: 100, size: 200, omittedReason: "Budget exceeded" }] }),
      selectedPaths: ["src/a.ts"],
    });
    const missing = getPatchMissingEvidence("src/target.ts", report, retrieval, makeContextPlan({ filePlans: [{ path: "src/target.ts", included: false, tokens: 100, size: 200, omittedReason: "Budget exceeded" }] }));
    expect(missing.missing).toContain("src/target.ts");
    expect(missing.warnings.length).toBeGreaterThan(0);
    expect(missing.warnings.join(" ")).toMatch(/not included|partial/i);
  });

  it("patch not missing when target in included and budget", () => {
    const retrieval = makeRetrieval({
      included: [{ path: "src/target.ts", reason: "selected by you" }],
      includedPaths: ["src/target.ts"],
    });
    const plan = makeContextPlan({ filePlans: [{ path: "src/target.ts", included: true, tokens: 100, size: 200 }] });
    const report = buildEvidenceReport({
      symbolIndex: makeSymbolIndex(),
      analysisGraph: makeAnalysisGraph(),
      retrievalResult: retrieval,
      contextPlan: plan,
      selectedPaths: ["src/target.ts"],
    });
    const missing = getPatchMissingEvidence("src/target.ts", report, retrieval, plan);
    expect(missing.missing.length).toBe(0);
    // warnings may still exist if partial elsewhere, but should not claim missing target
    expect(missing.warnings.join(" ")).not.toContain("src/target.ts");
  });

  it("patch sensitive target is flagged", () => {
    const retrieval = makeRetrieval();
    const report = buildEvidenceReport({
      symbolIndex: null,
      analysisGraph: null,
      retrievalResult: retrieval,
      contextPlan: makeContextPlan(),
    });
    const missing = getPatchMissingEvidence(".env", report, retrieval, makeContextPlan());
    expect(missing.missing).toContain(".env");
    expect(missing.warnings.join(" ")).toMatch(/sensitive/i);
  });

  it("handles unsupported languages in limitations and unsupported chip", () => {
    const report = buildEvidenceReport({
      symbolIndex: makeSymbolIndex({ unsupported: 1 }),
      analysisGraph: makeAnalysisGraph(),
      retrievalResult: makeRetrieval({ included: [{ path: "docs/README.md", reason: "selected by you" }], includedPaths: ["docs/README.md"] }),
      contextPlan: makeContextPlan(),
      selectedPaths: ["docs/README.md"],
    });
    expect(report.isUnsupported).toBe(true);
    expect(report.limitations.unsupportedLanguages).toContain("md");
    expect(formatEvidenceForPrompt(report)).toContain("unsupported");
  });

  it("summary reflects partial and unsupported", () => {
    const report = buildEvidenceReport({
      symbolIndex: makeSymbolIndex({ truncated: true, unsupported: 1 }),
      analysisGraph: makeAnalysisGraph({ truncated: true }),
      retrievalResult: makeRetrieval({ included: [{ path: "src/a.ts", reason: "selected by you" }], includedPaths: ["src/a.ts"] }),
      contextPlan: makeContextPlan({ warnings: ["1 file(s) omitted"] , filePlans: [{ path: "src/b.ts", included: false, tokens: 10, size: 10, omittedReason: "Budget exceeded"}]}),
    });
    expect(report.summary).toMatch(/Partial context/);
    expect(report.summary).toMatch(/Unsupported/);
  });

  it("cancellation/incomplete streaming is communicated via manifest uncertainty note", () => {
    const report = buildEvidenceReport({
      symbolIndex: makeSymbolIndex(),
      analysisGraph: makeAnalysisGraph(),
      retrievalResult: makeRetrieval({ included: [{ path: "src/a.ts", reason: "selected by you" }], includedPaths: ["src/a.ts"] }),
      contextPlan: makeContextPlan(),
    });
    const manifest = formatEvidenceForPrompt(report);
    expect(manifest).toContain("If this response was truncated or cancelled, note uncertainty");
  });
});

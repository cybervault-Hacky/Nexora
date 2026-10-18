import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { EvidencePanel } from "./EvidencePanel";
import { buildEvidenceReport } from "../lib/evidence";
import type { SymbolIndex } from "../lib/symbolIndex";
import type { AnalysisGraph } from "../lib/analysis";
import type { RetrievalResult } from "../features/workspace/retrieval";
import type { ContextPlan } from "../features/workspace/contextBudget";

function idx(overrides: Partial<SymbolIndex> = {}): SymbolIndex {
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
function graph(overrides: Partial<AnalysisGraph> = {}): AnalysisGraph {
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
function retrieval(overrides: Partial<RetrievalResult> = {}): RetrievalResult {
  return { included: [], omitted: [], warnings: [], includedPaths: [], omittedPaths: [], ...overrides };
}
function plan(overrides: Partial<ContextPlan> = {}): ContextPlan {
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

describe("EvidencePanel", () => {
  it("returns null when no report", () => {
    const { container } = render(<EvidencePanel report={null} />);
    expect(container.innerHTML).toBe("");
  });

  it("shows Evidence available when verified", () => {
    const report = buildEvidenceReport({
      symbolIndex: idx(),
      analysisGraph: graph(),
      retrievalResult: retrieval({ included: [{ path: "src/a.ts", reason: "selected by you" }], includedPaths: ["src/a.ts"] }),
      contextPlan: plan(),
      selectedPaths: ["src/a.ts"],
    });
    render(<EvidencePanel report={report} />);
    expect(screen.getByText("Evidence available")).toBeTruthy();
  });

  it("shows Partial context when truncated or omitted", () => {
    const report = buildEvidenceReport({
      symbolIndex: idx({ truncated: true }),
      analysisGraph: graph({ truncated: true }),
      retrievalResult: retrieval({ included: [{ path: "src/a.ts", reason: "selected by you" }], includedPaths: ["src/a.ts"], omitted: [{ path: "src/b.ts", reason: "omitted (budget)" }], omittedPaths: ["src/b.ts"] }),
      contextPlan: plan(),
    });
    render(<EvidencePanel report={report} />);
    expect(screen.getByText("Partial context")).toBeTruthy();
  });

  it("shows Inferred relationship when inferred entries exist", () => {
    const report = buildEvidenceReport({
      symbolIndex: idx({ symbols: [{ name: "foo", kind: "function", path: "src/b.ts", startLine: 10, endLine: 12 } as any] }),
      analysisGraph: graph(),
      retrievalResult: retrieval({ included: [{ path: "src/b.ts", reason: "defines symbol `foo`", symbol: { name: "foo", kind: "function", path: "src/b.ts", startLine: 10, endLine: 12 } as any }], includedPaths: ["src/b.ts"] }),
      contextPlan: plan(),
    });
    render(<EvidencePanel report={report} />);
    expect(screen.getByText("Inferred relationship")).toBeTruthy();
  });

  it("shows Unsupported analysis when unsupported", () => {
    const report = buildEvidenceReport({
      symbolIndex: idx({ unsupported: 1 }),
      analysisGraph: graph({ unsupported: 1 }),
      retrievalResult: retrieval({ included: [{ path: "README.md", reason: "selected by you" }], includedPaths: ["README.md"] }),
      contextPlan: plan(),
      selectedPaths: ["README.md"],
    });
    render(<EvidencePanel report={report} />);
    expect(screen.getByText("Unsupported analysis")).toBeTruthy();
  });

  it("shows Verification not performed when not verified", () => {
    const report = buildEvidenceReport({
      symbolIndex: idx(),
      analysisGraph: graph(),
      retrievalResult: retrieval(),
      contextPlan: plan(),
      verificationPerformed: false,
    });
    render(<EvidencePanel report={report} />);
    expect(screen.getByText("Verification not performed")).toBeTruthy();
  });

  it("hides Verification not performed when verification was performed", () => {
    const report = buildEvidenceReport({
      symbolIndex: idx(),
      analysisGraph: graph(),
      retrievalResult: retrieval(),
      contextPlan: plan(),
      verificationPerformed: true,
    });
    render(<EvidencePanel report={report} />);
    expect(screen.queryByText("Verification not performed")).toBeFalsy();
  });

  it("shows Response incomplete when streaming", () => {
    const report = buildEvidenceReport({
      symbolIndex: idx(),
      analysisGraph: graph(),
      retrievalResult: retrieval({ included: [{ path: "src/a.ts", reason: "selected by you" }], includedPaths: ["src/a.ts"] }),
      contextPlan: plan(),
      verificationPerformed: true,
    });
    render(<EvidencePanel report={report} isStreaming={true} />);
    expect(screen.getByText("Response incomplete")).toBeTruthy();
  });

  it("renders manifest details bounded", () => {
    const report = buildEvidenceReport({
      symbolIndex: idx(),
      analysisGraph: graph(),
      retrievalResult: retrieval({ included: [{ path: "src/a.ts", reason: "selected by you" }], includedPaths: ["src/a.ts"] }),
      contextPlan: plan(),
    });
    render(<EvidencePanel report={report} />);
    // details summary should show manifest count
    expect(screen.getByText(/Evidence manifest \(1/)).toBeTruthy();
  });

  it("shows hint when no workspace files grounded", () => {
    const report = buildEvidenceReport({
      symbolIndex: null,
      analysisGraph: null,
      retrievalResult: null,
      contextPlan: null,
    });
    render(<EvidencePanel report={report} />);
    expect(screen.getAllByText(/No workspace files grounded/).length).toBeGreaterThanOrEqual(1);
  });

  it("shows summary with partial and unsupported", () => {
    const report = buildEvidenceReport({
      symbolIndex: idx({ truncated: true, unsupported: 1 }),
      analysisGraph: graph(),
      retrievalResult: retrieval({ included: [{ path: "src/a.ts", reason: "selected by you" }], includedPaths: ["src/a.ts"] }),
      contextPlan: plan({ filePlans: [{ path: "src/b.ts", included: false, tokens: 10, size: 10, omittedReason: "Budget exceeded" }] }),
    });
    render(<EvidencePanel report={report} />);
    expect(screen.getAllByText(/Partial context/).length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText(/Unsupported/).length).toBeGreaterThanOrEqual(1);
  });
});

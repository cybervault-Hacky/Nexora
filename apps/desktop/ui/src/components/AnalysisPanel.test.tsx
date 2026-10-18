import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { markDesktop } from "../test/fixtures";

vi.mock("../lib/analysis", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../lib/analysis")>();
  return {
    ...actual,
    getDependencyGraph: vi.fn(),
    getFileRelationships: vi.fn(),
    getChangeImpact: vi.fn(),
    clearAnalysisGraph: vi.fn().mockResolvedValue(false),
  };
});

import { getDependencyGraph, getFileRelationships, getChangeImpact } from "../lib/analysis";
import { AnalysisPanel } from "./AnalysisPanel";

const graphMock = vi.mocked(getDependencyGraph);
const relMock = vi.mocked(getFileRelationships);
const impactMock = vi.mocked(getChangeImpact);

function graph(overrides: any = {}) {
  return {
    version: 1,
    root: "/tmp/proj",
    nodes: [
      { path: "src/a.ts", imports: [{ from: "src/a.ts", to: "src/b.ts", raw: "./b", line: 2, verified: true, kind: "import", note: null }], importedBy: [] },
      { path: "src/b.ts", imports: [], importedBy: ["src/a.ts"] },
    ],
    edges: [{ from: "src/a.ts", to: "src/b.ts", raw: "./b", line: 2, verified: true, kind: "import", note: null }],
    truncated: false,
    unsupported: 0,
    cycles: [],
    generatedAt: Date.now(),
    ...overrides,
  };
}

beforeEach(() => {
  vi.clearAllMocks();
  graphMock.mockReset();
  relMock.mockReset();
  impactMock.mockReset();
  markDesktop(true);
});

describe("AnalysisPanel", () => {
  it("shows hint when no workspace", () => {
    render(<AnalysisPanel workspacePath={null} targetPath={null} onPreview={vi.fn()} />);
    expect(screen.getByText(/Open a project to see deep codebase analysis/)).toBeTruthy();
  });

  it("shows loading then graph overview", async () => {
    graphMock.mockImplementation(() => new Promise(() => {}));
    render(<AnalysisPanel workspacePath="/tmp/proj" targetPath={null} onPreview={vi.fn()} />);
    expect(await screen.findByText(/Building dependency graph/)).toBeTruthy();
  });

  it("renders graph with verified edges and cycles", async () => {
    const g = graph({ cycles: [["src/a.ts", "src/b.ts", "src/a.ts"]], truncated: true, unsupported: 1 });
    graphMock.mockResolvedValue(g as any);
    relMock.mockResolvedValue({ path: "src/a.ts", imports: [], importedBy: [], definedSymbols: [], referencedBy: [], unresolved: [], notes: [] } as any);
    impactMock.mockResolvedValue({ target: "src/a.ts", direct: [], transitive: [], risks: [], truncated: false, notes: [] } as any);
    render(<AnalysisPanel workspacePath="/tmp/proj" targetPath="src/a.ts" onPreview={vi.fn()} />);
    await waitFor(() => expect(screen.getByText(/files indexed/)).toBeTruthy());
    expect(screen.getByText(/1 circular/)).toBeTruthy();
    expect(screen.getAllByText(/truncated/).length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText(/verified/).length).toBeGreaterThanOrEqual(1);
  });

  it("shows unresolved and unsupported handling", async () => {
    const g = graph({
      nodes: [
        { path: "src/app.ts", imports: [{ from: "src/app.ts", to: "./missing", raw: "./missing", line: 2, verified: false, kind: "import", note: "unresolved — file not found" }], importedBy: [] },
      ],
      edges: [{ from: "src/app.ts", to: "./missing", raw: "./missing", line: 2, verified: false, kind: "import", note: "unresolved — file not found" }],
      unsupported: 2,
    });
    graphMock.mockResolvedValue(g as any);
    relMock.mockResolvedValue({
      path: "src/app.ts",
      imports: [{ from: "src/app.ts", to: "./missing", raw: "./missing", line: 2, verified: false, kind: "import", note: "unresolved — file not found" }],
      importedBy: [],
      definedSymbols: [],
      referencedBy: [],
      unresolved: [{ from: "src/app.ts", to: "./missing", raw: "./missing", line: 2, verified: false, kind: "import", note: "unresolved — file not found" }],
      notes: ["unresolved"],
    } as any);
    impactMock.mockResolvedValue({ target: "src/app.ts", direct: [], transitive: [], risks: ["No dependents"], truncated: false, notes: [] } as any);
    render(<AnalysisPanel workspacePath="/tmp/proj" targetPath="src/app.ts" onPreview={vi.fn()} />);
    await waitFor(() => expect(screen.getAllByText(/unresolved/).length).toBeGreaterThanOrEqual(1));
    expect(screen.getAllByText(/unresolved/)[0]).toBeTruthy();
  });

  it("distinguishes verified vs inferred in file relationships", async () => {
    const g = graph();
    graphMock.mockResolvedValue(g as any);
    relMock.mockResolvedValue({
      path: "src/a.ts",
      imports: [{ from: "src/a.ts", to: "src/b.ts", raw: "./b", line: 2, verified: true, kind: "import", note: null }],
      importedBy: ["src/c.ts"],
      definedSymbols: [{ name: "foo", kind: "function", path: "src/a.ts", startLine: 1, endLine: 1 }],
      referencedBy: [{ path: "src/c.ts", line: 5, symbol: "foo", snippet: "foo()" }],
      unresolved: [],
      notes: [],
    } as any);
    impactMock.mockResolvedValue({ target: "src/a.ts", direct: [], transitive: [], risks: [], truncated: false, notes: [] } as any);
    render(<AnalysisPanel workspacePath="/tmp/proj" targetPath="src/a.ts" onPreview={vi.fn()} />);
    await waitFor(() => expect(screen.getByText(/files indexed/)).toBeTruthy());
    // Click File tab
    const user = userEvent.setup();
    await user.click(screen.getByRole("tab", { name: "File" }));
    await waitFor(() => expect(screen.getByText(/Imported by/)).toBeTruthy());
    expect(screen.getAllByText(/verified/).length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText(/inferred/).length).toBeGreaterThanOrEqual(1);
  });

  it("shows impact with verified and inferred and risks", async () => {
    const g = graph();
    graphMock.mockResolvedValue(g as any);
    relMock.mockResolvedValue({ path: "src/base.ts", imports: [], importedBy: [], definedSymbols: [], referencedBy: [], unresolved: [], notes: [] } as any);
    impactMock.mockResolvedValue({
      target: "src/base.ts",
      direct: [{ path: "src/mid.ts", reason: "imports src/base.ts", evidence: "src/mid.ts imports src/base.ts at line 1", line: 1, confidence: "verified" }],
      transitive: [{ path: "src/top.ts", reason: "transitively via src/mid.ts", evidence: "top", line: null, confidence: "inferred" }],
      risks: ["1 direct + 1 transitive potential dependents", "Static analysis does not prove runtime"],
      truncated: false,
      notes: ["Confidence: verified = resolved"],
    } as any);
    render(<AnalysisPanel workspacePath="/tmp/proj" targetPath="src/base.ts" onPreview={vi.fn()} />);
    await waitFor(() => expect(screen.getByText(/files indexed/)).toBeTruthy());
    // Need to switch to Impact tab
    const user = userEvent.setup();
    await user.click(screen.getByRole("tab", { name: "Impact" }));
    await waitFor(() => expect(screen.getByText(/Impact for src\/base.ts/)).toBeTruthy());
    expect(screen.getAllByText(/src\/mid.ts/).length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText(/verified/).length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText(/inferred/).length).toBeGreaterThanOrEqual(1);
    expect(screen.getByText(/potential risk/)).toBeTruthy();
  });

  it("handles error states", async () => {
    graphMock.mockRejectedValue({ code: "invalid_request", message: "Project not found" });
    render(<AnalysisPanel workspacePath="/tmp/proj" targetPath={null} onPreview={vi.fn()} />);
    await waitFor(() => expect(screen.getByText(/Project not found/)).toBeTruthy());
  });

  it("handles workspace switching clears", async () => {
    const g = graph();
    graphMock.mockResolvedValue(g as any);
    relMock.mockResolvedValue({ path: "src/a.ts", imports: [], importedBy: [], definedSymbols: [], referencedBy: [], unresolved: [], notes: [] } as any);
    impactMock.mockResolvedValue({ target: "src/a.ts", direct: [], transitive: [], risks: [], truncated: false, notes: [] } as any);
    const { rerender } = render(<AnalysisPanel workspacePath="/tmp/proj" targetPath="src/a.ts" onPreview={vi.fn()} />);
    await waitFor(() => expect(screen.getByText(/files indexed/)).toBeTruthy());
    rerender(<AnalysisPanel workspacePath={null} targetPath={null} onPreview={vi.fn()} />);
    expect(screen.getByText(/Open a project/)).toBeTruthy();
  });

  it("shows sensitive exclusion via graph (no .env)", async () => {
    const g = graph({
      nodes: [{ path: "src/app.ts", imports: [], importedBy: [] }],
      edges: [],
      unsupported: 0,
    });
    graphMock.mockResolvedValue(g as any);
    relMock.mockResolvedValue({ path: "src/app.ts", imports: [], importedBy: [], definedSymbols: [], referencedBy: [], unresolved: [], notes: [] } as any);
    impactMock.mockResolvedValue({ target: "src/app.ts", direct: [], transitive: [], risks: [], truncated: false, notes: [] } as any);
    render(<AnalysisPanel workspacePath="/tmp/proj" targetPath="src/app.ts" onPreview={vi.fn()} />);
    await waitFor(() => expect(screen.getByText(/files indexed/)).toBeTruthy());
    expect(screen.queryByText(".env")).toBeFalsy();
  });

  it("handles unsupported language (no symbols)", async () => {
    const g = graph({ unsupported: 3 });
    graphMock.mockResolvedValue(g as any);
    relMock.mockResolvedValue({
      path: "README.md",
      imports: [],
      importedBy: [],
      definedSymbols: [],
      referencedBy: [],
      unresolved: [],
      notes: ["No symbols indexed for this file (unsupported language or empty)"],
    } as any);
    impactMock.mockResolvedValue({ target: "README.md", direct: [], transitive: [], risks: [], truncated: false, notes: [] } as any);
    render(<AnalysisPanel workspacePath="/tmp/proj" targetPath="README.md" onPreview={vi.fn()} />);
    await waitFor(() => expect(screen.getByText(/unsupported skipped/)).toBeTruthy());
    // Click file tab to see notes
    const user = userEvent.setup();
    await user.click(screen.getByRole("tab", { name: "File" }));
    await waitFor(() => expect(screen.getAllByText(/No symbols indexed/).length).toBeGreaterThanOrEqual(1));
  });
});

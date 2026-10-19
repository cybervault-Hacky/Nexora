import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { CodeIntelligencePage } from "./CodeIntelligencePage";
import type { SymbolIndex } from "../lib/symbolIndex";

vi.mock("../components/CodeIntelligencePanel", () => ({
  CodeIntelligencePanel: () => <div data-testid="mock-code-intel-panel">Mock Code Intelligence Panel</div>,
}));

vi.mock("../components/AnalysisPanel", () => ({
  AnalysisPanel: () => <div data-testid="mock-analysis-panel">Mock Analysis Panel</div>,
}));

function makeIndex(overrides: Partial<SymbolIndex> = {}): SymbolIndex {
  return {
    version: 1,
    root: "C:\\projects\\my-project",
    symbols: [
      { name: "handleRequest", kind: "function", path: "src/server.ts", startLine: 12, endLine: 30 },
      { name: "AppState", kind: "interface", path: "src/types.ts", startLine: 1, endLine: 10 },
      { name: "CONFIG", kind: "constant", path: "src/config.ts", startLine: 5, endLine: 5 },
    ],
    fileCount: 3,
    truncated: false,
    unsupported: 2,
    parseErrors: 1,
    generatedAt: Date.now(),
    ...overrides,
  };
}

describe("Phase 27 — CodeIntelligencePage component", () => {
  it("renders empty state call to action when no project is open", async () => {
    const onOpenWorkspace = vi.fn();
    const user = userEvent.setup();

    render(
      <CodeIntelligencePage
        workspacePath={null}
        onPreview={vi.fn()}
        symbolIndex={null}
        onIndexChange={vi.fn()}
        analysisTargetPath={null}
        onOpenWorkspace={onOpenWorkspace}
      />,
    );

    expect(screen.getByRole("heading", { name: "No project open" })).toBeTruthy();
    expect(screen.getByText(/Open a project directory to index source symbols/i)).toBeTruthy();

    const openBtn = screen.getByRole("button", { name: "Open project folder" });
    await user.click(openBtn);
    expect(onOpenWorkspace).toHaveBeenCalled();
  });

  it("renders active project header, summary metrics, and panels when project is open", () => {
    const idx = makeIndex();

    render(
      <CodeIntelligencePage
        workspacePath="C:\\projects\\my-project"
        workspaceName="my-project"
        onPreview={vi.fn()}
        symbolIndex={idx}
        onIndexChange={vi.fn()}
        analysisTargetPath="src/server.ts"
        onOpenWorkspace={vi.fn()}
      />,
    );

    expect(screen.getByRole("heading", { name: "Code intelligence · my-project" })).toBeTruthy();
    expect(screen.getAllByText(/my-project/).length).toBeGreaterThanOrEqual(1);

    const summary = screen.getByRole("status", { name: "Index summary" });
    expect(summary).toBeTruthy();
    expect(within(summary).getAllByText("3").length).toBe(2); // files & symbols
    expect(within(summary).getByText("Indexed files")).toBeTruthy();
    expect(within(summary).getByText("Symbols")).toBeTruthy();
    expect(within(summary).getByText("2")).toBeTruthy(); // unsupported
    expect(within(summary).getByText("Unsupported")).toBeTruthy();
    expect(within(summary).getByText("1")).toBeTruthy(); // parse errors
    expect(within(summary).getByText("Parse errors")).toBeTruthy();

    expect(screen.getByTestId("mock-code-intel-panel")).toBeTruthy();
    expect(screen.getByTestId("mock-analysis-panel")).toBeTruthy();
  });

  it("switches views using tabs between all, symbols, and analysis", async () => {
    const idx = makeIndex();
    const user = userEvent.setup();

    render(
      <CodeIntelligencePage
        workspacePath="C:\\projects\\my-project"
        workspaceName="my-project"
        onPreview={vi.fn()}
        symbolIndex={idx}
        onIndexChange={vi.fn()}
        analysisTargetPath="src/server.ts"
      />,
    );

    expect(screen.getByTestId("mock-code-intel-panel")).toBeTruthy();
    expect(screen.getByTestId("mock-analysis-panel")).toBeTruthy();

    // Switch to Symbols only
    const symbolsTab = screen.getByRole("tab", { name: /Symbol index & search/i });
    await user.click(symbolsTab);
    expect(screen.getByTestId("mock-code-intel-panel")).toBeTruthy();
    expect(screen.queryByTestId("mock-analysis-panel")).toBeNull();

    // Switch to Analysis only
    const analysisTab = screen.getByRole("tab", { name: /Dependency & impact analysis/i });
    await user.click(analysisTab);
    expect(screen.queryByTestId("mock-code-intel-panel")).toBeNull();
    expect(screen.getByTestId("mock-analysis-panel")).toBeTruthy();

    // Switch back to All
    const allTab = screen.getByRole("tab", { name: /All views/i });
    await user.click(allTab);
    expect(screen.getByTestId("mock-code-intel-panel")).toBeTruthy();
    expect(screen.getByTestId("mock-analysis-panel")).toBeTruthy();
  });

  it("handles switch project button click in header", async () => {
    const onOpenWorkspace = vi.fn();
    const user = userEvent.setup();

    render(
      <CodeIntelligencePage
        workspacePath="C:\\projects\\my-project"
        workspaceName="my-project"
        onPreview={vi.fn()}
        symbolIndex={null}
        onIndexChange={vi.fn()}
        analysisTargetPath={null}
        onOpenWorkspace={onOpenWorkspace}
      />,
    );

    const switchBtn = screen.getByRole("button", { name: "Switch project" });
    await user.click(switchBtn);
    expect(onOpenWorkspace).toHaveBeenCalled();
  });
});

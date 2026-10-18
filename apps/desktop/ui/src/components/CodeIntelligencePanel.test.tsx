import { describe, it, expect, vi, beforeEach } from "vitest";
import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { CodeIntelligencePanel } from "./CodeIntelligencePanel";
import type { SymbolIndex, Symbol } from "../lib/symbolIndex";

vi.mock("../lib/symbolIndex", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../lib/symbolIndex")>();
  return {
    ...actual,
    getSymbolIndex: vi.fn(),
    refreshSymbolIndex: vi.fn(),
    searchSymbols: vi.fn(),
  };
});

import { getSymbolIndex, refreshSymbolIndex, searchSymbols } from "../lib/symbolIndex";

const getMock = vi.mocked(getSymbolIndex);
const refreshMock = vi.mocked(refreshSymbolIndex);
const searchMock = vi.mocked(searchSymbols);

function makeIndex(overrides: Partial<SymbolIndex> = {}): SymbolIndex {
  return {
    version: 1,
    root: "/tmp/my-app",
    symbols: [
      { name: "hello", kind: "function", path: "src/main.rs", startLine: 10, endLine: 10, detail: "fn hello()" },
      { name: "MyClass", kind: "class", path: "src/lib.ts", startLine: 5, endLine: 20 },
    ],
    fileCount: 2,
    truncated: false,
    unsupported: 1,
    parseErrors: 0,
    generatedAt: Date.now(),
    ...overrides,
  };
}

beforeEach(() => {
  vi.clearAllMocks();
});

describe("CodeIntelligencePanel", () => {
  it("shows hint when no workspace", () => {
    render(<CodeIntelligencePanel workspacePath={null} onPreview={vi.fn()} />);
    expect(screen.getByText(/Open a project/i)).toBeTruthy();
  });

  it("shows overview after loading index", async () => {
    const idx = makeIndex();
    getMock.mockResolvedValue(idx);
    render(<CodeIntelligencePanel workspacePath="/tmp/my-app" onPreview={vi.fn()} />);
    await waitFor(() => expect(getMock).toHaveBeenCalledWith("/tmp/my-app"));
    await waitFor(() => expect(screen.getByText(/2 files indexed/i)).toBeTruthy());
    expect(screen.getByText(/2 symbols/i)).toBeTruthy();
    expect(screen.getByText(/1 unsupported/i)).toBeTruthy();
    expect(screen.getByText(/Rust, TypeScript, JavaScript, and Python via regex/i)).toBeTruthy();
    // file types
    expect(screen.getByText(".rs")).toBeTruthy();
    expect(screen.getByText(".ts")).toBeTruthy();
  });

  it("handles loading and error", async () => {
    getMock.mockRejectedValue({ code: "forbidden", message: "Project directory not found" });
    render(<CodeIntelligencePanel workspacePath="/tmp/my-app" onPreview={vi.fn()} />);
    await waitFor(() => expect(screen.getByText(/Project directory not found/i)).toBeTruthy());
    expect(screen.getByRole("button", { name: "Retry" })).toBeTruthy();
  });

  it("refreshes index on button click", async () => {
    const idx = makeIndex();
    const refreshed = makeIndex({ fileCount: 3, symbols: [...idx.symbols, { name: "newFn", kind: "function", path: "src/new.py", startLine: 1, endLine: 1 }] });
    getMock.mockResolvedValue(idx);
    refreshMock.mockResolvedValue(refreshed);
    const user = userEvent.setup();
    render(<CodeIntelligencePanel workspacePath="/tmp/my-app" onPreview={vi.fn()} />);
    await waitFor(() => expect(screen.getByText(/2 files indexed/i)).toBeTruthy());
    await user.click(screen.getByRole("button", { name: "Refresh index" }));
    await waitFor(() => expect(refreshMock).toHaveBeenCalled());
    await waitFor(() => expect(screen.getByText(/3 files indexed/i)).toBeTruthy());
  });

  it("searches symbols and previews file", async () => {
    const idx = makeIndex();
    getMock.mockResolvedValue(idx);
    const results: Symbol[] = [
      { name: "hello", kind: "function", path: "src/main.rs", startLine: 10, endLine: 10 },
      { name: "MyClass", kind: "class", path: "src/lib.ts", startLine: 5, endLine: 20 },
    ];
    searchMock.mockResolvedValue(results);
    const onPreview = vi.fn();
    const user = userEvent.setup();
    render(<CodeIntelligencePanel workspacePath="/tmp/my-app" onPreview={onPreview} />);
    await waitFor(() => expect(screen.getByText(/2 files indexed/i)).toBeTruthy());

    const input = screen.getByRole("textbox", { name: "Search symbols by name" });
    await user.type(input, "hello");
    await user.keyboard("{Enter}");
    await waitFor(() => expect(searchMock).toHaveBeenCalledWith("/tmp/my-app", "hello", 20));
    expect(screen.getByText("hello")).toBeTruthy();
    expect(screen.getByText("MyClass")).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "Open src/main.rs:10" }));
    expect(onPreview).toHaveBeenCalledWith("src/main.rs");
  });

  it("clears the symbol query and stale results", async () => {
    getMock.mockResolvedValue(makeIndex());
    searchMock.mockResolvedValue([
      { name: "hello", kind: "function", path: "src/main.rs", startLine: 10, endLine: 10 },
    ]);
    const user = userEvent.setup();
    render(<CodeIntelligencePanel workspacePath="/tmp/my-app" onPreview={vi.fn()} />);
    await waitFor(() => expect(screen.getByText(/2 files indexed/i)).toBeTruthy());

    const input = screen.getByRole("textbox", { name: "Search symbols by name" }) as HTMLInputElement;
    await user.type(input, "hello");
    await user.click(screen.getByRole("button", { name: "Search symbols" }));
    await waitFor(() => expect(screen.getByText("hello")).toBeTruthy());

    await user.click(screen.getByRole("button", { name: "Clear symbol search" }));
    expect(input.value).toBe("");
    expect(screen.queryByText("hello")).toBeNull();
    expect(searchMock).toHaveBeenCalledOnce();
  });

  it("ignores stale results after the symbol query changes", async () => {
    getMock.mockResolvedValue(makeIndex());
    let resolveOld: ((results: Symbol[]) => void) | undefined;
    let resolveNew: ((results: Symbol[]) => void) | undefined;
    searchMock.mockImplementation((_root, query) => new Promise<Symbol[]>((resolve) => {
      if (query === "old") resolveOld = resolve;
      if (query === "new") resolveNew = resolve;
    }));
    const user = userEvent.setup();
    render(<CodeIntelligencePanel workspacePath="/tmp/my-app" onPreview={vi.fn()} />);
    await waitFor(() => expect(screen.getByText(/2 files indexed/i)).toBeTruthy());

    const input = screen.getByRole("textbox", { name: "Search symbols by name" });
    await user.type(input, "old");
    await user.click(screen.getByRole("button", { name: "Search symbols" }));
    await waitFor(() => expect(searchMock).toHaveBeenCalledWith("/tmp/my-app", "old", 20));

    await user.clear(input);
    await user.type(input, "new");
    await user.click(screen.getByRole("button", { name: "Search symbols" }));
    await waitFor(() => expect(searchMock).toHaveBeenCalledWith("/tmp/my-app", "new", 20));

    await act(async () => {
      resolveNew?.([{ name: "newResult", kind: "function", path: "src/new.ts", startLine: 4, endLine: 4 }]);
    });
    expect(screen.getByText("newResult")).toBeTruthy();

    await act(async () => {
      resolveOld?.([{ name: "oldResult", kind: "function", path: "src/old.ts", startLine: 2, endLine: 2 }]);
    });
    expect(screen.getByText("newResult")).toBeTruthy();
    expect(screen.queryByText("oldResult")).toBeNull();
  });

  it("shows no symbols found for unsupported query", async () => {
    const idx = makeIndex();
    getMock.mockResolvedValue(idx);
    searchMock.mockResolvedValue([]);
    const user = userEvent.setup();
    render(<CodeIntelligencePanel workspacePath="/tmp/my-app" onPreview={vi.fn()} />);
    await waitFor(() => expect(screen.getByText(/2 files indexed/i)).toBeTruthy());
    const input = screen.getByRole("textbox", { name: "Search symbols by name" });
    await user.type(input, "unknownXYZ");
    expect(screen.queryByText(/No symbols found for/i)).toBeNull();
    await user.click(screen.getByRole("button", { name: "Search symbols" }));
    await waitFor(() => expect(searchMock).toHaveBeenCalled());
    expect(screen.getByText(/No symbols found for/i)).toBeTruthy();
    expect(screen.getByText(/Unsupported languages return no symbols/i)).toBeTruthy();
  });

  it("notifies parent via onIndexChange", async () => {
    const idx = makeIndex();
    getMock.mockResolvedValue(idx);
    const onIndexChange = vi.fn();
    render(<CodeIntelligencePanel workspacePath="/tmp/my-app" onPreview={vi.fn()} onIndexChange={onIndexChange} />);
    await waitFor(() => expect(onIndexChange).toHaveBeenCalledWith(expect.objectContaining({ fileCount: 2 })));
  });

  it("handles search error", async () => {
    const idx = makeIndex();
    getMock.mockResolvedValue(idx);
    searchMock.mockRejectedValue({ code: "invalid", message: "Invalid search" });
    const user = userEvent.setup();
    render(<CodeIntelligencePanel workspacePath="/tmp/my-app" onPreview={vi.fn()} />);
    await waitFor(() => expect(screen.getByText(/2 files indexed/i)).toBeTruthy());
    const input = screen.getByRole("textbox", { name: "Search symbols by name" });
    await user.type(input, "a");
    await user.click(screen.getByRole("button", { name: "Search symbols" }));
    await waitFor(() => expect(screen.getByText(/Invalid search/i)).toBeTruthy());
  });

  it("truncated warning", async () => {
    const idx = makeIndex({ truncated: true });
    getMock.mockResolvedValue(idx);
    render(<CodeIntelligencePanel workspacePath="/tmp/my-app" onPreview={vi.fn()} />);
    await waitFor(() => expect(screen.getByText(/truncated/i)).toBeTruthy());
  });
});

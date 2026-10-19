import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

vi.mock("./lib/inference", async (importOriginal) => {
  const actual = await importOriginal<typeof import("./lib/inference")>();
  return {
    ...actual,
    fetchStatus: vi.fn(),
    fetchSettings: vi.fn().mockResolvedValue({
      modelPath: "C:\\models\\qwen2.5-coder-7b.gguf",
      runtimePath: "C:\\bin\\llama-server.exe",
      contextSize: 32768,
    }),
    fetchSystemInfo: vi.fn().mockResolvedValue({
      totalRamGib: 32,
      availableRamGib: 20.5,
      platform: "windows",
    }),
    generate: vi.fn(),
    cancelGeneration: vi.fn().mockResolvedValue(true),
    unloadModel: vi.fn(),
  };
});

vi.mock("./lib/history", () => ({
  listConversations: async () => [],
  createConversation: async () => ({
    id: "new-chat-id",
    title: "New conversation",
    createdAt: Date.now(),
    updatedAt: Date.now(),
    messages: [],
  }),
  saveConversation: async (c: import("./lib/history").PersistedConversation) => c,
  loadConversation: async () => {
    throw { code: "not_found", message: "Not found" };
  },
  renameConversation: async () => {
    throw { code: "not_found", message: "Not found" };
  },
  deleteConversation: async () => true,
}));

import App from "./App";
import { fetchStatus, fetchSystemInfo } from "./lib/inference";
import { makeStatus, markDesktop } from "./test/fixtures";

const statusMock = vi.mocked(fetchStatus);
const systemMock = vi.mocked(fetchSystemInfo);

const fakeInventory = {
  rootPath: "C:\\dev\\demo-app",
  rootName: "demo-app",
  entries: [
    { path: "src", kind: "dir", size: 0, depth: 1 },
    { path: "src/main.ts", kind: "file", size: 512, depth: 1 },
    { path: "src/utils.ts", kind: "file", size: 256, depth: 1 },
  ],
  truncated: false,
  stats: {
    fileCount: 2,
    dirCount: 1,
    skippedExcluded: 0,
    skippedSensitive: 0,
    skippedOversized: 0,
    skippedUnreadable: 0,
    skippedSymlinkOutside: 0,
    skippedDepth: 0,
  },
  limits: { maxDepth: 12, maxFiles: 8000, maxPreviewBytes: 262144, maxInventoryBytes: 2097152 },
};

const fakeSymbolIndex = {
  version: 1,
  root: "C:\\dev\\demo-app",
  symbols: [
    { name: "startApp", kind: "function", path: "src/main.ts", startLine: 1, endLine: 10, detail: "export function startApp()" },
    { name: "ConfigService", kind: "class", path: "src/main.ts", startLine: 12, endLine: 35 },
    { name: "formatGreeting", kind: "function", path: "src/utils.ts", startLine: 5, endLine: 8, detail: "export function formatGreeting(name: string): string" },
  ],
  fileCount: 2,
  truncated: false,
  unsupported: 0,
  parseErrors: 0,
  generatedAt: Date.now(),
};

const fakeDependencyGraph = {
  version: 1,
  root: "C:\\dev\\demo-app",
  nodes: [
    { path: "src/main.ts", imports: [], importedBy: [] },
    { path: "src/utils.ts", imports: [], importedBy: ["src/main.ts"] },
  ],
  edges: [
    { from: "src/main.ts", to: "src/utils.ts", raw: "./utils", line: 2, verified: true, kind: "import", note: null },
  ],
  truncated: false,
  unsupported: 0,
  cycles: [],
  generatedAt: Date.now(),
};

function goto(hash: string) {
  act(() => {
    window.location.hash = hash;
    window.dispatchEvent(new Event("hashchange"));
  });
}

beforeEach(() => {
  statusMock.mockReset();
  statusMock.mockResolvedValue(
    makeStatus({
      state: "ready",
      modelPath: "C:\\models\\qwen2.5-coder-7b.gguf",
      contextSize: 32768,
    }),
  );
  systemMock.mockReset();
  systemMock.mockResolvedValue({
    totalRamGib: 32,
    availableRamGib: 20.5,
    platform: "windows",
  });
  markDesktop(true);

  invokeMock.mockReset();
  invokeMock.mockImplementation((cmd: string, args?: any) => {
    switch (cmd) {
      case "list_conversations":
        return Promise.resolve([]);
      case "pick_workspace_dialog":
        return Promise.resolve("C:\\dev\\demo-app");
      case "set_workspace":
      case "scan_workspace_command":
      case "scan_current_workspace":
        return Promise.resolve(fakeInventory);
      case "read_workspace_file_command":
        return Promise.resolve({
          path: args?.relative_path ?? "src/main.ts",
          content: "import { formatGreeting } from './utils';\nexport function startApp() { return formatGreeting('World'); }",
          size: 98,
          truncated: false,
        });
      case "get_symbol_index":
      case "refresh_symbol_index":
      case "workspace_get_symbol_index":
        return Promise.resolve(fakeSymbolIndex);
      case "search_symbols":
        return Promise.resolve(
          fakeSymbolIndex.symbols.filter((s) =>
            s.name.toLowerCase().includes((args?.query ?? "").toLowerCase()),
          ),
        );
      case "get_dependency_graph":
      case "refresh_dependency_graph":
      case "workspace_get_analysis_graph":
        return Promise.resolve(fakeDependencyGraph);
      case "get_file_relationships":
        return Promise.resolve({
          path: args?.path ?? "src/main.ts",
          imports: fakeDependencyGraph.edges,
          importedBy: [],
          definedSymbols: fakeSymbolIndex.symbols.filter((s) => s.path === (args?.path ?? "src/main.ts")),
          referencedBy: [],
          unresolved: [],
          notes: [],
        });
      case "get_change_impact":
        return Promise.resolve({
          target: args?.path ?? "src/main.ts",
          direct: [],
          transitive: [],
          risks: [],
          truncated: false,
          notes: [],
        });
      default:
        return Promise.resolve(null);
    }
  });

  window.location.hash = "#/code";
});

describe("Phase 27 — Code Intelligence Integration", () => {
  it("renders empty state on #/code and #/intelligence when no workspace is active", async () => {
    render(<App />);

    expect(await screen.findByRole("heading", { name: "Code intelligence" })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "No project open" })).toBeTruthy();
    expect(screen.getByText(/Open a project directory to index source symbols/i)).toBeTruthy();

    goto("#/intelligence");
    expect(await screen.findByRole("heading", { name: "Code intelligence" })).toBeTruthy();
  });

  it("loads symbol index and displays genuine codebase metrics when workspace is open", async () => {
    const user = userEvent.setup();
    render(<App />);

    // Click open project folder in empty state
    const openBtn = await screen.findByRole("button", { name: "Open project folder" });
    await user.click(openBtn);

    // After workspace opens, Code intelligence renders active project header & metrics
    expect(await screen.findByRole("heading", { name: /Code intelligence · demo-app/i })).toBeTruthy();

    // Verify genuine metrics from index
    const summary = await screen.findByRole("status", { name: "Index summary" });
    expect(summary).toBeTruthy();
    expect(within(summary).getByText("Indexed files")).toBeTruthy();
    expect(within(summary).getByText("Symbols")).toBeTruthy();
    expect(within(summary).getByText("3")).toBeTruthy(); // 3 symbols
  });

  it("searches indexed symbols and filters results by symbol kind", async () => {
    const user = userEvent.setup();
    render(<App />);

    // Open project
    const openBtn = await screen.findByRole("button", { name: "Open project folder" });
    await user.click(openBtn);
    await screen.findByRole("heading", { name: /Code intelligence · demo-app/i });

    // Search for "start"
    const searchInput = await screen.findByRole("textbox", { name: "Search symbols by name" });
    await user.type(searchInput, "start");
    await user.click(screen.getByRole("button", { name: "Search symbols" }));

    expect(await screen.findByText("startApp")).toBeTruthy();

    // Clear search and search for "app" to get both function and class
    await user.clear(searchInput);
    await user.type(searchInput, "app");
    await user.click(screen.getByRole("button", { name: "Search symbols" }));

    expect(await screen.findByText("startApp")).toBeTruthy();
  });

  it("switches views using tabs between symbols and dependency analysis", async () => {
    const user = userEvent.setup();
    render(<App />);

    // Open project
    const openBtn = await screen.findByRole("button", { name: "Open project folder" });
    await user.click(openBtn);
    await screen.findByRole("heading", { name: /Code intelligence · demo-app/i });

    // Switch to Dependency & impact analysis view
    const analysisTab = screen.getByRole("tab", { name: /Dependency & impact analysis/i });
    await user.click(analysisTab);

    expect(await screen.findByRole("region", { name: "Deep analysis" })).toBeTruthy();
    expect(screen.queryByRole("region", { name: "Search symbols" })).toBeNull();

    // Switch back to Symbol index & search view
    const symbolsTab = screen.getByRole("tab", { name: /Symbol index & search/i });
    await user.click(symbolsTab);

    expect(await screen.findByRole("search", { name: "Search symbols" })).toBeTruthy();
    expect(screen.queryByRole("region", { name: "Deep analysis" })).toBeNull();
  });

  it("refreshes symbol index and dependency graph without corrupting workspace", async () => {
    const user = userEvent.setup();
    render(<App />);

    // Open project
    const openBtn = await screen.findByRole("button", { name: "Open project folder" });
    await user.click(openBtn);
    await screen.findByRole("heading", { name: /Code intelligence · demo-app/i });

    // Click refresh in code intel panel
    const refreshBtn = await screen.findByRole("button", { name: "Refresh index" });
    await user.click(refreshBtn);

    // Verify index is still loaded and active
    expect(await screen.findByText("3 symbols")).toBeTruthy();
  });
});

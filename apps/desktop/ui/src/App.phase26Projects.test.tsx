import { render, screen } from "@testing-library/react";
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
    { path: "README.md", kind: "file", size: 128, depth: 0 },
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
        return Promise.resolve(fakeInventory);
      case "scan_workspace_command":
      case "scan_current_workspace":
        return Promise.resolve(fakeInventory);
      case "read_workspace_file_command":
        return Promise.resolve({
          path: args?.relative_path ?? "src/main.ts",
          content: "console.log('Hello from Nexora Demo');\nexport const VERSION = 1;",
          size: 58,
          truncated: false,
        });
      case "get_workspace_context":
        return Promise.resolve([
          {
            path: args?.paths?.[0] ?? "src/main.ts",
            content: "console.log('Hello from Nexora Demo');\nexport const VERSION = 1;",
            size: 58,
            truncated: false,
          },
        ]);
      case "close_workspace":
        return Promise.resolve(true);
      case "workspace_get_symbol_index":
        return Promise.resolve(null);
      case "workspace_get_analysis_graph":
        return Promise.resolve(null);
      default:
        return Promise.resolve(null);
    }
  });

  window.location.hash = "#/projects";
});

describe("Phase 26 — Projects Management & File Operations Integration", () => {
  it("renders Projects view with empty state when no project is open", async () => {
    render(<App />);

    expect(await screen.findByRole("heading", { name: "Projects" })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "No project open" })).toBeTruthy();
    expect(screen.getByText(/Open a project directory to browse files/i)).toBeTruthy();
  });

  it("opens a project, renders file explorer tree, and shows project header controls", async () => {
    const user = userEvent.setup();
    render(<App />);

    const openBtn = await screen.findByRole("button", { name: "Open project folder" });
    await user.click(openBtn);

    expect(await screen.findByRole("heading", { name: "demo-app" })).toBeTruthy();
    expect(await screen.findByText("src/main.ts")).toBeTruthy();
    expect(screen.getByText("README.md")).toBeTruthy();

    expect(screen.getAllByRole("button", { name: /Switch project/i }).length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByRole("button", { name: /Refresh/i }).length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByRole("button", { name: /Close/i }).length).toBeGreaterThanOrEqual(1);
  });

  it("selects a file for read-only preview without modifying disk", async () => {
    const user = userEvent.setup();
    render(<App />);

    const openBtn = await screen.findByRole("button", { name: "Open project folder" });
    await user.click(openBtn);

    const fileBtn = await screen.findByRole("button", { name: "Preview src/main.ts" });
    await user.click(fileBtn);

    expect(await screen.findByText(/Hello from Nexora Demo/)).toBeTruthy();
    expect(screen.getByRole("button", { name: "Propose change" })).toBeTruthy();
  });

  it("toggles file selection for grounded chat context", async () => {
    const user = userEvent.setup();
    render(<App />);

    const openBtn = await screen.findByRole("button", { name: "Open project folder" });
    await user.click(openBtn);

    const checkbox = await screen.findByRole("checkbox", { name: "Include src/main.ts as context" });
    expect((checkbox as HTMLInputElement).checked).toBe(false);

    await user.click(checkbox);
    expect((checkbox as HTMLInputElement).checked).toBe(true);
  });

  it("closes the active project cleanly and returns to empty state", async () => {
    const user = userEvent.setup();
    render(<App />);

    const openBtn = await screen.findByRole("button", { name: "Open project folder" });
    await user.click(openBtn);
    expect(await screen.findByRole("heading", { name: "demo-app" })).toBeTruthy();

    const closeBtn = screen.getByTitle("Close the current project");
    await user.click(closeBtn);

    expect(await screen.findByRole("heading", { name: "No project open" })).toBeTruthy();
  });
});

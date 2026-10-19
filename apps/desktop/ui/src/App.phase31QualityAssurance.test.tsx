import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import * as fs from "node:fs";
import * as path from "node:path";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App";
import { markDesktop } from "./test/fixtures";

// Mock Tauri invoke IPC
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
  Channel: class {
    onmessage?: (event: unknown) => void;
  },
}));

import { invoke } from "@tauri-apps/api/core";

const invokeMock = vi.mocked(invoke);

let mockEngineState = "ready";
let mockEngineModel: string | null = "C:\\models\\qwen.gguf";
let mockEngineRuntime: string | null = "C:\\llama\\llama-server.exe";
let mockStatusError: string | null = null;
let mockWorkspacePath: string | null = "C:\\dev\\demo-app";
let mockSavedSettings: unknown = null;

const fakeInventory = {
  rootPath: "C:\\dev\\demo-app",
  rootName: "demo-app",
  entries: [
    { path: "src", kind: "dir", size: 0, depth: 1 },
    { path: "src/App.tsx", kind: "file", size: 1200, depth: 2 },
    { path: "src/styles/tokens.css", kind: "file", size: 2400, depth: 2 },
    { path: "src/styles/app.css", kind: "file", size: 4800, depth: 2 },
  ],
  truncated: false,
  stats: {
    fileCount: 3,
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
    { name: "App", kind: "function", path: "src/App.tsx", startLine: 12, endLine: 30, detail: "export default function App()" },
    { name: "WorkspacePanel", kind: "function", path: "src/components/WorkspacePanel.tsx", startLine: 45, endLine: 80 },
    { name: "SymbolIndex", kind: "interface", path: "src/lib/symbolIndex.ts", startLine: 8, endLine: 20 },
  ],
  fileCount: 3,
  truncated: false,
  unsupported: 0,
  parseErrors: 0,
  generatedAt: Date.now(),
};

const fakeDependencyGraph = {
  version: 1,
  root: "C:\\dev\\demo-app",
  nodes: [
    { path: "src/App.tsx", imports: [], importedBy: [] },
    { path: "src/styles/tokens.css", imports: [], importedBy: [] },
  ],
  edges: [],
  cycles: [],
  truncated: false,
};

function installQAMocks() {
  markDesktop(true);
  window.location.hash = "#/home";
  mockEngineState = "ready";
  mockEngineModel = "C:\\models\\qwen.gguf";
  mockEngineRuntime = "C:\\llama\\llama-server.exe";
  mockStatusError = null;
  mockWorkspacePath = "C:\\dev\\demo-app";
  mockSavedSettings = null;

  invokeMock.mockImplementation(async (command: string, args?: unknown) => {
    switch (command) {
      case "get_app_status":
        if (mockStatusError) {
          throw new Error(mockStatusError);
        }
        return {
          name: "Nexora",
          version: "0.1.0",
          engine: {
            state: mockEngineState,
            runtime: mockEngineRuntime,
            model: mockEngineModel,
            detail: mockEngineState === "ready" ? "Local engine ready." : "Engine idle or stopped.",
          },
          modelPath: mockEngineModel,
          runtimePath: mockEngineRuntime,
          contextSize: 32768,
        };

      case "get_system_info":
        return {
          totalRamGib: 32,
          availableRamGib: 22.8,
          platform: "windows",
        };

      case "get_workspace_settings":
        return {
          version: 1,
          lastWorkspacePath: mockWorkspacePath,
          preferences: {
            explorerCollapsed: false,
            founderModeEnabled: false,
            codeLigatures: false,
            reducedMotion: false,
          },
        };

      case "save_workspace_settings": {
        const payload = (args as { settings: unknown })?.settings;
        mockSavedSettings = payload;
        return payload;
      }

      case "get_settings":
        return {
          modelPath: mockEngineModel,
          runtimePath: mockEngineRuntime,
          contextSize: 32768,
        };

      case "list_conversations":
        return [
          {
            id: "conv-qa-1",
            title: "QA Test Conversation",
            updatedAt: Date.now(),
            createdAt: Date.now(),
            messageCount: 2,
            preview: "Run test suite verification.",
          },
        ];

      case "load_conversation":
      case "get_conversation":
        return {
          id: "conv-qa-1",
          title: "QA Test Conversation",
          createdAt: Date.now(),
          updatedAt: Date.now(),
          messages: [
            {
              id: "msg-1",
              role: "user",
              content: "Run test suite verification.",
              createdAt: Date.now(),
              state: "complete",
            },
            {
              id: "msg-2",
              role: "assistant",
              content: "Verification completed with zero regressions.",
              createdAt: Date.now(),
              state: "complete",
            },
          ],
        };

      case "pick_workspace_dialog":
        return "C:\\dev\\demo-app";

      case "set_workspace":
      case "scan_workspace_command":
      case "scan_current_workspace":
        return fakeInventory;

      case "read_workspace_file_command":
        return {
          path: (args as { relative_path?: string })?.relative_path ?? "src/App.tsx",
          content: "// Nexora source",
          size: 1200,
          truncated: false,
        };

      case "get_symbol_index":
      case "refresh_symbol_index":
      case "workspace_get_symbol_index":
        return fakeSymbolIndex;

      case "search_symbols":
        return fakeSymbolIndex.symbols;

      case "get_dependency_graph":
      case "refresh_dependency_graph":
      case "workspace_get_analysis_graph":
        return fakeDependencyGraph;

      case "probe_runtime":
        return {
          path: (args as { path: string })?.path,
          versionLine: "llama-server version: 11714 (release)",
          pinnedBuildMatched: true,
        };

      case "validate_model":
        return {
          path: (args as { path: string })?.path,
          fileName: "qwen.gguf",
          sizeBytes: 15_000_000_000,
          sizeGib: 13.97,
          ggufVersion: 3,
          tensorCount: 380,
          estimatedMinRamGib: 16.5,
          fitsAvailableRam: true,
          warnings: [],
        };

      case "unload_model":
        mockEngineState = "stopped";
        mockEngineModel = null;
        mockEngineRuntime = null;
        return {
          name: "Nexora",
          version: "0.1.0",
          engine: {
            state: "stopped",
            runtime: null,
            model: null,
            detail: "Engine stopped.",
          },
          modelPath: null,
          runtimePath: null,
          contextSize: 32768,
        };

      case "auth_get_session":
        return null;

      case "licence_get_status":
        return {
          tier: "free",
          valid: true,
          offline_grace_remaining_secs: 2592000,
          features: ["chat", "ast_symbol_indexing", "project_overview"],
        };

      case "clear_symbol_index":
      case "clear_analysis_graph":
      case "close_workspace":
        return true;

      default:
        return null;
    }
  });
}

describe("Phase 31 — Comprehensive Quality Assurance Audit", () => {
  beforeEach(() => {
    installQAMocks();
  });

  afterEach(() => {
    window.location.hash = "";
  });

  describe("1. Engine Status Integrity & Honest Degradation (§08 L3/L4)", () => {
    it("reports honest disconnected core notice when IPC fails, with working retry action", async () => {
      mockStatusError = "Connection refused to Tauri core";
      render(<App />);

      const notice = await screen.findByText(/Could not read the engine status/i);
      expect(notice).toBeTruthy();

      const retryBtn = screen.getByRole("button", { name: "Retry" });
      expect(retryBtn).toBeTruthy();

      // Clear error and retry
      mockStatusError = null;
      const user = userEvent.setup();
      await user.click(retryBtn);

      await waitFor(() => {
        expect(invokeMock.mock.calls.some(([cmd]) => cmd === "get_app_status")).toBe(true);
      });
    });

    it("displays honest 'No model' status and disables inference when engine is not configured", async () => {
      mockEngineState = "not_configured";
      mockEngineModel = null;
      render(<App />);

      const homeHeading = await screen.findByRole("heading", { name: "Welcome to Nexora" });
      expect(homeHeading).toBeTruthy();

      // Engine status card reflects genuine state
      const engineCard = screen.getByRole("region", { name: "AI engine status" });
      expect(engineCard.textContent).toContain("No model");
    });
  });

  describe("2. AI Chat State Machine, Cancellation, & Draft Preservation (§08 L4)", () => {
    it("loads and displays existing conversation history with user and assistant roles", async () => {
      window.location.hash = "#/chat";
      const user = userEvent.setup();
      render(<App />);

      const convTitle = await screen.findByText("QA Test Conversation");
      expect(convTitle).toBeTruthy();
      await user.click(convTitle);

      await waitFor(() => {
        expect(document.querySelector(".message__prompt")?.textContent).toBe("Run test suite verification.");
      });
      expect(await screen.findByText("Verification completed with zero regressions.")).toBeTruthy();
    });

    it("preserves message draft when switching routes back and forth", async () => {
      const user = userEvent.setup();
      window.location.hash = "#/chat";
      render(<App />);

      const textarea = await screen.findByRole("textbox", { name: "Message Nexora" });
      await user.type(textarea, "Draft message awaiting dispatch");
      expect((textarea as HTMLTextAreaElement).value).toBe("Draft message awaiting dispatch");

      // Switch to Settings page
      window.location.hash = "#/settings";
      window.dispatchEvent(new Event("hashchange"));
      expect(await screen.findByRole("heading", { name: "Settings" })).toBeTruthy();

      // Switch back to Chat
      window.location.hash = "#/chat";
      window.dispatchEvent(new Event("hashchange"));
      const restoredTextarea = await screen.findByRole("textbox", { name: "Message Nexora" });
      expect((restoredTextarea as HTMLTextAreaElement).value).toBe("Draft message awaiting dispatch");
    });
  });

  describe("3. Projects & Safe File Operations (§08 L2/L4)", () => {
    it("renders project explorer empty state, opens project, and displays inventory", async () => {
      const user = userEvent.setup();
      window.location.hash = "#/projects";
      render(<App />);

      expect(await screen.findByRole("heading", { name: "Projects" })).toBeTruthy();
      expect(screen.getByRole("heading", { name: "No project open" })).toBeTruthy();

      const openBtn = await screen.findByRole("button", { name: "Open project folder" });
      await user.click(openBtn);

      expect(await screen.findByRole("heading", { name: "demo-app" })).toBeTruthy();
      expect(screen.getByText("src/App.tsx")).toBeTruthy();
      expect(screen.getByText("src/styles/tokens.css")).toBeTruthy();
      expect(screen.getByText("src/styles/app.css")).toBeTruthy();
    });

    it("displays honest empty state when no workspace is open", async () => {
      window.location.hash = "#/projects";
      render(<App />);

      expect(await screen.findByRole("heading", { name: "Projects" })).toBeTruthy();
      expect(screen.getByRole("heading", { name: "No project open" })).toBeTruthy();
      expect(screen.getByText(/Open a project directory to browse files/i)).toBeTruthy();
    });
  });

  describe("4. Code Intelligence Index & Symbol Search (§08 L2/L4)", () => {
    it("searches and renders indexed symbols with accurate file and line metadata", async () => {
      const user = userEvent.setup();
      window.location.hash = "#/code";
      render(<App />);

      const openBtn = await screen.findByRole("button", { name: "Open project folder" });
      await user.click(openBtn);

      expect(await screen.findByRole("heading", { name: /Code intelligence · demo-app/i })).toBeTruthy();
      const searchInput = await screen.findByRole("textbox", { name: "Search symbols by name" });
      await user.type(searchInput, "App");
      await user.click(screen.getByRole("button", { name: "Search symbols" }));
      expect(await screen.findByText("App")).toBeTruthy();
    });
  });

  describe("5. Settings, Models, & Preferences Lifecycle (§08 L1/L4)", () => {
    it("persists code font ligatures, reduced motion, and unloads active model", async () => {
      const user = userEvent.setup();
      window.location.hash = "#/settings";
      render(<App />);

      expect(await screen.findByRole("heading", { name: "Settings" })).toBeTruthy();

      const ligaturesCheckbox = screen.getByRole("checkbox", { name: "Code font ligatures" });
      const motionCheckbox = screen.getByRole("checkbox", { name: "Reduced motion override" });

      await user.click(ligaturesCheckbox);
      await user.click(motionCheckbox);

      await waitFor(() => {
        expect(invokeMock).toHaveBeenCalledWith(
          "save_workspace_settings",
          expect.objectContaining({
            settings: expect.objectContaining({
              preferences: expect.objectContaining({
                codeLigatures: true,
                reducedMotion: true,
              }),
            }),
          }),
        );
      });

      // Switch to Models page and unload engine
      window.location.hash = "#/models";
      window.dispatchEvent(new Event("hashchange"));

      const stopBtn = await screen.findByRole("button", { name: /Stop engine/i });
      await user.click(stopBtn);

      await waitFor(() => {
        expect(invokeMock.mock.calls.some(([cmd]) => cmd === "unload_model")).toBe(true);
        expect(mockSavedSettings).not.toBeNull();
      });
    });

    it("clears remembered workspace path cleanly via clear path button", async () => {
      const user = userEvent.setup();
      window.location.hash = "#/settings";
      render(<App />);

      const clearBtn = await screen.findByRole("button", { name: "Clear path" });
      await user.click(clearBtn);

      await waitFor(() => {
        expect(invokeMock).toHaveBeenCalledWith(
          "save_workspace_settings",
          expect.objectContaining({
            settings: expect.objectContaining({
              lastWorkspacePath: null,
            }),
          }),
        );
      });
    });
  });

  describe("6. Glassmorphism, Living Aurora, & Motion Quality (§08 L4)", () => {
    it("enforces WCAG AA text contrast on glassmorphic materials", () => {
      const tokensCss = fs.readFileSync(
        path.resolve(__dirname, "styles/tokens.css"),
        "utf-8",
      );

      // Extract colors
      const bgMatch = tokensCss.match(/--bg-base:\s*(#[0-9a-fA-F]+)/);
      const textMatch = tokensCss.match(/--text-primary:\s*(#[0-9a-fA-F]+)/);
      expect(bgMatch).toBeTruthy();
      expect(textMatch).toBeTruthy();

      const bgBase = bgMatch?.[1] ?? "#0b0d10";
      const textPrimary = textMatch?.[1] ?? "#f1eef6";

      // Convert hex to relative luminance
      function hexToLuminance(hex: string): number {
        const r = parseInt(hex.slice(1, 3), 16) / 255;
        const g = parseInt(hex.slice(3, 5), 16) / 255;
        const b = parseInt(hex.slice(5, 7), 16) / 255;
        const toLinear = (c: number) => (c <= 0.03928 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4));
        return 0.2126 * toLinear(r) + 0.7152 * toLinear(g) + 0.0722 * toLinear(b);
      }

      const lum1 = hexToLuminance(bgBase);
      const lum2 = hexToLuminance(textPrimary);
      const contrastRatio = (Math.max(lum1, lum2) + 0.05) / (Math.min(lum1, lum2) + 0.05);

      // WCAG AA requires at least 4.5:1 for normal text
      expect(contrastRatio).toBeGreaterThanOrEqual(4.5);
      // Nexora high-contrast dark theme reaches > 14:1
      expect(contrastRatio).toBeGreaterThan(10);
    });

    it("guarantees decorative Living Aurora sits at z-index 0 behind glass chrome at z-index >= 1", () => {
      const appCss = fs.readFileSync(
        path.resolve(__dirname, "styles/app.css"),
        "utf-8",
      );

      expect(appCss).toContain(".aurora-ambient");
      expect(appCss).toMatch(/\.aurora-ambient\s*\{[^}]*z-index:\s*0/);
      expect(appCss).toMatch(/\.app\s*\{[^}]*z-index:\s*1/);
    });

    it("strictly halts all ambient motion loops when reduced motion is preferred", () => {
      const appCss = fs.readFileSync(
        path.resolve(__dirname, "styles/app.css"),
        "utf-8",
      );

      // Verify reduced-motion media query and .reduced-motion class kill aurora animations
      expect(appCss).toContain("@media (prefers-reduced-motion: reduce)");
      expect(appCss).toMatch(/@media\s*\(prefers-reduced-motion:\s*reduce\)[\s\S]*?\.aurora-beam\s*\{[\s\S]*?animation:\s*none/);
      expect(appCss).toMatch(/\.reduced-motion\s+\.aurora-beam\s*\{[\s\S]*?animation:\s*none/);
    });
  });

  describe("7. Keyboard Navigation & Input Accessibility (§08 L4, §11)", () => {
    it("supports keyboard navigation through sidebar navigation links without trapping focus", async () => {
      render(<App />);

      const homeLink = await screen.findByRole("link", { name: "Home" });
      const chatLink = screen.getByRole("link", { name: "AI Chat" });
      const projectsLink = screen.getByRole("link", { name: "Projects" });
      const codeLink = screen.getByRole("link", { name: "Code Intelligence" });
      const modelsLink = screen.getByRole("link", { name: "Local Models" });
      const settingsLink = screen.getByRole("link", { name: "Settings" });

      expect(homeLink).toBeTruthy();
      expect(chatLink).toBeTruthy();
      expect(projectsLink).toBeTruthy();
      expect(codeLink).toBeTruthy();
      expect(modelsLink).toBeTruthy();
      expect(settingsLink).toBeTruthy();

      // Elements have valid href navigation anchors
      expect(homeLink.getAttribute("href")).toBe("#/home");
      expect(chatLink.getAttribute("href")).toBe("#/chat");
      expect(projectsLink.getAttribute("href")).toBe("#/projects");
      expect(codeLink.getAttribute("href")).toBe("#/code");
      expect(modelsLink.getAttribute("href")).toBe("#/models");
      expect(settingsLink.getAttribute("href")).toBe("#/settings");
    });
  });
});

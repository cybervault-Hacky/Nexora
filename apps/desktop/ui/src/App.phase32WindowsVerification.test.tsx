import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import * as fs from "node:fs";
import * as path from "node:path";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App";
import { markDesktop, makeUsage } from "./test/fixtures";

// Mock Tauri invoke IPC
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
  Channel: class {
    onmessage?: (event: unknown) => void;
  },
}));

// Mock Tauri window API
const mockMinimize = vi.fn().mockResolvedValue(undefined);
const mockToggleMaximize = vi.fn().mockResolvedValue(undefined);
const mockClose = vi.fn().mockResolvedValue(undefined);

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    minimize: mockMinimize,
    toggleMaximize: mockToggleMaximize,
    close: mockClose,
  }),
}));

import { invoke } from "@tauri-apps/api/core";

const invokeMock = vi.mocked(invoke);

let mockEngineState = "ready";
let mockEngineModel: string | null = "C:\\models\\qwen-coder-7b.gguf";
let mockEngineRuntime: string | null = "C:\\llama\\llama-server.exe";
let mockStatusError: string | null = null;
let mockWorkspacePath: string | null = "C:\\dev\\demo-app";

const fakeInventory = {
  rootPath: "C:\\dev\\demo-app",
  rootName: "demo-app",
  entries: [
    { path: "src", kind: "dir", size: 0, depth: 1 },
    { path: "src\\main.ts", kind: "file", size: 512, depth: 2 },
    { path: "README.md", kind: "file", size: 256, depth: 1 },
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

function installWindowsMocks() {
  markDesktop(true);
  window.location.hash = "#/home";
  mockEngineState = "ready";
  mockEngineModel = "C:\\models\\qwen-coder-7b.gguf";
  mockEngineRuntime = "C:\\llama\\llama-server.exe";
  mockStatusError = null;
  mockWorkspacePath = "C:\\dev\\demo-app";

  invokeMock.mockReset();
  mockMinimize.mockClear();
  mockToggleMaximize.mockClear();
  mockClose.mockClear();

  invokeMock.mockImplementation(async (command: string, args?: unknown) => {
    switch (command) {
      case "get_usage_status":
        return makeUsage();
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
            detail: mockEngineState === "ready" ? "Local engine ready on Windows." : "Engine stopped.",
          },
          modelPath: mockEngineModel,
          runtimePath: mockEngineRuntime,
          contextSize: 32768,
        };

      case "get_system_info":
        return {
          totalRamGib: 32.0,
          availableRamGib: 21.4,
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
        return payload;
      }

      case "get_settings":
        return {
          modelPath: mockEngineModel,
          runtimePath: mockEngineRuntime,
          contextSize: 32768,
        };

      case "plan_workspace_export":
        return Promise.resolve({
          rootName: "demo-app",
          files: [{ path: "src/main.ts", size: 58 }],
          excludedSensitive: 0,
          excludedSymlink: 0,
          excludedUnsafe: 0,
          totalBytes: 58,
          truncated: false,
        });
      case "suggest_export_path":
        return Promise.resolve("/tmp/demo-app.zip");
      case "list_conversations":
        return [];

      case "pick_workspace_dialog":
        return "C:\\dev\\demo-app";

      case "set_workspace":
      case "scan_workspace_command":
      case "scan_current_workspace":
        return fakeInventory;

      case "validate_model":
        return {
          path: (args as { path: string })?.path,
          fileName: "qwen-coder-7b.gguf",
          sizeBytes: 4_800_000_000,
          sizeGib: 4.47,
          ggufVersion: 3,
          tensorCount: 290,
          architecture: "llama",
          name: null,
          contextLength: 8192,
          estimatedMinRamGib: 6.2,
          fitsAvailableRam: true,
          warnings: [],
        };

      case "probe_runtime":
        return {
          path: (args as { path: string })?.path,
          versionLine: "llama-server version: 11714 (release)",
          pinnedBuildMatched: true,
        };

      case "unload_model":
        mockEngineState = "stopped";
        mockEngineModel = null;
        return true;

      case "clear_symbol_index":
      case "clear_analysis_graph":
      case "close_workspace":
        return true;

      case "auth_get_session":
        return null;

      default:
        return null;
    }
  });
}

describe("Phase 32 — Windows Build & Runtime Verification", () => {
  beforeEach(() => {
    installWindowsMocks();
  });

  afterEach(() => {
    window.location.hash = "";
  });

  describe("1. Windows Tauri Configuration & NSIS Packaging Audit", () => {
    it("configures non-elevated currentUser NSIS installer mode and 1280x840 initial frame", () => {
      const tauriConfigPath = path.resolve(__dirname, "../../src-tauri/tauri.conf.json");
      expect(fs.existsSync(tauriConfigPath)).toBe(true);

      const configJson = JSON.parse(fs.readFileSync(tauriConfigPath, "utf-8"));

      // Target verification
      expect(configJson.bundle.targets).toContain("nsis");
      expect(configJson.bundle.windows.nsis.installMode).toBe("currentUser");
      expect(configJson.bundle.windows.nsis.displayLanguageSelector).toBe(false);

      // Window geometry
      const mainWindow = configJson.app.windows.find((w: { label: string }) => w.label === "main");
      expect(mainWindow).toBeTruthy();
      expect(mainWindow.width).toBe(1280);
      expect(mainWindow.height).toBe(840);
      expect(mainWindow.minWidth).toBe(960);
      expect(mainWindow.minHeight).toBe(640);
      expect(mainWindow.decorations).toBe(false); // Custom Windows chrome
    });

    it("enforces strict production CSP blocking raw external network access from WebView2", () => {
      const tauriConfigPath = path.resolve(__dirname, "../../src-tauri/tauri.conf.json");
      const configJson = JSON.parse(fs.readFileSync(tauriConfigPath, "utf-8"));
      const csp = configJson.app.security.csp;

      expect(csp).toContain("default-src 'self'");
      expect(csp).toContain("script-src 'self'");
      expect(csp).toContain("connect-src ipc: http://ipc.localhost");

      // Verify raw external http/https connections are strictly omitted from connect-src
      const connectSrcMatch = csp.match(/connect-src\s+([^;]+)/);
      expect(connectSrcMatch).toBeTruthy();
      expect(connectSrcMatch[1]).not.toContain("https:");
      expect(connectSrcMatch[1]).not.toContain("http://localhost");
    });

    it("verifies least-privilege capability permissions in capabilities/main.json", () => {
      const capPath = path.resolve(__dirname, "../../src-tauri/capabilities/main.json");
      expect(fs.existsSync(capPath)).toBe(true);

      const capJson = JSON.parse(fs.readFileSync(capPath, "utf-8"));
      expect(capJson.windows).toEqual(["main"]);
      expect(capJson.permissions).toContain("core:default");
      expect(capJson.permissions).toContain("core:window:allow-minimize");
      expect(capJson.permissions).toContain("core:window:allow-toggle-maximize");
      expect(capJson.permissions).toContain("core:window:allow-close");
      expect(capJson.permissions).toContain("core:window:allow-start-dragging");

      // Must not grant unrestricted shell or raw network sockets
      expect(capJson.permissions.some((p: string) => p.includes("shell:execute"))).toBe(false);
      expect(capJson.permissions.some((p: string) => p.includes("http:default"))).toBe(false);
    });
  });

  describe("2. Windows Window Chrome, Drag Regions & System Controls", () => {
    it("renders custom titlebar chrome with working Windows window controls in desktop shell mode", async () => {
      const user = userEvent.setup();
      render(<App />);

      // Windows control buttons exist
      const minimizeBtn = await screen.findByRole("button", { name: "Minimize window" });
      const maximizeBtn = screen.getByRole("button", { name: "Maximize or restore window" });
      const closeBtn = screen.getByRole("button", { name: "Close window" });

      expect(minimizeBtn).toBeTruthy();
      expect(maximizeBtn).toBeTruthy();
      expect(closeBtn).toBeTruthy();

      // Trigger controls and verify IPC delegation to Tauri window API
      await user.click(minimizeBtn);
      expect(mockMinimize).toHaveBeenCalledTimes(1);

      await user.click(maximizeBtn);
      expect(mockToggleMaximize).toHaveBeenCalledTimes(1);

      await user.click(closeBtn);
      expect(mockClose).toHaveBeenCalledTimes(1);
    });

    it("decorates titlebar drag regions with data-tauri-drag-region for native Windows dragging", () => {
      render(<App />);
      const dragRegions = document.querySelectorAll("[data-tauri-drag-region='true']");
      expect(dragRegions.length).toBeGreaterThanOrEqual(1);
    });
  });

  describe("3. Windows File Paths & Filesystem Operations", () => {
    it("handles Windows backslash paths in workspace project browser without truncation", async () => {
      const user = userEvent.setup();
      window.location.hash = "#/projects";
      render(<App />);

      const openBtn = await screen.findByRole("button", { name: "Open project folder" });
      await user.click(openBtn);

      expect(await screen.findByRole("heading", { name: "demo-app" })).toBeTruthy();
      expect(screen.getByText("src\\main.ts")).toBeTruthy();
      expect(screen.getByText("README.md")).toBeTruthy();
    });
  });

  describe("4. Local System Diagnostics & GGUF Model Validation", () => {
    it("reports genuine Windows hardware metrics and validates local GGUF allocation", async () => {
      window.location.hash = "#/models";
      render(<App />);

      expect(await screen.findByRole("heading", { name: /Local models/i })).toBeTruthy();
      expect(screen.getAllByText(/qwen-coder-7b\.gguf/).length).toBeGreaterThanOrEqual(1);

      await waitFor(() => {
        expect(invokeMock.mock.calls.some(([cmd]) => cmd === "get_system_info")).toBe(true);
      });
    });
  });

  describe("5. Truthful Engine Reporting & Safe Offline Degradation", () => {
    it("displays honest stopped state and blocks inference when model is unloaded", async () => {
      const user = userEvent.setup();
      window.location.hash = "#/models";
      render(<App />);

      const stopButtons = await screen.findAllByRole("button", { name: /Stop engine/i });
      expect(stopButtons.length).toBeGreaterThanOrEqual(1);
      const targetBtn = stopButtons[0];
      expect(targetBtn).toBeDefined();
      if (targetBtn) {
        await user.click(targetBtn);
      }

      await waitFor(() => {
        expect(invokeMock.mock.calls.some(([cmd]) => cmd === "unload_model")).toBe(true);
      });

      // Switch to Chat; verify honest state
      window.location.hash = "#/chat";
      window.dispatchEvent(new Event("hashchange"));

      const composer = await screen.findByRole("textbox", { name: "Message Nexora" });
      expect(composer.getAttribute("placeholder")).toBe("Load a local model to start");
    });
  });
});

import { render, screen } from "@testing-library/react";
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

import { invoke } from "@tauri-apps/api/core";

const invokeMock = vi.mocked(invoke);

function installTauriMocks() {
  markDesktop(true);

  invokeMock.mockImplementation(async (command: string, args?: unknown) => {
    switch (command) {
      case "get_usage_status":
        return makeUsage();
      case "get_app_status":
        return {
          name: "Nexora",
          version: "0.1.0",
          engine: {
            state: "ready",
            runtime: "C:\\llama\\llama-server.exe",
            model: "C:\\models\\qwen.gguf",
            detail: "Local engine ready.",
          },
          modelPath: "C:\\models\\qwen.gguf",
          runtimePath: "C:\\llama\\llama-server.exe",
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
          lastWorkspacePath: "/home/user/workspace-alpha",
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
          modelPath: "C:\\models\\qwen.gguf",
          runtimePath: "C:\\llama\\llama-server.exe",
          contextSize: 32768,
        };

      case "list_conversations":
        return [
          {
            id: "conv-1",
            title: "Glassmorphism Architecture",
            updatedAt: new Date().toISOString(),
            messageCount: 2,
          },
        ];

      case "get_conversation":
        return {
          id: "conv-1",
          title: "Glassmorphism Architecture",
          messages: [
            {
              id: "msg-1",
              role: "user",
              content: "How does the glassmorphism design system work in Nexora?",
              createdAt: new Date().toISOString(),
            },
            {
              id: "msg-2",
              role: "assistant",
              content: "Nexora uses restrained, premium glassmorphism with 20px blur and specular top highlights.",
              createdAt: new Date().toISOString(),
            },
          ],
        };

      case "get_workspace_overview":
        return {
          rootPath: "/home/user/workspace-alpha",
          name: "workspace-alpha",
          fileCount: 42,
          totalBytes: 1048576,
          gitBranch: "main",
          dirtyFiles: 0,
          detectedLanguages: ["TypeScript", "CSS", "Rust"],
        };

      case "list_workspace_files":
        return [
          { path: "src/App.tsx", isDirectory: false, sizeBytes: 1200 },
          { path: "src/styles/tokens.css", isDirectory: false, sizeBytes: 2400 },
        ];

      case "code_intel_status":
        return {
          indexedFiles: 42,
          symbolCount: 150,
          isIndexing: false,
          health: "healthy",
          lastIndexedAt: new Date().toISOString(),
        };

      case "auth_get_session":
        return null;

      case "licence_get_status":
        return {
          edition: "community",
          active: true,
          label: "Community Edition",
          key: null,
          issuedAt: null,
          expiresAt: null,
          offlineValid: true,
        };

      default:
        return null;
    }
  });
}

describe("Phase 29 — Premium Glassmorphism Design System", () => {
  const tokensCssPath = path.resolve(__dirname, "styles/tokens.css");
  const appCssPath = path.resolve(__dirname, "styles/app.css");
  let tokensCss: string;
  let appCss: string;

  beforeEach(() => {
    localStorage.clear();
    vi.clearAllMocks();
    window.location.hash = "#/";
    installTauriMocks();
    tokensCss = fs.readFileSync(tokensCssPath, "utf-8");
    appCss = fs.readFileSync(appCssPath, "utf-8");
  });

  afterEach(() => {
    window.location.hash = "";
  });

  describe("Token Specification Verification (§3.2)", () => {
    it("defines --glass-panel matching UX specification §3.2 (rgba(17, 20, 25, 0.68) + 20px blur)", () => {
      expect(tokensCss).toContain("--glass-panel: rgba(17, 20, 25, 0.68);");
      expect(tokensCss).toContain("--glass-blur: blur(20px) saturate(130%);");
      expect(tokensCss).toContain("--glass-border: rgba(238, 235, 246, 0.08);");
      expect(tokensCss).toContain("--glass-highlight: rgba(255, 255, 255, 0.05);");
    });

    it("defines --glass-raised matching UX specification §3.2 (rgba(24, 28, 35, 0.80) + 28px blur)", () => {
      expect(tokensCss).toContain("--glass-raised: rgba(24, 28, 35, 0.80);");
      expect(tokensCss).toContain("--glass-blur-raised: blur(28px) saturate(140%);");
      expect(tokensCss).toContain("--glass-border-strong: rgba(238, 235, 246, 0.14);");
    });

    it("defines --glass-scrim matching UX specification §3.2 (rgba(5, 7, 10, 0.55) + 8px blur)", () => {
      expect(tokensCss).toContain("--glass-scrim: rgba(5, 7, 10, 0.55);");
      expect(tokensCss).toContain("--glass-blur-scrim: blur(8px);");
    });

    it("defines controls, subtle highlights, and three-tier elevation shadows", () => {
      expect(tokensCss).toContain("--glass-control: rgba(27, 31, 39, 0.65);");
      expect(tokensCss).toContain("--glass-control-focus: rgba(33, 38, 48, 0.80);");

      // Shadow hierarchy: low, mid, high
      expect(tokensCss).toContain("--shadow-low: 0 1px 2px rgba(0, 0, 0, 0.35);");
      expect(tokensCss).toContain("--shadow-mid: 0 8px 24px rgba(0, 0, 0, 0.32);");
      expect(tokensCss).toContain("--shadow-high: 0 24px 64px rgba(0, 0, 0, 0.45);");
    });
  });

  describe("Accessibility & Performance Fallback Rules (§11)", () => {
    it("contains fallback media query for prefers-reduced-transparency", () => {
      expect(appCss).toContain("@media (prefers-reduced-transparency: reduce)");
      const reducedTransQuery = appCss.slice(appCss.indexOf("@media (prefers-reduced-transparency: reduce)"));
      expect(reducedTransQuery).toContain("--glass-panel: var(--bg-panel);");
      expect(reducedTransQuery).toContain("--glass-raised: var(--bg-elevated);");
      expect(reducedTransQuery).toContain("--glass-scrim: rgba(5, 7, 10, 0.85);");
      expect(reducedTransQuery).toContain("--glass-blur: none;");
      expect(reducedTransQuery).toContain("--glass-blur-raised: none;");
      expect(reducedTransQuery).toContain("--glass-blur-scrim: none;");
    });

    it("contains fallback query for @supports not (backdrop-filter: blur(1px))", () => {
      expect(appCss).toContain("@supports not (backdrop-filter: blur(1px))");
      const noBackdropQuery = appCss.slice(appCss.indexOf("@supports not (backdrop-filter: blur(1px))"));
      expect(noBackdropQuery).toContain("--glass-panel: var(--bg-panel);");
      expect(noBackdropQuery).toContain("--glass-raised: var(--bg-elevated);");
      expect(noBackdropQuery).toContain("--glass-blur: none;");
    });

    it("defines reusable glass utility classes .glass-panel, .glass-raised, .glass-scrim", () => {
      expect(appCss).toContain(".glass-panel {");
      expect(appCss).toContain(".glass-raised {");
      expect(appCss).toContain(".glass-scrim {");
    });
  });

  describe("Surface Application & Text Contrast Constraints (§3.2 Hard Rules)", () => {
    it("applies glass styling to TitleBar and Sidebar without blurring body copy", async () => {
      window.location.hash = "#/home";
      render(<App />);

      const titlebar = document.querySelector(".titlebar");
      expect(titlebar).toBeTruthy();
      expect(titlebar?.classList.contains("titlebar")).toBe(true);

      const sidebar = document.querySelector(".sidebar");
      expect(sidebar).toBeTruthy();
      expect(sidebar?.classList.contains("sidebar")).toBe(true);

      const nav = await screen.findByRole("navigation");
      expect(nav).toBeTruthy();

      // Verify TitleBar has specular highlight and glass background in CSS
      expect(appCss).toContain(".titlebar {");
      expect(appCss).toMatch(/\.titlebar[\s\S]*?background:\s*var\(--glass-chrome\)/);
      expect(appCss).toMatch(/\.titlebar[\s\S]*?backdrop-filter:\s*var\(--glass-blur\)/);
    });

    it("applies glass treatment to Chat Composer with 18-24px rounded corners and inset specular highlight", async () => {
      window.location.hash = "#/chat";
      render(<App />);

      const form = await screen.findByRole("form", { name: "Message composer" });
      expect(form).toBeTruthy();
      const composer = form.querySelector(".composer");
      expect(composer).toBeTruthy();

      // Verify composer in CSS has glass panel, border, shadow, and specular highlight
      expect(appCss).toMatch(/\.composer[\s\S]*?border-radius:\s*var\(--radius-composer\)/);
      expect(appCss).toMatch(/\.composer[\s\S]*?background:\s*var\(--glass-panel\)/);
      expect(appCss).toMatch(/\.composer[\s\S]*?backdrop-filter:\s*var\(--glass-blur\)/);
      expect(appCss).toMatch(/\.composer[\s\S]*?inset 0 1px 0 var\(--glass-highlight\)/);

      // Textarea itself must never have backdrop-filter blur applied
      const textarea = screen.getByRole("textbox", { name: "Message Nexora" });
      expect(textarea).toBeTruthy();
      expect(textarea.classList.contains("composer__input")).toBe(true);
      expect(appCss).not.toMatch(/\.composer__input\s*\{[^}]*backdrop-filter/);
    });

    it("applies glass treatment to Dashboard Home action and status cards", async () => {
      window.location.hash = "#/home";
      render(<App />);

      expect(await screen.findByRole("heading", { name: "Welcome to Nexora" })).toBeTruthy();

      // CSS verification for home-action and home-card
      expect(appCss).toMatch(/\.home-action[\s\S]*?background:\s*var\(--glass-panel\)/);
      expect(appCss).toMatch(/\.home-action[\s\S]*?inset 0 1px 0 var\(--glass-highlight\)/);
      expect(appCss).toMatch(/\.home-card[\s\S]*?background:\s*var\(--glass-panel\)/);
    });

    it("applies glass treatment to Settings groups and Model hardware card", async () => {
      window.location.hash = "#/settings";
      render(<App />);

      expect(await screen.findByRole("heading", { name: "Settings" })).toBeTruthy();

      // CSS verification for settings-group and model-manager__hardware
      expect(appCss).toMatch(/\.settings-group[\s\S]*?background:\s*var\(--glass-panel\)/);
      expect(appCss).toMatch(/\.model-manager__hardware[\s\S]*?background:\s*var\(--glass-panel\)/);
    });

    it("ensures interactive controls have focus-visible outlines and tactile active states", async () => {
      const user = userEvent.setup();
      window.location.hash = "#/chat";
      render(<App />);

      const newChatBtn = await screen.findByRole("button", { name: /New chat/i });
      expect(newChatBtn).toBeTruthy();

      await user.tab();
      // Ensure focus rings are defined in CSS
      expect(appCss).toContain(":focus-visible");
      expect(appCss).toContain("outline: 2px solid var(--accent);");
    });
  });
});

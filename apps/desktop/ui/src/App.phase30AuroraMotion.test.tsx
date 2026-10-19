import { render, screen } from "@testing-library/react";
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

function installTauriMocks() {
  markDesktop(true);

  invokeMock.mockImplementation(async (command: string) => {
    switch (command) {
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
            title: "Aurora Animation Architecture",
            updatedAt: new Date().toISOString(),
            messageCount: 2,
          },
        ];

      case "get_conversation":
        return {
          id: "conv-1",
          title: "Aurora Animation Architecture",
          messages: [
            {
              id: "msg-1",
              role: "user",
              content: "Describe the Living Aurora and advanced motion system.",
              createdAt: new Date().toISOString(),
            },
            {
              id: "msg-2",
              role: "assistant",
              content: "The Living Aurora provides tranquil, hardware-accelerated ambient lighting.",
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
        return [];

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

describe("Phase 30 — Living Aurora and Advanced Animations", () => {
  const tokensCssPath = path.resolve(__dirname, "styles/tokens.css");
  const appCssPath = path.resolve(__dirname, "styles/app.css");
  let tokensCss: string;
  let appCss: string;

  beforeEach(() => {
    localStorage.clear();
    vi.clearAllMocks();
    window.location.hash = "#/home";
    installTauriMocks();
    tokensCss = fs.readFileSync(tokensCssPath, "utf-8");
    appCss = fs.readFileSync(appCssPath, "utf-8");
  });

  afterEach(() => {
    window.location.hash = "";
  });

  describe("Living Aurora Specification (§3.1, §3.2, Phase 30)", () => {
    it("defines restrained Living Aurora design tokens in tokens.css", () => {
      expect(tokensCss).toContain("--aurora-color-1: rgba(170, 145, 241, 0.10);");
      expect(tokensCss).toContain("--aurora-color-2: rgba(141, 199, 223, 0.07);");
      expect(tokensCss).toContain("--aurora-color-3: rgba(105, 213, 160, 0.05);");
      expect(tokensCss).toContain("--aurora-speed: 22s;");
    });

    it("renders aria-hidden decorative aurora ambient container and beams in DOM", async () => {
      render(<App />);

      const aurora = await screen.findByTestId("aurora-ambient");
      expect(aurora).toBeTruthy();
      expect(aurora.getAttribute("aria-hidden")).toBe("true");

      const beam1 = aurora.querySelector(".aurora-beam--1");
      const beam2 = aurora.querySelector(".aurora-beam--2");
      const beam3 = aurora.querySelector(".aurora-beam--3");

      expect(beam1).toBeTruthy();
      expect(beam2).toBeTruthy();
      expect(beam3).toBeTruthy();
    });

    it("uses compositor-only GPU transforms in aurora keyframe animations without animating backdrop-filter or layout", () => {
      expect(appCss).toContain("@keyframes aurora-drift-1");
      expect(appCss).toContain("@keyframes aurora-drift-2");
      expect(appCss).toContain("@keyframes aurora-drift-3");

      const drift1 = appCss.slice(
        appCss.indexOf("@keyframes aurora-drift-1"),
        appCss.indexOf("@keyframes aurora-drift-2"),
      );

      // Verifies only transform translate3d/scale and opacity are animated
      expect(drift1).toContain("translate3d");
      expect(drift1).toContain("opacity");
      expect(drift1).not.toContain("backdrop-filter");
      expect(drift1).not.toContain("width");
      expect(drift1).not.toContain("height");
    });
  });

  describe("Motion Token Specification (§3.6)", () => {
    it("defines full motion token suite with standard easing in tokens.css", () => {
      expect(tokensCss).toContain("--ease-standard: cubic-bezier(0.2, 0.8, 0.2, 1);");
      expect(tokensCss).toContain("--ease-exit: cubic-bezier(0.4, 0, 1, 1);");
      expect(tokensCss).toContain("--motion-instant: 80ms linear;");
      expect(tokensCss).toContain("--motion-fast: 140ms var(--ease-standard);");
      expect(tokensCss).toContain("--motion-base: 200ms var(--ease-standard);");
      expect(tokensCss).toContain("--motion-slow: 280ms var(--ease-standard);");
      expect(tokensCss).toContain("--motion-exit: 120ms var(--ease-exit);");
    });
  });

  describe("Advanced Animations: Page Transitions, Text Reveal, Caret, Shimmer (§3.6)", () => {
    it("defines page-enter route transition animation", () => {
      expect(appCss).toContain("@keyframes page-enter");
      expect(appCss).toMatch(/\.page[\s\S]*?animation:\s*page-enter var\(--motion-base\)/);
    });

    it("defines non-blocking text-reveal typographic animation for titles and lead copy", () => {
      expect(appCss).toContain("@keyframes text-reveal");
      expect(appCss).toMatch(/\.page-header__title[\s\S]*?animation:\s*text-reveal/);
      expect(appCss).toMatch(/\.welcome__title[\s\S]*?animation:\s*text-reveal/);
    });

    it("defines 1.2s smooth opacity pulse for streaming caret", () => {
      expect(appCss).toContain("@keyframes caret-pulse");
      expect(appCss).toMatch(/\.caret[\s\S]*?animation:\s*caret-pulse 1\.2s/);
    });

    it("defines 2px indeterminate progress shimmer on tracks without animating body text", () => {
      expect(appCss).toContain(".shimmer-track");
      expect(appCss).toContain("@keyframes shimmer-slide");
      expect(appCss).toMatch(/\.shimmer-track[\s\S]*?height:\s*2px/);
      expect(appCss).toMatch(/\.shimmer-track::after[\s\S]*?animation:\s*shimmer-slide/);
    });
  });

  describe("Accessibility & Reduced-Motion Safeguards (§3.6, §11)", () => {
    it("disables aurora drift and animations under prefers-reduced-motion media query", () => {
      expect(appCss).toContain("@media (prefers-reduced-motion: reduce)");
      const reducedMotionBlock = appCss.slice(appCss.indexOf("@media (prefers-reduced-motion: reduce)"));

      expect(reducedMotionBlock).toContain(".aurora-ambient,");
      expect(reducedMotionBlock).toContain(".aurora-beam {");
      expect(reducedMotionBlock).toMatch(/\.aurora-beam\s*\{[^}]*animation:\s*none !important;/);
      expect(reducedMotionBlock).toMatch(/\.caret\s*\{[^}]*animation:\s*none !important;/);
      expect(reducedMotionBlock).toMatch(/\.shimmer-track::after\s*\{[^}]*animation:\s*none !important;/);
    });

    it("disables aurora drift and animations under in-app .app.reduced-motion override", () => {
      expect(appCss).toContain(".app.reduced-motion .aurora-ambient,");
      expect(appCss).toMatch(/\.app\.reduced-motion \.aurora-beam\s*\{[^}]*animation:\s*none !important;/);
      expect(appCss).toMatch(/\.app\.reduced-motion \.caret\s*\{[^}]*animation:\s*none !important;/);
      expect(appCss).toMatch(/\.app\.reduced-motion \.shimmer-track::after\s*\{[^}]*animation:\s*none !important;/);
    });

    it("hides ambient aurora under prefers-reduced-transparency media query", () => {
      expect(appCss).toContain("@media (prefers-reduced-transparency: reduce)");
      const reducedTransBlock = appCss.slice(appCss.indexOf("@media (prefers-reduced-transparency: reduce)"));
      expect(reducedTransBlock).toMatch(/\.aurora-ambient\s*\{[^}]*display:\s*none !important;/);
    });

    it("ensures interactive controls remain accessible and responsive while aurora is rendered", async () => {
      window.location.hash = "#/chat";
      render(<App />);

      const form = await screen.findByRole("form", { name: "Message composer" });
      expect(form).toBeTruthy();

      const textarea = screen.getByRole("textbox", { name: "Message Nexora" });
      expect(textarea).toBeTruthy();
      expect(textarea.getAttribute("disabled")).toBeNull();
    });
  });
});

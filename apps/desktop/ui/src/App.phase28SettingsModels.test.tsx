import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
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

  let engineState = "stopped";

  invokeMock.mockImplementation(async (command: string, args?: unknown) => {
    switch (command) {
      case "get_app_status":
        return {
          name: "Nexora",
          version: "0.1.0",
          engine: {
            state: engineState,
            runtime: "C:\\llama\\llama-server.exe",
            model: "C:\\models\\qwen.gguf",
            detail: engineState === "ready" ? "Local engine ready." : "Model loaded previously, engine is idle.",
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

      case "save_settings":
        return (args as { settings: unknown })?.settings;

      case "load_model":
        engineState = "ready";
        return {
          name: "Nexora",
          version: "0.1.0",
          engine: {
            state: "ready",
            runtime: "C:\\llama\\llama-server.exe",
            model: "qwen.gguf",
            detail: "Local engine ready.",
          },
          modelPath: "C:\\models\\qwen.gguf",
          runtimePath: "C:\\llama\\llama-server.exe",
          contextSize: 32768,
        };

      case "unload_model":
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

      case "list_conversations":
        return [];

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

describe("Phase 28 — Settings and Models Integration", () => {
  beforeEach(() => {
    localStorage.clear();
    vi.clearAllMocks();
    window.location.hash = "#/settings";
    installTauriMocks();
  });

  afterEach(() => {
    window.location.hash = "";
  });

  it("renders Settings page on #/settings and alias #/preferences with genuine preferences", async () => {
    render(<App />);

    expect(await screen.findByRole("heading", { name: "Settings" })).toBeTruthy();
    expect(screen.getByRole("region", { name: "Interface" })).toBeTruthy();
    expect(screen.getByRole("region", { name: "Workspace" })).toBeTruthy();
    expect(screen.getByRole("region", { name: "Privacy & Network" })).toBeTruthy();
    expect(screen.getByRole("region", { name: "Engine configuration" })).toBeTruthy();

    // Verify engine read-only parameters
    expect(screen.getByText("C:\\llama\\llama-server.exe")).toBeTruthy();
    expect(screen.getByText("C:\\models\\qwen.gguf")).toBeTruthy();
    expect(screen.getByText("32768 tokens")).toBeTruthy();

    // Test alias #/preferences
    window.location.hash = "#/preferences";
    window.dispatchEvent(new Event("hashchange"));

    await waitFor(() => {
      expect(screen.getByRole("heading", { name: "Settings" })).toBeTruthy();
    });
  });

  it("toggles and persists interface preferences (ligatures, reduced motion, sidebar)", async () => {
    const user = userEvent.setup();
    render(<App />);

    await screen.findByRole("heading", { name: "Settings" });

    const ligatures = screen.getByRole("checkbox", { name: "Code font ligatures" });
    const motion = screen.getByRole("checkbox", { name: "Reduced motion override" });

    await user.click(ligatures);
    await user.click(motion);

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
  });

  it("navigates to #/models and aliases #/model-setup and #/engine, displaying hardware info", async () => {
    window.location.hash = "#/models";
    render(<App />);

    expect(await screen.findByRole("heading", { name: "Local models" })).toBeTruthy();
    expect(screen.getByRole("region", { name: "System hardware & runtime target" })).toBeTruthy();
    expect(await screen.findByText("22.8 of 32 GiB available")).toBeTruthy();
    expect(screen.getByText("llama.cpp b11714")).toBeTruthy();

    // Test alias #/model-setup
    window.location.hash = "#/model-setup";
    window.dispatchEvent(new Event("hashchange"));

    await waitFor(() => {
      expect(screen.getByRole("heading", { name: "Local models" })).toBeTruthy();
    });

    // Test alias #/engine
    window.location.hash = "#/engine";
    window.dispatchEvent(new Event("hashchange"));

    await waitFor(() => {
      expect(screen.getByRole("heading", { name: "Local models" })).toBeTruthy();
    });
  });

  it("probes runtime and validates model header with genuine IPC calls", async () => {
    window.location.hash = "#/models";
    const user = userEvent.setup();
    render(<App />);

    await screen.findByRole("heading", { name: "Local models" });

    const checkButtons = screen.getAllByRole("button", { name: "Check" });
    expect(checkButtons.length).toBe(2);

    // Click runtime check
    await user.click(checkButtons[0]!);
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "probe_runtime",
        expect.objectContaining({ path: "C:\\llama\\llama-server.exe" }),
      );
    });
    expect((await screen.findAllByText(/llama-server version: 11714/)).length).toBeGreaterThan(0);

    // Click model check
    await user.click(checkButtons[1]!);
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "validate_model",
        expect.objectContaining({ path: "C:\\models\\qwen.gguf" }),
      );
    });
    expect(await screen.findByText(/13\.97 GiB · GGUF v3/)).toBeTruthy();
  });

  it("saves engine configuration and loads model, transitioning engine status to ready", async () => {
    window.location.hash = "#/models";
    const user = userEvent.setup();
    render(<App />);

    await screen.findByRole("heading", { name: "Local models" });

    const loadButton = screen.getByRole("button", { name: "Save and reload" });
    await user.click(loadButton);

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "save_settings",
        expect.objectContaining({
          settings: expect.objectContaining({
            modelPath: "C:\\models\\qwen.gguf",
            runtimePath: "C:\\llama\\llama-server.exe",
            contextSize: 32768,
          }),
        }),
      );
      expect(invokeMock).toHaveBeenCalledWith("load_model", undefined);
    });

    // Engine should report ready
    expect((await screen.findAllByText("Ready")).length).toBeGreaterThan(0);
  });

  it("handles model load failure honestly without faking readiness", async () => {
    invokeMock.mockImplementation(async (command: string, args?: unknown) => {
      if (command === "load_model") {
        throw {
          code: "load_failed",
          message: "The local engine stopped unexpectedly (exit status: 1).",
        };
      }
      if (command === "save_settings") {
        return (args as { settings: unknown })?.settings;
      }
      if (command === "get_app_status") {
        return {
          name: "Nexora",
          version: "0.1.0",
          engine: {
            state: "error",
            runtime: "C:\\llama\\llama-server.exe",
            model: "C:\\models\\qwen.gguf",
            detail: "The local engine stopped unexpectedly.",
          },
          modelPath: "C:\\models\\qwen.gguf",
          runtimePath: "C:\\llama\\llama-server.exe",
          contextSize: 32768,
        };
      }
      if (command === "list_conversations") {
        return [];
      }
      if (command === "get_system_info") {
        return {
          totalRamGib: 32,
          availableRamGib: 22.8,
          platform: "windows",
        };
      }
      if (command === "get_workspace_settings") {
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
      }
      if (command === "licence_get_status") {
        return {
          edition: "community",
          active: true,
          label: "Community Edition",
          key: null,
          issuedAt: null,
          expiresAt: null,
          offlineValid: true,
        };
      }
      return null;
    });

    window.location.hash = "#/models";
    const user = userEvent.setup();
    render(<App />);

    await screen.findByRole("heading", { name: "Local models" });

    const loadButton = screen.getByRole("button", { name: "Save and reload" });
    await user.click(loadButton);

    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("The local engine stopped unexpectedly");
    // Must NOT claim ready
    expect(screen.queryByText("Ready")).toBeNull();
  });
});

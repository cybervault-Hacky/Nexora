import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { SettingsPage } from "./SettingsPage";
import { makeStatus } from "../test/fixtures";

vi.mock("../lib/workspaceSettings", () => ({
  getWorkspaceSettings: vi.fn().mockResolvedValue({
    version: 1,
    lastWorkspacePath: "/home/user/test-project",
    preferences: {
      explorerCollapsed: false,
      founderModeEnabled: false,
      codeLigatures: false,
      reducedMotion: false,
    },
  }),
  saveWorkspaceSettings: vi.fn().mockResolvedValue({
    version: 1,
    lastWorkspacePath: null,
    preferences: {
      explorerCollapsed: false,
      founderModeEnabled: false,
      codeLigatures: false,
      reducedMotion: false,
    },
  }),
}));

import { getWorkspaceSettings, saveWorkspaceSettings } from "../lib/workspaceSettings";

describe("Phase 28 — SettingsPage component", () => {
  beforeEach(() => {
    vi.mocked(getWorkspaceSettings).mockResolvedValue({
      version: 1,
      lastWorkspacePath: "/home/user/test-project",
      preferences: {
        explorerCollapsed: false,
        founderModeEnabled: false,
        codeLigatures: false,
        reducedMotion: false,
      },
    });
    vi.mocked(saveWorkspaceSettings).mockResolvedValue({
      version: 1,
      lastWorkspacePath: null,
      preferences: {
        explorerCollapsed: false,
        founderModeEnabled: false,
        codeLigatures: false,
        reducedMotion: false,
      },
    });
  });

  it("renders Settings page with header, preferences, and privacy assurances", async () => {
    const status = makeStatus({
      state: "ready",
      runtimePath: "C:\\Tools\\llama-server.exe",
      model: "model.gguf",
    });

    render(
      <SettingsPage
        status={status}
        sidebarCollapsed={false}
        onToggleSidebar={vi.fn()}
        workspacePath="/home/user/current-repo"
      />,
    );

    expect(screen.getByRole("heading", { name: "Settings" })).toBeTruthy();
    expect(screen.getByRole("region", { name: "Account" })).toBeTruthy();
    expect(screen.getByRole("region", { name: "Interface" })).toBeTruthy();
    expect(screen.getByRole("region", { name: "Workspace" })).toBeTruthy();
    expect(screen.getByRole("region", { name: "Privacy & Network" })).toBeTruthy();
    expect(screen.getByRole("region", { name: "Engine configuration" })).toBeTruthy();
    expect(screen.getByRole("region", { name: "Keyboard shortcuts" })).toBeTruthy();

    // Verifies privacy & local boundary guarantees
    expect(screen.getByText(/100% offline sovereign/i)).toBeTruthy();
    expect(screen.getAllByText(/127\.0\.0\.1/).length).toBeGreaterThan(0);
    expect(screen.getByText(/Zero Cloud/i)).toBeTruthy();
  });

  it("toggles interface controls including sidebar, ligatures, and reduced motion", async () => {
    const onToggleSidebar = vi.fn();
    const onToggleCodeLigatures = vi.fn();
    const onToggleReducedMotion = vi.fn();
    const user = userEvent.setup();

    render(
      <SettingsPage
        status={null}
        sidebarCollapsed={false}
        onToggleSidebar={onToggleSidebar}
        workspacePath={null}
        codeLigatures={false}
        onToggleCodeLigatures={onToggleCodeLigatures}
        reducedMotion={false}
        onToggleReducedMotion={onToggleReducedMotion}
      />,
    );

    const sidebarCheckbox = screen.getByRole("checkbox", { name: "Collapse sidebar" });
    await user.click(sidebarCheckbox);
    expect(onToggleSidebar).toHaveBeenCalled();

    const ligaturesCheckbox = screen.getByRole("checkbox", { name: "Code font ligatures" });
    await user.click(ligaturesCheckbox);
    expect(onToggleCodeLigatures).toHaveBeenCalled();

    const motionCheckbox = screen.getByRole("checkbox", { name: "Reduced motion override" });
    await user.click(motionCheckbox);
    expect(onToggleReducedMotion).toHaveBeenCalled();
  });

  it("displays current project and allows clearing remembered last workspace path", async () => {
    const onClear = vi.fn();
    const user = userEvent.setup();

    render(
      <SettingsPage
        status={null}
        sidebarCollapsed={false}
        onToggleSidebar={vi.fn()}
        workspacePath="/home/user/my-project"
        onClearLastWorkspace={onClear}
      />,
    );

    expect(screen.getByText("/home/user/my-project")).toBeTruthy();

    await waitFor(() => {
      expect(screen.getByText("/home/user/test-project")).toBeTruthy();
    });

    const clearButton = screen.getByRole("button", { name: "Clear path" });
    await user.click(clearButton);

    expect(saveWorkspaceSettings).toHaveBeenCalledWith(
      expect.objectContaining({
        lastWorkspacePath: null,
      }),
    );
    expect(screen.getByText("Cleared remembered workspace path.")).toBeTruthy();
  });

  it("displays read-only engine configuration with link to local models", () => {
    const status = makeStatus({
      state: "ready",
      runtimePath: "C:\\llama\\llama-server.exe",
      modelPath: "qwen-2.5-coder.gguf",
    });

    render(
      <SettingsPage
        status={status}
        sidebarCollapsed={false}
        onToggleSidebar={vi.fn()}
        workspacePath={null}
      />,
    );

    expect(screen.getByText(/Current state: ready/)).toBeTruthy();
    expect(screen.getByText("C:\\llama\\llama-server.exe")).toBeTruthy();
    expect(screen.getByText("qwen-2.5-coder.gguf")).toBeTruthy();
    expect(screen.getByText("32768 tokens")).toBeTruthy();
    expect(screen.getByRole("link", { name: /Configure local models/i })).toBeTruthy();
  });

  it("toggles founder mode and reveals founder console link", async () => {
    const onToggleFounder = vi.fn();
    const user = userEvent.setup();

    const { rerender } = render(
      <SettingsPage
        status={null}
        sidebarCollapsed={false}
        onToggleSidebar={vi.fn()}
        workspacePath={null}
        founderModeEnabled={false}
        onToggleFounderMode={onToggleFounder}
      />,
    );

    expect(screen.queryByRole("link", { name: /Open Console/i })).toBeNull();

    const founderCheckbox = screen.getByRole("checkbox", { name: "Founder Control Centre" });
    await user.click(founderCheckbox);
    expect(onToggleFounder).toHaveBeenCalled();

    rerender(
      <SettingsPage
        status={null}
        sidebarCollapsed={false}
        onToggleSidebar={vi.fn()}
        workspacePath={null}
        founderModeEnabled={true}
        onToggleFounderMode={onToggleFounder}
      />,
    );

    expect(screen.getByRole("link", { name: /Open Console/i })).toBeTruthy();
  });
});

import { describe, expect, it, beforeEach, vi } from "vitest";
import { getWorkspaceSettings, saveWorkspaceSettings } from "./workspaceSettings";
import { markDesktop } from "../test/fixtures";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";

const invokeMock = vi.mocked(invoke);

beforeEach(() => {
  localStorage.clear();
  vi.clearAllMocks();
});

describe("workspaceSettings", () => {
  it("falls back to defaults when not desktop", async () => {
    markDesktop(false);
    const s = await getWorkspaceSettings();
    expect(s.version).toBe(1);
    expect(s.lastWorkspacePath).toBeNull();
  });

  it("falls back to localStorage when desktop invoke fails", async () => {
    markDesktop(true);
    invokeMock.mockRejectedValue(new Error("missing"));
    localStorage.setItem("nexora:workspace-settings:v1", JSON.stringify({ version: 1, lastWorkspacePath: "/tmp/foo", preferences: { explorerCollapsed: true } }));
    const s = await getWorkspaceSettings();
    expect(s.lastWorkspacePath).toBe("/tmp/foo");
    expect(s.preferences.explorerCollapsed).toBe(true);
  });

  it("handles malformed JSON gracefully", async () => {
    markDesktop(false);
    localStorage.setItem("nexora:workspace-settings:v1", "not-json");
    const s = await getWorkspaceSettings();
    expect(s.version).toBe(1);
    expect(s.lastWorkspacePath).toBeNull();
  });

  it("saves and loads roundtrip in browser fallback", async () => {
    markDesktop(false);
    const input = { version: 1 as const, lastWorkspacePath: "/tmp/bar", preferences: { explorerCollapsed: false } };
    const saved = await saveWorkspaceSettings(input);
    expect(saved.lastWorkspacePath).toBe("/tmp/bar");
    const loaded = await getWorkspaceSettings();
    expect(loaded.lastWorkspacePath).toBe("/tmp/bar");
  });

  it("never includes secrets — only path and prefs", async () => {
    markDesktop(false);
    const s = await saveWorkspaceSettings({ version: 1, lastWorkspacePath: "/tmp/secret", preferences: { explorerCollapsed: false } });
    const raw = localStorage.getItem("nexora:workspace-settings:v1")!;
    expect(raw).not.toContain("secret contents");
    expect(s).not.toHaveProperty("contents");
    expect(JSON.parse(raw)).toHaveProperty("lastWorkspacePath");
    expect(JSON.parse(raw)).not.toHaveProperty("tokens");
  });
});

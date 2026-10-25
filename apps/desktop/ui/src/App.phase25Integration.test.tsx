import { act, render, screen, waitFor, within } from "@testing-library/react";
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
      modelPath: null,
      runtimePath: null,
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
import { makeStatus, markDesktop, makeUsage } from "./test/fixtures";

const statusMock = vi.mocked(fetchStatus);
const systemMock = vi.mocked(fetchSystemInfo);

function goto(hash: string) {
  act(() => {
    window.location.hash = hash;
    window.dispatchEvent(new Event("hashchange"));
  });
}

beforeEach(() => {
  statusMock.mockReset();
  statusMock.mockResolvedValue(makeStatus({ state: "ready" }));
  systemMock.mockReset();
  systemMock.mockResolvedValue({
    totalRamGib: 32,
    availableRamGib: 20.5,
    platform: "windows",
  });
  markDesktop(true);

  invokeMock.mockReset();
  // Safe default mock for command-aware invoke
  invokeMock.mockImplementation((cmd: string) => {
    switch (cmd) {
      case "get_usage_status":
        return Promise.resolve(makeUsage());
      case "auth_get_session":
        return Promise.resolve({
          authenticated: false,
          user: null,
        });
      case "get_developer_diagnostics":
        return Promise.resolve({
          platform: "linux",
          arch: "x86_64",
          appVersion: "0.1.0",
          credentialVault: "Keyring OS Vault",
          authServiceUrl: "http://127.0.0.1:8787",
          cspEnforced: true,
          offlineCapable: true,
        });
      case "workspace_get_symbol_index":
        return Promise.resolve(null);
      case "workspace_get_analysis_graph":
        return Promise.resolve(null);
      default:
        return Promise.resolve(null);
    }
  });

  window.location.hash = "#/home";
});

describe("Phase 25 — Developer Account & Founder Control Centre Integration", () => {
  it("renders Account page in unauthenticated guest mode", async () => {
    render(<App />);
    goto("#/account");

    expect(await screen.findByRole("region", { name: "Sign in required" })).toBeTruthy();
    expect(screen.queryByRole("heading", { name: "Developer Account" })).toBeNull();
    expect(within(screen.getByRole("region", { name: "Sign in required" })).getByRole("button", { name: "Sign in" })).toBeTruthy();
  });

  it("renders authenticated profile and allows password change", async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "auth_get_session") {
        return Promise.resolve({
          authenticated: true,
          user: {
            id: "usr_founder_01",
            name: "Hacky Founder",
            email: "founder@nexora.dev",
            emailVerified: true,
          },
        });
      }
      if (cmd === "account_get_profile") {
        return Promise.resolve({
          id: "usr_founder_01",
          name: "Hacky Founder",
          email: "founder@nexora.dev",
          emailVerified: true,
          hasPassword: true,
          providers: ["password"],
          planId: "free",
          planName: "Free",
          createdAt: "2026-01-15T12:00:00Z",
        });
      }
      if (cmd === "auth_change_password") {
        return Promise.resolve({
          status: "success",
          message: "Developer password updated.",
        });
      }
      return Promise.resolve(null);
    });

    const user = userEvent.setup();
    render(<App />);
    goto("#/account");

    expect(await screen.findByText("Hacky Founder")).toBeTruthy();
    expect(await screen.findByText("founder@nexora.dev")).toBeTruthy();
    expect(screen.getByText("usr_founder_01")).toBeTruthy();
    expect(screen.getByText("Active Session")).toBeTruthy();

    // Fill and submit password change
    const currentInput = screen.getByLabelText(/Current password/i);
    const newInput = screen.getByLabelText(/New password \(at least 8 characters\)/i);
    const confirmInput = screen.getByLabelText(/Confirm new password/i);

    await user.type(currentInput, "CurrentSecret123");
    await user.type(newInput, "UpdatedSecret456");
    await user.type(confirmInput, "UpdatedSecret456");

    const submitBtn = screen.getByRole("button", { name: "Update password" });
    await user.click(submitBtn);

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("auth_change_password", {
        request: {
          currentPassword: "CurrentSecret123",
          newPassword: "UpdatedSecret456",
        },
      });
    });

    expect(await screen.findByText("Developer password updated.")).toBeTruthy();
  });

  it("Settings page displays Account and Founder Control toggle", async () => {
    const original = invokeMock.getMockImplementation()!;
    invokeMock.mockImplementation((cmd: string, args?: unknown) => cmd === "auth_get_session"
      ? Promise.resolve({ authenticated: true, user: { id: "test-account", name: "Test User", email: "test@example.test", emailVerified: true } }) : original(cmd, args));
    const user = userEvent.setup();
    render(<App />);
    goto("#/settings");

    expect(await screen.findByRole("heading", { name: "Settings" })).toBeTruthy();
    expect(screen.getByRole("region", { name: "Account" })).toBeTruthy();
    expect(screen.getByRole("region", { name: "Developer & Founder Tools" })).toBeTruthy();

    const founderCheckbox = screen.getByRole("checkbox", {
      name: /Founder Control Centre/i,
    }) as HTMLInputElement;
    expect(founderCheckbox.checked).toBe(false);

    // Toggle founder mode on
    await user.click(founderCheckbox);
    expect(founderCheckbox.checked).toBe(true);

    // Verify open console button appears in Settings
    expect(screen.getByRole("link", { name: /Open Console/i })).toBeTruthy();

    // Verify navigation bar now includes Founder Control
    const nav = screen.getByRole("navigation", { name: "Primary" });
    expect(within(nav).getByRole("link", { name: "Founder Control" })).toBeTruthy();
  });

  it("renders Founder Control Centre with system metrics and live IPC telemetry", async () => {
    const original = invokeMock.getMockImplementation()!;
    invokeMock.mockImplementation((cmd: string, args?: unknown) => {
      if (cmd === "auth_get_session") return Promise.resolve({ authenticated: true, user: { id: "test-account", name: "Test User", email: "test@example.test", emailVerified: true } });
      if (cmd === "get_workspace_settings") return Promise.resolve({ version: 1, lastWorkspacePath: null, preferences: { explorerCollapsed: false, founderModeEnabled: true, codeLigatures: false, reducedMotion: false } });
      return original(cmd, args);
    });
    render(<App />);
    goto("#/founder");

    expect(await screen.findByRole("heading", { name: "Founder Control Centre" })).toBeTruthy();
    expect(await screen.findByText("linux (x86_64)")).toBeTruthy();
    expect(screen.getByText("Keyring OS Vault")).toBeTruthy();
    expect(screen.getByRole("region", { name: "IPC Traffic Telemetry" })).toBeTruthy();
  });
});

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
  invokeMock.mockImplementation((cmd: string) => {
    switch (cmd) {
      case "get_usage_status":
        return Promise.resolve(makeUsage());
      case "licence_get_status":
        return Promise.resolve({
          tier: "community",
          planName: "Community Edition",
          active: true,
          licenceKey: null,
          licensee: null,
          expiresAt: null,
          features: ["daily_local_inference"],
        });
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

describe("Phase 26 — Developer Licensing & Workspace Tiering Integration", () => {
  it("navigates to Licensing page via hash router and displays Community Edition", async () => {
    render(<App />);
    goto("#/licensing");

    expect(await screen.findByRole("heading", { name: "Developer Licensing" })).toBeTruthy();
    expect(screen.getByText("Community Edition")).toBeTruthy();
    expect(screen.getByText(/Community Sovereignty Guarantee/i)).toBeTruthy();
  });

  it("navigates to Licensing page via /billing alias", async () => {
    render(<App />);
    goto("#/billing");

    expect(await screen.findByRole("heading", { name: "Developer Licensing" })).toBeTruthy();
  });

  it("Settings page provides direct navigation link to Licensing page", async () => {
    const user = userEvent.setup();
    render(<App />);
    goto("#/settings");

    expect(await screen.findByRole("heading", { name: "Settings" })).toBeTruthy();
    const plansLink = screen.getByRole("link", { name: /View Plans & Licences/i });
    expect(plansLink).toBeTruthy();

    await user.click(plansLink);
    expect(await screen.findByRole("heading", { name: "Developer Licensing" })).toBeTruthy();
  });

  it("preserves full unauthenticated offline local AI chat and workspace sovereignty", async () => {
    const user = userEvent.setup();
    render(<App />);
    goto("#/chat");

    expect(await screen.findByRole("region", { name: "Conversation messages" })).toBeTruthy();
    expect(screen.getByRole("textbox", { name: "Message Nexora" })).toBeTruthy();

    const nav = screen.getByRole("navigation", { name: "Primary" });
    await user.click(within(nav).getByRole("link", { name: "Local Models" }));
    expect(await screen.findByRole("heading", { name: "Local models" })).toBeTruthy();
  });
});

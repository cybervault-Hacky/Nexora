import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("./lib/inference", async (importOriginal) => {
  const actual = await importOriginal<typeof import("./lib/inference")>();

  return {
    ...actual,
    fetchStatus: vi.fn(),
    fetchSettings: vi.fn().mockResolvedValue({
      modelPath: "C:\\models\\qwen2.5-coder-7b.gguf",
      runtimePath: "C:\\llama\\llama-server.exe",
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
  listConversations: async () => [
    {
      id: "offline-conv-1",
      title: "Local offline work",
      createdAt: Date.now() - 1000,
      updatedAt: Date.now(),
      messages: [],
    },
  ],
  createConversation: async () => ({
    id: "new-offline-chat",
    title: "New local conversation",
    createdAt: Date.now(),
    updatedAt: Date.now(),
    messages: [],
  }),
  saveConversation: async (c: import("./lib/history").PersistedConversation) => c,
  loadConversation: async () => ({
    id: "offline-conv-1",
    title: "Local offline work",
    createdAt: Date.now() - 1000,
    updatedAt: Date.now(),
    messages: [],
  }),
  renameConversation: async () => ({
    id: "offline-conv-1",
    title: "Renamed offline",
    createdAt: Date.now() - 1000,
    updatedAt: Date.now(),
    messages: [],
  }),
  deleteConversation: async () => true,
}));

vi.mock("./lib/usage", async (importOriginal) => {
  const actual = await importOriginal<typeof import("./lib/usage")>();

  return { ...actual, fetchUsage: vi.fn() };
});

import App from "./App";
import { fetchStatus, fetchSystemInfo } from "./lib/inference";
import { fetchUsage } from "./lib/usage";
import { makeStatus, makeUsage, markDesktop } from "./test/fixtures";

const statusMock = vi.mocked(fetchStatus);
const systemMock = vi.mocked(fetchSystemInfo);
const usageMock = vi.mocked(fetchUsage);

function goto(hash: string) {
  act(() => {
    window.location.hash = hash;
    window.dispatchEvent(new Event("hashchange"));
  });
}

describe("Phase 23 Step 7 — Offline AI & Unauthenticated Regression Suite", () => {
  let originalFetch: typeof globalThis.fetch;
  let fetchMock: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    statusMock.mockReset();
    statusMock.mockResolvedValue(makeStatus({ state: "ready" }));
    systemMock.mockReset();
    systemMock.mockResolvedValue({
      totalRamGib: 32,
      availableRamGib: 20.5,
      platform: "windows",
    });
    usageMock.mockReset();
    usageMock.mockResolvedValue(makeUsage());
    markDesktop();

    // Link-down simulation: simulate auth API unreachable on network
    originalFetch = globalThis.fetch;
    fetchMock = vi.fn().mockRejectedValue(
      new TypeError("Failed to fetch: auth service unreachable (link-down simulation)"),
    );
    globalThis.fetch = fetchMock as unknown as typeof globalThis.fetch;
    window.fetch = globalThis.fetch;

    window.location.hash = "#/home";
  });

  afterEach(() => {
    globalThis.fetch = originalFetch;
    window.fetch = originalFetch;
    markDesktop(false);
  });

  it("never attempts any HTTP fetch to an auth backend during normal usage", async () => {
    render(<App />);

    expect(await screen.findByRole("heading", { name: "Welcome to Nexora" })).toBeTruthy();
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("permits full access to all workspace routes without authentication", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("heading", { name: "Welcome to Nexora" });

    const nav = screen.getByRole("navigation", { name: "Primary" });

    // AI Chat
    await user.click(within(nav).getByRole("link", { name: "AI Chat" }));
    expect(await screen.findByRole("region", { name: "Conversation messages" })).toBeTruthy();
    expect(screen.queryByText(/sign in required/i)).toBeNull();

    // Projects
    await user.click(within(nav).getByRole("link", { name: "Projects" }));
    expect(await screen.findByRole("complementary", { name: "Project explorer" })).toBeTruthy();
    expect(screen.queryByText(/sign in required/i)).toBeNull();

    // Code Intelligence
    await user.click(within(nav).getByRole("link", { name: "Code Intelligence" }));
    expect(await screen.findByRole("heading", { name: "Code intelligence" })).toBeTruthy();

    // Local Models
    await user.click(within(nav).getByRole("link", { name: "Local Models" }));
    expect(await screen.findByRole("heading", { name: "Local models" })).toBeTruthy();

    // Local destinations stay public; account pages are protected.
    expect(within(nav).queryByRole("link", { name: "Settings" })).toBeNull();
    window.location.hash = "#/settings";
    window.dispatchEvent(new Event("hashchange"));
    expect(await screen.findByRole("region", { name: "Sign in required" })).toBeTruthy();

    // Zero fetch calls occurred during entire multi-page navigation
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("allows chat interaction with local engine ready offline without login", async () => {
    render(<App />);
    goto("#/chat");

    expect(await screen.findByRole("region", { name: "Conversation messages" })).toBeTruthy();
    expect(screen.getByText("Engine ready")).toBeTruthy();
    expect(screen.getByText("What's New Today?")).toBeTruthy();
    expect(screen.getByRole("textbox", { name: "Message Nexora" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Send message" })).toBeTruthy();

    // No auth network activity
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("allows closing the floating sign-in modal back to the workspace without logging in", async () => {
    const user = userEvent.setup();
    render(<App />);

    // Navigate to sign-in alias -> opens floating modal directly over AI Chat
    goto("#/signin");
    expect(await screen.findByRole("dialog")).toBeTruthy();

    // Close modal back to AI Chat workspace without needing credentials
    await user.click(screen.getByRole("button", { name: "Close authentication dialog" }));
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(await screen.findByRole("region", { name: "Conversation messages" })).toBeTruthy();
    expect(window.location.hash).toBe("#/chat");
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("keeps local inference operational even when network is completely offline", async () => {
    render(<App />);
    goto("#/chat");

    expect(await screen.findByText("Engine ready")).toBeTruthy();
    expect(screen.getByText(/qwen2\.5-coder-7b/)).toBeTruthy();
    expect(screen.queryByRole("alert")).toBeNull();
    expect(fetchMock).not.toHaveBeenCalled();
  });
});

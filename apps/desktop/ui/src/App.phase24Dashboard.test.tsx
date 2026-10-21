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
      modelPath: "C:\\models\\qwen2.5-coder-7b.gguf",
      runtimePath: "C:\\bin\\llama-server.exe",
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
    { id: "chat-alpha", title: "Implement Dashboard Tests", createdAt: Date.now() - 3600000, updatedAt: Date.now() - 120000, messages: [] },
    { id: "chat-beta", title: "Refactor Model Supervisor", createdAt: Date.now() - 7200000, updatedAt: Date.now() - 600000, messages: [] },
  ],
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
  statusMock.mockResolvedValue(
    makeStatus({
      state: "ready",
      modelPath: "C:\\models\\qwen2.5-coder-7b.gguf",
      contextSize: 32768,
    }),
  );
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
      case "list_conversations":
        return Promise.resolve([
          { id: "chat-alpha", title: "Implement Dashboard Tests", createdAt: Date.now() - 3600000, updatedAt: Date.now() - 120000, messageCount: 2, preview: "Test" },
          { id: "chat-beta", title: "Refactor Model Supervisor", createdAt: Date.now() - 7200000, updatedAt: Date.now() - 600000, messageCount: 5, preview: "Model" },
        ]);
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

describe("Phase 24 — Central Dashboard Integration", () => {
  it("renders the Dashboard on #/home and #/dashboard alias", async () => {
    render(<App />);

    expect(await screen.findByRole("heading", { name: "Welcome to Nexora" })).toBeTruthy();
    expect(screen.getByRole("region", { name: "Application status" })).toBeTruthy();

    goto("#/dashboard");
    expect(await screen.findByRole("heading", { name: "Welcome to Nexora" })).toBeTruthy();
  });

  it("displays genuine local AI readiness on the dashboard", async () => {
    render(<App />);

    const statusRegion = await screen.findByRole("region", { name: "Application status" });
    expect(within(statusRegion).getByText("Ready")).toBeTruthy();
    expect(within(statusRegion).getByText(/Active model: qwen2\.5-coder-7b\.gguf/i)).toBeTruthy();
  });

  it("surfaces recent conversations with direct continuation links", async () => {
    const user = userEvent.setup();
    render(<App />);

    const main = await screen.findByRole("main");
    const continueBtn = await within(main).findByRole("button", { name: /Implement Dashboard Tests/i });
    expect(continueBtn).toBeTruthy();
    await user.click(continueBtn);

    expect(await screen.findByRole("region", { name: "Conversation messages" })).toBeTruthy();
  });

  it("provides manual path input on Dashboard when no project is open", async () => {
    render(<App />);

    expect(await screen.findByText("No project open")).toBeTruthy();
    expect(screen.getByPlaceholderText(/Enter absolute directory path/i)).toBeTruthy();
    expect(screen.getByRole("button", { name: /Open path/i })).toBeTruthy();
  });

  it("allows navigating to other primary product sections from the Dashboard", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("heading", { name: "Welcome to Nexora" });

    const nav = screen.getByRole("navigation", { name: "Primary" });

    await user.click(within(nav).getByRole("link", { name: "Projects" }));
    expect(await screen.findByRole("complementary", { name: "Project explorer" })).toBeTruthy();

    await user.click(within(nav).getByRole("link", { name: "Home" }));
    expect(await screen.findByRole("heading", { name: "Welcome to Nexora" })).toBeTruthy();
  });
});

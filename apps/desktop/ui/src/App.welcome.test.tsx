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
    id: "chat-direct-1",
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
import { fetchStatus, generate } from "./lib/inference";
import { makeStatus, makeUsage, markDesktop } from "./test/fixtures";

const statusMock = vi.mocked(fetchStatus);
const generateMock = vi.mocked(generate);

describe("Direct-to-Chat launch, Signed-Out Local Chat & Floating Auth Modal", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "auth_get_session") {
        return { user: null, authenticated: false };
      }
      if (cmd === "get_usage_status") {
        return makeUsage();
      }
      if (cmd === "clear_symbol_index" || cmd === "clear_analysis_graph") {
        return true;
      }
      return undefined;
    });
    statusMock.mockReset();
    statusMock.mockResolvedValue(makeStatus({ state: "ready" }));
    generateMock.mockReset();
    markDesktop(true);
    window.location.hash = "";
    act(() => {
      window.dispatchEvent(new Event("hashchange"));
    });
  });

  it("opens directly into the main Chat interface with zero welcome screen or forced modal", async () => {
    render(<App />);

    expect(await screen.findByRole("region", { name: "Conversation messages" })).toBeTruthy();
    expect(screen.getByRole("textbox", { name: "Message Nexora" })).toBeTruthy();
    expect(screen.getByRole("navigation", { name: "Primary" })).toBeTruthy();
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("allows signed-out users to send local chat prompts directly without forced sign-in", async () => {
    generateMock.mockImplementation(async (request, onEvent) => {
      onEvent({ type: "delta", requestId: request.requestId, text: "Local Rust stream answer." });
      return {
        requestId: request.requestId,
        finishReason: "stop",
        cancelled: false,
      };
    });

    const user = userEvent.setup();
    render(<App />);

    const composer = await screen.findByRole("textbox", { name: "Message Nexora" });
    await user.type(composer, "Explain zero-copy parsing in Rust");
    await user.click(screen.getByRole("button", { name: "Send message" }));

    expect(screen.queryByTestId("auth-modal")).toBeNull();
    expect(generateMock).toHaveBeenCalledTimes(1);
    expect(
      await within(screen.getByRole("main")).findByText("Local Rust stream answer."),
    ).toBeTruthy();
  });

  it("opens centred floating AuthModal when explicitly requested and preserves the unsent prompt across dismiss and login", async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "auth_get_session") {
        return { user: null, authenticated: false };
      }
      if (cmd === "auth_login") {
        return {
          id: "u-ada",
          name: "Ada Lovelace",
          email: "ada@example.com",
          emailVerified: true,
        };
      }
      if (cmd === "usage_get_server_summary") {
        return {
          planId: "free",
          planName: "Free",
          periodDayUtc: "2026-10-22",
          resetsAtUtc: "2026-10-23T00:00:00Z",
          models: [
            { modelId: "nexora-mini", dailyLimit: 25, used: 1, reserved: 0, remaining: 24 },
            { modelId: "nexora-sonnet", dailyLimit: 5, used: 0, reserved: 0, remaining: 5 },
          ],
        };
      }
      if (cmd === "get_usage_status") return makeUsage();
      return true;
    });

    const user = userEvent.setup();
    render(<App />);

    const composer = (await screen.findByRole("textbox", {
      name: "Message Nexora",
    })) as HTMLTextAreaElement;

    const unsentPrompt = "Refactor this async stream parser in Rust without allocations";
    await user.type(composer, unsentPrompt);

    // 1. User explicitly opens Sign in from sidebar while keeping unsent draft in Composer
    await user.click(screen.getByRole("button", { name: "Sign in" }));
    const modal = await screen.findByTestId("auth-modal");
    expect(modal).toBeTruthy();
    expect(screen.getByTestId("auth-preserved-prompt")).toBeTruthy();
    expect(composer.value).toBe(unsentPrompt);

    // Dismissing with Escape leaves active chat & unsent draft intact
    await user.keyboard("{Escape}");
    expect(screen.queryByTestId("auth-modal")).toBeNull();
    expect(composer.value).toBe(unsentPrompt);

    // 2. Re-open and authenticate inside floating modal
    await user.click(screen.getByRole("button", { name: "Sign in" }));
    const reopened = await screen.findByTestId("auth-modal");
    expect(within(reopened).getByRole("button", { name: /Continue with Google/i })).toBeTruthy();
    expect(within(reopened).getByRole("tab", { name: "Create account" })).toBeTruthy();

    await user.type(within(reopened).getByLabelText("Email address"), "ada@example.com");
    await user.type(within(reopened).getByLabelText("Password"), "correct horse battery staple");
    await user.click(within(reopened).getByRole("button", { name: "Sign in" }));

    // 3. Modal closes back onto the exact same Chat view with unsent prompt intact
    await vi.waitFor(() => {
      expect(screen.queryByTestId("auth-modal")).toBeNull();
    });
    expect(composer.value).toBe(unsentPrompt);
    expect(await screen.findByLabelText("Daily plan allowance")).toBeTruthy();
  });
});

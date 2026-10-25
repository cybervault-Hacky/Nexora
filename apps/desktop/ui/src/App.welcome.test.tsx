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
    fetchModelCatalog: vi.fn(),
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
import { fetchModelCatalog, fetchStatus, generate } from "./lib/inference";
import { makeStatus, makeUsage, markDesktop } from "./test/fixtures";
import { makeMiniModelCatalog } from "./test/modelFixtures";

const statusMock = vi.mocked(fetchStatus);
const catalogMock = vi.mocked(fetchModelCatalog);
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
    catalogMock.mockReset();
    catalogMock.mockResolvedValue(makeMiniModelCatalog());
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

  it("consumes a legacy verification link into the modal and replaces its one-time token URL", async () => {
    const verificationToken = "d".repeat(64);
    window.location.hash = `#/verify-email?token=${verificationToken}`;
    act(() => {
      window.dispatchEvent(new Event("hashchange"));
    });
    render(<App />);

    const modal = await screen.findByRole("dialog", { name: "Verify your email address" });
    expect((within(modal).getByLabelText("Verification code") as HTMLInputElement).value).toBe(
      verificationToken,
    );
    await vi.waitFor(() => expect(window.location.hash).toBe("#/chat"));
    expect(window.location.href).not.toContain(verificationToken);
  });

  it("keeps the Account route on the shared auth controller and opens its Create account flow only on request", async () => {
    invokeMock.mockImplementation(async (cmd: string, args?: unknown) => {
      if (cmd === "auth_get_session") return { user: null, authenticated: false };
      if (cmd === "get_developer_diagnostics") {
        return {
          platform: "windows",
          arch: "x86_64",
          appVersion: "0.1.0",
          credentialVault: "Windows Credential Manager",
          authServiceUrl: "https://accounts.example.test",
          cspEnforced: true,
          offlineCapable: true,
        };
      }
      if (cmd === "auth_login") {
        return {
          id: "u-ada",
          name: "Ada Lovelace",
          email: "ada@example.com",
          emailVerified: true,
        };
      }
      if (cmd === "account_get_profile") {
        return {
          id: "u-ada",
          name: "Ada Lovelace",
          email: "ada@example.com",
          emailVerified: true,
          hasPassword: true,
          providers: ["password"],
          planId: "free",
          planName: "Free",
          createdAt: "2026-01-15T12:00:00Z",
        };
      }
      if (cmd === "get_usage_status") return makeUsage();
      if (cmd === "clear_symbol_index" || cmd === "clear_analysis_graph") return true;
      return args ? true : undefined;
    });

    window.location.hash = "#/account";
    act(() => {
      window.dispatchEvent(new Event("hashchange"));
    });
    const user = userEvent.setup();
    render(<App />);

    const profile = await screen.findByRole("region", { name: "Account profile" });
    expect(within(profile).getByText("Local Guest Mode (Unauthenticated)")).toBeTruthy();
    expect(screen.queryByRole("dialog")).toBeNull();
    const createAccount = within(profile).getByRole("link", { name: "Create account" });
    await user.click(createAccount);
    const modal = await screen.findByRole("dialog", { name: "Create your Nexora account" });
    expect(document.querySelector(".app")?.hasAttribute("inert")).toBe(true);
    expect(within(modal).getByLabelText("Name")).toBeTruthy();

    await user.click(within(modal).getByRole("button", { name: "Close authentication dialog" }));
    expect(document.querySelector(".app")?.hasAttribute("inert")).toBe(false);
    expect(within(profile).getByText("Local Guest Mode (Unauthenticated)")).toBeTruthy();

    await user.click(within(profile).getByRole("link", { name: "Sign in" }));
    const signInDialog = await screen.findByRole("dialog", { name: "Sign in to Nexora" });
    await user.type(within(signInDialog).getByLabelText("Email address"), "ada@example.com");
    await user.type(within(signInDialog).getByLabelText("Password"), "correct horse battery staple");
    await user.click(within(signInDialog).getByRole("button", { name: "Sign in" }));

    expect(await within(profile).findByText("Ada Lovelace")).toBeTruthy();
    expect(within(profile).queryByText("Local Guest Mode (Unauthenticated)")).toBeNull();
    expect(document.activeElement).toBe(screen.getByRole("main"));
    expect(invokeMock.mock.calls.filter(([command]) => command === "auth_get_session")).toHaveLength(1);
  });

  it("allows signed-out users to send local chat prompts directly without forced sign-in", async () => {
    generateMock.mockImplementation(async (request, onEvent) => {
      onEvent({ type: "delta", requestId: request.requestId, text: "Local Rust stream answer." });
      return {
        requestId: request.requestId,
        finishReason: "stop",
        cancelled: false,
        usageWarnings: [],
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
    expect(document.activeElement).toBe(screen.getByRole("main"));
    expect(composer.value).toBe(unsentPrompt);
    expect(await screen.findByLabelText("Local generation usage")).toBeTruthy();
  });
});

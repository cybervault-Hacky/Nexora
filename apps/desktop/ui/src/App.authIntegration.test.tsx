import { act, render, screen } from "@testing-library/react";
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
import { fetchStatus } from "./lib/inference";
import { makeStatus, markDesktop } from "./test/fixtures";

const statusMock = vi.mocked(fetchStatus);

function goto(hash: string) {
  act(() => {
    window.location.hash = hash;
    window.dispatchEvent(new Event("hashchange"));
  });
}

function emailField() {
  return screen.getByRole("textbox", { name: "Email address" }) as HTMLInputElement;
}

function passwordField() {
  return screen.getByLabelText("Password") as HTMLInputElement;
}

describe("Phase 24 — Desktop Authentication Integration & Flows", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "clear_symbol_index" || cmd === "clear_analysis_graph") {
        return Promise.resolve(true);
      }
      return Promise.resolve(undefined);
    });
    statusMock.mockReset();
    statusMock.mockResolvedValue(makeStatus({ state: "ready" }));
    markDesktop(true);
    window.location.hash = "";
  });

  it("submits login via Tauri IPC command and transitions to home on success", async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "clear_symbol_index" || cmd === "clear_analysis_graph") return true;
      if (cmd === "auth_login") {
        return {
          id: "u-123",
          name: "Alice",
          email: "alice@example.com",
          emailVerified: true,
        };
      }
      return undefined;
    });

    const user = userEvent.setup();
    render(<App />);
    goto("#/signin");

    await screen.findByRole("heading", { name: "Welcome back" });
    await user.type(emailField(), "alice@example.com");
    await user.type(passwordField(), "validpassword8");
    await user.click(screen.getByRole("button", { name: "Sign in" }));

    expect(invokeMock).toHaveBeenCalledWith("auth_login", {
      request: { email: "alice@example.com", password: "validpassword8" },
    });

    // Successfully routed to workspace Home
    expect(await screen.findByRole("heading", { name: "Welcome to Nexora" })).toBeTruthy();
    expect(window.location.hash).toBe("#/home");
  });

  it("displays honest error when credentials are rejected", async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "clear_symbol_index" || cmd === "clear_analysis_graph") return true;
      if (cmd === "auth_login") {
        throw {
          code: "invalid_credentials",
          message: "Invalid email or password.",
        };
      }
      return undefined;
    });

    const user = userEvent.setup();
    render(<App />);
    goto("#/signin");

    await screen.findByRole("heading", { name: "Welcome back" });
    await user.type(emailField(), "wrong@example.com");
    await user.type(passwordField(), "wrongpassword");
    await user.click(screen.getByRole("button", { name: "Sign in" }));

    expect(await screen.findByText("Invalid email or password.")).toBeTruthy();
    expect(window.location.hash).toBe("#/signin");
  });

  it("displays rate limiting notification with retry duration", async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "clear_symbol_index" || cmd === "clear_analysis_graph") return true;
      if (cmd === "auth_login") {
        throw {
          code: "rate_limited",
          message: "Too many attempts.",
          retryAfterSeconds: 120,
        };
      }
      return undefined;
    });

    const user = userEvent.setup();
    render(<App />);
    goto("#/signin");

    await screen.findByRole("heading", { name: "Welcome back" });
    await user.type(emailField(), "abuser@example.com");
    await user.type(passwordField(), "somepassword");
    await user.click(screen.getByRole("button", { name: "Sign in" }));

    expect(await screen.findByText(/Too many attempts\. Try again in 2 minutes\./)).toBeTruthy();
  });

  it("completes Google sign-in flow via Tauri IPC", async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "clear_symbol_index" || cmd === "clear_analysis_graph") return true;
      if (cmd === "auth_google_begin") {
        return {
          id: "g-456",
          name: "Google User",
          email: "user@gmail.com",
          emailVerified: true,
        };
      }
      return undefined;
    });

    const user = userEvent.setup();
    render(<App />);
    goto("#/signin");

    await screen.findByRole("heading", { name: "Welcome back" });
    await user.click(screen.getByRole("button", { name: "Sign in with Google" }));

    expect(invokeMock).toHaveBeenCalledWith("auth_google_begin");
    expect(await screen.findByRole("heading", { name: "Welcome to Nexora" })).toBeTruthy();
    expect(window.location.hash).toBe("#/home");
  });

  it("submits account creation via Tauri IPC and routes to verify email", async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "clear_symbol_index" || cmd === "clear_analysis_graph") return true;
      if (cmd === "auth_register") {
        return {
          status: "verification_pending",
          message: "Account created.",
        };
      }
      return undefined;
    });

    const user = userEvent.setup();
    render(<App />);
    goto("#/signup");

    await screen.findByRole("heading", { name: "Create your account" });
    await user.type(screen.getByRole("textbox", { name: "Full name" }), "Bob Smith");
    await user.type(screen.getByRole("textbox", { name: "Email address" }), "bob@example.com");
    await user.type(screen.getByLabelText("Password"), "strongpassword8");
    await user.type(screen.getByLabelText("Confirm password"), "strongpassword8");
    await user.click(screen.getByRole("button", { name: "Create account" }));

    expect(invokeMock).toHaveBeenCalledWith("auth_register", {
      request: {
        name: "Bob Smith",
        email: "bob@example.com",
        password: "strongpassword8",
      },
    });

    expect(await screen.findByRole("heading", { name: "Email verification is next." })).toBeTruthy();
    expect(window.location.hash).toBe("#/verify-email");
  });

  it("submits password recovery via Tauri IPC", async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "clear_symbol_index" || cmd === "clear_analysis_graph") return true;
      if (cmd === "auth_recover_password") {
        return {
          status: "sent",
          message: "Sent.",
        };
      }
      return undefined;
    });

    const user = userEvent.setup();
    render(<App />);
    goto("#/recover-password");

    await screen.findByRole("heading", { name: "Reset your password" });
    await user.type(screen.getByRole("textbox", { name: "Email address" }), "bob@example.com");
    await user.click(screen.getByRole("button", { name: "Continue" }));

    expect(invokeMock).toHaveBeenCalledWith("auth_recover_password", {
      request: { email: "bob@example.com" },
    });

    expect(
      await screen.findByText("If an account exists for that email, password reset instructions have been sent."),
    ).toBeTruthy();
  });

  it("never leaks raw passwords or tokens into storage or logs", async () => {
    const setItemSpy = vi.spyOn(Storage.prototype, "setItem");
    const logSpy = vi.spyOn(console, "log").mockImplementation(() => {});
    const errorSpy = vi.spyOn(console, "error").mockImplementation(() => {});

    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "clear_symbol_index" || cmd === "clear_analysis_graph") return true;
      if (cmd === "auth_login") {
        return {
          id: "u-99",
          name: "Secret User",
          email: "secret@example.com",
          emailVerified: true,
        };
      }
      return undefined;
    });

    const user = userEvent.setup();
    render(<App />);
    goto("#/signin");

    await screen.findByRole("heading", { name: "Welcome back" });
    await user.type(emailField(), "secret@example.com");
    await user.type(passwordField(), "super-secret-password-xyz");
    await user.click(screen.getByRole("button", { name: "Sign in" }));

    const storedData = setItemSpy.mock.calls.flat().join(" ");
    expect(storedData).not.toContain("super-secret-password-xyz");

    const loggedData = [...logSpy.mock.calls, ...errorSpy.mock.calls].flat().join(" ");
    expect(loggedData).not.toContain("super-secret-password-xyz");

    setItemSpy.mockRestore();
    logSpy.mockRestore();
    errorSpy.mockRestore();
  });
});

import { act, fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

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
  createConversation: async () => ({ id: "new-chat-id", title: "New conversation", createdAt: Date.now(), updatedAt: Date.now(), messages: [] }),
  saveConversation: async (c: import("./lib/history").PersistedConversation) => c,
  loadConversation: async () => { throw { code: "not_found", message: "Not found" }; },
  renameConversation: async () => { throw { code: "not_found", message: "Not found" }; },
  deleteConversation: async () => true,
}));

import App from "./App";
import { fetchStatus } from "./lib/inference";
import { makeStatus, markDesktop } from "./test/fixtures";

const statusMock = vi.mocked(fetchStatus);

/** Sets the route the way a deep link or refresh would, before render. */
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

beforeEach(() => {
  statusMock.mockReset();
  statusMock.mockResolvedValue(makeStatus({ state: "ready" }));
  markDesktop(false);
  window.location.hash = "";
});

describe("Phase 21 sign-in page", () => {
  it("renders the sign-in page with labelled fields and actions", async () => {
    render(<App />);
    goto("#/signin");

    expect(await screen.findByRole("heading", { name: "Welcome back" })).toBeTruthy();
    expect(screen.getByRole("form", { name: "Sign in form" })).toBeTruthy();
    expect(screen.getByLabelText("Email address").getAttribute("type")).toBe("email");
    expect(screen.getByLabelText("Email address").getAttribute("autocomplete")).toBe("email");
    expect(passwordField().getAttribute("type")).toBe("password");
    expect(passwordField().getAttribute("autocomplete")).toBe("current-password");
    expect(screen.getByRole("button", { name: "Sign in" })).toBeTruthy();
    expect(screen.getByRole("link", { name: "Forgot password?" })).toBeTruthy();
    expect(screen.getByRole("link", { name: "Create account" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Sign in with Google" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Back to onboarding" })).toBeTruthy();
  });

  it("shows required-field errors beside each field and keeps input", async () => {
    const user = userEvent.setup();
    render(<App />);
    goto("#/signin");

    await screen.findByRole("heading", { name: "Welcome back" });
    await user.click(screen.getByRole("button", { name: "Sign in" }));

    expect(screen.getByText("Enter your email address.")).toBeTruthy();
    expect(screen.getByText("Enter your password.")).toBeTruthy();
    expect(emailField().getAttribute("aria-invalid")).toBe("true");
    expect(passwordField().getAttribute("aria-invalid")).toBe("true");
    // Errors are associated with their inputs.
    expect(emailField().getAttribute("aria-describedby")).toBe("signin-email-error");
    expect(passwordField().getAttribute("aria-describedby")).toBe("signin-password-error");
    // No success or connection claim on a failed validation.
    expect(screen.queryByText(/not connected yet/i)).toBeNull();
  });

  it("rejects an invalid email format without clearing the password", async () => {
    const user = userEvent.setup();
    render(<App />);
    goto("#/signin");

    await screen.findByRole("heading", { name: "Welcome back" });
    await user.type(passwordField(), "hunter2-stays");
    await user.type(emailField(), "not-an-email");
    await user.click(screen.getByRole("button", { name: "Sign in" }));

    expect(screen.getByText("Enter a valid email address.")).toBeTruthy();
    expect(screen.queryByText("Enter your password.")).toBeNull();
    expect(passwordField().value).toBe("hunter2-stays");
  });

  it("toggles password visibility accessibly", async () => {
    const user = userEvent.setup();
    render(<App />);
    goto("#/signin");

    await screen.findByRole("heading", { name: "Welcome back" });
    await user.type(passwordField(), "secret-value");

    const toggle = screen.getByRole("button", { name: "Show password" });
    await user.click(toggle);

    expect(passwordField().getAttribute("type")).toBe("text");
    expect(screen.getByRole("button", { name: "Hide password" })).toBeTruthy();
    expect(passwordField().value).toBe("secret-value");

    await user.click(screen.getByRole("button", { name: "Hide password" }));
    expect(passwordField().getAttribute("type")).toBe("password");
  });

  it("submits with the keyboard and reports the honest not-connected status", async () => {
    const user = userEvent.setup();
    render(<App />);
    goto("#/signin");

    await screen.findByRole("heading", { name: "Welcome back" });
    await user.type(emailField(), "dev@example.com");
    await user.type(passwordField(), "correct horse");
    await user.keyboard("{Enter}");

    expect(
      await screen.findByText(
        "Sign-in is not connected yet. Account authentication will be enabled in a later phase.",
      ),
    ).toBeTruthy();
    // Never claims success, never navigates into an authenticated area.
    expect(window.location.hash).toBe("#/signin");
    expect(screen.queryByText(/signed in/i)).toBeNull();
  });

  it("shows the loading state and blocks repeated submissions while active", async () => {
    render(<App />);
    goto("#/signin");

    await screen.findByRole("heading", { name: "Welcome back" });
    fireEvent.change(emailField(), { target: { value: "dev@example.com" } });
    fireEvent.change(passwordField(), { target: { value: "pw" } });
    fireEvent.click(screen.getByRole("button", { name: "Sign in" }));

    // Local submission state is active: the button is loading and disabled.
    const busy = screen.getByRole("button", { name: "Signing in…" }) as HTMLButtonElement;
    expect(busy.disabled).toBe(true);
    expect(busy.getAttribute("aria-busy")).toBe("true");

    // Repeated submission attempts during the active state are ignored.
    fireEvent.click(busy);
    fireEvent.click(busy);

    expect(
      await screen.findByText(
        "Sign-in is not connected yet. Account authentication will be enabled in a later phase.",
      ),
    ).toBeTruthy();
    expect(screen.getAllByText(/not connected yet/i)).toHaveLength(1);
    expect(screen.getByRole("button", { name: "Sign in" })).toBeTruthy();
  });

  it("never stores entered credentials", async () => {
    const setItem = vi.spyOn(Storage.prototype, "setItem");
    const log = vi.spyOn(console, "log").mockImplementation(() => {});
    const error = vi.spyOn(console, "error").mockImplementation(() => {});

    render(<App />);
    goto("#/signin");

    await screen.findByRole("heading", { name: "Welcome back" });
    fireEvent.change(emailField(), { target: { value: "dev@example.com" } });
    fireEvent.change(passwordField(), { target: { value: "super-secret" } });
    fireEvent.click(screen.getByRole("button", { name: "Sign in" }));
    await screen.findByText(/not connected yet/i);

    // Credentials must never reach storage (the app may store unrelated
    // preferences — what matters is that the entered values do not appear).
    const stored = setItem.mock.calls.flat().join(" ");
    expect(stored).not.toContain("super-secret");
    expect(stored).not.toContain("dev@example.com");
    const logged = [...log.mock.calls, ...error.mock.calls].flat().join(" ");
    expect(logged).not.toContain("super-secret");
    expect(logged).not.toContain("dev@example.com");
    setItem.mockRestore();
    log.mockRestore();
    error.mockRestore();
  });

  it("explains that Google sign-in is not connected yet", async () => {
    const user = userEvent.setup();
    render(<App />);
    goto("#/signin");

    await screen.findByRole("heading", { name: "Welcome back" });
    await user.click(screen.getByRole("button", { name: "Sign in with Google" }));

    expect(
      await screen.findByText(
        "Google sign-in will be connected with the authentication backend in a later phase.",
      ),
    ).toBeTruthy();
    expect(window.location.hash).toBe("#/signin");
  });

  it("navigates to the recovery and create-account pages", async () => {
    const user = userEvent.setup();
    render(<App />);
    goto("#/signin");

    await screen.findByRole("heading", { name: "Welcome back" });

    await user.click(screen.getByRole("link", { name: "Forgot password?" }));
    expect(window.location.hash).toBe("#/recover-password");
    expect(await screen.findByRole("heading", { name: "Reset your password" })).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "Back to Sign In" }));
    expect(await screen.findByRole("heading", { name: "Welcome back" })).toBeTruthy();

    await user.click(screen.getByRole("link", { name: "Create account" }));
    expect(window.location.hash).toBe("#/signup");
    expect(await screen.findByRole("heading", { name: "Create your account" })).toBeTruthy();
  });

  it("goes back to onboarding and integrates with the welcome flow", async () => {
    const user = userEvent.setup();
    render(<App />);

    // Welcome → Continue → onboarding → Continue to Sign in.
    await screen.findByRole("button", { name: "Continue" });
    await user.click(screen.getByRole("button", { name: "Continue" }));
    await screen.findByRole("heading", { name: "Sign-in is the next step." });
    await user.click(screen.getByRole("button", { name: "Continue to Sign in" }));

    expect(await screen.findByRole("heading", { name: "Welcome back" })).toBeTruthy();
    expect(window.location.hash).toBe("#/signin");

    await user.click(screen.getByRole("button", { name: "Back to onboarding" }));
    expect(window.location.hash).toBe("#/onboarding");
    expect(await screen.findByRole("heading", { name: "Sign-in is the next step." })).toBeTruthy();
  });

  it("keeps unknown routes graceful and the workspace reachable", async () => {
    const user = userEvent.setup();
    render(<App />);
    goto("#/not-a-real-page");

    expect(await screen.findByRole("heading", { name: "Page not found." })).toBeTruthy();
    await user.click(screen.getByRole("link", { name: /Return to Home/i }));
    expect(window.location.hash).toBe("#/home");
    expect(screen.getByRole("navigation", { name: "Primary" })).toBeTruthy();

    await user.click(screen.getByRole("link", { name: "AI Chat" }));
    expect(await screen.findByRole("region", { name: "Conversation messages" })).toBeTruthy();
  });
});

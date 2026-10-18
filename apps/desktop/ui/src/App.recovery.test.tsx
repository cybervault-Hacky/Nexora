import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
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

const HONEST_RECOVERY_STATUS =
  "Password recovery is not connected yet. No reset email has been sent.";
const HONEST_RESET_STATUS =
  "Password reset is not connected yet. No password has been changed.";

function goto(hash: string) {
  act(() => {
    window.location.hash = hash;
    window.dispatchEvent(new Event("hashchange"));
  });
}

function field(id: string) {
  return document.getElementById(id) as HTMLInputElement;
}

beforeEach(() => {
  statusMock.mockReset();
  statusMock.mockResolvedValue(makeStatus({ state: "ready" }));
  markDesktop(false);
  window.location.hash = "";
});

describe("Phase 22 password recovery page", () => {
  it("renders with an accessible email field and actions", async () => {
    render(<App />);
    goto("#/recover-password");

    expect(await screen.findByRole("heading", { name: "Reset your password" })).toBeTruthy();
    expect(screen.getByRole("form", { name: "Password recovery form" })).toBeTruthy();
    expect(screen.getByLabelText("Email address").getAttribute("type")).toBe("email");
    expect(screen.getByRole("button", { name: "Continue" })).toBeTruthy();
    expect(screen.getByRole("link", { name: "Sign in" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Back to Sign In" })).toBeTruthy();
  });

  it("validates required and malformed email input", async () => {
    const user = userEvent.setup();
    render(<App />);
    goto("#/recover-password");

    await screen.findByRole("heading", { name: "Reset your password" });
    await user.click(screen.getByRole("button", { name: "Continue" }));
    expect(screen.getByText("Enter your email address.")).toBeTruthy();

    await user.type(screen.getByLabelText("Email address"), "nope");
    await user.click(screen.getByRole("button", { name: "Continue" }));
    expect(screen.getByText("Enter a valid email address.")).toBeTruthy();
    expect(field("recover-email").getAttribute("aria-invalid")).toBe("true");
    expect(field("recover-email").getAttribute("aria-describedby")).toBe("recover-email-error");
    expect(screen.queryByText(HONEST_RECOVERY_STATUS)).toBeNull();
  });

  it("reports the honest status after valid input without sending anything", async () => {
    const fetchSpy = vi.fn();
    vi.stubGlobal("fetch", fetchSpy);
    const setItem = vi.spyOn(Storage.prototype, "setItem");

    render(<App />);
    goto("#/recover-password");

    await screen.findByRole("heading", { name: "Reset your password" });
    fireEvent.change(field("recover-email"), { target: { value: "ada@example.com" } });
    fireEvent.click(screen.getByRole("button", { name: "Continue" }));

    expect(await screen.findByText(HONEST_RECOVERY_STATUS)).toBeTruthy();
    // No network submission, no storage, no "email sent" claim, no account claim.
    expect(fetchSpy).not.toHaveBeenCalled();
    expect(setItem.mock.calls.flat().join(" ")).not.toContain("ada@example.com");
    expect(screen.queryByText(/^An email has been sent/i)).toBeNull();
    expect(screen.queryByText(/account (exists|does not)/i)).toBeNull();
    expect(window.location.hash).toBe("#/recover-password");

    setItem.mockRestore();
    vi.unstubAllGlobals();
  });

  it("shows loading state and blocks repeated submissions while active", async () => {
    render(<App />);
    goto("#/recover-password");

    await screen.findByRole("heading", { name: "Reset your password" });
    fireEvent.change(field("recover-email"), { target: { value: "ada@example.com" } });
    fireEvent.click(screen.getByRole("button", { name: "Continue" }));

    const busy = screen.getByRole("button", { name: "Continuing…" }) as HTMLButtonElement;
    expect(busy.disabled).toBe(true);
    fireEvent.click(busy);
    fireEvent.click(busy);

    expect(await screen.findByText(HONEST_RECOVERY_STATUS)).toBeTruthy();
    expect(screen.getAllByText(HONEST_RECOVERY_STATUS)).toHaveLength(1);
  });
});

describe("Phase 22 password reset page", () => {
  it("renders on direct load with new-password fields and an honest note", async () => {
    render(<App />);
    goto("#/reset-password");

    expect(await screen.findByRole("heading", { name: "Choose a new password" })).toBeTruthy();
    expect(screen.getByLabelText("New password").getAttribute("autocomplete")).toBe("new-password");
    expect(screen.getByLabelText("Confirm new password")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Reset password" })).toBeTruthy();
    expect(screen.getByRole("note", { name: "About reset links" })).toBeTruthy();
    // No token field exists — tokens require the future backend.
    expect(screen.queryByLabelText(/token/i)).toBeNull();
  });

  it("validates length and confirmation, then reports the honest status", async () => {
    const user = userEvent.setup();
    render(<App />);
    goto("#/reset-password");

    await screen.findByRole("heading", { name: "Choose a new password" });
    await user.type(screen.getByLabelText("New password"), "short");
    await user.type(screen.getByLabelText("Confirm new password"), "short");
    await user.click(screen.getByRole("button", { name: "Reset password" }));
    expect(screen.getByText("Use at least 8 characters.")).toBeTruthy();
    expect(field("reset-password").value).toBe("short");

    await user.clear(screen.getByLabelText("New password"));
    await user.type(screen.getByLabelText("New password"), "long-enough-one");
    await user.clear(screen.getByLabelText("Confirm new password"));
    await user.type(screen.getByLabelText("Confirm new password"), "long-enough-two");
    await user.click(screen.getByRole("button", { name: "Reset password" }));
    expect(screen.getByText("Passwords do not match.")).toBeTruthy();

    await user.clear(screen.getByLabelText("Confirm new password"));
    await user.type(screen.getByLabelText("Confirm new password"), "long-enough-one");
    await user.click(screen.getByRole("button", { name: "Reset password" }));

    expect(await screen.findByText(HONEST_RESET_STATUS)).toBeTruthy();
    expect(window.location.hash).toBe("#/reset-password");
    expect(field("reset-password").value).toBe("");
    expect(field("reset-confirm").value).toBe("");
  });
});

describe("Phase 22 routing and regression", () => {
  it("connects Sign In with both account pages and back again", async () => {
    const user = userEvent.setup();
    render(<App />);
    goto("#/signin");

    await screen.findByRole("heading", { name: "Welcome back" });
    await user.click(screen.getByRole("link", { name: "Create account" }));
    expect(await screen.findByRole("heading", { name: "Create your account" })).toBeTruthy();
    await user.click(screen.getByRole("link", { name: "Sign in" }));
    expect(await screen.findByRole("heading", { name: "Welcome back" })).toBeTruthy();
    await user.click(screen.getByRole("link", { name: "Forgot password?" }));
    expect(await screen.findByRole("heading", { name: "Reset your password" })).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Back to Sign In" }));
    expect(await screen.findByRole("heading", { name: "Welcome back" })).toBeTruthy();
  });

  it("supports browser history back across the account routes", async () => {
    render(<App />);
    goto("#/signin");
    await screen.findByRole("heading", { name: "Welcome back" });

    goto("#/signup");
    await screen.findByRole("heading", { name: "Create your account" });

    act(() => {
      window.history.back();
    });
    await waitFor(() => expect(window.location.hash).toBe("#/signin"));
    expect(await screen.findByRole("heading", { name: "Welcome back" })).toBeTruthy();
  });

  it("keeps unknown routes graceful and the welcome/workspace flow intact", async () => {
    const user = userEvent.setup();
    render(<App />);
    goto("#/no-such-route");

    expect(await screen.findByRole("heading", { name: "Page not found." })).toBeTruthy();
    await user.click(screen.getByRole("link", { name: /Return to Home/i }));
    expect(window.location.hash).toBe("#/home");
    expect(screen.getByRole("navigation", { name: "Primary" })).toBeTruthy();

    goto("");
    expect(await screen.findByRole("button", { name: "Continue" })).toBeTruthy();
    expect(screen.queryByRole("navigation", { name: "Primary" })).toBeNull();
  });
});

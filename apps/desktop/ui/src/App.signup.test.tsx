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

const HONEST_SIGNUP_STATUS =
  "Account creation is not connected yet. Your details have not been submitted.";

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

describe("Phase 22 create account page", () => {
  it("renders with accessible labels and actions", async () => {
    render(<App />);
    goto("#/signup");

    expect(await screen.findByRole("heading", { name: "Create your account" })).toBeTruthy();
    expect(screen.getByRole("form", { name: "Create account form" })).toBeTruthy();
    expect(screen.getByLabelText("Full name")).toBeTruthy();
    expect(screen.getByLabelText("Email address").getAttribute("autocomplete")).toBe("email");
    expect(screen.getByLabelText("Password").getAttribute("autocomplete")).toBe("new-password");
    expect(screen.getByLabelText("Confirm password")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Create account" })).toBeTruthy();
    expect(screen.getByRole("link", { name: "Sign in" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Back to Sign In" })).toBeTruthy();
  });

  it("validates required fields beside each input", async () => {
    const user = userEvent.setup();
    render(<App />);
    goto("#/signup");

    await screen.findByRole("heading", { name: "Create your account" });
    await user.click(screen.getByRole("button", { name: "Create account" }));

    expect(screen.getByText("Enter your full name.")).toBeTruthy();
    expect(screen.getByText("Enter your email address.")).toBeTruthy();
    expect(screen.getByText("Enter your password.")).toBeTruthy();
    expect(screen.getByText("Re-enter your password.")).toBeTruthy();
    expect(field("signup-name").getAttribute("aria-invalid")).toBe("true");
    expect(field("signup-name").getAttribute("aria-describedby")).toBe("signup-name-error");
    expect(field("signup-confirm").getAttribute("aria-describedby")).toBe("signup-confirm-error");
    expect(screen.queryByText(HONEST_SIGNUP_STATUS)).toBeNull();
  });

  it("rejects an invalid email and a short password", async () => {
    const user = userEvent.setup();
    render(<App />);
    goto("#/signup");

    await screen.findByRole("heading", { name: "Create your account" });
    await user.type(screen.getByLabelText("Full name"), "Ada Lovelace");
    await user.type(screen.getByLabelText("Email address"), "not-an-email");
    await user.type(screen.getByLabelText("Password"), "short");
    await user.type(screen.getByLabelText("Confirm password"), "short");
    await user.click(screen.getByRole("button", { name: "Create account" }));

    expect(screen.getByText("Enter a valid email address.")).toBeTruthy();
    expect(screen.getByText("Use at least 8 characters.")).toBeTruthy();
  });

  it("rejects a confirmation that does not match and keeps all values", async () => {
    const user = userEvent.setup();
    render(<App />);
    goto("#/signup");

    await screen.findByRole("heading", { name: "Create your account" });
    await user.type(screen.getByLabelText("Full name"), "Ada Lovelace");
    await user.type(screen.getByLabelText("Email address"), "ada@example.com");
    await user.type(screen.getByLabelText("Password"), "long-enough-one");
    await user.type(screen.getByLabelText("Confirm password"), "long-enough-two");
    await user.click(screen.getByRole("button", { name: "Create account" }));

    expect(screen.getByText("Passwords do not match.")).toBeTruthy();
    // Validation failure never clears what the user typed.
    expect(field("signup-password").value).toBe("long-enough-one");
    expect(field("signup-confirm").value).toBe("long-enough-two");
  });

  it("toggles both password fields accessibly", async () => {
    const user = userEvent.setup();
    render(<App />);
    goto("#/signup");

    await screen.findByRole("heading", { name: "Create your account" });
    await user.type(screen.getByLabelText("Password"), "secret-one");
    await user.type(screen.getByLabelText("Confirm password"), "secret-one");

    await user.click(screen.getByRole("button", { name: "Show password" }));
    expect(field("signup-password").getAttribute("type")).toBe("text");
    expect(screen.getByRole("button", { name: "Hide password" })).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "Show password confirmation" }));
    expect(field("signup-confirm").getAttribute("type")).toBe("text");
    expect(screen.getByRole("button", { name: "Hide password confirmation" })).toBeTruthy();
  });

  it("submits with the keyboard and reports the honest status, clearing passwords", async () => {
    const user = userEvent.setup();
    render(<App />);
    goto("#/signup");

    await screen.findByRole("heading", { name: "Create your account" });
    await user.type(screen.getByLabelText("Full name"), "Ada Lovelace");
    await user.type(screen.getByLabelText("Email address"), "ada@example.com");
    await user.type(screen.getByLabelText("Password"), "long-enough-one");
    await user.type(screen.getByLabelText("Confirm password"), "long-enough-one");
    await user.keyboard("{Enter}");

    expect(await screen.findByText(HONEST_SIGNUP_STATUS)).toBeTruthy();
    // No account, no session, no navigation away.
    expect(window.location.hash).toBe("#/signup");
    // Sensitive values are cleared after the completed submission.
    expect(field("signup-password").value).toBe("");
    expect(field("signup-confirm").value).toBe("");
    expect(field("signup-name").value).toBe("Ada Lovelace");
    expect(field("signup-email").value).toBe("ada@example.com");
  });

  it("shows loading state and blocks repeated submissions while active", async () => {
    render(<App />);
    goto("#/signup");

    await screen.findByRole("heading", { name: "Create your account" });
    fireEvent.change(field("signup-name"), { target: { value: "Ada" } });
    fireEvent.change(field("signup-email"), { target: { value: "ada@example.com" } });
    fireEvent.change(field("signup-password"), { target: { value: "long-enough-one" } });
    fireEvent.change(field("signup-confirm"), { target: { value: "long-enough-one" } });
    fireEvent.click(screen.getByRole("button", { name: "Create account" }));

    const busy = screen.getByRole("button", { name: "Creating account…" }) as HTMLButtonElement;
    expect(busy.disabled).toBe(true);
    expect(busy.getAttribute("aria-busy")).toBe("true");
    fireEvent.click(busy);
    fireEvent.click(busy);

    expect(await screen.findByText(HONEST_SIGNUP_STATUS)).toBeTruthy();
    expect(screen.getAllByText(HONEST_SIGNUP_STATUS)).toHaveLength(1);
  });

  it("never stores or logs the submitted details", async () => {
    const setItem = vi.spyOn(Storage.prototype, "setItem");
    const fetchSpy = vi.fn();
    vi.stubGlobal("fetch", fetchSpy);
    const log = vi.spyOn(console, "log").mockImplementation(() => {});
    const error = vi.spyOn(console, "error").mockImplementation(() => {});

    render(<App />);
    goto("#/signup");

    await screen.findByRole("heading", { name: "Create your account" });
    fireEvent.change(field("signup-name"), { target: { value: "Ada Lovelace" } });
    fireEvent.change(field("signup-email"), { target: { value: "ada@example.com" } });
    fireEvent.change(field("signup-password"), { target: { value: "super-secret-pw" } });
    fireEvent.change(field("signup-confirm"), { target: { value: "super-secret-pw" } });
    fireEvent.click(screen.getByRole("button", { name: "Create account" }));
    await screen.findByText(HONEST_SIGNUP_STATUS);

    expect(fetchSpy).not.toHaveBeenCalled();
    const stored = setItem.mock.calls.flat().join(" ");
    expect(stored).not.toContain("super-secret-pw");
    expect(stored).not.toContain("ada@example.com");
    const logged = [...log.mock.calls, ...error.mock.calls].flat().join(" ");
    expect(logged).not.toContain("super-secret-pw");
    expect(logged).not.toContain("Ada Lovelace");

    setItem.mockRestore();
    log.mockRestore();
    error.mockRestore();
    vi.unstubAllGlobals();
  });

  it("leads to the honest verification-pending experience", async () => {
    const user = userEvent.setup();
    render(<App />);
    goto("#/signup");

    await screen.findByRole("heading", { name: "Create your account" });
    fireEvent.change(field("signup-name"), { target: { value: "Ada" } });
    fireEvent.change(field("signup-email"), { target: { value: "ada@example.com" } });
    fireEvent.change(field("signup-password"), { target: { value: "long-enough-one" } });
    fireEvent.change(field("signup-confirm"), { target: { value: "long-enough-one" } });
    fireEvent.click(screen.getByRole("button", { name: "Create account" }));
    await screen.findByText(HONEST_SIGNUP_STATUS);

    await user.click(screen.getByRole("link", { name: "About email verification" }));
    expect(window.location.hash).toBe("#/verify-email");

    expect(await screen.findByRole("heading", { name: "Email verification is next." })).toBeTruthy();
    expect(screen.getByText("No email has been sent.")).toBeTruthy();
    // No fake code, token, or resend controls exist.
    expect(screen.queryByRole("button", { name: /resend/i })).toBeNull();
    expect(screen.queryByLabelText(/code/i)).toBeNull();
    expect(screen.getByRole("note", { name: "Verification status" })).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "Back to Sign In" }));
    expect(window.location.hash).toBe("#/signin");
    expect(await screen.findByRole("heading", { name: "Welcome back" })).toBeTruthy();
  });
});

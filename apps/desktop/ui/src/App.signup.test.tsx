import { render, screen, within } from "@testing-library/react";
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
    fetchSettings: vi.fn().mockResolvedValue({ modelPath: null, runtimePath: null, contextSize: 32768 }),
    fetchSystemInfo: vi.fn().mockResolvedValue({ totalRamGib: 32, availableRamGib: 20, platform: "windows" }),
    generate: vi.fn(),
    cancelGeneration: vi.fn().mockResolvedValue(true),
    unloadModel: vi.fn(),
  };
});

import App from "./App";
import { fetchStatus } from "./lib/inference";
import { makeStatus, makeUsage, markDesktop } from "./test/fixtures";

describe("Create Account & Email Verification Bottom Sheet Flow", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "auth_get_session") return { user: null, authenticated: false };
      if (cmd === "auth_register") {
        return {
          status: "verification_pending",
          message: "Account created. Enter your verification code.",
        };
      }
      if (cmd === "auth_verify_email") {
        return { status: "verified", message: "Email verified." };
      }
      if (cmd === "get_usage_status") return makeUsage();
      return true;
    });
    vi.mocked(fetchStatus).mockResolvedValue(makeStatus({ state: "ready" }));
    markDesktop(true);
    window.location.hash = "";
  });

  it("validates registration inputs and transitions cleanly to inline email verification", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "Sign in" }));
    const modal = await screen.findByRole("dialog");
    await user.click(within(modal).getByRole("button", { name: "Create an account" }));

    // Mismatched confirmation password
    await user.type(within(modal).getByLabelText("Full name"), "Ada Lovelace");
    await user.type(within(modal).getByLabelText("Email address"), "ada@example.com");
    await user.type(within(modal).getByLabelText("Password"), "strongpassword123");
    await user.type(within(modal).getByLabelText("Confirm password"), "mismatch123");
    await user.click(within(modal).getByRole("button", { name: "Create account" }));
    expect(within(modal).getByRole("alert")).toBeTruthy();

    // Fix confirmation and register -> transitions straight to Verify Email step in same modal
    const confirmInput = within(modal).getByLabelText("Confirm password");
    await user.clear(confirmInput);
    await user.type(confirmInput, "strongpassword123");
    await user.click(within(modal).getByRole("button", { name: "Create account" }));

    expect(invokeMock).toHaveBeenCalledWith("auth_register", {
      request: { name: "Ada Lovelace", email: "ada@example.com", password: "strongpassword123" },
    });
    expect(await within(modal).findByLabelText("Verification code")).toBeTruthy();

    const verificationToken = "a".repeat(64);
    await user.type(within(modal).getByLabelText("Verification code"), verificationToken);
    await user.click(within(modal).getByRole("button", { name: "Verify email" }));
    expect(invokeMock).toHaveBeenCalledWith("auth_verify_email", {
      request: { token: verificationToken },
    });
    expect(await within(modal).findByText("Email verified.")).toBeTruthy();
    expect(modal.getAttribute("aria-modal")).toBe("true");
  });
});

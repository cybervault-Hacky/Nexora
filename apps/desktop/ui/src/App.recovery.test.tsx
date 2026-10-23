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

describe("Floating Password Recovery & Reset Modal Flow", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "auth_get_session") return { user: null, authenticated: false };
      if (cmd === "auth_recover_password") {
        return { status: "sent", message: "Reset token sent if account exists." };
      }
      if (cmd === "auth_reset_password") {
        return { status: "reset", message: "Password updated." };
      }
      if (cmd === "get_usage_status") return makeUsage();
      return true;
    });
    vi.mocked(fetchStatus).mockResolvedValue(makeStatus({ state: "ready" }));
    markDesktop(true);
    window.location.hash = "";
  });

  it("handles forgot-password request and password reset cleanly inside the floating modal", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "Sign in" }));
    const modal = await screen.findByRole("dialog");

    await user.click(within(modal).getByRole("button", { name: "Forgot password?" }));
    await user.type(within(modal).getByLabelText("Email address"), "ada@example.com");
    await user.click(within(modal).getByRole("button", { name: "Send reset token" }));

    expect(invokeMock).toHaveBeenCalledWith("auth_recover_password", {
      request: { email: "ada@example.com" },
    });

    // Transitions directly to reset token + new password step
    expect(await within(modal).findByLabelText("Reset token")).toBeTruthy();
    await user.type(within(modal).getByLabelText("Reset token"), "reset-tok-999");
    await user.type(within(modal).getByLabelText("New password"), "brandnewpassword99");
    await user.click(within(modal).getByRole("button", { name: "Update password" }));

    expect(invokeMock).toHaveBeenCalledWith("auth_reset_password", {
      request: { token: "reset-tok-999", password: "brandnewpassword99" },
    });
  });
});

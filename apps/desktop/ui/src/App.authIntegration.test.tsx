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

import App from "./App";
import { fetchStatus } from "./lib/inference";
import { makeStatus, makeUsage, markDesktop } from "./test/fixtures";

describe("Desktop Authentication IPC Integration (Email, Rate-Limit, Google PKCE)", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "auth_get_session") return { user: null, authenticated: false };
      if (cmd === "get_usage_status") return makeUsage();
      return true;
    });
    vi.mocked(fetchStatus).mockResolvedValue(makeStatus({ state: "ready" }));
    markDesktop(true);
    window.location.hash = "";
    act(() => {
      window.dispatchEvent(new Event("hashchange"));
    });
  });

  it("authenticates via auth_login IPC and stays on Chat with Account link in sidebar", async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "auth_get_session") return { user: null, authenticated: false };
      if (cmd === "auth_login") {
        return { id: "u-123", name: "Alice", email: "alice@example.com", emailVerified: true };
      }
      if (cmd === "get_usage_status") return makeUsage();
      return true;
    });

    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "Sign in" }));
    const modal = await screen.findByRole("dialog");

    await user.type(within(modal).getByLabelText("Email address"), "alice@example.com");
    await user.type(within(modal).getByLabelText("Password"), "validpassword8");
    await user.click(within(modal).getByRole("button", { name: "Sign in" }));

    expect(invokeMock).toHaveBeenCalledWith("auth_login", {
      request: { email: "alice@example.com", password: "validpassword8" },
    });
    await vi.waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    expect(await screen.findByRole("link", { name: "Developer Account" })).toBeTruthy();
  });

  it("surfaces invalid-credential and rate-limit errors accurately inside the modal", async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "auth_get_session") return { user: null, authenticated: false };
      if (cmd === "auth_login") {
        throw { code: "rate_limited", message: "Too many attempts. Try again in a few minutes." };
      }
      if (cmd === "get_usage_status") return makeUsage();
      return true;
    });

    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "Sign in" }));
    const modal = await screen.findByRole("dialog");

    await user.type(within(modal).getByLabelText("Email address"), "alice@example.com");
    await user.type(within(modal).getByLabelText("Password"), "wrongpassword8");
    await user.click(within(modal).getByRole("button", { name: "Sign in" }));

    expect(await within(modal).findByText(/Too many attempts/i)).toBeTruthy();
  });

  it("completes Google PKCE sign-in via auth_google_begin IPC", async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "auth_get_session") return { user: null, authenticated: false };
      if (cmd === "auth_google_begin") {
        return { id: "g-1", name: "Grace Hopper", email: "grace@example.com", emailVerified: true };
      }
      if (cmd === "get_usage_status") return makeUsage();
      return true;
    });

    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "Sign in" }));
    const modal = await screen.findByRole("dialog");

    await user.click(within(modal).getByRole("button", { name: /Continue with Google/i }));
    expect(invokeMock).toHaveBeenCalledWith("auth_google_begin");

    await vi.waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
  });
});

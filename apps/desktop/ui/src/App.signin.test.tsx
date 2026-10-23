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

describe("Floating Sign-In Modal over Chat", () => {
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
    window.localStorage.clear();
  });

  it("opens from sidebar Sign in action with accessible controls and zero stored secrets", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "Sign in" }));
    const modal = await screen.findByRole("dialog");

    expect(within(modal).getByLabelText("Email address")).toBeTruthy();
    expect(within(modal).getByLabelText("Password")).toBeTruthy();
    expect(within(modal).getByRole("button", { name: /Continue with Google/i })).toBeTruthy();

    // Rejects invalid email without losing password input
    const pw = within(modal).getByLabelText("Password") as HTMLInputElement;
    await user.type(within(modal).getByLabelText("Email address"), "not-an-email");
    await user.type(pw, "super-secret-pass");
    await user.click(within(modal).getByRole("button", { name: "Sign in" }));

    expect(within(modal).getByRole("alert")).toBeTruthy();
    expect(pw.value).toBe("super-secret-pass");
    expect(JSON.stringify(window.localStorage)).not.toContain("super-secret-pass");
  });

  it("supports Escape to dismiss the floating modal cleanly back to Chat", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "Sign in" }));
    expect(await screen.findByRole("dialog")).toBeTruthy();

    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).toBeNull();
  });
});

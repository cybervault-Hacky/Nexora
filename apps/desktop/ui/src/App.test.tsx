import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

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



// These engine/shell tests isolate the new history IPC boundary as well.
vi.mock("./lib/history", () => ({
  listConversations: async () => [],
  createConversation: async () => ({ id: "new-chat-id", title: "New conversation", createdAt: Date.now(), updatedAt: Date.now(), messages: [] }),
  saveConversation: async (c: import("./lib/history").PersistedConversation) => c,
  loadConversation: async () => { throw { code: "not_found", message: "Not found" }; },
  renameConversation: async () => { throw { code: "not_found", message: "Not found" }; },
  deleteConversation: async () => true,
}));

import App from "./App";
import { fetchSettings, fetchStatus, fetchSystemInfo } from "./lib/inference";
import { previewStatus } from "./lib/ipc";
import { makeStatus, markDesktop } from "./test/fixtures";

const statusMock = vi.mocked(fetchStatus);
const settingsMock = vi.mocked(fetchSettings);
const systemMock = vi.mocked(fetchSystemInfo);

beforeEach(() => {
  statusMock.mockReset();
  settingsMock.mockClear();
  systemMock.mockClear();
  systemMock.mockResolvedValue({
    totalRamGib: 32,
    availableRamGib: 20.5,
    platform: "windows",
  });
  markDesktop(false);
  // Phase 19: the app opens on Home; these suites cover the AI Chat page.
  window.location.hash = "#/chat";
});

afterEach(() => {
  markDesktop(false);
});

describe("shell lifecycle", () => {
  it("removes its keyboard listener when it unmounts", () => {
    const remove = vi.spyOn(window, "removeEventListener");
    const add = vi.spyOn(window, "addEventListener");

    statusMock.mockResolvedValue(makeStatus({ state: "not_configured" }));

    const { unmount } = render(<App />);

    const registered = add.mock.calls.filter(([type]) => type === "keydown");

    expect(registered).toHaveLength(1);

    const handler = registered[0]?.[1];

    unmount();

    expect(
      remove.mock.calls.some(
        ([type, listener]) => type === "keydown" && listener === handler,
      ),
    ).toBe(true);
  });
});

describe("browser preview", () => {
  it("says the desktop app is required instead of pretending inference works", async () => {
    statusMock.mockResolvedValue(previewStatus());

    render(<App />);

    expect(await screen.findByText("Local inference requires the Nexora desktop app.")).toBeTruthy();
    expect(screen.getByText(/Local inference requires the Nexora desktop app/i)).toBeTruthy();
    expect(screen.queryByText("Engine ready")).toBeNull();
    expect(
      (screen.getByRole("button", { name: "Send message" }) as HTMLButtonElement)
        .disabled,
    ).toBe(true);
  });

  it("still renders the real shell", async () => {
    statusMock.mockResolvedValue(previewStatus());

    render(<App />);
    await screen.findByText("Local inference requires the Nexora desktop app.");

    expect(screen.getAllByText(/Nexora/).length).toBeGreaterThan(0);
    expect(screen.getByRole("button", { name: "New chat" })).toBeTruthy();
    expect(screen.getByRole("textbox", { name: "Message Nexora" })).toBeTruthy();
    expect(
      screen.getByRole("region", { name: "Conversation messages" }).getAttribute("tabindex"),
    ).toBe("0");
  });
});

describe("desktop status routing", () => {
  it("shows setup when no model is configured", async () => {
    markDesktop();
    statusMock.mockResolvedValue(makeStatus({ state: "not_configured" }));

    render(<App />);

    expect(await screen.findByText("What's New Today?")).toBeTruthy();
    // Model setup moved to the Local Models page in Phase 19; the chat empty
    // state links there with a real navigation target.
    await userEvent.setup().click(
      screen.getByRole("link", { name: "Open Local Models" }),
    );
    expect(
      await screen.findByRole("textbox", { name: "Runtime executable" }),
    ).toBeTruthy();
    expect(
      screen.getByRole("textbox", { name: "Model file (.gguf)" }),
    ).toBeTruthy();
    expect(screen.queryByText("What's New Today?")).toBeNull();
  });

  it("shows the ready conversation surface when a model is loaded", async () => {
    markDesktop();
    statusMock.mockResolvedValue(makeStatus({ state: "ready" }));

    render(<App />);

    expect(await screen.findByText("Engine ready")).toBeTruthy();
    expect(screen.getByText("What's New Today?")).toBeTruthy();
    expect(screen.getByText(/qwen2\.5-coder-7b/)).toBeTruthy();
  });

  it("reports a load in progress without claiming readiness", async () => {
    markDesktop();
    statusMock.mockResolvedValue(
      makeStatus({ state: "loading", detail: "Loading weights into memory." }),
    );

    render(<App />);

    expect(await screen.findByText("Loading model")).toBeTruthy();
    expect(screen.getByText("Loading weights into memory.")).toBeTruthy();
    expect(screen.queryByText("Engine ready")).toBeNull();
    expect(screen.getByText("What's New Today?")).toBeTruthy();
  });

  it("reports an engine failure with the core's own words", async () => {
    markDesktop();
    statusMock.mockResolvedValue(
      makeStatus({
        state: "error",
        detail: "The local engine stopped unexpectedly (exit status: 1).",
      }),
    );

    render(<App />);

    expect(await screen.findByText("Engine failed")).toBeTruthy();
    expect(
      screen.getByTitle(/stopped unexpectedly \(exit status: 1\)/),
    ).toBeTruthy();
    expect(screen.queryByText("Engine ready")).toBeNull();
    expect(screen.getByText("What's New Today?")).toBeTruthy();
  });

  it("never confuses an IPC failure with a healthy but model-less engine", async () => {
    markDesktop();
    statusMock.mockRejectedValue({
      code: "ipc",
      message: "The desktop core did not answer.",
    });

    render(<App />);

    expect(
      await screen.findByText(/Could not read the engine status/),
    ).toBeTruthy();
    expect(screen.getByText("Engine status unknown")).toBeTruthy();
    expect(await screen.findByText(/Engine status unavailable\. Retry/)).toBeTruthy();
    expect(screen.getByText("What's New Today?")).toBeTruthy();
    expect(
      (screen.getByRole("button", { name: "Send message" }) as HTMLButtonElement)
        .disabled,
    ).toBe(true);
  });

  it("retries the status read from the notice", async () => {
    markDesktop();
    statusMock
      .mockRejectedValueOnce({ code: "ipc", message: "no answer" })
      .mockResolvedValue(makeStatus({ state: "ready" }));

    render(<App />);

    const retry = await screen.findByRole("button", { name: "Retry", hidden: false });
    // Ensure we click the status retry, not a history retry (history is mocked to empty)
    expect(retry.className).toContain("notice__action");
    await userEvent.setup().click(retry);

    expect(await screen.findByText("Engine ready")).toBeTruthy();
    expect(statusMock.mock.calls.length).toBeGreaterThanOrEqual(2);
  });
});

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
});

afterEach(() => {
  markDesktop(false);
});

describe("browser preview", () => {
  it("says the desktop app is required instead of pretending inference works", async () => {
    statusMock.mockResolvedValue(previewStatus());

    render(<App />);

    expect(await screen.findByText("Desktop app required.")).toBeTruthy();
    expect(screen.getByText(/browser preview of the Nexora interface/i)).toBeTruthy();
    expect(screen.queryByText("Engine ready")).toBeNull();
    expect(
      (screen.getByRole("button", { name: "Send message" }) as HTMLButtonElement)
        .disabled,
    ).toBe(true);
  });

  it("still renders the real shell", async () => {
    statusMock.mockResolvedValue(previewStatus());

    render(<App />);
    await screen.findByText("Desktop app required.");

    expect(screen.getAllByText(/Nexora/).length).toBeGreaterThan(0);
    expect(screen.getByRole("button", { name: "New chat" })).toBeTruthy();
    expect(screen.getByRole("textbox")).toBeTruthy();
  });
});

describe("desktop status routing", () => {
  it("shows setup when no model is configured", async () => {
    markDesktop();
    statusMock.mockResolvedValue(makeStatus({ state: "not_configured" }));

    render(<App />);

    expect(await screen.findByText("Bring your own model.")).toBeTruthy();
    expect(
      screen.getByRole("textbox", { name: "Runtime executable" }),
    ).toBeTruthy();
    expect(
      screen.getByRole("textbox", { name: "Model file (.gguf)" }),
    ).toBeTruthy();
    expect(screen.queryByText("Build something real.")).toBeNull();
  });

  it("shows the ready conversation surface when a model is loaded", async () => {
    markDesktop();
    statusMock.mockResolvedValue(makeStatus({ state: "ready" }));

    render(<App />);

    expect(await screen.findByText("Engine ready")).toBeTruthy();
    expect(screen.getByText("Build something real.")).toBeTruthy();
    expect(screen.queryByText("Bring your own model.")).toBeNull();
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
    expect(screen.queryByText("Build something real.")).toBeNull();
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
    expect(await screen.findByText("Engine state unavailable.")).toBeTruthy();
    expect(screen.queryByText("Bring your own model.")).toBeNull();
    expect(screen.queryByText("Build something real.")).toBeNull();
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

    const retry = await screen.findByRole("button", { name: "Retry" });
    await userEvent.setup().click(retry);

    expect(await screen.findByText("Engine ready")).toBeTruthy();
    expect(statusMock.mock.calls.length).toBeGreaterThanOrEqual(2);
  });
});

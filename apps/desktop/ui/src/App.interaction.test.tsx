import { act, render, screen, waitFor } from "@testing-library/react";
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
import {
  cancelGeneration,
  fetchSettings,
  fetchStatus,
  fetchSystemInfo,
  generate,
  unloadModel,
} from "./lib/inference";
import type { GenerationEvent, GenerationOutcome } from "./lib/ipc";
import { makeStatus, markDesktop } from "./test/fixtures";

const statusMock = vi.mocked(fetchStatus);
const settingsMock = vi.mocked(fetchSettings);
const systemMock = vi.mocked(fetchSystemInfo);
const generateMock = vi.mocked(generate);
const cancelMock = vi.mocked(cancelGeneration);
const unloadMock = vi.mocked(unloadModel);

/**
 * These are UI integration tests: the single IPC boundary (`lib/inference`) is
 * replaced with a scripted double. They prove the React layer wires streaming
 * text, cancellation, and errors correctly — never that a real model ran.
 */
function streamHarness() {
  let emit: (event: GenerationEvent) => void = () => undefined;
  let requestId = "";
  let resolve!: (outcome: GenerationOutcome) => void;
  let reject!: (cause: unknown) => void;

  const promise = new Promise<GenerationOutcome>((res, rej) => {
    resolve = res;
    reject = rej;
  });

  generateMock.mockImplementation((request, onEvent) => {
    emit = onEvent;
    requestId = request.requestId;
    return promise;
  });

  return {
    push: (text: string) => emit({ type: "delta", requestId, text }),
    settle: (overrides: Partial<GenerationOutcome> = {}) =>
      resolve({ requestId, finishReason: "stop", cancelled: false, ...overrides }),
    fail: (cause: unknown) => reject(cause),
    requestId: () => requestId,
  };
}

beforeEach(() => {
  markDesktop();
  statusMock.mockReset();
  statusMock.mockResolvedValue(makeStatus({ state: "ready" }));
  settingsMock.mockClear();
  systemMock.mockClear();
  systemMock.mockResolvedValue({
    totalRamGib: 32,
    availableRamGib: 20.5,
    platform: "windows",
  });
  generateMock.mockReset();
  cancelMock.mockReset();
  cancelMock.mockResolvedValue(true);
  unloadMock.mockReset();
  unloadMock.mockResolvedValue(makeStatus({ state: "stopped" }));
});

afterEach(() => {
  markDesktop(false);
});

function compose() {
  return screen.getByRole("textbox", { name: "Message Nexora" });
}

describe("conversation surface", () => {
  it("streams model text into the transcript as it arrives", async () => {
    const run = streamHarness();
    const user = userEvent.setup();

    render(<App />);
    await screen.findByText("Engine ready");

    await user.type(compose(), "write a hello world{Enter}");

    expect(await screen.findByText("write a hello world")).toBeTruthy();
    expect(screen.getByText("Waiting for the first token…")).toBeTruthy();

    run.push("Here is a minimal ");

    expect(await screen.findByText(/Here is a minimal/)).toBeTruthy();

    run.push("program.");

    expect(await screen.findByText("Here is a minimal program.")).toBeTruthy();
    expect(screen.queryByText("Waiting for the first token…")).toBeNull();
  });

  it("shows the reply as complete once the request settles", async () => {
    const run = streamHarness();
    const user = userEvent.setup();

    render(<App />);
    await screen.findByText("Engine ready");
    await user.type(compose(), "hello{Enter}");
    run.push("Hi.");

    await screen.findByText("Hi.");
    expect(screen.getByText("Generating")).toBeTruthy();

    await act(async () => {
      run.settle();
      await Promise.resolve();
    });

    await waitFor(() => {
      expect(screen.queryByText("Generating")).toBeNull();
    });
  });

  it("stops generation from the composer and labels the partial reply", async () => {
    const run = streamHarness();
    const user = userEvent.setup();

    render(<App />);
    await screen.findByText("Engine ready");
    await user.type(compose(), "explain closures{Enter}");
    run.push("A closure captures");

    await screen.findByText(/A closure captures/);

    await user.click(screen.getByRole("button", { name: "Stop generating" }));

    expect(cancelMock).toHaveBeenCalledWith(run.requestId());

    await act(async () => {
      run.settle({ cancelled: true, finishReason: "stopped" });
      await Promise.resolve();
    });

    expect(await screen.findByText("Stopped")).toBeTruthy();
    expect(screen.getByText(/A closure captures/)).toBeTruthy();
  });

  it("reports a runtime failure honestly and keeps no invented answer", async () => {
    generateMock.mockRejectedValue({
      code: "not_running",
      message: "The local engine is not running. Load a model first.",
    });

    const user = userEvent.setup();
    render(<App />);
    await screen.findByText("Engine ready");

    await user.type(compose(), "hello{Enter}");

    const banner = await screen.findByText(
      "The local engine is not running. Load a model first.",
    );

    expect(banner).toBeTruthy();
    expect(screen.getByText("hello")).toBeTruthy();
    expect(screen.queryByText("Failed")).toBeNull();
    expect(screen.queryByText("Waiting for the first token…")).toBeNull();
  });

  it("marks a broken stream and keeps the text that did arrive", async () => {
    const run = streamHarness();
    const user = userEvent.setup();

    render(<App />);
    await screen.findByText("Engine ready");
    await user.type(compose(), "hello{Enter}");
    run.push("partial answer");

    await screen.findByText("partial answer");

    await act(async () => {
      run.fail({ code: "timeout", message: "The model stopped responding." });
      await Promise.resolve();
    });

    expect(await screen.findByText("Failed")).toBeTruthy();
    expect(screen.getByText("partial answer")).toBeTruthy();
    expect(
      screen.getAllByText("The model stopped responding.").length,
    ).toBeGreaterThan(0);
  });

  it("clears the transcript and cancels the request on a new chat", async () => {
    const run = streamHarness();
    const user = userEvent.setup();

    render(<App />);
    await screen.findByText("Engine ready");
    await user.type(compose(), "temporary question{Enter}");
    run.push("partial");

    await screen.findByText("partial");

    await user.click(screen.getByRole("button", { name: "New chat" }));

    expect(cancelMock).toHaveBeenCalledWith(run.requestId());
    expect(screen.queryByText("temporary question")).toBeNull();
    expect(screen.queryByText("partial")).toBeNull();
    expect(screen.getByText("Build something real.")).toBeTruthy();
  });

  it("refuses an empty message with an accessible error", async () => {
    const user = userEvent.setup();

    render(<App />);
    await screen.findByText("Engine ready");

    await user.type(compose(), "{Enter}");

    expect(screen.getByRole("alert").textContent).toContain(
      "Enter a message before sending.",
    );
    expect(generateMock).not.toHaveBeenCalled();
  });

  it("stops the engine on request and reports the real resulting state", async () => {
    statusMock.mockReset();
    statusMock
      .mockResolvedValueOnce(makeStatus({ state: "ready" }))
      .mockResolvedValue(makeStatus({ state: "stopped" }));

    const user = userEvent.setup();
    render(<App />);

    await screen.findByText("Engine ready");
    await user.click(screen.getByRole("button", { name: "Stop engine" }));

    expect(unloadMock).toHaveBeenCalledTimes(1);

    await waitFor(() => {
      expect(screen.queryByText("Engine ready")).toBeNull();
    });

    expect(screen.getByText("Stopped")).toBeTruthy();
    expect(screen.getByText("Bring your own model.")).toBeTruthy();
    expect(
      (screen.getByRole("button", { name: "Send message" }) as HTMLButtonElement)
        .disabled,
    ).toBe(true);
  });

  it("reports a failed engine stop instead of pretending it worked", async () => {
    unloadMock.mockRejectedValue({
      code: "internal",
      message: "The engine state is unavailable. Restart Nexora.",
    });

    const user = userEvent.setup();
    render(<App />);

    await screen.findByText("Engine ready");
    await user.click(screen.getByRole("button", { name: "Stop engine" }));

    expect(
      await screen.findByText("The engine state is unavailable. Restart Nexora."),
    ).toBeTruthy();
  });

  it("collapses and expands the sidebar from the title bar", async () => {
    const user = userEvent.setup();

    render(<App />);
    await screen.findByText("Engine ready");

    await user.click(screen.getByRole("button", { name: "Collapse sidebar" }));
    expect(
      screen.getByRole("button", { name: "Expand sidebar" }).getAttribute(
        "aria-expanded",
      ),
    ).toBe("false");

    await user.click(screen.getByRole("button", { name: "Expand sidebar" }));
    expect(
      screen.getByRole("button", { name: "Collapse sidebar" }).getAttribute(
        "aria-expanded",
      ),
    ).toBe("true");
  });

  it("fills the composer from an example prompt", async () => {
    const user = userEvent.setup();

    render(<App />);
    await screen.findByText("Engine ready");

    await user.click(screen.getByText("Build a landing page"));

    expect((compose() as HTMLTextAreaElement).value).toContain("landing page");
    expect(generateMock).not.toHaveBeenCalled();
  });
});

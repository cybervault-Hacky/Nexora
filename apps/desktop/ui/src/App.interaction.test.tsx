import { act, render, screen, waitFor, within } from "@testing-library/react";
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

    // The prompt appears both as the conversation title and as the message bubble;
    // assert the bubble specifically so a title change does not make the query ambiguous.
    expect((await screen.findAllByText("write a hello world")).length).toBeGreaterThanOrEqual(1);
    expect(document.querySelector(".message__prompt")?.textContent).toBe("write a hello world");
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
    expect(within(screen.getByRole("log")).getByText(/A closure captures/)).toBeTruthy();
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
    expect(screen.getAllByText("hello").length).toBeGreaterThanOrEqual(1);
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
    expect(within(screen.getByRole("log")).getByText("partial answer")).toBeTruthy();
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
    // Transcript is cleared, but history retains the conversation title
    expect(document.querySelector(".message__prompt")).toBeNull();
    expect(screen.queryByRole("log")).toBeNull();
    expect(screen.getByText("Build something real.")).toBeTruthy();
    // History should still show the previous conversation
    expect(screen.getByText("temporary question")).toBeTruthy();
  });

  it("refuses an empty message with an accessible error", async () => {
    const user = userEvent.setup();

    render(<App />);
    await screen.findByText("Engine ready");

    await user.type(compose(), "{Enter}");

    expect(screen.getByText("Enter a message before sending.")).toBeTruthy();
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

  it("renders a streamed code block with its language", async () => {
    const run = streamHarness();

    render(<App />);

    await userEvent.type(compose(), "write a function{Enter}");

    act(() => run.push("Here is one:\n\n```ts\nconst add = (a: number) => a;\n```\n"));

    // The fence closes as the final chunk arrives, so the block is complete.
    expect(await screen.findByText("ts")).toBeDefined();
    expect(screen.getByRole("code").textContent).toBe("const add = (a: number) => a;");
    expect(screen.getByRole("button", { name: "Copy ts code" })).toBeDefined();

    await act(async () => {
      run.settle();
      await Promise.resolve();
    });

    // Completion must not discard the code that already arrived.
    expect(screen.getByRole("code").textContent).toBe("const add = (a: number) => a;");
    expect(screen.queryByText(/streaming/)).toBeNull();
  });

  it("shows a half-streamed fence as code while tokens are still arriving", async () => {
    const run = streamHarness();

    render(<App />);

    await userEvent.type(compose(), "show me{Enter}");
    act(() => run.push("```py\nprint("));

    const code = await screen.findByRole("code");

    expect(code.textContent).toBe("print(");
    expect(screen.getByText(/streaming/)).toBeDefined();
    // Both the message chip and the engine strip report the busy state.
    expect(screen.getAllByText(/Generating/).length).toBeGreaterThan(0);
  });

  it("renders inline code without turning the rest into code", async () => {
    const run = streamHarness();

    render(<App />);

    await userEvent.type(compose(), "how do I test{Enter}");
    act(() => run.push("Run `pnpm test` to see results."));

    const inline = screen.getByText("pnpm test");

    expect(inline.tagName).toBe("CODE");
    expect(screen.getByText(/to see results/)).toBeDefined();
  });

  it("displays model markup literally instead of executing it", async () => {
    const run = streamHarness();
    const payload = '<img src=x onerror="window.__pwned=1">';

    render(<App />);

    await userEvent.type(compose(), "html please{Enter}");
    act(() => run.push(payload));

    const { container } = render(<div />);

    expect(container.querySelector("img")).toBeNull();
    expect(screen.getByText(payload)).toBeDefined();
    expect((window as unknown as { __pwned?: number }).__pwned).toBeUndefined();

    await act(async () => {
      run.settle();
      await Promise.resolve();
    });
  });

  it("copies fenced code straight from the transcript", async () => {
    const run = streamHarness();
    const writeText = vi.fn().mockResolvedValue(undefined);

    Object.defineProperty(navigator, "clipboard", {
      value: { writeText },
      configurable: true,
    });

    render(<App />);

    await userEvent.type(compose(), "give me json{Enter}");
    act(() => run.push('```json\n{"ok":true}\n```'));

    await userEvent.click(await screen.findByRole("button", { name: "Copy json code" }));

    expect(writeText).toHaveBeenCalledWith('{"ok":true}');
    expect(await screen.findByText("Copied")).toBeDefined();

    Reflect.deleteProperty(navigator, "clipboard");

    await act(async () => {
      run.settle();
      await Promise.resolve();
    });
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

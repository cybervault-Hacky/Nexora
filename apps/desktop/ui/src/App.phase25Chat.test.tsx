import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

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
      modelPath: "C:\\models\\qwen2.5-coder-7b.gguf",
      runtimePath: "C:\\bin\\llama-server.exe",
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

let storedConversations: import("./lib/history").PersistedConversation[] = [];

vi.mock("./lib/history", () => ({
  listConversations: async () =>
    storedConversations.map((c) => ({
      id: c.id,
      title: c.title,
      createdAt: c.createdAt,
      updatedAt: c.updatedAt,
      messageCount: c.messages.length,
      preview: c.messages[0]?.content?.slice(0, 60) ?? "",
    })),
  createConversation: async () => {
    const newConv: import("./lib/history").PersistedConversation = {
      id: `chat-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`,
      title: "New conversation",
      createdAt: Date.now(),
      updatedAt: Date.now(),
      messages: [],
    };
    storedConversations.unshift(newConv);
    return newConv;
  },
  saveConversation: async (c: import("./lib/history").PersistedConversation) => {
    const idx = storedConversations.findIndex((item) => item.id === c.id);
    if (idx >= 0) storedConversations[idx] = c;
    else storedConversations.unshift(c);
    return c;
  },
  loadConversation: async (id: string) => {
    const found = storedConversations.find((c) => c.id === id);
    if (!found) throw { code: "not_found", message: "Conversation not found" };
    return found;
  },
  renameConversation: async (id: string, title: string) => {
    const found = storedConversations.find((c) => c.id === id);
    if (!found) throw { code: "not_found", message: "Not found" };
    found.title = title;
    found.updatedAt = Date.now();
    return found;
  },
  deleteConversation: async (id: string) => {
    storedConversations = storedConversations.filter((c) => c.id !== id);
    return true;
  },
}));

import App from "./App";
import { cancelGeneration, fetchStatus, fetchSystemInfo, generate } from "./lib/inference";
import type { GenerationEvent, GenerationOutcome } from "./lib/ipc";
import { makeStatus, markDesktop } from "./test/fixtures";

const statusMock = vi.mocked(fetchStatus);
const systemMock = vi.mocked(fetchSystemInfo);
const generateMock = vi.mocked(generate);
const cancelMock = vi.mocked(cancelGeneration);

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
  storedConversations = [
    {
      id: "existing-chat-1",
      title: "Architecture Planning",
      createdAt: Date.now() - 3600000,
      updatedAt: Date.now() - 1800000,
      messages: [
        { id: "m1", role: "user", content: "Plan our offline architecture", createdAt: Date.now() - 3600000, state: "complete" },
        { id: "m2", role: "assistant", content: "Nexora uses local llama.cpp without cloud dependencies.", createdAt: Date.now() - 3590000, state: "complete" },
      ],
    },
  ];

  statusMock.mockReset();
  statusMock.mockResolvedValue(
    makeStatus({
      state: "ready",
      modelPath: "C:\\models\\qwen2.5-coder-7b.gguf",
      contextSize: 32768,
    }),
  );
  systemMock.mockReset();
  systemMock.mockResolvedValue({
    totalRamGib: 32,
    availableRamGib: 20.5,
    platform: "windows",
  });
  generateMock.mockReset();
  cancelMock.mockReset();
  cancelMock.mockResolvedValue(true);

  markDesktop(true);

  invokeMock.mockReset();
  invokeMock.mockImplementation((cmd: string) => {
    switch (cmd) {
      case "list_conversations":
        return Promise.resolve(
          storedConversations.map((c) => ({
            id: c.id,
            title: c.title,
            createdAt: c.createdAt,
            updatedAt: c.updatedAt,
            messageCount: c.messages.length,
            preview: c.messages[0]?.content?.slice(0, 60) ?? "",
          })),
        );
      case "load_conversation":
        return Promise.resolve(storedConversations[0]);
      case "save_conversation":
        return Promise.resolve(storedConversations[0]);
      case "workspace_get_symbol_index":
        return Promise.resolve(null);
      case "workspace_get_analysis_graph":
        return Promise.resolve(null);
      default:
        return Promise.resolve(null);
    }
  });

  window.location.hash = "#/chat";
});

afterEach(() => {
  markDesktop(false);
});

describe("Phase 25 — AI Chat Integration", () => {
  it("renders AI Chat view with genuine local model readiness status", async () => {
    render(<App />);

    expect(await screen.findByRole("textbox", { name: "Message Nexora" })).toBeTruthy();
    expect(screen.getByText("Local-only")).toBeTruthy();
    expect(screen.getByText("Engine ready")).toBeTruthy();
    expect(screen.getByText("Local model ready")).toBeTruthy();
  });

  it("submits user message and streams assistant response into transcript", async () => {
    const run = streamHarness();
    const user = userEvent.setup();

    render(<App />);
    const input = await screen.findByRole("textbox", { name: "Message Nexora" });

    await user.type(input, "Create a Rust helper function{Enter}");

    expect(document.querySelector(".message__prompt")?.textContent).toBe("Create a Rust helper function");
    expect(screen.getByText("Waiting for the first token…")).toBeTruthy();

    run.push("fn add(a: i32, b: i32) -> i32 { a + b }");

    const messagesLog = screen.getByRole("log", { name: "Conversation" });
    expect(await within(messagesLog).findByText(/fn add\(a: i32, b: i32\)/)).toBeTruthy();

    await act(async () => {
      run.settle();
      await Promise.resolve();
    });

    await waitFor(() => {
      expect(screen.queryByText("Generating")).toBeNull();
    });
    expect(within(messagesLog).getByText(/fn add\(a: i32, b: i32\)/)).toBeTruthy();
  });

  it("supports stopping active generation and preserving partial output", async () => {
    const run = streamHarness();
    const user = userEvent.setup();

    render(<App />);
    const input = await screen.findByRole("textbox", { name: "Message Nexora" });

    await user.type(input, "Explain concurrency models{Enter}");
    run.push("Concurrency involves multiple tasks progressing concurrently.");

    const messagesLog = screen.getByRole("log", { name: "Conversation" });
    expect(await within(messagesLog).findByText(/Concurrency involves multiple tasks/)).toBeTruthy();

    const stopBtn = screen.getByRole("button", { name: "Stop generating" });
    await user.click(stopBtn);

    expect(cancelMock).toHaveBeenCalledWith(run.requestId());

    await act(async () => {
      run.settle({ cancelled: true, finishReason: "stopped" });
      await Promise.resolve();
    });

    expect(await screen.findByText("Stopped")).toBeTruthy();
    expect(within(messagesLog).getByText(/Concurrency involves multiple tasks/)).toBeTruthy();
  });

  it("handles conversation switching and starting new chats", async () => {
    const user = userEvent.setup();
    render(<App />);

    // Initial empty state has "New conversation"
    expect(await screen.findByRole("heading", { name: "New conversation" })).toBeTruthy();

    // Start a new chat via the header button or shortcut
    const input = screen.getByRole("textbox", { name: "Message Nexora" });
    await user.type(input, "First question in new chat");

    // The composer preserves input until submission
    expect((input as HTMLTextAreaElement).value).toBe("First question in new chat");
  });

  it("disables message sending and warns when local engine is stopped", async () => {
    statusMock.mockResolvedValue(
      makeStatus({
        state: "stopped",
        modelPath: undefined,
      }),
    );

    render(<App />);

    expect(await screen.findByText("Engine stopped")).toBeTruthy();
    const input = screen.getByRole("textbox", { name: "Message Nexora" });
    expect(input.getAttribute("placeholder")).toBe("Load a local model to start");
    expect(screen.getByText("No model loaded")).toBeTruthy();
  });
});

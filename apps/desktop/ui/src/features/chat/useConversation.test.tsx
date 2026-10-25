import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { GenerationEvent, GenerationOutcome } from "../../lib/ipc";

vi.mock("../../lib/inference", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../lib/inference")>();

  return {
    ...actual,
    generate: vi.fn(),
    cancelGeneration: vi.fn().mockResolvedValue(true),
  };
});

import { cancelGeneration, generate } from "../../lib/inference";
import { useConversation as useConversationImpl } from "./useConversation";

const MINI_ID = "nexora-mini-qwen2.5-coder-7b-instruct-q4-k-m";
const useConversation = (options?: Parameters<typeof useConversationImpl>[0]) =>
  useConversationImpl({
    ...options,
    profileId: options?.profileId === undefined ? MINI_ID : options.profileId,
  });

const generateMock = vi.mocked(generate);
const cancelMock = vi.mocked(cancelGeneration);

/**
 * Drives one generation: captures the channel callback, and lets the test
 * resolve or reject the request when it chooses.
 */
function session() {
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
    requestId: () => requestId,
    raw: (event: GenerationEvent) => emit(event),
    send: (text: string) =>
      emit({ type: "delta", requestId, text }),
    settle: (overrides: Partial<GenerationOutcome> = {}) =>
      resolve({ requestId, finishReason: "stop", cancelled: false, usageWarnings: [], ...overrides }),
    fail: (cause: unknown) => reject(cause),
  };
}

beforeEach(() => {
  generateMock.mockReset();
  cancelMock.mockReset();
  cancelMock.mockResolvedValue(true);
});

describe("useConversation", () => {
  it("starts empty and idle", () => {
    const { result } = renderHook(() => useConversation());

    expect(result.current.messages).toHaveLength(0);
    expect(result.current.streaming).toBe(false);
  });

  it("rejects an empty draft without calling the engine", () => {
    const { result } = renderHook(() => useConversation());

    act(() => result.current.setDraft("   "));
    act(() => result.current.submit());

    expect(result.current.messages).toHaveLength(0);
    expect(result.current.error?.code).toBe("empty");
    expect(generateMock).not.toHaveBeenCalled();
  });

  it("refuses to submit when no approved profile is available", () => {
    const { result } = renderHook(() => useConversation({ profileId: null }));

    act(() => result.current.setDraft("hello"));
    act(() => result.current.submit());

    expect(generateMock).not.toHaveBeenCalled();
    expect(result.current.messages).toHaveLength(0);
    expect(result.current.runtimeError).toBe(
      "Select an approved local model before sending.",
    );
  });

  it("adds the user turn and streams text into a single reply", async () => {
    const run = session();
    const { result } = renderHook(() => useConversation());

    act(() => result.current.setDraft("hello"));
    act(() => result.current.submit());

    expect(result.current.messages).toHaveLength(2);
    expect(result.current.messages[0]).toMatchObject({
      role: "user",
      content: "hello",
    });
    expect(result.current.messages[1]).toMatchObject({
      role: "assistant",
      state: "streaming",
      content: "",
    });

    act(() => {
      run.send("Hi ");
      run.send("there");
    });

    expect(result.current.messages[1]?.content).toBe("Hi there");
  });

  it("names the selected profile on the request", async () => {
    const run = session();
    const { result } = renderHook(() =>
      useConversation({ profileId: "nexora-sonnet-qwen2.5-coder-14b-instruct-q4-k-m" }),
    );

    act(() => result.current.setDraft("hello"));
    act(() => result.current.submit());

    const call = generateMock.mock.calls[0];
    expect(call?.[0].profileId).toBe(
      "nexora-sonnet-qwen2.5-coder-14b-instruct-q4-k-m",
    );

    await act(async () => run.settle());
  });

  it("sends the visible conversation as engine turns", () => {
    session();
    const { result } = renderHook(() => useConversation());

    act(() => result.current.setDraft("first"));
    act(() => result.current.submit());

    expect(generateMock.mock.calls[0]?.[0].messages).toEqual([
      { role: "user", content: "first" },
    ]);
  });

  it("marks the reply complete when generation settles", async () => {
    const run = session();
    const { result } = renderHook(() => useConversation());

    act(() => result.current.setDraft("hello"));
    act(() => result.current.submit());
    act(() => run.send("answer"));

    await act(async () => {
      run.settle();
    });

    await waitFor(() => {
      expect(result.current.messages[1]?.state).toBe("complete");
      expect(result.current.streaming).toBe(false);
    });
  });

  it("stops generation and keeps the partial text", async () => {
    const run = session();
    const { result } = renderHook(() => useConversation());

    act(() => result.current.setDraft("hello"));
    act(() => result.current.submit());
    act(() => run.send("partial"));

    act(() => result.current.stop());

    expect(cancelMock).toHaveBeenCalledWith(run.requestId());

    await act(async () => {
      run.settle({ cancelled: true, finishReason: "stopped" });
    });

    expect(result.current.messages[1]?.state).toBe("stopped");
    expect(result.current.messages[1]?.content).toBe("partial");
  });

  it("keeps a stopped reply empty instead of inventing assistant text", async () => {
    const run = session();
    const { result } = renderHook(() => useConversation());

    act(() => result.current.setDraft("hello"));
    act(() => result.current.submit());
    act(() => result.current.stop());

    await act(async () => {
      run.settle({ cancelled: true, finishReason: "stopped" });
      await Promise.resolve();
    });

    expect(result.current.messages[1]?.state).toBe("stopped");
    expect(result.current.messages[1]?.content).toBe("");
  });

  it("reports a runtime failure and removes the empty reply", async () => {
    generateMock.mockRejectedValue({
      code: "not_running",
      message: "Load a local model before sending a message.",
    });

    const { result } = renderHook(() => useConversation());

    act(() => result.current.setDraft("hello"));
    act(() => result.current.submit());

    await waitFor(() => {
      expect(result.current.runtimeError).toBe(
        "Load a local model before sending a message.",
      );
    });

    expect(result.current.messages).toHaveLength(1);
    expect(result.current.streaming).toBe(false);
  });

  it("keeps partial text and marks it failed when the stream breaks", async () => {
    const run = session();
    const { result } = renderHook(() => useConversation());

    act(() => result.current.setDraft("hello"));
    act(() => result.current.submit());
    act(() => run.send("half of an answer"));

    await act(async () => {
      run.fail({ code: "timeout", message: "The model stopped responding." });
    });

    await waitFor(() => {
      expect(result.current.messages[1]?.state).toBe("error");
    });

    expect(result.current.messages[1]?.content).toBe("half of an answer");
    expect(result.current.messages[1]?.failure).toContain("stopped responding");
  });

  it("refuses a second submit while a reply is streaming", () => {
    session();
    const { result } = renderHook(() => useConversation());

    act(() => result.current.setDraft("one"));
    act(() => result.current.submit());
    act(() => result.current.setDraft("two"));
    act(() => result.current.submit());

    expect(result.current.error?.code).toBe("busy");
    expect(generateMock).toHaveBeenCalledTimes(1);
    expect(result.current.messages).toHaveLength(2);
  });

  it("drops deltas and results that belong to a previous chat", async () => {
    const run = session();
    const { result } = renderHook(() => useConversation());

    act(() => result.current.setDraft("hello"));
    act(() => result.current.submit());

    const staleId = run.requestId();

    act(() => result.current.newChat());
    act(() => run.send("stale"));

    await act(async () => {
      run.settle();
    });

    expect(result.current.messages).toHaveLength(0);
    expect(result.current.streaming).toBe(false);
    expect(staleId.length).toBeGreaterThan(0);
  });

  it("cancels the active request when a new chat starts", () => {
    const run = session();
    const { result } = renderHook(() => useConversation());

    act(() => result.current.setDraft("hello"));
    act(() => result.current.submit());
    act(() => result.current.newChat());

    expect(cancelMock).toHaveBeenCalledWith(run.requestId());
  });

  it("ignores malformed and unknown channel events", () => {
    const run = session();
    const { result } = renderHook(() => useConversation());

    act(() => result.current.setDraft("hello"));
    act(() => result.current.submit());
    act(() => run.send("kept"));

    expect(() => {
      // A delta without text, an unknown variant, and an empty payload must
      // not append "undefined" or end the stream.
      act(() => run.raw({ type: "delta", requestId: run.requestId() } as never));
      act(() => run.raw({ type: "finished" } as never));
      act(() => run.raw({} as never));
    }).not.toThrow();

    expect(result.current.messages[1]?.content).toBe("kept");
    expect(result.current.streaming).toBe(true);
    expect(result.current.runtimeError).toBeNull();
  });

  it("drops a delta that carries another request id", () => {
    const run = session();
    const { result } = renderHook(() => useConversation());

    act(() => result.current.setDraft("hello"));
    act(() => result.current.submit());
    act(() => run.send("mine"));

    act(() =>
      run.raw({ type: "delta", requestId: "someone-else", text: " not mine" }),
    );

    expect(result.current.messages[1]?.content).toBe("mine");
  });

  it("does not ask the core to cancel when nothing is running", () => {
    const { result } = renderHook(() => useConversation());

    expect(() => act(() => result.current.stop())).not.toThrow();
    expect(cancelMock).not.toHaveBeenCalled();
  });

  it("accepts the next prompt after a stop", async () => {
    const first = session();
    const { result } = renderHook(() => useConversation());
    const clock = vi.spyOn(Date, "now");
    const started = Date.now();

    act(() => result.current.setDraft("first"));
    act(() => result.current.submit());
    act(() => first.send("partial"));
    act(() => result.current.stop());

    await act(async () => {
      first.settle({ cancelled: true, finishReason: "stopped" });
      await Promise.resolve();
    });

    expect(result.current.streaming).toBe(false);

    // Past the duplicate-submission guard, as a person typing would be.
    clock.mockReturnValue(started + 1_000);

    const second = session();

    act(() => result.current.setDraft("second"));
    act(() => result.current.submit());

    expect(generateMock).toHaveBeenCalledTimes(2);
    expect(result.current.streaming).toBe(true);

    await act(async () => {
      second.send("fresh answer");
      second.settle();
      await Promise.resolve();
    });

    expect(result.current.messages).toHaveLength(4);
    expect(result.current.messages[2]?.content).toBe("second");
    expect(result.current.messages[3]?.content).toBe("fresh answer");
    expect(result.current.messages[3]?.state).toBe("complete");
  });

  it("accepts a retry after a runtime failure", async () => {
    const retry = session();
    const { result } = renderHook(() => useConversation());
    const clock = vi.spyOn(Date, "now");
    const started = Date.now();

    generateMock.mockRejectedValueOnce({
      code: "not_running",
      message: "Load a local model before sending a message.",
    });

    act(() => result.current.setDraft("first"));
    act(() => result.current.submit());

    await waitFor(() => {
      expect(result.current.runtimeError).toContain("Load a local model");
    });

    expect(result.current.streaming).toBe(false);
    expect(result.current.messages).toHaveLength(1);

    clock.mockReturnValue(started + 1_000);

    act(() => result.current.setDraft("second"));
    act(() => result.current.submit());

    expect(generateMock).toHaveBeenCalledTimes(2);
    expect(result.current.streaming).toBe(true);
    expect(result.current.runtimeError).toBeNull();

    await act(async () => {
      retry.settle();
      await Promise.resolve();
    });

    expect(result.current.messages).toHaveLength(2);
    expect(result.current.messages[1]?.state).toBe("complete");
  });

  it("does not call back into a tree that has unmounted", async () => {
    const onSettled = vi.fn();
    const run = session();
    const { result, unmount } = renderHook(() => useConversation({ onSettled }));

    act(() => result.current.setDraft("hello"));
    act(() => result.current.submit());
    act(() => run.send("partial"));

    unmount();

    await act(async () => {
      run.settle();
      await Promise.resolve();
    });

    // `onSettled` triggers a status refresh; firing it after unmount would
    // spend an IPC round trip on a component that no longer exists.
    expect(onSettled).not.toHaveBeenCalled();
  });

  it("drops a late failure after unmount", async () => {
    const onSettled = vi.fn();
    const run = session();
    const { result, unmount } = renderHook(() => useConversation({ onSettled }));

    act(() => result.current.setDraft("hello"));
    act(() => result.current.submit());
    unmount();

    await act(async () => {
      run.fail({ code: "network", message: "The connection failed." });
      await Promise.resolve();
    });

    expect(onSettled).not.toHaveBeenCalled();
  });

  it("clears messages, draft, and errors on a new chat", () => {
    session();
    const { result } = renderHook(() => useConversation());

    act(() => result.current.setDraft("hello"));
    act(() => result.current.submit());
    act(() => result.current.setDraft("typing"));
    act(() => result.current.newChat());

    expect(result.current.messages).toHaveLength(0);
    expect(result.current.draft).toBe("");
    expect(result.current.error).toBeNull();
    expect(result.current.runtimeError).toBeNull();
  });

  it("drops late deltas after stop() and retries without duplicating the user turn", async () => {
    const first = session();
    const { result } = renderHook(() => useConversation());

    act(() => result.current.setDraft("Explain borrow checker"));
    act(() => result.current.submit());
    act(() => first.send("partial"));
    act(() => result.current.stop());
    // Late delta arriving after stop() must be dropped immediately.
    act(() => first.send(" LATE-GARBAGE"));
    await act(async () => {
      first.settle({ cancelled: false, finishReason: "stop" });
    });

    expect(result.current.messages[1]?.content).toBe("partial");
    expect(result.current.messages[1]?.state).toBe("stopped");
    expect(result.current.canRetry).toBe(true);

    // Retry reuses the existing user turn without duplicating it or mixing partial text!
    const second = session();
    act(() => result.current.retry());
    expect(result.current.messages).toHaveLength(2);
    expect(result.current.messages[0]?.content).toBe("Explain borrow checker");
    expect(result.current.messages[1]?.content).toBe("");
    act(() => second.send("Complete fresh explanation."));
    await act(async () => {
      second.settle();
    });

    expect(result.current.messages).toHaveLength(2);
    expect(result.current.messages[0]?.content).toBe("Explain borrow checker");
    expect(result.current.messages[1]?.content).toBe("Complete fresh explanation.");
    expect(result.current.messages[1]?.state).toBe("complete");
    expect(result.current.canRetry).toBe(false);
  });
});

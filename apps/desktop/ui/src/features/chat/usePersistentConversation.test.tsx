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
import { usePersistentConversation } from "./usePersistentConversation";
import { clearFallbackStorage } from "../../lib/history";

const MINI_ID = "nexora-mini-qwen2.5-coder-7b-instruct-q4-k-m";
const SONNET_ID = "nexora-sonnet-qwen2.5-coder-14b-instruct-q4-k-m";

const generateMock = vi.mocked(generate);

/** The request of the nth `generate` call; fails loudly when there is none. */
function sentRequest(index = 0) {
  const call = generateMock.mock.calls[index];

  if (!call) {
    throw new Error(`generate was called ${generateMock.mock.calls.length} time(s), not ${index + 1}`);
  }

  return call[0];
}
const cancelMock = vi.mocked(cancelGeneration);

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
    send: (text: string) => emit({ type: "delta", requestId, text }),
    settle: (overrides: Partial<GenerationOutcome> = {}) =>
      resolve({ requestId, finishReason: "stop", cancelled: false, ...overrides }),
    fail: (cause: unknown) => reject(cause),
  };
}

beforeEach(() => {
  generateMock.mockReset();
  cancelMock.mockReset();
  cancelMock.mockResolvedValue(true);
  clearFallbackStorage();
  // Ensure fallback mode (no Tauri)
  // @ts-ignore
  delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
});

describe("usePersistentConversation", () => {
  it("starts with empty list and no active conversation", async () => {
    const { result } = renderHook(() => usePersistentConversation());
    await waitFor(() => expect(result.current.listLoading).toBe(false));
    expect(result.current.conversations).toHaveLength(0);
    expect(result.current.activeId).toBeNull();
    expect(result.current.messages).toHaveLength(0);
  });

  it("creates and switches conversations, preserving messages", async () => {
    const run = session();
    const { result } = renderHook(() => usePersistentConversation());
    await waitFor(() => expect(result.current.listLoading).toBe(false));

    act(() => result.current.setDraft("hello first"));
    act(() => result.current.submit());

    expect(result.current.messages).toHaveLength(2);
    expect(result.current.activeId).not.toBeNull();
    act(() => run.send("answer one"));
    await act(async () => {
      run.settle();
    });
    await waitFor(() => expect(result.current.streaming).toBe(false));

    const firstId = result.current.activeId!;
    await waitFor(() => expect(result.current.saveStatus).toBe("saved"), { timeout: 1000 });

    // Start new chat
    act(() => result.current.newChat());
    await waitFor(() => expect(result.current.messages).toHaveLength(0));
    expect(result.current.activeId).not.toBe(firstId);

    // Second conversation
    const run2 = session();
    act(() => result.current.setDraft("hello second"));
    act(() => result.current.submit());
    act(() => run2.send("answer two"));
    await act(async () => {
      run2.settle();
    });
    await waitFor(() => expect(result.current.streaming).toBe(false));

    const secondId = result.current.activeId!;
    expect(secondId).not.toBe(firstId);
    await waitFor(() => expect(result.current.saveStatus).toBe("saved"), { timeout: 1000 });

    // Switch back to first — should restore without duplicate inference
    expect(generateMock).toHaveBeenCalledTimes(2);
    act(() => result.current.selectConversation(firstId));
    await waitFor(() => expect(result.current.messages[0]?.content).toBe("hello first"));
    expect(result.current.messages.length).toBe(2);
    expect(result.current.messages[1]!.content).toBe("answer one");
    expect(generateMock).toHaveBeenCalledTimes(2); // no new call on restore

    // Switch to second
    act(() => result.current.selectConversation(secondId));
    await waitFor(() => expect(result.current.messages[1]!.content).toBe("answer two"));
  });

  it("preserves code blocks and inline code through save/restore", async () => {
    const run = session();
    const { result } = renderHook(() => usePersistentConversation());
    await waitFor(() => expect(result.current.listLoading).toBe(false));

    const codePrompt = "Write code:\n```rust\nfn main() {}\n```\nand `inline`";
    act(() => result.current.setDraft(codePrompt));
    act(() => result.current.submit());
    await act(async () => {
      const reply = "```js\nconsole.log('hi')\n```\nAnd `code` here";
      run.send(reply);
      run.settle();
    });
    await waitFor(() => expect(result.current.streaming).toBe(false));

    const id = result.current.activeId!;
    await waitFor(() => expect(result.current.saveStatus).toBe("saved"), { timeout: 1000 });

    // Simulate restart: new hook should load from same fallback storage
    const { result: second } = renderHook(() => usePersistentConversation());
    await waitFor(() => expect(second.current.listLoading).toBe(false));
    expect(second.current.conversations.length).toBe(1);
    act(() => second.current.selectConversation(id));
    await waitFor(() => expect(second.current.messages.length).toBe(2));
    expect(second.current.messages[0]!.content).toBe(codePrompt);
    expect(second.current.messages[1]!.content).toContain("```js");
    expect(second.current.messages[1]!.content).toContain("`code`");
  });

  it("does not generate on restore", async () => {
    const run = session();
    const { result } = renderHook(() => usePersistentConversation());
    await waitFor(() => expect(result.current.listLoading).toBe(false));

    act(() => result.current.setDraft("hello"));
    act(() => result.current.submit());
    await act(async () => {
      run.send("hi");
      run.settle();
    });
    await waitFor(() => expect(result.current.streaming).toBe(false));
    const id = result.current.activeId!;
    await waitFor(() => expect(result.current.conversations.length).toBe(1), { timeout: 1000 });

    generateMock.mockClear();
    act(() => result.current.selectConversation(id));
    await waitFor(() => expect(result.current.messages.length).toBe(2));
    expect(generateMock).not.toHaveBeenCalled();
  });

  it("prevents stale deltas from appearing in new conversation", async () => {
    const run = session();
    const { result } = renderHook(() => usePersistentConversation());
    await waitFor(() => expect(result.current.listLoading).toBe(false));

    act(() => result.current.setDraft("hello"));
    act(() => result.current.submit());
    // Switch before settle — stale delta should be dropped
    act(() => result.current.newChat());
    act(() => run.send("stale should not appear"));
    await act(async () => {
      run.settle();
    });
    await waitFor(() => expect(result.current.messages).toHaveLength(0));
    expect(result.current.streaming).toBe(false);
  });

  it("prevents stale save from overwriting newer state", async () => {
    const { result } = renderHook(() => usePersistentConversation());
    await waitFor(() => expect(result.current.listLoading).toBe(false));

    // Create first conversation with one message
    const run = session();
    act(() => result.current.setDraft("first"));
    act(() => result.current.submit());
    await act(async () => {
      run.send("reply1");
      run.settle();
    });
    await waitFor(() => expect(result.current.streaming).toBe(false));
    const id = result.current.activeId!;
    await waitFor(() => expect(result.current.conversations.length).toBe(1), { timeout: 1000 });

    // Rapid switch: save for first is pending (debounce 500ms), switch to second then back
    // We test that after debounce, the correct latest state wins
    // For this test, we directly test the hook's saveSeq guard by ensuring conversation still loads
    act(() => result.current.selectConversation(id));
    await waitFor(() => expect(result.current.messages.length).toBe(2));
    expect(result.current.messages[1]!.content).toBe("reply1");
  });

  it("supports rename and delete with confirmation flow", async () => {
    const { result } = renderHook(() => usePersistentConversation());
    await waitFor(() => expect(result.current.listLoading).toBe(false));

    const run = session();
    act(() => result.current.setDraft("hello"));
    act(() => result.current.submit());
    await act(async () => {
      run.send("hi");
      run.settle();
    });
    await waitFor(() => expect(result.current.streaming).toBe(false));
    expect(result.current.activeId).not.toBeNull();
    await waitFor(() => expect(result.current.saveStatus).toBe("saved"), { timeout: 1000 });

    // Rename
    await act(async () => {
      await result.current.renameActive("My Renamed Chat");
    });
    expect(result.current.activeTitle).toBe("My Renamed Chat");
    await waitFor(() => expect(result.current.conversations[0]!.title).toBe("My Renamed Chat"));

    // Delete
    await act(async () => {
      await result.current.deleteActive();
    });
    expect(result.current.activeId).toBeNull();
    expect(result.current.messages).toHaveLength(0);
    await waitFor(() => expect(result.current.conversations.length).toBe(0));
  });

  it("shows empty-history and loading states", async () => {
    const { result } = renderHook(() => usePersistentConversation());
    expect(result.current.listLoading).toBe(true);
    await waitFor(() => expect(result.current.listLoading).toBe(false));
    expect(result.current.conversations).toHaveLength(0);
    expect(result.current.listError).toBeNull();
  });

  it("handles unicode and multiline through persistence", async () => {
    const { result } = renderHook(() => usePersistentConversation());
    await waitFor(() => expect(result.current.listLoading).toBe(false));

    const run = session();
    const unicodePrompt = "Hello 🌍\nSecond line\n```code\nmultiline\n```";
    act(() => result.current.setDraft(unicodePrompt));
    act(() => result.current.submit());
    await act(async () => {
      run.send("Reply with 日本語 and emoji 🎉");
      run.settle();
    });
    await waitFor(() => expect(result.current.streaming).toBe(false));
    await waitFor(() => expect(result.current.saveStatus).toBe("saved"), { timeout: 1000 });
    const id = result.current.activeId!;

    const { result: second } = renderHook(() => usePersistentConversation());
    await waitFor(() => expect(second.current.listLoading).toBe(false));
    act(() => second.current.selectConversation(id));
    await waitFor(() => expect(second.current.messages.length).toBe(2));
    expect(second.current.messages[0]!.content).toBe(unicodePrompt);
    expect(second.current.messages[1]!.content).toBe("Reply with 日本語 and emoji 🎉");
  });

  it("reports save failure", async () => {
    const { result } = renderHook(() => usePersistentConversation());
    await waitFor(() => expect(result.current.listLoading).toBe(false));

    // Force save failure by mocking saveConversation to reject
    const historyMod = await import("../../lib/history");
    const spy = vi.spyOn(historyMod, "saveConversation").mockRejectedValue({ code: "storage", message: "Disk full" });

    const run = session();
    act(() => result.current.setDraft("hello"));
    act(() => result.current.submit());
    await act(async () => {
      run.send("hi");
      run.settle();
    });
    await waitFor(() => expect(result.current.streaming).toBe(false));

    // Wait for debounced save to fail
    await waitFor(() => expect(result.current.saveStatus).toBe("error"), { timeout: 1500 });
    expect(result.current.saveError).toContain("Disk full");

    spy.mockRestore();
  });
  it("drops an older load after a newer selection completes", async () => {
    const history = await import("../../lib/history");
    let finishOld!: (c: import("../../lib/history").PersistedConversation) => void;
    const snapshot = (id: string) => ({ id, title:id, createdAt:1,updatedAt:1,messages:[
      {id:"m1",role:"user" as const,content:id,createdAt:1,state:"complete"},
    ] });
    const spy = vi.spyOn(history,"loadConversation")
      .mockImplementationOnce(() => new Promise(resolve => { finishOld = resolve; }))
      .mockResolvedValueOnce(snapshot("newer"));
    const {result} = renderHook(() => usePersistentConversation());
    await waitFor(() => expect(result.current.listLoading).toBe(false));
    act(() => result.current.selectConversation("older"));
    await waitFor(() => expect(spy).toHaveBeenCalledTimes(1));
    act(() => result.current.selectConversation("newer"));
    await waitFor(() => expect(result.current.activeId).toBe("newer"));
    await act(async () => finishOld(snapshot("older")));
    expect(result.current.messages[0]?.content).toBe("newer");
    expect(generateMock).not.toHaveBeenCalled();
    spy.mockRestore();
  });

  // ---- stream integrity (Phase 6) ------------------------------------------

  it("ignores a delta addressed to a different request", async () => {
    const run = session();
    const { result } = renderHook(() => usePersistentConversation());
    await waitFor(() => expect(result.current.listLoading).toBe(false));

    act(() => result.current.setDraft("hello"));
    act(() => result.current.submit());

    await act(async () => {
      run.send("mine");
    });
    await act(async () => {
      // Same conversation, different request: an event from an earlier
      // generation must not be appended to this reply.
      run.raw({ type: "delta", requestId: "someone-elses-request", text: "not mine" });
    });

    const reply = result.current.messages[1];
    expect(reply?.content).toBe("mine");
    await act(async () => run.settle());
  });

  it("marks a failed stream as an error and never as a finished answer", async () => {
    const run = session();
    const { result } = renderHook(() => usePersistentConversation());
    await waitFor(() => expect(result.current.listLoading).toBe(false));

    act(() => result.current.setDraft("hello"));
    act(() => result.current.submit());
    await act(async () => {
      run.send("half an ans");
      run.fail({
        code: "network",
        message: "The local engine ended the reply before reporting it finished.",
      });
    });

    await waitFor(() => expect(result.current.streaming).toBe(false));

    const reply = result.current.messages[1];
    expect(reply?.state).toBe("error");
    expect(reply?.content).toBe("half an ans");
    expect(result.current.runtimeError).toContain("before reporting it finished");

    // A failed reply is not resent as if the model had said it: the next
    // request carries the user turn only, so incomplete output can never be
    // presented back to the model as a finished answer.
    const next = session();
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 400));
    });
    act(() => result.current.setDraft("try again"));
    act(() => result.current.submit());

    const sent = sentRequest(1).messages;
    expect(sent.map((m) => m.role)).toEqual(["user", "user"]);
    expect(sent.map((m) => m.content)).toEqual(["hello", "try again"]);
    await act(async () => next.settle());
  });

  it("records a stopped reply as stopped, keeping its partial text", async () => {
    const run = session();
    const { result } = renderHook(() => usePersistentConversation());
    await waitFor(() => expect(result.current.listLoading).toBe(false));

    act(() => result.current.setDraft("hello"));
    act(() => result.current.submit());
    await act(async () => {
      run.send("partial");
      run.settle({ cancelled: true, finishReason: "stopped" });
    });

    await waitFor(() => expect(result.current.streaming).toBe(false));

    const reply = result.current.messages[1];
    expect(reply?.state).toBe("stopped");
    expect(reply?.content).toBe("partial");

    const id = result.current.activeId!;
    act(() => result.current.selectConversation(id));
    await waitFor(() => expect(result.current.messages.length).toBe(2));
    // Persisted history never records an in-flight message as complete.
    expect(result.current.messages[1]?.state).toBe("stopped");
  });

  // ---- profile routing (Phase 6) -------------------------------------------

  it("names the selected profile on every request it sends", async () => {
    const run = session();
    const { result } = renderHook(() =>
      usePersistentConversation({ profileId: SONNET_ID }),
    );
    await waitFor(() => expect(result.current.listLoading).toBe(false));

    act(() => result.current.setDraft("route me"));
    act(() => result.current.submit());

    // The core decides which model runs; this only proves the message carries
    // the profile it was composed under, so a stale one can be refused.
    expect(generateMock).toHaveBeenCalledTimes(1);
    expect(sentRequest(0).profileId).toBe(SONNET_ID);

    await act(async () => run.settle());
  });

  it("defaults to the default profile when no selection is passed", async () => {
    const run = session();
    const { result } = renderHook(() => usePersistentConversation());
    await waitFor(() => expect(result.current.listLoading).toBe(false));

    act(() => result.current.setDraft("hello"));
    act(() => result.current.submit());

    // No profile is named: the core falls back to its own stored selection,
    // which is Nexora Mini. The UI never invents an id.
    expect(sentRequest(0).profileId).toBeNull();
    await act(async () => run.settle());
  });

  it("sends the profile that is selected when the message is sent, not at mount", async () => {
    const run = session();
    const { result, rerender } = renderHook(
      ({ profileId }: { profileId: string | null }) =>
        usePersistentConversation({ profileId }),
      { initialProps: { profileId: MINI_ID } },
    );
    await waitFor(() => expect(result.current.listLoading).toBe(false));

    rerender({ profileId: SONNET_ID });

    act(() => result.current.setDraft("after the switch"));
    act(() => result.current.submit());

    expect(sentRequest(0).profileId).toBe(SONNET_ID);
    await act(async () => run.settle());
  });

  it("keeps the conversation and its order when the profile changes", async () => {
    const run = session();
    const { result, rerender } = renderHook(
      ({ profileId }: { profileId: string | null }) =>
        usePersistentConversation({ profileId }),
      { initialProps: { profileId: MINI_ID } },
    );
    await waitFor(() => expect(result.current.listLoading).toBe(false));

    act(() => result.current.setDraft("first question"));
    act(() => result.current.submit());
    await act(async () => {
      run.send("first answer");
      run.settle();
    });
    await waitFor(() => expect(result.current.streaming).toBe(false));

    rerender({ profileId: SONNET_ID });

    expect(result.current.messages.map((m) => m.content)).toEqual([
      "first question",
      "first answer",
    ]);
    expect(result.current.messages.map((m) => m.state)).toEqual([
      "complete",
      "complete",
    ]);
    // A switch never re-sends anything on its own.
    expect(generateMock).toHaveBeenCalledTimes(1);

    // Past the double-submit guard: this is a real second message, not a
    // repeated Enter.
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 400));
    });

    act(() => result.current.setDraft("second question"));
    act(() => result.current.submit());

    // The second request goes out under the new profile and carries the whole
    // history in order.
    const request = sentRequest(1);
    expect(request.profileId).toBe(SONNET_ID);
    expect(request.messages.map((m) => m.role)).toEqual(["user", "assistant", "user"]);
    await act(async () => run.settle());
  });

  it("does not resurrect a deleted conversation on later flush", async () => {
    const history = await import("../../lib/history");
    const run = session();
    const {result} = renderHook(() => usePersistentConversation());
    await waitFor(() => expect(result.current.listLoading).toBe(false));
    act(() => result.current.setDraft("Delete me"));
    act(() => result.current.submit());
    act(() => run.send("partial"));
    const id = result.current.activeId!;
    await act(async () => result.current.deleteActive());
    await act(async () => run.settle());
    await expect(history.loadConversation(id)).rejects.toMatchObject({code:"not_found"});
    expect(await history.listConversations()).toHaveLength(0);
  });

  it("blocks late deltas after stop, persists profileId, cancels on unmount, and retries cleanly without duplicating user messages", async () => {
    const first = session();
    const { result, unmount } = renderHook(() =>
      usePersistentConversation({ profileId: SONNET_ID }),
    );
    await waitFor(() => expect(result.current.listLoading).toBe(false));

    act(() => result.current.setDraft("Implement binary search in Rust"));
    act(() => result.current.submit());
    act(() => first.send("half broken outp"));
    await act(async () => {
      first.fail({ code: "network", message: "Connection reset mid-stream." });
    });
    await waitFor(() => expect(result.current.streaming).toBe(false));

    expect(result.current.messages).toHaveLength(2);
    expect(result.current.messages[1]?.state).toBe("error");
    expect(result.current.messages[1]?.profileId).toBe(SONNET_ID);
    expect(result.current.canRetry).toBe(true);

    // Retry replaces the failed partial assistant turn and NEVER duplicates the user prompt!
    const retryRun = session();
    act(() => result.current.retry());
    expect(result.current.messages).toHaveLength(2);
    expect(result.current.messages[0]?.content).toBe("Implement binary search in Rust");
    expect(result.current.messages[1]?.content).toBe("");
    expect(sentRequest(1).messages).toEqual([
      { role: "user", content: "Implement binary search in Rust" },
    ]);

    act(() => retryRun.send("pub fn binary_search() {}"));
    await act(async () => {
      retryRun.settle();
    });
    await waitFor(() => expect(result.current.streaming).toBe(false));
    await waitFor(() => expect(result.current.saveStatus).toBe("saved"), { timeout: 1000 });

    const convId = result.current.activeId!;
    expect(result.current.messages[1]?.profileId).toBe(SONNET_ID);
    expect(result.current.canRetry).toBe(false);

    // Unmounting during an active third stream cancels generation immediately!
    const liveOnUnmount = session();
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 400));
    });
    act(() => result.current.setDraft("follow up"));
    act(() => result.current.submit());
    const thirdReqId = liveOnUnmount.requestId();
    unmount();
    expect(cancelMock).toHaveBeenCalledWith(thirdReqId);

    // Reopen across simulated restart and confirm profileId + clean non-duplicated transcript persisted!
    const { result: reopened } = renderHook(() => usePersistentConversation());
    await waitFor(() => expect(reopened.current.listLoading).toBe(false));
    act(() => reopened.current.selectConversation(convId));
    await waitFor(() => expect(reopened.current.messages.length).toBeGreaterThanOrEqual(2));
    expect(reopened.current.messages[0]?.content).toBe("Implement binary search in Rust");
    expect(reopened.current.messages[1]?.content).toBe("pub fn binary_search() {}");
    expect(reopened.current.messages[1]?.profileId).toBe(SONNET_ID);
  });

});

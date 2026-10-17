import { act, renderHook } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { useConversation } from "./useConversation";
import { SCAFFOLD_REPLY } from "./chat";

afterEach(() => {
  vi.useRealTimers();
});

describe("useConversation", () => {
  it("starts empty", () => {
    const { result } = renderHook(() => useConversation());

    expect(result.current.messages).toHaveLength(0);
    expect(result.current.draft).toBe("");
    expect(result.current.error).toBeNull();
  });

  it("rejects a whitespace-only submission without adding messages", () => {
    const { result } = renderHook(() => useConversation());

    act(() => result.current.setDraft("   "));
    act(() => result.current.submit());

    expect(result.current.messages).toHaveLength(0);
    expect(result.current.error?.code).toBe("empty");
  });

  it("appends the user turn and the scaffold reply, then clears the draft", () => {
    const { result } = renderHook(() => useConversation());

    act(() => result.current.setDraft("  build a landing page  "));
    act(() => result.current.submit());

    expect(result.current.messages).toHaveLength(2);
    expect(result.current.messages[0]).toMatchObject({
      role: "user",
      content: "build a landing page",
    });
    expect(result.current.messages[1]).toMatchObject({
      role: "assistant",
      kind: "scaffold",
      content: SCAFFOLD_REPLY,
    });
    expect(result.current.draft).toBe("");
    expect(result.current.error).toBeNull();
  });

  it("ignores a rapid second submit instead of duplicating the turn", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-10-18T10:00:00Z"));

    const { result } = renderHook(() => useConversation());

    act(() => result.current.setDraft("hello"));
    act(() => {
      result.current.submit();
      result.current.submit();
      result.current.submit();
    });

    expect(result.current.messages).toHaveLength(2);

    act(() => {
      vi.advanceTimersByTime(400);
    });

    act(() => result.current.setDraft("second"));
    act(() => result.current.submit());

    expect(result.current.messages).toHaveLength(4);
  });

  it("gives every message a distinct id", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-10-18T10:00:00Z"));

    const { result } = renderHook(() => useConversation());

    act(() => result.current.setDraft("first"));
    act(() => result.current.submit());

    vi.advanceTimersByTime(500);

    act(() => result.current.setDraft("second"));
    act(() => result.current.submit());

    const ids = result.current.messages.map((message) => message.id);

    expect(new Set(ids).size).toBe(ids.length);
  });

  it("clears messages, draft, and errors on a new chat", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-10-18T10:00:00Z"));

    const { result } = renderHook(() => useConversation());

    act(() => result.current.setDraft("hello"));
    act(() => result.current.submit());
    act(() => result.current.setDraft("still typing"));

    expect(result.current.messages).toHaveLength(2);

    act(() => result.current.newChat());

    expect(result.current.messages).toHaveLength(0);
    expect(result.current.draft).toBe("");
    expect(result.current.error).toBeNull();
  });

  it("accepts a new message immediately after a new chat", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-10-18T10:00:00Z"));

    const { result } = renderHook(() => useConversation());

    act(() => result.current.setDraft("hello"));
    act(() => result.current.submit());
    act(() => result.current.newChat());
    act(() => result.current.setDraft("fresh"));
    act(() => result.current.submit());

    expect(result.current.messages).toHaveLength(2);
    expect(result.current.messages[0]?.content).toBe("fresh");
  });

  it("dismisses a composer error", () => {
    const { result } = renderHook(() => useConversation());

    act(() => result.current.submit());
    expect(result.current.error).not.toBeNull();

    act(() => result.current.dismissError());
    expect(result.current.error).toBeNull();
  });
});

import { describe, expect, it } from "vitest";
import {
  appendDelta,
  isCurrentConversation,
  patchMessage,
  removeMessage,
  shouldIgnoreSubmission,
} from "./conversation";
import type { ChatMessage } from "./chat";

function message(id: string, overrides: Partial<ChatMessage> = {}): ChatMessage {
  return {
    id,
    role: "assistant",
    content: "",
    createdAt: 1,
    state: "streaming",
    ...overrides,
  };
}

describe("shouldIgnoreSubmission", () => {
  it("allows the first submission", () => {
    expect(shouldIgnoreSubmission(0, 5_000)).toBe(false);
  });

  it("blocks a second submission inside the guard window", () => {
    expect(shouldIgnoreSubmission(1_000, 1_100)).toBe(true);
  });

  it("allows a submission at the edge of the guard window", () => {
    expect(shouldIgnoreSubmission(1_000, 1_350)).toBe(false);
  });

  it("does not lock when the clock moves backwards", () => {
    expect(shouldIgnoreSubmission(10_000, 9_000)).toBe(false);
  });
});

describe("patchMessage", () => {
  it("replaces one message without touching the rest", () => {
    const before = [message("a"), message("b")];
    const after = patchMessage(before, "b", { state: "complete" });

    expect(after).not.toBe(before);
    expect(after[0]).toBe(before[0]);
    expect(after[1]?.state).toBe("complete");
  });

  it("returns the same array when the id is unknown", () => {
    const before = [message("a")];

    expect(patchMessage(before, "missing", { state: "complete" })).toBe(before);
  });
});

describe("appendDelta", () => {
  it("appends streamed text to the matching reply", () => {
    const after = appendDelta([message("a", { content: "Hel" })], "a", "lo");

    expect(after[0]?.content).toBe("Hello");
  });

  it("ignores text for an unknown message", () => {
    const before = [message("a")];

    expect(appendDelta(before, "other", "lo")).toBe(before);
  });

  it("ignores text once the reply is no longer streaming", () => {
    const before = [message("a", { state: "stopped" })];

    expect(appendDelta(before, "a", "late")).toBe(before);
  });

  it("does not mutate the previous array", () => {
    const before = [message("a", { content: "x" })];
    appendDelta(before, "a", "y");

    expect(before[0]?.content).toBe("x");
  });
});

describe("removeMessage", () => {
  it("removes a message and returns a new array", () => {
    const before = [message("a"), message("b")];
    const after = removeMessage(before, "a");

    expect(after.map((item) => item.id)).toEqual(["b"]);
    expect(before).toHaveLength(2);
  });

  it("returns the same array when nothing matched", () => {
    const before = [message("a")];

    expect(removeMessage(before, "z")).toBe(before);
  });
});

describe("isCurrentConversation", () => {
  it("matches only the live conversation", () => {
    expect(isCurrentConversation("c1", "c1")).toBe(true);
    expect(isCurrentConversation("c1", "c2")).toBe(false);
  });
});

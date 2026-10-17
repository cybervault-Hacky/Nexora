import { describe, expect, it } from "vitest";
import { appendForConversation, shouldIgnoreSubmission } from "./conversation";
import type { ChatMessage } from "./chat";

function message(id: string): ChatMessage {
  return {
    id,
    role: "user",
    content: "hello",
    createdAt: 1,
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

  it("allows a submission after the guard window", () => {
    expect(shouldIgnoreSubmission(1_000, 9_000)).toBe(false);
  });

  it("does not lock when the clock moves backwards", () => {
    expect(shouldIgnoreSubmission(10_000, 9_000)).toBe(false);
  });

  it("respects a custom guard", () => {
    expect(shouldIgnoreSubmission(1_000, 1_050, 0)).toBe(false);
  });
});

describe("appendForConversation", () => {
  it("appends a turn from the active conversation", () => {
    const next = appendForConversation([message("a")], [message("b")], "c1", "c1");

    expect(next.map((item) => item.id)).toEqual(["a", "b"]);
  });

  it("drops a turn from a conversation the user already left", () => {
    const current = [message("a")];
    const next = appendForConversation(current, [message("stale")], "c1", "c2");

    expect(next).toBe(current);
    expect(next.map((item) => item.id)).toEqual(["a"]);
  });

  it("returns the same array when there is nothing to append", () => {
    const current = [message("a")];

    expect(appendForConversation(current, [], "c1", "c1")).toBe(current);
  });
});

import { describe, expect, it } from "vitest";
import {
  deriveTitle,
  formatPreview,
  isValidTitle,
  sanitizeTitle,
  truncateTitle,
} from "./history";
import type { ChatMessage } from "./chat";

function msg(content: string): ChatMessage {
  return {
    id: "id-1",
    role: "user",
    content,
    createdAt: Date.now(),
    state: "complete",
  };
}

describe("deriveTitle", () => {
  it("returns fallback for empty messages", () => {
    expect(deriveTitle([])).toBe("New conversation");
    expect(deriveTitle([], "Fallback")).toBe("Fallback");
  });

  it("derives from first user prompt", () => {
    const messages: ChatMessage[] = [
      msg("Write a hello world in Rust"),
      { id: "2", role: "assistant", content: "here", createdAt: 0, state: "complete" },
    ];
    expect(deriveTitle(messages)).toBe("Write a hello world in Rust");
  });

  it("takes first line only and truncates", () => {
    const long = "A".repeat(100);
    expect(deriveTitle([msg(`${long}\nsecond line`) ]).length).toBeLessThanOrEqual(60);
    expect(deriveTitle([msg("First line\nSecond line")])).toBe("First line");
  });

  it("collapses whitespace", () => {
    expect(deriveTitle([msg("  hello   world  ")])).toBe("hello world");
  });

  it("ignores assistant-only messages", () => {
    const assistantOnly: ChatMessage[] = [
      { id: "1", role: "assistant", content: "hello", createdAt: 0, state: "complete" },
    ];
    expect(deriveTitle(assistantOnly)).toBe("New conversation");
  });
});

describe("truncateTitle", () => {
  it("keeps short titles", () => {
    expect(truncateTitle("short")).toBe("short");
  });

  it("adds ellipsis for long titles", () => {
    const long = "a".repeat(100);
    const out = truncateTitle(long, 60);
    expect(out.endsWith("…")).toBe(true);
    expect(out.length).toBe(60);
  });

  it("returns fallback for empty", () => {
    expect(truncateTitle("   ")).toBe("New conversation");
  });
});

describe("sanitizeTitle", () => {
  it("trims and collapses newlines", () => {
    expect(sanitizeTitle("hello\nworld")).toBe("hello world");
    expect(sanitizeTitle("  hello  ")).toBe("hello");
  });

  it("returns fallback for empty", () => {
    expect(sanitizeTitle("")).toBe("New conversation");
    expect(sanitizeTitle("   ")).toBe("New conversation");
  });

  it("caps at 200", () => {
    const long = "a".repeat(300);
    expect(sanitizeTitle(long).length).toBe(200);
  });
});

describe("isValidTitle", () => {
  it("validates", () => {
    expect(isValidTitle("hello")).toBe(true);
    expect(isValidTitle("")).toBe(false);
    expect(isValidTitle("   ")).toBe(false);
    expect(isValidTitle("a".repeat(201))).toBe(false);
  });
});

describe("formatPreview", () => {
  it("returns empty for empty", () => {
    expect(formatPreview("")).toBe("");
    expect(formatPreview("   ")).toBe("");
  });

  it("takes first line and truncates", () => {
    expect(formatPreview("first\nsecond")).toBe("first");
    const long = "a".repeat(200);
    expect(formatPreview(long, 120).length).toBe(120);
  });
});

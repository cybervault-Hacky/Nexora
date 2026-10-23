import { describe, expect, it } from "vitest";
import {
  normaliseLanguage,
  parseInline,
  parseMessage,
  parseRichInline,
  parseRichMessage,
  sanitizeMarkdownUrl,
} from "./code";

describe("parseMessage", () => {
  it("returns one text block for plain prose", () => {
    expect(parseMessage("Just a sentence.")).toEqual([
      { kind: "text", text: "Just a sentence." },
    ]);
  });

  it("keeps blank lines as paragraph boundaries and preserves inner newlines", () => {
    expect(parseMessage("First line\nsecond line.\n\nNew paragraph.")).toEqual([
      { kind: "text", text: "First line\nsecond line." },
      { kind: "text", text: "New paragraph." },
    ]);
  });

  it("extracts a fenced block with its language", () => {
    const blocks = parseMessage("Here:\n\n```ts\nconst a = 1;\n```\n\nDone.");

    expect(blocks).toEqual([
      { kind: "text", text: "Here:" },
      { kind: "code", language: "ts", code: "const a = 1;", closed: true },
      { kind: "text", text: "Done." },
    ]);
  });

  it("treats a fence without a language as an unlabelled block", () => {
    expect(parseMessage("```\nplain\n```")).toEqual([
      { kind: "code", language: null, code: "plain", closed: true },
    ]);
  });

  it("keeps an unclosed fence as code, which is the streaming state", () => {
    expect(parseMessage("```rust\nfn main() {")).toEqual([
      { kind: "code", language: "rust", code: "fn main() {", closed: false },
    ]);
  });

  it("preserves indentation and blank lines inside code", () => {
    const code = "fn a() {\n    x();\n\n    y();\n}";

    expect(parseMessage("```\n" + code + "\n```")[0]).toEqual({
      kind: "code",
      language: null,
      code,
      closed: true,
    });
  });

  it("returns nothing for empty or whitespace-only content", () => {
    expect(parseMessage("")).toEqual([]);
    expect(parseMessage("\n\n   \n")).toEqual([]);
  });

  it("auto-closes a fence at the end of the text", () => {
    const blocks = parseMessage("```js\na\n```\nafter");

    expect(blocks).toHaveLength(2);
    expect(blocks[1]).toEqual({ kind: "text", text: "after" });
  });

  it("does not close a backtick fence with tildes", () => {
    const blocks = parseMessage("```\na\n~~~\nb\n```");

    expect(blocks).toHaveLength(1);
    expect(blocks[0]).toMatchObject({ kind: "code", closed: true });
    expect((blocks[0] as { code: string }).code).toBe("a\n~~~\nb");
  });

  it("treats a nested-looking fence inside a block as code", () => {
    const blocks = parseMessage("```md\n```js\nx\n```\n```");

    // The inner fence opens nothing: the first closing fence ends the block.
    expect(blocks).toHaveLength(2);
    expect(blocks[0]).toMatchObject({ kind: "code", code: "```js\nx" });
  });

  it("keeps text before and after several blocks in order", () => {
    const blocks = parseMessage(
      "Intro\n\n```sh\nnpm test\n```\n\nMiddle\n\n```json\n{}\n```\n\nEnd",
    );

    expect(blocks.map((block) => block.kind)).toEqual([
      "text",
      "code",
      "text",
      "code",
      "text",
    ]);
  });
});

describe("normaliseLanguage", () => {
  it("takes the first word, lower-cased", () => {
    expect(normaliseLanguage("TypeScript title=x")).toBe("typescript");
  });

  it("accepts characters real fence tags use", () => {
    expect(normaliseLanguage("c++")).toBe("c++");
    expect(normaliseLanguage("c#")).toBe("c#");
    expect(normaliseLanguage("objective-c")).toBe("objective-c");
    expect(normaliseLanguage("shell_session")).toBe("shell_session");
  });

  it("rejects empty, over-long, or markup-like tags", () => {
    expect(normaliseLanguage("   ")).toBeNull();
    expect(normaliseLanguage("a".repeat(25))).toBeNull();
    expect(normaliseLanguage("<script>")).toBeNull();
    expect(normaliseLanguage('x"onload="y')).toBeNull();
  });
});

describe("parseInline", () => {
  it("splits plain text and code spans", () => {
    expect(parseInline("Use `npm test` to run.")).toEqual([
      { code: false, text: "Use " },
      { code: true, text: "npm test" },
      { code: false, text: " to run." },
    ]);
  });

  it("supports several spans in one paragraph", () => {
    expect(parseInline("`a` and `b`").filter((part) => part.code)).toEqual([
      { code: true, text: "a" },
      { code: true, text: "b" },
    ]);
  });

  it("leaves an unmatched backtick literal", () => {
    expect(parseInline("a ` b")).toEqual([{ code: false, text: "a ` b" }]);
    expect(parseInline("waiting `")).toEqual([{ code: false, text: "waiting `" }]);
  });

  it("ignores empty spans", () => {
    expect(parseInline("a `` b")).toEqual([{ code: false, text: "a `` b" }]);
  });

  it("returns one plain segment for text without backticks", () => {
    expect(parseInline("plain")).toEqual([{ code: false, text: "plain" }]);
  });

  it("keeps HTML-looking text as text", () => {
    expect(parseInline("<b>not bold</b>")).toEqual([
      { code: false, text: "<b>not bold</b>" },
    ]);
  });
});

describe("parseRichMessage & parseRichInline (Phase 07 Markdown hierarchy)", () => {
  it("parses headings, unordered and ordered lists, tables, blockquotes, and code blocks", () => {
    const md = [
      "# Overview",
      "",
      "## Key Steps",
      "",
      "- Validate checksum",
      "- Start `llama-server` on loopback",
      "",
      "1. Parse SSE lines",
      "2. Commit token allowance",
      "",
      "| Model | Size |",
      "| --- | --- |",
      "| Nexora Mini | 4.68 GB |",
      "| Nexora Sonnet | 8.99 GB |",
      "",
      "> Local execution never leaves your device.",
    ].join("\n");

    const blocks = parseRichMessage(md);
    expect(blocks).toEqual([
      { kind: "heading", level: 1, text: "Overview" },
      { kind: "heading", level: 2, text: "Key Steps" },
      {
        kind: "list",
        ordered: false,
        start: 1,
        items: ["Validate checksum", "Start `llama-server` on loopback"],
      },
      {
        kind: "list",
        ordered: true,
        start: 1,
        items: ["Parse SSE lines", "Commit token allowance"],
      },
      {
        kind: "table",
        headers: ["Model", "Size"],
        rows: [
          ["Nexora Mini", "4.68 GB"],
          ["Nexora Sonnet", "8.99 GB"],
        ],
      },
      { kind: "blockquote", text: "Local execution never leaves your device." },
    ]);
  });

  it("allows only validated http/https URLs and neutralizes dangerous schemes", () => {
    expect(sanitizeMarkdownUrl("https://rust-lang.org/learn")).toBe(
      "https://rust-lang.org/learn",
    );
    expect(sanitizeMarkdownUrl("javascript:alert(1)")).toBeNull();
    expect(sanitizeMarkdownUrl("data:text/html,<script>alert(1)</script>")).toBeNull();
    expect(sanitizeMarkdownUrl("file:///etc/passwd")).toBeNull();

    const safe = parseRichInline("Read [Rust Docs](https://doc.rust-lang.org) and **test**.");
    expect(safe).toEqual([
      { kind: "text", text: "Read " },
      { kind: "link", label: "Rust Docs", href: "https://doc.rust-lang.org/" },
      { kind: "text", text: " and " },
      { kind: "strong", text: "test" },
      { kind: "text", text: "." },
    ]);

    const unsafe = parseRichInline("Ignore [evil](javascript:alert(1)) payload.");
    expect(unsafe.some((n) => n.kind === "link")).toBe(false);
  });
});

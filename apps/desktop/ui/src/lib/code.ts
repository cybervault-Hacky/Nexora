/**
 * Parser for the small Markdown subset a coding assistant actually needs.
 *
 * Model output is untrusted text. This module turns it into plain data — blocks
 * and inline segments — so the renderer only ever creates text nodes. Nothing
 * here interprets HTML, and nothing escapes to a raw-markup path.
 *
 * The subset is deliberately narrow: fenced code with an optional language,
 * paragraphs, and inline code spans. Anything else stays literal text, which is
 * the safe failure mode while a reply is still streaming.
 */

export interface TextBlock {
  kind: "text";
  /** Paragraph text, with single newlines preserved. */
  text: string;
}

export interface CodeBlockData {
  kind: "code";
  /** Lower-cased fence language, or `null` when the fence had none. */
  language: string | null;
  /** Code exactly as the model wrote it, without the fence lines. */
  code: string;
  /**
   * False while the closing fence has not arrived yet. A streamed reply is
   * usually in this state, and it must still render as code.
   */
  closed: boolean;
}

export type ContentBlock = TextBlock | CodeBlockData;

export interface InlineSegment {
  code: boolean;
  text: string;
}

/** Longest fence language Nexora will display; anything longer is not a tag. */
const MAX_LANGUAGE_LENGTH = 24;
/** Guards the UI against a pathological single reply. */
const MAX_BLOCKS = 4_000;

const FENCE = /^ {0,3}(`{3,}|~{3,})(.*)$/;

/**
 * Keeps a fence info word only when it looks like a language tag.
 *
 * The label is displayed, so it is restricted to characters that cannot be
 * confused with markup: letters, digits, and `+`, `-`, `_`, `#`, `.`, `/`.
 */
export function normaliseLanguage(info: string): string | null {
  const token = info.trim().split(/\s+/)[0] ?? "";

  if (token.length === 0 || token.length > MAX_LANGUAGE_LENGTH) {
    return null;
  }

  if (!/^[A-Za-z0-9+#._/-]+$/.test(token)) {
    return null;
  }

  return token.toLowerCase();
}

/**
 * Splits inline text on backtick spans.
 *
 * A single backtick opens a span; the matching run closes it. Backticks with no
 * partner stay literal, which is what a user sees mid-stream.
 */
export function parseInline(text: string): InlineSegment[] {
  const segments: InlineSegment[] = [];
  let index = 0;
  let plain = "";

  const flushPlain = () => {
    if (plain.length > 0) {
      segments.push({ code: false, text: plain });
      plain = "";
    }
  };

  while (index < text.length) {
    if (text[index] !== "`") {
      plain += text[index];
      index += 1;
      continue;
    }

    const runStart = index;

    while (text[index] === "`") {
      index += 1;
    }

    const fence = text.slice(runStart, index);
    const close = text.indexOf(fence, index);

    if (close === -1) {
      // No partner: the backticks are ordinary characters.
      plain += fence;
      continue;
    }

    const inner = text.slice(index, close);

    if (inner.trim().length === 0) {
      plain += text.slice(runStart, close + fence.length);
      index = close + fence.length;
      continue;
    }

    flushPlain();
    segments.push({ code: true, text: inner });
    index = close + fence.length;
  }

  flushPlain();

  return segments;
}

/**
 * Splits a reply into text paragraphs and code blocks.
 *
 * Blank lines separate paragraphs; single newlines inside a paragraph are kept
 * so a short code-ish answer does not lose its shape.
 */
export function parseMessage(content: string): ContentBlock[] {
  const blocks: ContentBlock[] = [];
  const lines = content.split("\n");
  let text: string[] = [];

  const flushText = () => {
    // A blank-line separated run becomes one paragraph per run, preserving the
    // inner newlines.
    const runs = text.join("\n").split(/\n{2,}/);

    for (const run of runs) {
      const trimmed = run.replace(/^\n+|\n+$/g, "");

      // A run of spaces is not a paragraph; rendering it would add dead space.
      if (trimmed.trim().length > 0) {
        blocks.push({ kind: "text", text: trimmed });
      }
    }

    text = [];
  };

  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index] ?? "";
    const fence = FENCE.exec(line);

    if (!fence) {
      text.push(line);
      continue;
    }

    const marker = fence[1] ?? "```";
    const info = fence[2] ?? "";
    const fenceCharacter = marker[0];

    flushText();

    const language = normaliseLanguage(info);
    const code: string[] = [];
    let closed = false;

    for (index += 1; index < lines.length; index += 1) {
      const inner = lines[index] ?? "";
      const closing = FENCE.exec(inner);
      const closingMarker = closing?.[1] ?? "";

      // A fence closes only with its own character, without an info string.
      if (
        closing &&
        closingMarker[0] === fenceCharacter &&
        (closing[2] ?? "").trim().length === 0
      ) {
        closed = true;
        break;
      }

      code.push(inner);
    }

    blocks.push({
      kind: "code",
      language,
      // The streamed tail may end with the fence's own trailing newline; keep
      // the code verbatim but drop a single trailing blank line added by it.
      code: code.join("\n"),
      closed,
    });
  }

  flushText();

  return blocks.slice(0, MAX_BLOCKS);
}

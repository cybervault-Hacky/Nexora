/**
 * Safe text-node Markdown parser for Nexora assistant responses.
 *
 * Model output is untrusted text. Every token produced here resolves strictly
 * to plain strings rendered as React text nodes — never HTML markup. Raw tags
 * such as `<script>` or `<img onerror=...>` stay inert literal text, and links
 * accept only validated `http:` / `https:` URLs.
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

export type RichInlineNode =
  | { kind: "text"; text: string }
  | { kind: "code"; text: string }
  | { kind: "strong"; text: string }
  | { kind: "em"; text: string }
  | { kind: "link"; label: string; href: string };

export type RichBlock =
  | { kind: "paragraph"; text: string }
  | { kind: "heading"; level: 1 | 2 | 3 | 4 | 5 | 6; text: string }
  | { kind: "list"; ordered: boolean; start: number; items: string[] }
  | { kind: "table"; headers: string[]; rows: string[][] }
  | { kind: "blockquote"; text: string }
  | { kind: "hr" }
  | CodeBlockData;

/** Longest fence language Nexora will display; anything longer is not a tag. */
const MAX_LANGUAGE_LENGTH = 24;
/** Guards the UI against a pathological single reply. */
const MAX_BLOCKS = 4_000;

const FENCE = /^ {0,3}(`{3,}|~{3,})(.*)$/;
const HEADING = /^ {0,3}(#{1,6})\s+(.+?)\s*#*$/;
const UNORDERED_ITEM = /^ {0,3}[-*+]\s+(.+)$/;
const ORDERED_ITEM = /^ {0,3}(\d+)\.\s+(.+)$/;
const BLOCKQUOTE_LINE = /^ {0,3}>\s?(.*)$/;
const HORIZONTAL_RULE = /^ {0,3}(?:-\s*){3,}$|^ {0,3}(?:\*\s*){3,}$|^ {0,3}(?:_\s*){3,}$/;
const TABLE_SEPARATOR = /^\s*\|?(?:\s*:?-{3,}:?\s*\|)+\s*:?-{3,}:?\s*\|?\s*$/;

/**
 * Keeps a fence info word only when it looks like a language tag.
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
 * Splits inline text on backtick spans (core contract preserved verbatim).
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
 * Validates that a Markdown link destination uses a safe external web protocol.
 * Any `javascript:`, `data:`, `vbscript:`, `file:`, or malformed URL is rejected
 * so the source text remains plain inert text.
 */
export function sanitizeMarkdownUrl(raw: string): string | null {
  const trimmed = raw.trim();
  if (trimmed.length === 0 || trimmed.length > 2048) {
    return null;
  }
  try {
    const parsed = new URL(trimmed);
    if (parsed.protocol !== "https:" && parsed.protocol !== "http:") {
      return null;
    }
    return parsed.toString();
  } catch {
    return null;
  }
}

const RICH_TOKEN = /(\[([^\]\n]+)\]\(([^)\s\n]+)\)|\*\*([^*\n]+)\*\*|\*([^*\n]+)\*)/g;

/**
 * Parses inline code, bold (`**text**`), emphasis (`*text*`), and validated
 * external links (`[label](https://...)`) without ever interpreting HTML.
 */
export function parseRichInline(text: string): RichInlineNode[] {
  const base = parseInline(text);
  const out: RichInlineNode[] = [];

  for (const segment of base) {
    if (segment.code) {
      out.push({ kind: "code", text: segment.text });
      continue;
    }

    let cursor = 0;
    RICH_TOKEN.lastIndex = 0;
    let match: RegExpExecArray | null;

    while ((match = RICH_TOKEN.exec(segment.text)) !== null) {
      if (match.index > cursor) {
        out.push({ kind: "text", text: segment.text.slice(cursor, match.index) });
      }

      const full = match[0];
      const linkLabel = match[2];
      const linkTarget = match[3];
      const boldText = match[4];
      const italicText = match[5];

      if (linkLabel !== undefined && linkTarget !== undefined) {
        const safeHref = sanitizeMarkdownUrl(linkTarget);
        if (safeHref) {
          out.push({ kind: "link", label: linkLabel, href: safeHref });
        } else {
          out.push({ kind: "text", text: full });
        }
      } else if (boldText !== undefined) {
        out.push({ kind: "strong", text: boldText });
      } else if (italicText !== undefined) {
        out.push({ kind: "em", text: italicText });
      } else {
        out.push({ kind: "text", text: full });
      }

      cursor = match.index + full.length;
    }

    if (cursor < segment.text.length) {
      out.push({ kind: "text", text: segment.text.slice(cursor) });
    }
  }

  return out;
}

/**
 * Splits a reply into text paragraphs and code blocks (core contract preserved verbatim).
 */
export function parseMessage(content: string): ContentBlock[] {
  const blocks: ContentBlock[] = [];
  const lines = content.split("\n");
  let text: string[] = [];

  const flushText = () => {
    const runs = text.join("\n").split(/\n{2,}/);

    for (const run of runs) {
      const trimmed = run.replace(/^\n+|\n+$/g, "");

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
      code: code.join("\n"),
      closed,
    });
  }

  flushText();

  return blocks.slice(0, MAX_BLOCKS);
}

function splitTableRow(line: string): string[] {
  const trimmed = line.trim().replace(/^\|/, "").replace(/\|$/, "");
  return trimmed.split("|").map((cell) => cell.trim());
}

/**
 * Converts a plain `TextBlock` into rich structural Markdown blocks:
 * headings, unordered/ordered lists, pipe tables, blockquotes, horizontal rules,
 * and standard paragraphs.
 */
function expandProseBlocks(text: string): RichBlock[] {
  const lines = text.split("\n");
  const out: RichBlock[] = [];
  let paragraph: string[] = [];

  const flushParagraph = () => {
    const joined = paragraph.join("\n").trim();
    if (joined.length > 0) {
      out.push({ kind: "paragraph", text: joined });
    }
    paragraph = [];
  };

  let idx = 0;
  while (idx < lines.length) {
    const line = lines[idx] ?? "";
    const trimmed = line.trim();

    if (trimmed.length === 0) {
      flushParagraph();
      idx += 1;
      continue;
    }

    // Horizontal rule
    if (HORIZONTAL_RULE.test(line)) {
      flushParagraph();
      out.push({ kind: "hr" });
      idx += 1;
      continue;
    }

    // ATX Heading (# .. ######)
    const heading = HEADING.exec(line);
    if (heading) {
      flushParagraph();
      const level = Math.min(6, Math.max(1, (heading[1] ?? "#").length)) as 1 | 2 | 3 | 4 | 5 | 6;
      out.push({ kind: "heading", level, text: heading[2] ?? "" });
      idx += 1;
      continue;
    }

    // Pipe Table (header line + separator line + 0..N rows)
    const nextLine = lines[idx + 1] ?? "";
    if (line.includes("|") && TABLE_SEPARATOR.test(nextLine)) {
      const headers = splitTableRow(line);
      if (headers.length >= 1 && headers.some((cell) => cell.length > 0)) {
        flushParagraph();
        const rows: string[][] = [];
        idx += 2;
        while (idx < lines.length) {
          const rowLine = lines[idx] ?? "";
          if (!rowLine.includes("|") || rowLine.trim().length === 0) {
            break;
          }
          const cells = splitTableRow(rowLine);
          const padded = headers.map((_, colIdx) => cells[colIdx] ?? "");
          rows.push(padded);
          idx += 1;
        }
        out.push({ kind: "table", headers, rows });
        continue;
      }
    }

    // Unordered list
    const ulMatch = UNORDERED_ITEM.exec(line);
    if (ulMatch) {
      flushParagraph();
      const items: string[] = [];
      while (idx < lines.length) {
        const candidate = UNORDERED_ITEM.exec(lines[idx] ?? "");
        if (!candidate) break;
        items.push(candidate[1] ?? "");
        idx += 1;
      }
      out.push({ kind: "list", ordered: false, start: 1, items });
      continue;
    }

    // Ordered list
    const olMatch = ORDERED_ITEM.exec(line);
    if (olMatch) {
      flushParagraph();
      const start = Number.parseInt(olMatch[1] ?? "1", 10) || 1;
      const items: string[] = [];
      while (idx < lines.length) {
        const candidate = ORDERED_ITEM.exec(lines[idx] ?? "");
        if (!candidate) break;
        items.push(candidate[2] ?? "");
        idx += 1;
      }
      out.push({ kind: "list", ordered: true, start, items });
      continue;
    }

    // Blockquote
    const quoteMatch = BLOCKQUOTE_LINE.exec(line);
    if (quoteMatch) {
      flushParagraph();
      const quoteLines: string[] = [];
      while (idx < lines.length) {
        const candidate = BLOCKQUOTE_LINE.exec(lines[idx] ?? "");
        if (!candidate) break;
        quoteLines.push(candidate[1] ?? "");
        idx += 1;
      }
      out.push({ kind: "blockquote", text: quoteLines.join("\n").trim() });
      continue;
    }

    paragraph.push(line);
    idx += 1;
  }

  flushParagraph();
  return out;
}

/**
 * Parses an assistant message into rich ChatGPT-style blocks (headings, lists,
 * tables, blockquotes, horizontal rules, fenced code blocks, and paragraphs).
 */
export function parseRichMessage(content: string): RichBlock[] {
  const base = parseMessage(content);
  const rich: RichBlock[] = [];

  for (const block of base) {
    if (block.kind === "code") {
      rich.push(block);
    } else {
      rich.push(...expandProseBlocks(block.text));
    }
  }

  return rich.slice(0, MAX_BLOCKS);
}

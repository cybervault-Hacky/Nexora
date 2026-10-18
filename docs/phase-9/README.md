# Phase 9 — core AI coding assistant

Phase 9 turns the existing local-inference chat into a usable coding workflow. The
transport, streaming, cancellation, and setup screens already existed; what was
missing was everything that makes a reply *read* like code.

**Verdict: implemented and unit/component-tested. Native window and genuine model
inference remain NOT RUN** — this environment has no Windows host and no approved
GGUF.

## 1. Starting state (verified, not assumed)

| Item | Observed |
|---|---|
| Branch / HEAD | `arena/00bd2df7-nexora` @ `2ed0047` |
| Working tree | clean; Phases 1–8 intact |
| Frontend baseline | typecheck PASS, 152 tests in 11 files, build PASS |
| Rust | compiled, linted and unit-tested in CI (Linux 48, Windows 45) |
| Native window | not run — no Windows host |
| Real model | none — no owner-approved GGUF exists in this sandbox |

Already working before this phase: real Tauri commands, streamed tokens into one
assistant message, Stop, retry, duplicate-submission guard, per-conversation
request identity, stale-drop, model setup with honest failures, typed errors.

**The gap:** `MessageList` split assistant content on blank lines and rendered
`<p>{paragraph}</p>`. There were no code fences, no language labels, no monospace,
no inline code, and no copy action; multi-line answers also lost their line
structure. For a coding assistant that is the core failure.

## 2. What was implemented

### Parser — `ui/src/lib/code.ts` (new)

`parseMessage(content)` → `ContentBlock[]` (`text` | `code`), and
`parseInline(text)` → inline segments.

- Fenced blocks open on three or more backticks or tildes; a block closes only
  with the **same character** and an empty info string, matching CommonMark.
- A fence that never closes is returned with `closed: false` and still renders as
  code — that is the normal mid-stream state, and it prevents text flashing into
  a paragraph before the closing fence arrives.
- The language is the first info word, lower-cased, accepted only if it matches
  `[A-Za-z0-9+#._/-]+` and is at most 24 characters. A tag containing quotes or
  angle brackets is discarded rather than displayed.
- Paragraphs keep single newlines; blank lines separate them. Whitespace-only runs
  are dropped (a bug the first test run caught).
- Inline spans pair backtick runs; an unmatched backtick stays literal, and empty
  spans are ignored.
- Bounded at 4,000 blocks per reply.

### Rendering — `ui/src/components/CodeBlock.tsx`, `MessageContent.tsx` (new)

`CodeBlock` renders a `figure`/`figcaption` bar with the language (`code` when
absent), a line count, a `· streaming` marker, and a Copy button, over a
`<pre><code>` body:

- long lines scroll horizontally (`white-space: pre`) so columns stay aligned;
- blocks cap at 420 px (320 px under 720 px wide) and scroll vertically;
- the `<pre>` is focusable (`tabIndex=0`) with `role="group"` and a label, so a
  keyboard user can scroll it;
- copy uses the async Clipboard API with a legacy fallback, reports **Copied** or
  **Copy failed**, resets after 1.6 s, clears its timer on unmount, and announces
  the result in a `role="status"` region.

`MessageContent` renders every assistant block and every user prompt as React
**text nodes**. There is no `dangerouslySetInnerHTML` and no HTML parsing path, so
`<script>`, `<img onerror=…>` and similar arrive as visible characters.

`MessageList` now delegates to these components; the existing Generating /
Stopped / Failed chips and the streaming caret are preserved.

### Styling — `ui/src/styles/app.css`, `tokens.css`

Adds `--font-mono` (system faces only, nothing downloaded) and a code section
built from the existing tokens and radii. No redesign: colours, spacing, and
component shapes are unchanged, per the Phase 18 reservation. User prompts get
`white-space: pre-wrap` so a multi-line prompt keeps its shape.

### Unchanged on purpose

Transport, Rust, model handling, capability set, and dependencies. No new package
was added.

## 3. Architecture and data flow

```
llama-server ──SSE──► Rust stream_chat ──Channel──► generate()
                                                        │
                                              useConversation (session id,
                                              stale-drop, stop, retry)
                                                        │
                                              ChatMessage.content
                                                        │
                                              parseMessage()  ──► blocks
                                                │                    │
                                       Paragraph + inline      CodeBlock
                                       (text nodes only)   (label, copy, scroll)
```

Parsing happens in the render path with `useMemo` keyed on content, so each
streamed delta re-parses only the message that changed.

## 4. Tests and actual results

| Suite | Tests | What it protects |
|---|---|---|
| `lib/code.test.ts` (new) | 20 | fence detection, unclosed fences, mixed fence characters, indentation, language sanitisation, inline spans, unmatched backticks, empty input |
| `components/CodeBlock.test.tsx` (new) | 10 | verbatim code, language fallback, streaming marker, line count, copy success, **copy failure**, reset timer, untrusted markup, 5,000-char line, `role="status"` |
| `App.interaction.test.tsx` (+4) | 16 total | streamed code block, half-streamed fence stays code, inline code vs prose, literal `<img onerror>` with no element created, copying from the transcript |

Existing suites (streaming, stop, retry, duplicate submit, stale drop, empty
input, runtime errors) were left untouched and still pass.

Executed locally:

| Check | Status |
|---|---|
| `pnpm typecheck` | **PASS** |
| `pnpm test` | **PASS** — 187 tests in 13 files (was 152 in 11) |
| `pnpm build` | **PASS** — 271.60 kB JS (83.70 gzip), 22.04 kB CSS (5.02 gzip) |
| `cargo fmt/check/clippy/test` | **PASS** in CI (no toolchain locally) |
| Native window / installer | **NOT RUN** |
| Genuine inference with a real model | **NOT RUN** |

No UI screenshot was captured: the sandbox has no headless browser, only jsdom.
Rendering is verified behaviourally, not visually.

## 5. Known limitations and risks

- **No syntax highlighting.** Deferred deliberately: a highlighter would add a
  second parser for untrusted input and bundle weight. Blocks are plain monospace
  with a language label.
- **Markdown subset only.** No headings, lists, tables, links, or bold. They stay
  literal text, which is honest but visually plain.
- **Copy depends on webview permissions.** The failure path is implemented and
  tested, but the Tauri webview's clipboard behaviour has not been observed on
  Windows.
- **Very large replies** are capped at 4,000 blocks in the parser; the Rust side
  already caps a reply at 400,000 characters.
- **Visual layout at real window sizes is unverified** — jsdom cannot measure.
- The parser is a source of truth for rendering only; it does not affect what is
  sent to the model.

## 6. Files changed

| File | Purpose |
|---|---|
| `ui/src/lib/code.ts` | new — fence/paragraph/inline parser |
| `ui/src/lib/code.test.ts` | new — 20 parser tests |
| `ui/src/lib/clipboard.ts` | new — copy with fallback, never throws |
| `ui/src/components/CodeBlock.tsx` | new — code block with label and copy |
| `ui/src/components/CodeBlock.test.tsx` | new — 10 component tests |
| `ui/src/components/MessageContent.tsx` | new — safe block/inline renderer |
| `ui/src/components/MessageList.tsx` | delegates to the new renderer |
| `ui/src/App.interaction.test.tsx` | +4 end-to-end rendering tests |
| `ui/src/styles/app.css`, `styles/tokens.css` | code styles + mono token |
| `docs/phase-9/README.md` | this report |

## 7. Deferred to later phases

- multi-file editing, patch application, and agent tools (a later coding phase);
- syntax highlighting and diff rendering;
- conversation persistence and a history switcher;
- the premium visual redesign, glassmorphism, and composer rework (Phase 18);
- any cloud model, account, telemetry, or non-local inference.

## 8. Commits

Recorded in the delivery message. Branch `arena/00bd2df7-nexora`, pushed by
fast-forward; `main` untouched; no release, PR, installer, or deployment.

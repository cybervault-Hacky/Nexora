# Phase 10 — Local conversation persistence & history switcher

> **Status: IN PROGRESS — audit complete, implementation in progress.**
> This document records the actual starting state verified before any Phase 10 edits and the implementation plan. It will be updated at completion with architecture, schema, privacy guarantees, test results, and commit IDs.

## 1. Actual starting state (verified, not assumed)

### Git state

| Item | Observed value |
|------|---------------|
| Branch | `arena/74aec262-nexora` |
| HEAD | `04deebabb9aeed56e68f99a0080a276b4796b53b` (grafted, merge commit `Merge pull request #1 … Phases 1-9`) |
| Remote | `https://github.com/cybervault-Hacky/Nexora.git` (origin/main, origin/HEAD → origin/main) |
| Working tree | clean, no staged or unstaged changes |
| Recent log | single grafted commit — all Phase 1-9 work delivered as one merge (22080 insertions, 103 files) |
| Phase 9 reference | requested `5108360` does not exist as a separate commit in this checkout; HEAD is the Phase 1-9 merge `04deeba`. No reset performed — plan uses actual HEAD. |

No `arena/00bd2df7-nexora` branch exists locally; `04deeba` is the effective Phase 9 baseline.

### Frontend baseline (pre-Phase 10)

- **Shell** `App.tsx`: desktop/browser routing, status-driven empty vs conversation, `useConversation({onSettled})` with `newChat`/`submit`/`stop`, sidebar toggle, keyboard shortcuts (Ctrl+N/L/B, Enter/Esc), scroll stick-to-bottom.
- **Conversation model** `features/chat/chat.ts`: `ChatMessage {id, role, content, createdAt, state, requestId?, failure?}`, `MAX_MESSAGE_LENGTH 32k`, `validateDraft`, `createUserMessage`, `createAssistantMessage`, `toChatTurns`.
- **Conversation helpers** `features/chat/conversation.ts`: `SUBMISSION_GUARD_MS 350`, `patchMessage`, `appendDelta`, `removeMessage`, `isCurrentConversation`.
- **Hook** `features/chat/useConversation.ts`: single in-memory conversation only. `conversationRef` UUID + `sessionRef {requestId, assistantId, conversationId}` guards stale deltas. `mountedRef` prevents post-unmount status refresh. No persistence — comment `Nothing is persisted.` Explicit stale-drop for deltas/completions after `newChat` or conversation switch.
- **IPC** `lib/ipc.ts` + `lib/inference.ts`: `invoke("get_app_status" | "get_settings" | "save_settings" | "validate_model" | "probe_runtime" | "load_model" | "unload_model" | "start_generation" | "cancel_generation")`. Typed errors via `toInferenceError`. Browser preview uses `previewStatus()` and disables send.
- **Sidebar** `components/Sidebar.tsx`: brand, `New chat` button, static `Recent` section with `No conversations yet. History is kept for this session only.` — no list, no persistence.
- **Rendering** `components/MessageContent.tsx` + `components/CodeBlock.tsx` + `lib/code.ts`: safe text-node renderer, fence/inline parser, no `dangerouslySetInnerHTML`, copy action with fallback. Preserved post-Phase 9.
- **CSS** `styles/app.css` + `tokens.css`: single codebase, no glassmorphism redesign (Phase 18 reserved).
- **Tests** 187 Vitest tests in 13 files (typecheck, rendering, streaming lifecycle, contract checks, build config). Rust unit tests run in CI (Linux/Windows) — not locally without toolchain.

### Rust/Core baseline (pre-Phase 10)

- `src/lib.rs` manages `InferenceState { supervisor, settings, active, config_dir }` where `config_dir = app.path().app_config_dir()`. Persists only `inference-settings.json` via `SettingsStore::load/save` (defaults on missing/corrupt, `create_dir_all`, `write` pretty JSON). No conversation storage.
- `inference/mod.rs` validates requests (`MAX_TURNS 64`, `MAX_TURN_CHARS 100k`, `MAX_CONVERSATION_CHARS 400k`, `MAX_OUTPUT_TOKENS 8192`), engine state classification, `status()`, `cancel_generation`, `finish_generation`.
- `inference/commands.rs` exposes 10 commands (status, systemInfo, settings, validate, probe, load, unload, startGeneration, cancel). No history commands.
- `Cargo.toml` dependencies: `serde`, `serde_json`, `tauri 2`, `reqwest`, `futures-util`, `tokio`, `getrandom`, `windows-sys` (cfg windows). No SQLite, no extra storage crate.
- `tauri.conf.json` product `Nexora 0.1.0`, `identifier com.nexora.desktop`, CSP `script-src 'self'`, `connect-src ipc: http://ipc.localhost`, per-user NSIS, no fs/shell/http plugins. Capability `main` grants only `core:default` + window controls + `allow-start-dragging`.

### Persistence audit

| Question | Finding |
|----------|---------|
| Any conversation file on disk? | No — no `conversations.json`, no `conversations/` dir, no history module. |
| Any Tauri command for history? | No. |
| Any localStorage/IndexedDB usage? | No. |
| `useConversation` persistence? | Explicitly none; `newChat` wipes in-memory array. |
| Settings persistence | Yes — `SettingsStore` pattern to reuse. |

**Conclusion:** Phase 10 starts from a clean non-persistent baseline. All Phase 1-9 work is intact; no data migration from prior history exists.

## 2. Storage decision (Step 2)

### Options evaluated

1. **SQLite (`rusqlite` + `sqlx`)** — ACID, per-row isolation, migrations. Adds ~500 KB, schema management, build complexity for a 200-conversation workload. Justified only if existing code already used it (it does not).
2. **Per-conversation JSON files (`<config_dir>/conversations/<id>.json` + `index.json`)** — fine-grained failure isolation, single-file deletion, but index can drift from files, requires directory creation + index sync, more edge cases for atomicity.
3. **Single versioned JSON file (`<config_dir>/conversations.json`) with atomic temp+rename** — simplest, matches `SettingsStore` pattern, trivial to test, low overhead (<5 MB typical), easy to validate entry-by-entry. Risk of whole-file corruption mitigated by atomic write + entry-level lenient parsing (skip malformed entries, never delete whole history on one bad record).

### Decision

**Option 3: single versioned JSON file with atomic replacement.**

- File: `<app_config_dir>/conversations.json`
- Schema: `{ version: 1, conversations: StoredConversation[] }` where `StoredConversation { id, title, createdAt, updatedAt, messages: StoredMessage[] }`
- Atomic write: write to `conversations.json.tmp.<rand>` in same directory, then `rename` (replace). No partial file survives interrupt. Temp files ignored on load.
- Validation: IDs checked against `^[A-Za-z0-9_-]{1,128}$`, titles trimmed to 200 chars, messages bounded (`≤2000` per conversation, each content `≤100_000` chars, total JSON `≤10 MB`), roles `user|assistant`, roles/IDs required. Malformed entries skipped, not deleted.
- Versioning: `version: 1` current. On load, if `version > CURRENT` → error `unsupported_version`; if missing or `0` → migrate by adding defaults (empty title fallback, `updatedAt = createdAt`, normalize messages). Future migrations keep file readable.
- No cloud, no telemetry, no secrets: only id/title/timestamps/messages persisted. Never logs full conversations. Disk encryption not claimed — documented as OS-dependent.
- Reuses `InferenceError`-shaped `{code, message}` errors (`storage`, `not_found`, `invalid_request`, `unsupported_version`).

SQLite rejected: overkill for local history; per-file approach rejected: adds sync complexity without measurable benefit for this scale.

## 3. Implementation plan (Steps 3-8)

### Step 3 — Persistence API (Rust)

New module `src/history` (or `src/history.rs` + `src/history/commands.rs`):

- Types: `StoredMessage`, `StoredConversation`, `ConversationSummary` (derived: messageCount, preview truncated 120 chars, first line), `ConversationsFile`.
- Constants: `CURRENT_VERSION = 1`, `MAX_CONVERSATIONS 500`, `MAX_MESSAGES_PER_CONVERSATION 2000`, `MAX_MESSAGE_CHARS 100_000`, `MAX_TITLE_CHARS 200`, `MAX_TOTAL_BYTES 10 MiB`, `ID_REGEX` equivalent via char check.
- `HistoryStore { path: PathBuf, cache: Mutex<Vec<StoredConversation>> }` with `load()` on startup (read file if exists, lenient parse, otherwise empty), `save()` atomic, `list()`, `load_by_id()`, `create()`, `upsert()`, `rename()`, `delete()`.
- Commands: `list_conversations`, `load_conversation`, `create_conversation`, `save_conversation`, `rename_conversation`, `delete_conversation` — all `#[tauri::command]` returning `Result<T, HistoryError>`.
- Register in `lib.rs`: `app.manage(HistoryStore::new(config_dir))`, add 6 commands to `generate_handler!`.

### Step 4 — Integrate persistence with chat

Frontend:

- `ui/src/lib/history.ts`: IPC wrappers (`listConversations`, `loadConversation`, `createConversation`, `saveConversation`, `renameConversation`, `deleteConversation`) with desktop `invoke` + browser `localStorage` fallback for jsdom/tests.
- `ui/src/features/chat/history.ts`: `deriveTitle(messages)`, `truncateTitle`, `sanitizeTitle`, `formatPreview`, `MAX_TITLE_LENGTH 60` (safe ellipsis).
- `ui/src/features/chat/usePersistentConversation.ts` (or `useConversations.ts`): manages `list`, `activeId`, `activeMessages`, `saveStatus (idle|saving|saved|error)`, `saveError`, `listError`. Debounced save (500 ms) + immediate on completion/delete/rename/newChat. Stale-save guard via `saveSeq`. Conversation switch cancels in-flight generation (`cancelGeneration`) and flushes pending save for previous ID. Restore does **not** call `generate`. Persisted `streaming` messages coerced to `complete` on load. `beforeunload` flush.

### Step 5 — History switcher (UI)

- Extend `Sidebar.tsx`: props for `conversations`, `activeId`, `loading`, `error`, `onSelect`, `onRename`, `onDelete`, `saveStatus`. Render list with selected state, timestamp, message count, preview. Actions: inline rename (input + confirm/cancel, Enter/Esc), delete with two-step confirm (`Delete?` → `Confirm`). Empty/loading/error states. Keyboard: Tab, Enter to select, `Ctrl+N` still new chat, `Delete` key on focused item triggers confirm.
- `App.tsx`: replace `useConversation` with persistent hook, wire sidebar, show save indicator in `conversation-header__chips` (Saved/Saving/Failed with retry).
- CSS: add `.history`, `.history__item`, `.history__item--selected`, `.history__empty`, `.history__error`, etc., reusing tokens, no glassmorphism.

### Step 6 — Data integrity & privacy

- All persisted messages rendered through `MessageContent` text nodes — no HTML injection.
- Never persist `requestId`/`sessionKey`/`apiKey` beyond message metadata needed for UI; no credentials stored.
- Deletion: `remove_file` (single file) or splice vector (single file) — only matching ID removed; never `clear` or `remove_dir`.
- Error handling: read/write failures returned as typed errors, surfaced in UI banner (`Failed to save conversation — retry`), not silent.
- Docs: state that data is stored plaintext in OS app-data dir, protected by OS file permissions (0600 on Unix for session key, but history file uses default). No encryption claimed.

### Step 7 — Tests

- **Rust** (`cargo test`): `history::tests` covering create/save/list/load/rename/delete, multiple independent conversations, unicode/multiline/large messages, empty conversations, missing/malformed records, unsupported version, ID validation, size limits, isolation, atomic write (tmp+rename, no corrupt file left), permission failure (read-only dir simulation where possible).
- **Frontend** (`pnpm test`): `lib/history.test.ts` (title derivation, truncation, validation), `features/chat/history.test.ts`, `usePersistentConversation.test.tsx` (create/switch, save/restore after restart, code block preservation, no duplicate inference, stale races, rename/delete confirm, save failure, loading/empty states), plus updated `rustContract` guards if needed.

### Step 8 — Verification

Run and report: `pnpm typecheck`, `pnpm test`, `pnpm build`, `cargo fmt --check`, `cargo check`, `cargo clippy -- -D warnings`, `cargo test`. Use CI workflow if local toolchain absent. No claim of native window/model execution unless actually run.

### Strict constraints respected

- Preserve Git history, no reset/force-push.
- Keep `main` private, no release/installer.
- No secrets/model weights committed.
- Phase 9 renderer preserved, no Phase 18 redesign, no multi-file editing.

---

*Initial audit completed 2026-10-18 on `arena/74aec262-nexora` @ `04deeba`. Next commits will implement the above; this file will be updated with architecture, file list, schema, privacy, test results, Git state, and risks at completion.*

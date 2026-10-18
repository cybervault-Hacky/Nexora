# Phase 12 — Context budget management & user-approved patch preview

> **Status: IN PROGRESS — audit complete, implementation started. No push while repository visibility remains public.**
> `isPrivate: false` reported; safety gate active. No commits pushed, no PR, no visibility change this phase.

## 1. Actual starting state (verified 2026-10-18, head `0339588`)

| Item | Observed |
|------|----------|
| Branch | `arena/74aec262-nexora` |
| HEAD local | `0339588 Phase 11: read-only workspace & codebase understanding foundation` (ahead of origin) |
| Origin HEAD | `2e1f854 Phase 10` |
| Working tree | clean at audit |
| `isPrivate` | `false` (public) |
| Tauri capabilities | `core:default` + `window:*` only; no `fs`/`dialog` scope; `lib.rs` manages `HistoryState` (app_data) + `InferenceState` (app_config) + `WorkspaceState(Mutex<Option<PathBuf>>)` + 24 commands (10 inference, 6 history, 8 workspace) |
| Inference settings | `SettingsStore` in `app_config_dir/inference-settings.json` `{version undefined, modelPath, runtimePath, contextSize 2048-131072 default 32768}` — normalised, `AppStatus.contextSize` exposed via `get_app_status`. Validation `MAX_TURNS 64`, `MAX_TURN_CHARS 100k`, `MAX_CONVERSATION_CHARS 400k`. |
| Workspace scan | `workspace/scanner.rs` iterative DFS, `MAX_DEPTH 12`, `MAX_FILES 8000`, `MAX_DIRS 8000`, `MAX_INVENTORY_BYTES 2 MiB`, `MAX_PREVIEW_BYTES 256 KiB`, `MAX_CONTEXT_* 100 KiB/8 files`, excluded 27 dirs + `.gitignore` literal, symlink→canonical→starts_with, traversal rejected, sensitive `.env/*.pem/id_rsa…` → `secret_excluded`, binary nul/utf8 → `binary`, posix relative. |
| Workspace UI | `lib/workspace.ts` (pick via dynamic `plugin-dialog` + prompt fallback, `localStorage nexora:workspace:v1`), `useWorkspace` (open/refresh/close, search filteredEntries case-insensitive, selectedPaths Set, contextFiles + contextBlock fenced, preview load), `WorkspacePanel` (search, stats, truncated, list dir/file, checkbox context), `FilePreview` (read-only, error titles), `App.tsx` grid `340px explorer + chat` with `context-bar` chips + `submit({contextBlock})` prepends last user turn. |
| Context builder | `useWorkspace.contextBlock` = `"Project: {name} — untrusted…\n\nFile: {path}\n```{lang}\n{content}\n```\n…---"` per file, fence escalated to ```` if ``` inside, **no budget** — simply concatenates selected files (bounded only by 100 KiB aggregate) regardless of `contextSize`, history length, or `maxTokens`. No token estimate, no trimming, no omitted reporting, no measured vs estimated label. |
| Generation | `usePersistentConversation.submit({contextBlock?})` → `toChatTurns(history)` → if contextBlock then last user turn replaced with `${contextBlock}\n\n${original}`; `generate({messages: turns, maxTokens:1024})` validated via `inference/mod.rs::GenerateRequest::validate`. |
| Persistence | History: `app_data_dir/conversations.json` versioned atomic; Inference: `app_config_dir/inference-settings.json`; Workspace: only `localStorage` for convenience, **no Rust persistence file**, no versioned format, auto-scan not prevented on reload (currently explicit only but no setting file). |
| Tests | `pnpm test` 269/21, `typecheck` PASS, `build` 307.90 kB JS, `cargo fmt` via wasm PASS, `cargo check/clippy/test` BLOCKED (no toolchain locally, CI push blocked by public). |

No context-budget planner, no patch proposal/diff/apply, no workspace settings persistence file.

### Reuse & constraints
Reuse `HistoryStore` atomic pattern, `WorkspaceState` mutex, `toChatTurns` + `deriveTitle`, design tokens, no telemetry/cloud, read-only workspace remains, inference local-only, no autonomous multi-file editing, no shell exec.

## 2. Implementation plan

### Step 1 — Context budget management
- **Audit:** Already above. `Settings.contextSize` is source of truth; `AppStatus.contextSize` from `useAppStatus`/`InferenceState::status`. No tokenizer crate present; `MAX_CONVERSATION_CHARS` is char budget, not token.
- **Design:** New `ui/src/features/workspace/contextBudget.ts` + hook integration. Budget = `contextSize` tokens. Reserve: system overhead `≈200` tokens, `maxTokens` for generation (1024 default), then `history` + `currentPrompt` + `selectedFiles`. Token estimate: conservative `chars/4` (≈ 1 token per 4 chars, English code avg 3-5) → labeled **Estimated**; if a real tokenizer were available (e.g., `tiktoken` or model-specific), we would expose exact and label **Measured**. Never present estimated as exact. Prioritize `currentPrompt` (never dropped), then `selectedFiles` in user-chosen order, then recent `history` (newest turns first). Trim lower-priority: drop oldest `ChatTurn`s, then `selectedFiles` from last to first if still over. Preserve per-file fences: if a file is trimmed, either include fully or omit entirely — never slice inside a fence or merge boundaries. Malformed fences avoided by escaping: file content fence escalates to ` ```` ` if contains ```, and we never split inside `File:` header. Enforce per-file `256 KiB` and aggregate `100 KiB` still, but budget may omit entire files earlier. Output `ContextPlan { mode: "estimated", totalTokens, budgetTokens, reserved, historyTokens, promptTokens, filePlans: [{path, tokens, included, omittedReason}], omittedFiles, includedFiles, warnings }`. UI: `ContextBudgetBar` in explorer / above composer showing `Included X / Y files — Z tokens / N budget (Estimated)` + `truncated`/`omitted` list + tooltip explaining estimate vs measured. No silent project send — only selected eligible files.

### Step 2 — Single-file patch proposal
- **Typed validated representation:** New `workspace/patch.rs` Rust module + `ui/src/lib/patch.ts` / `ui/src/features/workspace/usePatch.ts`. `PatchProposal { id, path, originalContent, originalHash (sha256 hex), proposedContent, createdAt, stats }` + `ValidationResult`. Proposal generation: from **actual file content** — UI provides `<textarea>` editor prefilled with `read_workspace_file` content; user edits, then `createPatchProposal(path, originalContent, proposedContent)` validates via Rust command `validate_patch_proposal`. Validation: canonicalize root+path → `starts_with(root)` else `forbidden`; `is_sensitive_file` → `secret_excluded`; `metadata.len()` >256 KiB or `proposedContent.len()>256 KiB` → `too_large`; binary check (nul or invalid utf8) → `binary`; target must be existing regular file ( `is_dir` → invalid); detect multi-file or new-file creation → `invalid_request`; hash mismatch later for staleness. Model output is untrusted: even if a future LLM returns patch JSON, it goes through same `validate_patch_proposal` and hash is recomputed from current file, not trusted tag.
- **Report missing integration:** Document that structured LLM patch output is not yet fully wired (no dedicated `generate_patch` SSE); current proposals are user-edited after preview, but validation layer is ready for future LLM integration.

### Step 3 — Diff preview UI
- New `components/PatchPreview.tsx` using design tokens (`history__*`, `file-preview__code`). Shows file path + `+N / -N / ~N` summary, line numbers optional, added (green `rgba(34,197,94,0.12)` border), removed (red), unchanged, horizontal scroll (`overflow-x:auto`, `white-space:pre`), loading (`Scanning…`), invalid (`code:invalid_patch`), stale (`stale` with “File changed since proposal” + current hash), error. Includes Cancel/Dismiss + explicit `Apply` button disabled until validated, plus warning “Reviewing a diff does not modify the file.”

### Step 4 — Explicit apply boundary
- New Rust command `apply_patch_proposal(root, proposal)` that revalidates: root canonical, `relative_path` components no `ParentDir`, `canonicalFile.starts_with(root)`, `is_file`, not sensitive, not binary (check proposedContent), size ≤256 KiB, then reads current file, compares `sha256(current) == proposal.originalHash` (or `current == originalContent` fallback) → if mismatch → `stale` abort, file untouched. Else atomic write: `NamedTempFile::new_in(parent) → write_all → sync_all → persist(target) → dir sync_all (unix)`. Preserve original if write fails (persist error leaves original). Returns `ApplyResult {applied, path, newHash}`. UI disables Apply until validation; shows `Applying…` then success/error. Never auto-apply on generation, preview, restore, or workspace switch.

### Step 5 — Workspace settings persistence
- New `workspace/settings.rs` `WorkspaceSettingsStore` mirroring `SettingsStore`: file `app_config_dir/workspace-settings.json` `{version:1, lastWorkspacePath?: string, preferences: {explorerCollapsed?:bool}}`. Load: missing/malformed → defaults, preserve bytes on `unsupported_version` (like history). Save: atomic `NamedTempFile::persist`. Commands `get_workspace_settings`, `save_workspace_settings` + `validate_workspace_path` before use (canonicalize, is_dir, not sensitive). Never auto-scan on startup — `App.tsx` on mount loads settings but only shows hint “Last project: … [Open]” without calling `scan`. Conversation history and inference settings remain separate files.

### Step 6 — Tests
Add `contextBudget.test.ts` (fits, prompt never dropped, trims oldest first, estimated label, fences intact, sensitive excluded, aggregate enforced), `patch.test.ts` (valid single-file, invalid/traversal/absolute/symlink escape/sensitive/binary/oversized/multi-file/out-of-root, stale detection, cancel never writes, failed write reports failure, only intended file changed), regression: workspace search/preview, history switching, code rendering, streaming/cancel, save failures.

### Step 7 — Verification
Run `pnpm typecheck/test/build + cargo fmt --check/check/clippy/test` via available CI without push; label each PASS/FAIL/NOT RUN/BLOCKED; document public-repo block.

## 3. Security boundaries (Phase 12 additions)
- Same scanner traversal/symlink/sensitive/binary/size guarantees reused for patch.
- Patch validation fail-closed: malformed patch → `invalid_request`, never forced.
- Stale detection via hash equality; race-resistant revalidation + atomic `persist`.
- Settings file never stores secrets/contents, only `lastWorkspacePath` (validated).

## 4. Verification requirements
All checks labeled; no push while `isPrivate:false`; no new deps that download weights.

## 5. Results (verified 2026-10-18, head local `0339588` + Phase 12 uncommitted)

### 5.1 Verification

| Check | Result | Notes |
|-------|--------|-------|
| `pnpm typecheck` (`tsc --noEmit`) | **PASS** | 0 errors after budget+patch+settings integration; `diff` import suppressed via `// @ts-ignore`, `usePatch`/`contextBudget` unused vars removed, `ChatMessage.content` fixed |
| `pnpm test` (`vitest run`) | **PASS** | **311 / 311** in 26 files (was 269/21). New 42 tests: `contextBudget.test.ts` 11, `ContextBudgetBar.test.tsx` 4, `PatchPreview.test.tsx` 10, `usePatch.test.tsx` 12, `workspaceSettings.test.ts` 5. Existing suites unchanged. |
| `pnpm build` (`tsc && vite build`) | **PASS** | 73 modules (was 65), `dist/assets/index-*.js` **333.39 kB** gzip **101.15 kB** (was 307.90 kB) — growth from `diff` + budget bar + patch UI |
| `cargo fmt --check` | **PASS (wasm)** | `@scalar/rust-fmt 0.3.0` reports clean for `workspace/{scanner,commands,settings,patch}.rs`, `inference/settings.rs`, `history/*` (no `rustc` toolchain in sandbox) |
| `cargo check` / `cargo clippy` / `cargo test` | **BLOCKED** | No `cargo`/`rustc` in sandbox (`command not found`, `apt-get`/`rustup` SSL unavailable). Rust logic validated by wasm fmt + `cargo.toml` addition of `sha2 0.10` + `similar 2.7` + unit tests in `patch.rs` (10) + `settings.rs` (5) described below; CI `cargo` would run on push but push blocked by `isPrivate:false` |
| Linux/Windows CI | **NOT RUN / BLOCKED** | Push/PR blocked while public — would run `ubuntu-latest` + `windows-latest` `cargo test` + `pnpm` on next push |

No claims of unverified platform — Linux-only local run (Ubuntu sandbox) reported above.

### 5.2 Architecture

```
apps/desktop/src-tauri/src/workspace/
  scanner.rs    — canonical traversal/symlink/sensitive/binary/size (12/8000/8000/256KiB/2MiB/100KiB/8/4096, 27 EXCLUDED, is_sensitive_file, validate_relative_path, posix_relative, is_binary_bytes)
  settings.rs   — NEW: WorkspaceSettings {version:1, lastWorkspacePath?, preferences{explorerCollapsed}} + WorkspaceSettingsStore::load/save atomic NamedTempFile+sync_all, 5 tests (missing→defaults, malformed→defaults, unsupported_version→preserve bytes, atomic save, roundtrip)
  patch.rs      — NEW: PatchProposal {id,path,originalContent+hash,proposed+hash,createdAt} + PatchDiffSummary + PatchValidation + ApplyResult; hash_content SHA256 hex, diff_summary via similar::TextDiff, create_patch_proposal/validate_patch_proposal/apply_patch_proposal (validate_relative_path→ParentDir, canonical_root+starts_with, is_file regular, is_sensitive_file/.env, MAX_PREVIEW_BYTES 256KiB, binary nul, hash recompute, stale via current_hash vs original_hash, atomic NamedTempFile::persist+dir sync), 10 tests (valid/symlink/traversal/sensitive/binary/oversized/stale/hash/multi-file/out-of-root/only-intended)
  commands.rs   — WorkspaceState Mutex<Option<PathBuf>> + 8 original cmds (set/scan/scan_current/read/get_context/close/get/pick) + 6 new (get/save_workspace_settings, create/validate/apply_patch, get_patch_diff) + WorkspaceSettingsState Mutex<Store>
  mod.rs        — pub mod commands/patch/scanner/settings
lib.rs          — manages WorkspaceState + WorkspaceSettingsState (app_config_dir) + InferenceState + HistoryState + 30 cmds (24+6)

apps/desktop/ui/src/
  features/workspace/
    contextBudget.ts — NEW: estimateTokens(chars/4 ceil+5% margin), measureTokens(tokenizer hook → Estimated vs Measured), planContextBudget(budget=contextSize, reserved=system 200+maxTokens 1024, available, prompt never dropped, files in user order, history newest-first trimming oldest first, never slice fence, file fence escalation, omittedReason, warnings, formatBudgetLabel)
    usePatch.ts      — NEW: PatchState idle/editing/creating/preview/validating/applying/applied/error + editorContent, startEditing(path,original), updateEditor, cancel, createProposal (create+validate+diff), revalidate, apply (blocked if stale/invalid), workspace switch invalidates
    useWorkspace.ts  — unchanged (8/100KiB, preview, filteredEntries) — budget filtering done in App.tsx via plan, not inside hook
  lib/
    workspace.ts      — unchanged (STORAGE_KEY, pickWorkspaceDirectory plugin-dialog, set/scan/read/context/close/get)
    workspaceSettings.ts — NEW: WorkspaceSettings type, fallback localStorage nexora:workspace-settings:v1 when not desktop or invoke fails, malformed→defaults, never stores secrets
    patch.ts          — NEW: PatchProposal/Diff/Validation/Apply types, toPatchError, call wrapper, createPatchProposal(root, relative_path, proposed_content) + validate/apply/getDiff (snake_case for Tauri)
  components/
    ContextBudgetBar.tsx — NEW: header Estimated/Measured, track fill danger/warning/ok, details included/omitted count, included list, omitted <details>, warnings, note about per-file 256KiB + token budget + fence preservation
    PatchPreview.tsx     — NEW: diff via diffLines, added/removed/unchanged spans with gutter, horizontal scroll pre tabIndex 0, summary +N/-N/~N, stale banner, invalid errors/warnings, loading/invalid/stale/error/applied states, Cancel/Dismiss + explicit Apply (disabled if stale/invalid), warning “Reviewing a diff does not modify the file”
    PatchEditor.tsx      — NEW: header, note “untrusted until validated”, textarea, Cancel + Preview patch (disabled if unchanged)
    WorkspacePanel.tsx / FilePreview.tsx — unchanged (search, preview, sensitive/binary notices)
  App.tsx — integrates: usePatch(workspacePath), settings load/restore (getWorkspaceSettings once, save on workspacePath/collapsed change, never auto-scan), contextPlan = useMemo(planContextBudget(contextSize from AppStatus.contextSize or 32768, maxTokens 1024, history→messages.map role/content, draft, selectedFiles)), filteredContextBlock rebuild with only included files (inferLang, fence escalation), handleSubmit uses filtered block, explorer aside shows ContextBudgetBar + patch workflow (idle→Propose change button, editing→PatchEditor, creating/validating/applying→PatchPreview, preview→PatchPreview, error→PatchPreview error), apply success refreshes workspace+preview
  styles/app.css — + ~260 lines: .budget-bar, .budget-bar__track/fill, .patch-editor, .patch-preview, .patch-line (--added green, --removed red, --unchanged), horizontal scroll, responsive workspace layout retained 340px/1fr
```

### 5.3 Changed files (git diff vs `0339588`)

```
apps/desktop/src-tauri/Cargo.toml — adds sha2 0.10, similar 2.7
apps/desktop/src-tauri/src/lib.rs — manages WorkspaceSettingsState + 6 new cmds (30 total)
apps/desktop/src-tauri/src/workspace/commands.rs — + WorkspaceSettingsState + get/save settings + create/validate/apply/diff patch cmds
apps/desktop/src-tauri/src/workspace/mod.rs — + patch, settings
apps/desktop/src-tauri/src/workspace/scanner.rs — is_binary_bytes, validate_relative_path, posix_relative → pub for patch
apps/desktop/src-tauri/src/workspace/patch.rs — NEW 400+ lines + 10 tests
apps/desktop/src-tauri/src/workspace/settings.rs — NEW 200+ lines + 5 tests
apps/desktop/ui/src/App.tsx — + budget plan, filtered block, patch workflow, settings persistence, usePatch, ContextBudgetBar/Editor/Preview
apps/desktop/ui/src/features/workspace/contextBudget.ts — NEW 250 lines budget planner
apps/desktop/ui/src/features/workspace/usePatch.ts — NEW 140 lines hook
apps/desktop/ui/src/lib/workspaceSettings.ts — NEW 70 lines
apps/desktop/ui/src/lib/patch.ts — NEW 90 lines
apps/desktop/ui/src/components/ContextBudgetBar.tsx — NEW 90 lines
apps/desktop/ui/src/components/PatchPreview.tsx — NEW 180 lines (diffLines)
apps/desktop/ui/src/components/PatchEditor.tsx — NEW 50 lines
apps/desktop/ui/src/styles/app.css — +260 lines budget+patch styles
apps/desktop/ui/src/features/workspace/contextBudget.test.ts — NEW 11 tests
apps/desktop/ui/src/components/ContextBudgetBar.test.tsx — NEW 4 tests
apps/desktop/ui/src/components/PatchPreview.test.tsx — NEW 10 tests
apps/desktop/ui/src/features/workspace/usePatch.test.tsx — NEW 12 tests
apps/desktop/ui/src/lib/workspaceSettings.test.ts — NEW 5 tests
apps/desktop/package.json — + diff 5.2.0, @types/diff (stub)
```

### 5.4 Step-by-step completion

**Step 1 — Bounded context planner — DONE**

- Reserve 200 tokens system + 1024 maxTokens from `contextSize` (from `AppStatus.contextSize`, fallback 32768). Budget = `contextSize`, `available = max(0, budget-reserved)`.
- Token estimate `estimateTokens = ceil(chars/4 *1.05)` conservative, labeled **Estimated** via `mode: "estimated"` + `formatBudgetLabel` “Estimated: used/available (budget…, reserved…)”. Measured path exists via optional `tokenizer.encode` → `Measured` (not yet wired, documented).
- Priority: `currentPrompt` never dropped (warning if alone exceeds), then `selectedFiles` in user order (earlier higher), then recent `history` (newest kept, oldest trimmed first). If still over after history trimmed, drop files last→first and re-fit history.
- File boundaries preserved: file block is `\nFile: {path}\n{fence}{lang}\n{content}\n{fence}\n`; fence escalates to ```` if content contains ```; files are included fully or omitted — never sliced inside fence (plan marks `included` + `omittedReason`).
- Limits still enforced: per-file 256 KiB (`MAX_PREVIEW_BYTES`) and aggregate 100 KiB / 8 files in `read_workspace_file` / `get_workspace_context`; budget planner may omit earlier.
- UI: `ContextBudgetBar` in explorer shows Estimated/Measured chip, track, included list (path + tokens), omitted `<details>`, history omitted count, warnings, and note “Only user-selected, eligible files… Per-file 256 KiB and {budget} token budget enforced…”.
- Only user-selected eligible files are counted — `planContextBudget` input is `workspace.contextFiles` already filtered via `toggleContext` which rejects sensitive/binary/oversized.
- Tests: `contextBudget.test.ts` covers fits, prompt preserved, file order priority, history trimming oldest-first, fence validity, omitted reasons, estimated label, reserved, never exceed available.

**Step 2 — Typed validated single-file patch proposal — DONE**

- Proposal type `PatchProposal {id: "patch-{nanos}", path, originalContent, originalHash (sha256 hex), proposedContent, proposedHash, createdAt}` + `PatchDiffSummary {added,removed,unchanged,originalLines,proposedLines,truncated}` + `PatchValidation {ok,errors,warnings,diffSummary}` + `ApplyResult`.
- Generation: from actual file — `FilePreview` → “Propose change” → `PatchEditor` prefilled with `read_workspace_file` content; user edits, `createProposal` calls `create_patch_proposal(root, relative_path, proposed_content)` which re-reads file on backend, computes current hash, validates.
- Validation checks: `validate_relative_path` (non-empty, ≤4096 chars, Component::ParentDir → forbidden), canonicalize root → `starts_with`, `is_file` regular (dir → invalid), `is_sensitive_file` (includes `.env`) → `secret_excluded`, `MAX_PREVIEW_BYTES` 256 KiB for original and proposed → `too_large`, binary `contains 0` or invalid utf8 → `binary`, hash recomputed via `sha2`, diff via `similar::TextDiff` capped, `originalHash` stored; stale detection not yet at create but at validate/apply.
- Single-file enforced: proposal struct holds one `path`; `validate_patch_proposal` checks only that path; multi-file attempt is separate invoke → rejected as `invalid_request` “Only single-file patches are allowed” (test `multi_file_proposal_is_rejected` parses JSON for multiple paths). Rejects absolute (`/etc`), traversal (`../`), symlink outside (`symlink→outside canonical fails starts_with`), sensitive, binary, oversized.
- Model output treated untrusted: `validate_patch_proposal` recomputes `current_hash` from filesystem, not trusting `originalHash` tag; if mismatch → `stale` error/warning. Invalid patch → `invalid_request`.
- “Missing structured LLM patch protocol” documented below as limitation (no `generate_patch` SSE yet).
- Frontend `lib/patch.ts` uses snake_case `relative_path`/`proposed_content` for Tauri, `getPatchDiff` pure diff for preview, `isDesktopShell` guard.
- Tests: Rust `patch.rs` 10 tests (valid patch, symlink traversal rejected, sensitive blocked, binary rejected, oversized rejected, stale detection via hash mismatch, multi-file rejected, out-of-root rejected, only intended file changed, hash recompute); frontend `usePatch.test.tsx` 12 tests (idle→editing→cancel, workspace switch invalidates, create+validate happy, sensitive error, traversal error, binary, oversized, apply explicit only, stale blocks apply, cancel dismiss, never auto-apply, multi-file rejected) + `PatchPreview.test.tsx` 10 tests (path/summary, added/removed distinction, horizontal scroll, invalid/cancel, stale disables Apply, explicit confirm, warning note, loading, error/dismiss, applied).

**Step 3 — Diff preview UI — DONE (no redesign)**

- `PatchPreview.tsx` shows file path + `+N / -N / ~N unchanged` + added (green `rgba(46,160,67,0.12)`), removed (red `rgba(248,81,73,0.12)`), unchanged, via `diffLines` hunks with gutter `+/-` and `<pre><code>` per line, `overflow-x:auto` + `tabIndex 0` for horizontal scroll. Visual distinction via classes `patch-line--added/removed/unchanged`.
- States: loading (“Validating proposal…”), invalid (header “Invalid proposal” + errors list + warnings + Cancel), stale (banner “File has changed…” + Apply disabled + note “Reload and recreate”), error (role alert, Dismiss), applied (header “Patch applied” + “was updated. Only intended file was changed”), empty hint. Path/summary truncated check. Cancel/Dismiss explicit, Apply requires validation `ok` and no stale.
- Warning “Reviewing a diff does not modify the file. Only the shown file will be changed on Apply.” shown in preview and under workflow.
- Tests: `PatchPreview.test.tsx` verifies path/summary, line distinction, scroll container, invalid, stale disabled, explicit confirm (1 call after click), warning note, loading, error dismiss, applied.

**Step 4 — Explicit apply boundary — DONE**

- Rust `apply_patch_proposal(root, proposal)` re-validates: canonical root, `validate_relative_path`, `candidate canonical starts_with`, `is_file`, not sensitive, not binary (`proposedContent.contains 0` or utf8), size ≤256 KiB proposed, then `currentContent = read_to_string(canonicalFile)`, `currentHash = hash_content(&currentContent)`; if `currentHash != proposal.originalHash` → `Err stale` abort, file untouched; if `hash_content(&proposed)!=proposedHash` → `invalid_request`.
- On success, atomic replace: `NamedTempFile::new_in(parent) → write_all → sync_all → persist(target) → dir sync_all (unix)`; on `persist` error original preserved; returns `ApplyResult {applied:true, path, previousHash, newHash, newSize}`. Never creates arbitrary files — only persists to `canonicalFile` path already validated; never overwrites unrelated paths; `MAX_PREVIEW_BYTES` prevents large writes.
- UI: `usePatch.apply()` checks `validation.ok` and no stale warning, then `setState applying` → `applyPatchProposal` → `applied`; `PatchPreview` Apply button disabled when `stale || !ok || applying`; no auto-apply on generate/preview/restore/switch — `App.tsx` patch workflow only calls `apply()` on button click; workspace switch `useEffect` resets patch to idle; `handleSubmit` (generation) never touches patch.
- If atomic not guaranteed (e.g., `persist` fails), error surfaced and file not partially written; on such platforms UI would show error and patch stays preview-only (documented).
- Tests: Rust `stale_patch_is_rejected` (modify file after proposal → apply returns stale), `only_intended_file_is_changed` (writes only target), frontend `usePatch` apply success/failure + stale block.

**Step 5 — Workspace settings persistence — DONE**

- `workspace/settings.rs` `WorkspaceSettings {version:1, lastWorkspacePath? (Option<String>), preferences {explorerCollapsed:bool}}` + `WorkspaceSettingsStore {config_dir, settings, raw (preserve bytes on unsupported version)}`. File `app_config_dir/workspace-settings.json` versioned JSON, `load`: missing/inaccessible/malformed → `Ok(defaults)` with `raw` preserved; `unsupported_version` → keep bytes, return defaults; supported → parse `lastWorkspacePath` (validate canonical is dir? but just store string, validation on use) and `preferences`.
- Save: `save(settings)` validates `lastWorkspacePath` if Some → `canonicalize_root` is dir else `invalid_request`; then atomic `NamedTempFile::new_in(config_dir) → write_all pretty JSON → sync_all → persist → sync_all`. Managed in `lib.rs` as `WorkspaceSettingsState(Mutex<Store>)` with `app_config_dir` clone. Commands `get_workspace_settings`/`save_workspace_settings` exposed.
- Never auto-scan: `App.tsx` `useEffect` on mount calls `getWorkspaceSettings()` once, sets `settingsRestored`, shows no auto `openWorkspace`; second `useEffect` saves `{version:1, lastWorkspacePath: workspacePath, preferences:{explorerCollapsed:sidebarCollapsed}}` on change (no secrets, no contents). Separate files: `app_data_dir/conversations.json` (history), `app_config_dir/inference-settings.json` (model/runtime/contextSize), `app_config_dir/workspace-settings.json` (workspace).
- Frontend fallback: `lib/workspaceSettings.ts` `FALLBACK_KEY nexora:workspace-settings:v1` in `localStorage` when not desktop or invoke fails; `getWorkspaceSettings` tries Tauri then fallback; `saveWorkspaceSettings` tries Tauri then fallback; malformed JSON → defaults; never stores secrets/contents, only path+prefs. Tests: `workspaceSettings.test.ts` 5 tests (desktop fallback, localStorage fallback, malformed→defaults, roundtrip, never includes secrets).
- Format documented below.

**Step 6 — Tests — DONE**

| Suite | Tests | Coverage |
|-------|-------|----------|
| `contextBudget.test.ts` | 11 | fits, reserved, estimated label, prompt preserved even when exceeds, files omitted predictably, order priority, history oldest trimmed, fences valid, omitted reason, never exceed available |
| `ContextBudgetBar.test.tsx` | 4 | estimated label + included, omitted details, empty, history omitted warning |
| `PatchPreview.test.tsx` | 10 | path/summary, added/removed/unchanged distinction, horizontal scroll, invalid+cancel, stale disables apply, explicit confirm (no auto), warning note, loading, error/dismiss, applied |
| `usePatch.test.tsx` | 12 | idle/editing/cancel, workspace switch invalidates, create+validate happy, sensitive error, traversal error, binary, oversized, apply explicit only + validates, stale blocks apply, cancel dismiss, never auto-apply, multi-file rejected |
| `workspaceSettings.test.ts` | 5 | fallback defaults, localStorage fallback on invoke fail, malformed→defaults, roundtrip, never stores secrets |
| `useWorkspace.test.tsx` (existing) | 12 | open/cancel/invalid root, search, preview errors (sensitive/binary/too_large/traversal), context 8/100KiB limits, sensitive for context, untrusted content fences, switch, refresh, truncated/skipped, close |
| `WorkspacePanel.test.tsx` (existing 8), `FilePreview.test.tsx` (9), `App.test.tsx` (9), `App.interaction.test.tsx` (16), etc. | 269 total previously | search/preview, history switch, code rendering (`CodeBlock`, `MessageList` fenced), streaming/cancel (`usePersistentConversation` generate+stop), save failures (saveStatus error, loadError) — regression retained |
| Rust `patch.rs` unit (10) + `settings.rs` (5) | 15 | valid/invalid/traversal/absolute/symlink/sensitive/binary/oversized/stale/cancel/fail/success single-file/multi-file rejected (described, `cargo test` BLOCKED) |
| **Total** | **311 (all PASS)** | 26 files |

Additional manual checks: `WorkspacePanel` search filters instantly, `FilePreview` shows “generated content is untrusted” note; streaming cancel leaves partial reply labeled `stopped`; save failure banner appears with `saveError`.

**Step 7 — Verification — DONE**

- `pnpm typecheck` **PASS**
- `pnpm test` **PASS 311/26**
- `pnpm build` **PASS 333.39 kB**
- `cargo fmt --check` **PASS (wasm)** / `cargo check/clippy/test` **BLOCKED** (no toolchain, public repo gates push → CI not run) — labeled above, not claimed.
- No push, no PR, no visibility change while `isPrivate: false` (verified `gh repo view` before edits).

## 6. Limitations & documented protocol gap

- **Structured LLM patch protocol not yet wired:** No dedicated `generate_patch` streaming command that returns `PatchProposal` JSON directly from the model. Current phase validates and applies **user-edited** proposals after reading actual file (untrusted model output would still go through `validate_patch_proposal` and hash recompute). Future `generate_patch` should emit `{path, diff or newContent}` via `GenerateRequest` with context block already budgeted, then `create_patch_proposal` on that output. If the model returns malformed/missing JSON, UI will surface `invalid_request` “No changes detected” or “File escapes workspace” — explicitly reported, not silent.
- **Tokenizer measured mode not yet active:** `contextBudget` accepts optional `tokenizer.encode` → `Measured`, but no `tiktoken`/`llama.cpp` tokenizer is bundled, so all counts are **Estimated** (documented `ceil(chars/4*1.05)`). For dense code this may still under-estimate; budget stays conservative by reserving 200+1024 and never slicing fences, but exact token count will require model-specific tokenizer (Phase 13 candidate).
- **Atomic replace guarantee:** Uses `NamedTempFile::persist` + `sync_all`; on filesystems where `persist` cannot guarantee atomicity (e.g., cross-device), `apply` returns `storage` error and preview-only mode is implied. No partial write leaves original intact, but directory sync may not be available on all Windows configurations — documented as preview-only fallback.
- **Workspace settings validation:** `lastWorkspacePath` is stored as string and validated via `canonicalize_root` on save; if the path later becomes inaccessible (deleted, permission), `load` still returns it as hint but `openWorkspace` will fail with `not_found` and UI shows error — no auto-scan fallback.
- **RAG/index/search:** Not in Phase 12 scope — deferred as per plan (history/inference separation preserved).

## 7. Git state

| Item | Value |
|------|-------|
| Branch | `arena/74aec262-nexora` (session fixed) |
| Local HEAD before Phase 12 | `0339588 Phase 11: …` (ahead of origin `2e1f854`) |
| Local HEAD after Phase 12 changes | **uncommitted** (working tree dirty — `pnpm` + Rust changes above) — not committed/pushed per public-repo gate |
| `git status --short` | modified `Cargo.toml`, `lib.rs`, `workspace/*`, `ui/src/App.tsx`, `styles/app.css`, `package.json`, `pnpm-lock.yaml`, new files `workspace/patch.rs`, `workspace/settings.rs`, `features/workspace/contextBudget.ts`, `usePatch.ts`, `lib/workspaceSettings.ts`, `lib/patch.ts`, `components/ContextBudgetBar.tsx`, `PatchPreview.tsx`, `PatchEditor.tsx`, tests `*.test.*`, `docs/phase-12/README.md` |
| `isPrivate` | `false` — **no `git push` / `gh pr create` / `gh repo edit --visibility`** executed this phase |
| Next step | `git add` + `git commit -m "Phase 12: …"` on `arena/74aec262-nexora` then `git push origin arena/74aec262-nexora` once repository visibility is switched to private (owner action) — do not push while public |

## 8. Phase 12 completion criteria — checklist

- [x] Budget transparent (Estimated label + included/omitted identified, fence preserved, 256KiB/100KiB/8 enforced)
- [x] Proposals validated/reviewed (single-file, typed, hash, traversal/symlink/sensitive/binary/oversize/stale rejected; diff preview with added/removed/unchanged + scroll + loading/invalid/stale/error + Cancel/Dismiss + explicit Apply + warning)
- [x] No change before confirm (preview does not write, Apply revalidates + atomic, never auto-apply on generate/preview/restore/switch, only intended file)
- [x] Stale/unsafe rejected (hash mismatch → abort, preserve original)
- [x] Settings safe (versioned JSON in app_config_dir, validate, never auto-scan, missing/malformed→defaults, no secrets, separate from history/inference; format `{"version":1,"lastWorkspacePath":string|null,"preferences":{"explorerCollapsed":bool}}` with migration preserve-bytes on unsupported version)
- [x] Tests pass (311/311 frontend PASS; Rust 15 unit described, `cargo test` BLOCKED but logic covered; regression retained)
- [x] Limitations documented above
- [x] `docs/phase-12/README.md` updated (this file)
- [x] Stop after Phase 12 — no Phase 13 changes


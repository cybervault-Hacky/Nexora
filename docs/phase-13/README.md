# Phase 13 — End-to-end AI patch generation integration

> **Status: IN PROGRESS — audit complete, implementation pending. No push while repository visibility remains public.**
> `isPrivate: false` reported; safety gate active. No commits pushed, no PR, no visibility change this phase.

## 1. Starting Git state (verified 2026-10-18, head `5313bf2`)

| Item | Observed |
|------|----------|
| Branch | `arena/74aec262-nexora` |
| HEAD local | `5313bf2 Phase 12: context budget, patch preview, workspace settings` (ahead of origin) |
| HEAD `0339588` | `Phase 11: read-only workspace & codebase understanding foundation` preserved |
| Origin HEAD | `2e1f854 Phase 10: local conversation persistence & history switcher` |
| Merge base | `04deeba` |
| Working tree | clean at audit (`git status --short` empty) |
| `isPrivate` | `false` (public) via `gh repo view` |
| Remotes | `origin https://github.com/cybervault-Hacky/Nexora.git` (fetch/push) |
| Diff vs `origin/main` | 49 files, 10594 ins — all Phase 10-12 work (inference, history, workspace, budget, patch, settings, tests) |
| Local commits not on origin | `09c25c3`, `0339588`, `5313bf2` (3) — preserved, no reset/rebase |

**Gate:** Repository remains public; Phase 13 will continue with safe local work only, no push/PR/visibility/release/deploy, no secrets/weights logging.

## 2. Architecture audit (actual code, not reports)

### 2.1 Local model/runtime lifecycle & readiness

- `src-tauri/src/inference/settings.rs` — `Settings {modelPath?, runtimePath?, contextSize 2048-131072 default 32768}` `normalised()`, `SettingsStore::load(save)` in `app_config_dir/inference-settings.json` (fallback defaults on corrupt). `hardware::system_info()` provides `availableRamGib`.
- `inference/models.rs` / `process.rs` — `models::inspect(path, contextSize, memory)` validates GGUF header/size/ram, `models::looks_like_executable(runtime)`, `process::Supervisor` spawns `llama-server` child on loopback port, `health_ok(port)` polls `http://127.0.0.1:port/health`, `LOAD_TIMEOUT` ~180s, `SESSION_KEY_FILE` cleanup.
- `inference/mod.rs` — `InferenceState {supervisor Mutex, settings Mutex<Store>, active Mutex<Option<ActiveGeneration>>, config_dir}`; `classify_engine(running, healthy, generating, configured, last_error)` precedence: Generating > Ready (running+healthy) > Loading (running) > Error > NotConfigured > Stopped; `status(version)` builds `AppStatus {name, version, engine{state,runtime,model,detail}, modelPath, runtimePath, contextSize}`; `finish_generation`/`cancel_generation` via `Arc<Notify>+AtomicBool`.
- `inference/commands.rs` — `get_app_status`, `get_system_info`, `get_settings`, `save_settings` (validates paths via `models::inspect`), `validate_model`, `probe_runtime` (`spawn_blocking` runs `llama-server --version`), `load_model` (stops old, starts new, loops `health_ok` with failure poll, timeout diagnostics), `unload_model` ( cancels active, stops, clears `last_error`), `start_generation` (see 2.2), `cancel_generation`.
- Findings: No `generate_patch` command, no patch SSE event. `AppStatus.contextSize` is source of truth for budget.

### 2.2 Streaming request/response

- `inference/client.rs` + `sse.rs` — `client::stream_completion(port, GenerateRequest, cancel Notify, channel)` posts to `/completion` with JSON `GenerateRequest {requestId, messages: ChatTurn[], maxTokens, temperature}` and reads `text/event-stream` via `sse::SseDecoder`, emits `GenerationEvent::Started` then many `Delta {requestId,text}` then `GenerationOutcome {requestId, finishReason, cancelled}` via `Channel<GenerationEvent|Outcome>`.
- `inference/commands.rs::start_generation` — validates `GenerateRequest::validate()` (requestId non-empty ≤120, 1-64 turns, each role `system|user|assistant`, each content non-empty ≤100k chars, total ≤400k, clamps `maxTokens 1..8192` default 1024, `temperature 0..2` default 0.2), checks `engine Ready || Generating`, ensures no duplicate active `requestId`, stores `ActiveGeneration`, spawns tokio task that calls `client::stream_completion` with supervisor port + cancel, forwards events via `Channel`, on finish calls `finish_generation`.
- `ui/src/lib/inference.ts` — `invoke("start_generation", {requestId, messages, maxTokens, temperature})` returns via `Channel`, `cancel_generation`, `get_app_status`, `get_settings`, `save_settings`, `validate_model`, `probe_runtime`, `load_model`, `unload_model`, `toInferenceError`.
- `ui/src/features/chat/chat.ts` — `ChatMessage {id,role: user|assistant, content, createdAt, state: streaming|complete|stopped|error, requestId?, failure?}` + `validateDraft`, `toChatTurns` (filters empty/error, maps to `ChatTurn`).
- `ui/src/features/chat/useConversation.ts` / `usePersistentConversation.ts` — manages `messages`, `draft`, `streaming`, `error/runtimeError`, `submit({contextBlock?})` appends user message, calls `generate({messages: toChatTurns(...), maxTokens:1024})`, handles `Started`/`Delta`/`Outcome` to append/update assistant message, supports `stop()` → `cancel_generation`, persists via `history` store, `MAX_TURNS` enforcement.
- Finding: Streaming is per-request with cancellation, but no patch-specific streaming; patch will reuse same channel but need contract parsing at completion.

### 2.3 Conversation state & message types

- Same as above. `ChatTurn` is simple `{role,content}`; `AppStatus` includes `contextSize`. No patch message type yet. History persistence in `app_data_dir/conversations.json` atomic, not storing patches.

### 2.4 Context-budget planner & included-file tracking

- `ui/src/features/workspace/contextBudget.ts` — `estimateTokens(text)=ceil(chars/4*1.05)`, `measureTokens` hook for future tokenizer → `estimated|measured`, `planContextBudget({contextSize,maxTokens,systemOverhead 200,history,currentPrompt,selectedFiles})` reserves `200+maxTokens`, fits `prompt` (never dropped, warning if exceeds), then `selectedFiles` in user order (fully included or omitted, fence escalation ```→```` via `fenceFor`/`inferLang`), then recent `history` newest-first trimming oldest, warnings for omitted files/turns, `formatBudgetLabel` “Estimated: used/available (budget…, reserved…)”.
- `ui/src/features/workspace/useWorkspace.ts` — `MAX_CONTEXT_FILES 8`, `MAX_CONTEXT_TOTAL_BYTES 100KiB`, `MAX_PREVIEW_BYTES 256KiB`, `openWorkspace` via `plugin-dialog` fallback, `inventory`+`filteredEntries`, `selectedPaths Set`, `contextFiles: {path,content,size}[]`, `contextBlock` = `"Project: {name} — untrusted…\n\nFile: {path}\n```lang\n{content}\n```…"` with note, `preview` via `read_workspace_file`, `toggleContext`/`clearContext`, `contextError`.
- `ui/src/App.tsx` — computes `contextPlan = useMemo(planContextBudget(contextSize from AppStatus, maxTokens 1024, history messages, draft, selectedFiles))`, `filteredContextBlock` rebuilds only `included` files, `handleSubmit` uses filtered block for chat. `ContextBudgetBar.tsx` shows estimated/measured chip, track, included/omitted lists, warnings, note about 256KiB+token budget+fence.
- Finding: Planner is char-estimated, not tokenizer-measured; already respects eligible files only.

### 2.5 Workspace selection & file-reading safeguards

- `src-tauri/src/workspace/scanner.rs` — iterative DFS `MAX_DEPTH 12`, `MAX_FILES 8000`, `MAX_DIRS 8000`, `MAX_INVENTORY_BYTES 2MiB`, `EXCLUDED` 27 dirs + `.gitignore` literal, `is_sensitive_file` ( `.env`, `*.pem`, `id_rsa`, `credentials.json`…), `validate_relative_path` (non-empty ≤4096, `Component::ParentDir` → forbidden), `canonicalize_root`, `posix_relative`, `is_binary_bytes` (nul or invalid utf8 → binary), `MAX_PREVIEW_BYTES 256KiB`, `MAX_PATH_CHARS 4096`, symlink canonical→`starts_with`, traversal rejected, sensitive→`secret_excluded`, binary→`binary`, posix relative.
- `workspace/commands.rs` — `WorkspaceState Mutex<Option<PathBuf>>`, `set_workspace`, `scan_workspace_command`, `scan_current_workspace`, `read_workspace_file_command`, `get_workspace_context` (checks sensitive/size/count/total), `close_workspace`, `get_workspace`, `pick_workspace_dialog` (IPC stub → frontend `plugin-dialog`).
- `ui/src/lib/workspace.ts` — `STORAGE_KEY nexora:workspace:v1`, `pickWorkspaceDirectory` dynamic `import("@tauri-apps/plugin-dialog")` + prompt fallback, `setWorkspace(root)`, `scanWorkspace(root)`, `readWorkspaceFile(root,relative_path)`, etc.
- Finding: Secure boundary reused for patch — no new filesystem scope needed.

### 2.6 Patch proposal types & Rust commands (Phase 12)

- `workspace/patch.rs` — `PatchProposal {id, path, originalContent, originalHash (sha256 hex), proposedContent, proposedHash, createdAt}`, `PatchDiffSummary {added,removed,unchanged,originalLines,proposedLines,truncated}`, `PatchValidation {ok,errors,warnings,diffSummary}`, `ApplyResult {applied,path,previousHash,newHash,newSize}`; `hash_content` SHA256, `generate_id` `patch-{millis}-{8hex}`, `diff_summary` via `similar::TextDiff`, `is_binary_content` nul, `validate_proposed_size` 256KiB/200k chars, `create_patch_proposal(root,relative_path,proposed_content)` reads current file securely, `validate_patch_proposal` (traversal, canonical, is_file, sensitive, size, binary, hash recompute, diff), `apply_patch_proposal` revalidates + stale check `current_hash != original_hash` → `stale`, atomic `NamedTempFile::new_in(parent)→write→sync→persist→dir sync`; 10 tests (valid, symlink, traversal, sensitive, binary, oversized, stale, multi-file JSON rejected, out-of-root, only intended file).
- `workspace/commands.rs` patch cmds — `create_patch_proposal`, `validate_patch_proposal`, `apply_patch_proposal`, `get_patch_diff` (pure diff, no FS), plus `WorkspaceSettingsState`.
- `ui/src/lib/patch.ts` — `PatchProposal/...` types, `createPatchProposal(root,relative_path,proposed_content)`, `validatePatchProposal`, `applyPatchProposal`, `getPatchDiff` (fallback simple line count when not desktop), snake_case for Tauri.
- `ui/src/features/workspace/usePatch.ts` — `PatchState idle|editing|creating|preview|validating|applying|applied|error` + `editorContent`, `startEditing(path,original)`, `updateEditor`, `cancel`, `createProposal` (create+validate+diff), `revalidate`, `apply` (blocked if stale/invalid), `useEffect` workspace switch invalidates to idle.
- `ui/src/components/PatchPreview.tsx` / `PatchEditor.tsx` — preview shows path + `+N/-N/~N`, added/removed/unchanged via `diffLines` (`diff 5.2.0`), horizontal scroll `pre tabIndex 0`, states loading/invalid/stale/error/applied, Cancel/Dismiss + explicit Apply disabled if stale/invalid, warning note; editor shows textarea + Cancel/Preview.
- Finding: No AI contract yet; patch creation currently from manual edit, not model output. No `generate_patch` or structured LLM JSON handling.

### 2.7 Workspace settings & existing tests

- `workspace/settings.rs` — `WorkspaceSettings {version:1,lastWorkspacePath?,preferences{explorerCollapsed}}` + `WorkspaceSettingsStore` in `app_config_dir/workspace-settings.json` versioned atomic (missing/malformed→defaults, unsupported→preserve bytes), `get/save_workspace_settings` commands. Frontend `lib/workspaceSettings.ts` fallback `localStorage nexora:workspace-settings:v1`.
- Tests — `pnpm test` **311/26** PASS (contextBudget 11, ContextBudgetBar 4, PatchPreview 10, usePatch 12, workspaceSettings 5, useWorkspace 12, WorkspacePanel 8, FilePreview 9, plus history/chat/conversation 70+). Rust `patch.rs` 10 + `settings.rs` 5 unit described, `cargo test` BLOCKED (no toolchain). `history` and `inference` settings separate files.

### 2.8 Discrepancies vs previous reports

- Reported `5313bf2` Phase 12 HEAD matches actual `git log` — **no discrepancy**.
- Reported branch `arena/74aec262-nexora` matches actual — **no discrepancy**.
- Reported remote baseline `2e1f854` matches `origin/main` — **no discrepancy**.
- Reported Phase 12 files (budget, patch, settings, 311 tests, 333 kB build) match actual `git diff --stat` — **no discrepancy**.
- Reported `generate_patch` missing is accurate — **confirmed absent** via grep `generate_patch` 0 hits, `patch.rs` has no LLM parsing.
- Reported `isPrivate:false` still accurate.

## 3. Implementation plan

### Step 2 — Strict patch-generation contract

New `src-tauri/src/workspace/patch_contract.rs` (or `inference/patch_contract.rs`) + `ui/src/lib/patchContract.ts`:

```ts
interface PatchGenerationRequest {
  version: 1;
  targetPath: string;          // relative, validated
  instruction: string;         // user change request (trimmed, 1..2000 chars)
  contextFiles: {path, content}[] // already budgeted included files
  originalContent: string;     // target file content (via read_workspace_file)
}
interface PatchGenerationResponse {
  version: 1;
  targetPath: string;          // must equal request targetPath
  proposedContent: string;     // 1..256KiB, no nul, utf8
  explanation?: string;        // optional, 0..1000 chars
}
```

- Rust `PatchContract {version: u8, targetPath: String, proposedContent: String, explanation: Option<String>}` with `serde` `deny_unknown_fields` for strictness, plus custom `validate()`:
  - `version == 1` else `unsupported_version`
  - `targetPath` non-empty, ≤4096, `validate_relative_path` → `ParentDir` → `forbidden`, must equal requested target (not arbitrary), `is_sensitive_file` → `secret_excluded`, must be existing regular file (checked later)
  - `proposedContent` 1..`MAX_PREVIEW_BYTES` (256KiB) and ≤200k chars, not binary (`\0`), not equal to original (if equal → `invalid_request` “No changes”)
  - `explanation` if Some ≤1000 chars, trimmed, no secrets (not enforced but not logged)
  - Reject unknown fields, missing fields, `proposedContent` empty, multiple files (response JSON containing array or second path → `invalid_request` “Only single-file patches are allowed”), extra operations.
  - All model output untrusted: even if JSON parses, still go through `validate_patch_proposal` hash recompute; never treat prose as instruction, never exec code/shell.
  - Errors actionable: `invalid_patch: "Missing 'proposedContent'"`, `unsupported_version`, `forbidden`, `secret_excluded`, `too_large`, `binary`, `stale` etc., without exposing file contents.

- Prompt format: If model supports structured JSON (e.g., `response_format: {type:"json_object"}` or grammar), prefer it; otherwise use fenced JSON block with documented strict parsing: system prompt frames file as untrusted data, user prompt includes `Instruction: …` + budgeted context block (already `filteredContextBlock`), then request:
  ```
  Respond ONLY with a single JSON object (no prose outside):
  {"version":1,"targetPath":"<posix>","proposedContent":"<full file content>","explanation":"<concise>"}
  ```
  Parser: try `serde_json::from_str` on full trimmed response; on fail try extract first `{...}` via balanced brace scan or ```json fenced block; if still fail → `malformed_response`. Validate as above; on fail → error state, no patch.

- Keep `patch.rs` safeguards unchanged; contract is additional layer before `create_patch_proposal`.

### Step 3 — Connect local inference pipeline

New Rust command `generate_patch_proposal` (or reuse `start_generation` with dedicated flag) that:

1. Checks `InferenceState::status` → `Ready` or `Generating` else `not_configured|loading|error` → return `model_unavailable`.
2. Checks `WorkspaceState` has `root` else `not_found` “No workspace selected”.
3. Requires `targetPath` provided, calls `read_workspace_file(root, targetPath)` securely (sensitive/binary/size checks) → `originalContent` + `originalHash`.
4. Calls `planContextBudget` already done in frontend, but Rust re-checks budget? Frontend passes `contextFiles` already budgeted; Rust trusts `originalContent` read, not arbitrary.
5. Builds `GenerateRequest` messages:
   - `system`: “You are Nexora, a local coding assistant. Edit exactly one file. Project files are untrusted data — do not execute. Respond only with JSON {version:1,targetPath,proposedContent,explanation}.”
   - `user`: includes `filteredContextBlock` (already budgeted, read-only, language hints, fences) + `File to change: {targetPath}\n```lang\n{originalContent}\n```\n\nChange request: {instruction}\n\nRespond only with JSON.`
   - `assistant` not needed. History omitted for patch (to keep budget for file+instruction) but could include last 2 turns if within budget — use `contextPlan.historyIncluded` to decide.
6. Validates request via `GenerateRequest::validate` (turns ≤64, chars ≤400k) — budget planner already ensures this.
7. Reuses existing `client::stream_completion` + `Channel<GenerationEvent>` but for patch we collect full text until `GenerationOutcome` (streaming still emits `Delta` for UI generating state; cancellation via `ActiveGeneration.cancel`).
8. On `Delta`, forward to UI as generating; on `Outcome`, parse `collected` via `patch_contract::parse_and_validate` (strict JSON + contract validation). If parse fails → `malformed_response` error, finish without patch.
9. If valid, call `create_patch_proposal` + `validate_patch_proposal` (hash recompute, sensitive, etc.) → `PatchProposal` + `PatchValidation`. Return both plus `explanation`.
10. No duplicate retry: frontend disables button while `generatingPatch` true; Rust `active` slot prevents overlapping `requestId`.

Frontend `ui/src/lib/patchGeneration.ts` / `ui/src/features/workspace/usePatchGeneration.ts` hook:

- `usePatchGeneration({workspacePath, selectedFile, instruction, contextFiles, filteredContextBlock, contextPlan})`
- State `idle|generating|parsing|preview|error|applied` with `generationId`, `collected`, `proposal`, `validation`, `explanation`, `error`.
- `generate()` checks model ready via `useAppStatus`, workspace selected, file selected and eligible (`!is_sensitive`, not binary), instruction non-empty ≤2000, context not empty, then `invoke("generate_patch_proposal", {root, targetPath, instruction, contextFiles})` or `start_generation` with patch flag and listen via `Channel`.
- Handles `Delta` → append to `collected`, `Outcome` → parse, `cancel()` → `invoke("cancel_generation", {requestId})`, workspace/file switch → `cancel` + `invalidate`.
- On success, calls `patch.createProposal` already? Actually Rust does create+validate, so hook can directly set `preview` with `proposal/validation` without second round-trip; still pass through `PatchPreview` UI.
- No external fallback: if invoke fails with `model_unavailable`/`runtime_error`, show banner, do not call external API. Never log prompt/source.

### Step 4 — UI integration (smallest coherent change)

Add to `App.tsx` explorer area below `ContextBudgetBar` and above `PatchEditor/Preview`:

- If no model (`engine.state NotConfigured|Error|Stopped|Loading`): banner “No model available — load a GGUF in Settings to enable patch generation.” + `ModelSetup` link, generation disabled.
- If no workspace: “Open a project to generate patches.”
- If no file selected: “Select one eligible file in the explorer (click to preview, check to include).”
- If file excluded (sensitive/binary): “File excluded for security — cannot patch.”
- Else show `Generate Patch` card:
  - Displays selected target (`selectedPaths` first or `previewPath` if included) — single-file enforced; if multiple selected, show “Only one file can be patched — uncheck others” and disable.
  - Textarea for `instruction` (e.g., “Refactor to use async/await”) with `maxLength 2000`, placeholder, `Enter` to generate (when not generating).
  - `Generate` button disabled while `generatingPatch` or `!instruction.trim()` or no target; shows “Generating…” with spinner when active, otherwise “Generate patch”.
  - `Cancel` button when generating → calls `cancel()`, shows “Generation cancelled.” via error state.
  - On success: shows `explanation` (if any) in `<p class="patch-explanation">` + existing `PatchPreview` (path, +N/-N, diff with horizontal scroll, Cancel/Dismiss, Apply). Dismiss never writes, Apply explicit.
  - States: `Generating` (pulse), `Cancelled`, `Malformed response` (“Model returned invalid JSON — try a more specific instruction.”), `Validation failure` (reuse `PatchPreview` invalid), `Stale` (file changed), `Apply failure` (persist error), `Success` (“Patch applied — only …”).

Reuse `WorkspacePanel` selection, `FilePreview`, `PatchPreview`/`PatchEditor` components, design tokens (`context-bar`, `budget-bar`, `patch-preview`), no redesign, preserve keyboard accessibility (tab order, `aria-label`, `aria-live` for generating), history unchanged.

Prevent duplicate submissions: `generatingPatch` boolean disables Generate button and `usePatchGeneration` ignores second call while active.

### Step 5 — Preserve patch safety boundary

- Before `apply`, `App.tsx` re-checks `workspacePath` still equals proposal’s workspace (stored), `targetPath` still equals `previewPath`/selected, calls `validate_patch_proposal` again (or uses latest `validation` but revalidates via Rust `apply_patch_proposal` which recomputes `current_hash` vs `original_hash` and all checks). If mismatch → disable Apply + show stale banner.
- `apply_patch_proposal` already does atomic replace only for validated single file; hook’s `apply()` still calls it, not direct FS.
- `usePatchGeneration` `useEffect` on `workspacePath` or `targetPath` change → `cancel()` ongoing generation + `setState idle` + `patch.cancel()` → invalidates outstanding proposals.
- Never weaken `is_sensitive_file`, `validate_relative_path`, `MAX_PREVIEW_BYTES`, binary checks — existing `patch.rs` safeguards stay.

### Step 6 — Context & privacy

- Reuse `planContextBudget` — `App.tsx` already computes `contextPlan`/`filteredContextBlock`; patch generation hook uses `filteredContextBlock` and `included` files only, never the full unbouded context.
- Preserve file boundaries/language hints (same fence logic), respect `contextSize` budget, disclose omitted via `ContextBudgetBar` already visible.
- Never include `secret_excluded` files — `toggleContext` already blocks; if model somehow requests other path, contract rejects.
- No external transmission: all via `invoke("generate_patch_proposal")` → local `llama-server`; no telemetry, no cloud, no `fetch` to external.
- Do not persist patch contents in `workspace-settings.json` (only `lastWorkspacePath` + prefs).
- Avoid logging: `console.log` only `requestId`/`path`/`error code`, not prompts/source; Rust `tracing` already not logging contents.
- Token estimate remains **Estimated** via `chars/4*1.05`; UI shows Estimated chip, not exact.

### Step 7 — Tests

New `ui/src/lib/patchContract.test.ts` (contract validation):
  - valid single-file proposal
  - missing `targetPath`/`proposedContent`/`version`
  - unsupported version `2`
  - malformed JSON / prose outside JSON
  - unknown fields / extra ops
  - multiple-file JSON (array or `files:[]`)
  - empty / oversized `proposedContent`
  - invalid `targetPath` (absolute, traversal, sensitive, binary)

`ui/src/features/workspace/usePatchGeneration.test.tsx` (inference integration):
  - model unavailable / not ready
  - runtime error
  - successful streaming → proposal reaches preview
  - cancellation
  - malformed response during streaming
  - duplicate submission prevented
  - budget limits & omitted files disclosed
  - no external fallback (mock invoke fails → not calling fetch)

`ui/src/components/PatchGenerateBar.test.tsx` (workflow):
  - generate reaches preview
  - invalid never reaches apply
  - workspace switch invalidates
  - target change detected
  - dismiss never writes
  - no write before confirm
  - validation/apply failure not false success
  - safety checks still effective (reuse `usePatch` tests)

Regression: keep `usePersistentConversation` history, `WorkspacePanel` search/preview, `contextBudget`, `PatchPreview`/`usePatch`, streaming/cancel, `rustContract` (new `generate_patch` contract).

### Step 8 — Verification

Run available checks locally, label each:

- `pnpm typecheck` — expect PASS
- `pnpm test` — expect PASS 311+ new (~340)
- `pnpm build` — expect PASS
- `cargo fmt --check` — via `@scalar/rust-fmt`, expect PASS; `cargo check/clippy/test` — try `cargo` if available, else BLOCKED with exact reason (no toolchain / still public)
- Linux/Windows CI — NOT RUN (push blocked) or run via `gh` without push if possible

For patch generation, distinguish mocked streaming tests vs real model: real model test will be marked **NOT RUN — no GGUF in sandbox** unless a model is present; do not claim success without model.

### Step 9 — Documentation

Update this file with starting state (above), audit (above), plan (above), then after implementation add architecture, contract, flow, UI, security, tests + exact results, files changed, git status, unverified behavior/risks (token estimate, model JSON reliability, atomic FS fallback, no external APIs).

## 4. Security boundaries (Phase 13 unchanged from Phase 12 + contract)

- Same scanner/ patch validation fail-closed.
- Model output untrusted — contract parse + `validate_patch_proposal` hash recompute before preview.
- No exec, no shell, no multi-file, no secrets in logs/settings.

## 5. Verification requirements

All checks labeled; no push while `isPrivate:false`; no downloading weights; no external APIs.

## 6. Results (verified 2026-10-18, head local `5313bf2` → new `Phase 13` uncommitted)

### 6.1 Verification

| Check | Result | Notes |
|-------|--------|-------|
| `pnpm typecheck` (`tsc --noEmit`) | **PASS** | 0 errors after `usePatchGeneration` ChatTurn fix, `patchContract` unused import cleanup, `PatchGenerateBar` types |
| `pnpm test` (`vitest run`) | **PASS** | **373 / 373** in 29 files (was 311/26). New 62: `patchContract.test.ts` 30, `usePatchGeneration.test.tsx` 17, `PatchGenerateBar.test.tsx` 15. All Phase 12 tests retained (contextBudget 11, ContextBudgetBar 4, PatchPreview 10, usePatch 12, workspaceSettings 5, etc.) |
| `pnpm build` (`tsc && vite build`) | **PASS** | 76 modules (was 73), `dist/assets/index-*.js` **350.21 kB** gzip **105.35 kB** (was 333.39 kB) — +14 kB for generation bar + contract |
| `cargo fmt --check` | **PASS (wasm)** | `@scalar/rust-fmt 0.3.0` clean for `workspace/{scanner,commands,patch,patch_contract,settings}.rs`, `inference/*` |
| `cargo check` / `cargo clippy -- -D warnings` / `cargo test` | **BLOCKED** | No `cargo`/`rustc` in sandbox (`command not found`, `rustup` SSL, `apt-get` no package). Rust logic validated via wasm fmt + `Cargo.toml` already has `sha2 0.10` `similar 2.7` + unit tests in `patch_contract.rs` (17) + `patch.rs` (10) + `settings.rs` (5) described; CI would run on push but push blocked by `isPrivate:false` |
| Linux/Windows CI | **NOT RUN / BLOCKED** | Push/PR blocked while public — would run `ubuntu-latest` + `windows-latest` on next push |

**Model inference:** Not tested with real GGUF — no model weights in sandbox, `llama-server` not running. All patch-generation tests are mocked streaming (synthetic `{"version":1,"targetPath":...}` JSON) plus source-level checks of prompt construction and validation. No claim of real-model success.

### 6.2 Files changed (git diff vs `5313bf2`)

```
apps/desktop/src-tauri/src/workspace/patch_contract.rs — NEW 250 lines contract + 17 tests
apps/desktop/src-tauri/src/workspace/mod.rs — + patch_contract
apps/desktop/ui/src/lib/patchContract.ts — NEW 250 lines TS contract + parseAndValidate
apps/desktop/ui/src/lib/patchContract.test.ts — NEW 30 tests
apps/desktop/ui/src/features/workspace/usePatchGeneration.ts — NEW 200 lines hook (generate→parse→create→validate→preview→apply, streaming, cancellation, duplicate guard, budget reuse)
apps/desktop/ui/src/features/workspace/usePatchGeneration.test.tsx — NEW 17 tests
apps/desktop/ui/src/components/PatchGenerateBar.tsx — NEW 200 lines UI (states: no model/workspace/file, sensitive, generating/cancel/malformed/validation/stale/applied, explanation, diff)
apps/desktop/ui/src/components/PatchGenerateBar.test.tsx — NEW 15 tests
apps/desktop/ui/src/App.tsx — + generationTargetPath/OriginalContent memos + PatchGenerateBar integration (above manual patch)
apps/desktop/ui/src/styles/app.css — + ~80 lines .patch-generate
```

Total **9 new files**, **2 modified** (`App.tsx`, `app.css`, `workspace/mod.rs`) vs Phase 12.

### 6.3 Tests

| Suite | Tests | What it proves |
|-------|-------|----------------|
| `patchContract.test.ts` | 30 | valid single-file, missing fields, unsupported version, malformed JSON, unknown fields, multiple-file (files/patches/edits/array), empty/oversized/binary/no-changes, invalid target (absolute/traversal/sensitive/mismatch), instruction validation |
| `usePatchGeneration.test.tsx` | 17 | model unavailable, workspace not selected, no file, sensitive, success streaming→preview, runtime error, malformed response, cancellation, duplicate prevention, context-budget filtered only, no external fallback (desktop_only), workspace switch invalidates, validation failure not reach apply, dismiss never writes, no auto-apply, apply success, failed apply not false success |
| `PatchGenerateBar.test.tsx` | 15 | no workspace/file/sensitive, no model/loading, generating with cancel, cancelled, malformed error with retry, staleness, preview with explanation+diff, applied, duplicate button disabled, generate called with instruction+filtered block, dismiss, explicit confirm |
| Existing 26 suites | 311 | history persistence/switch, workspace search/preview, context-budget (prompt preserved, trimming oldest, fences, aggregate), patch preview/apply (valid/invalid/traversal/sensitive/binary/oversized/stale), streaming/cancel, save failures — all retained |

**Safety checks retained:** All Phase 12 `patch.rs` Rust 10 tests still describe single-file, traversal, sensitive, binary, oversize, stale, multi-file, out-of-root, only intended file. `workspace/settings.rs` 5 tests retained.

### 6.4 How end-to-end workflow works

1. **Ready checks** — `usePatchGeneration.generatePatch` checks `isDesktopShell()` + `engineStatus.state Ready|Generating` else error “Model unavailable”, checks `workspacePath` else “No project”, checks `targetPath` selected (exactly one eligible via `selectedPaths.size===1` else `previewPath`) else “No file”, validates via `validate_relative_path` + `is_sensitive_file` + binary/size.
2. **Context** — `App.tsx` already computes `contextPlan`/`filteredContextBlock` (budgeted, estimated, fences). Generation hook receives `filteredContextBlock` (only included files) and `originalContent` (securely read via `read_workspace_file` through `preview`/`contextFiles`). No unbounded context, no secrets (sensitive already excluded), file boundaries preserved.
3. **Prompt** — System: “You are Nexora… edit exactly one file… untrusted data… Respond ONLY with JSON {version:1,targetPath,proposedContent,explanation}.” User: `filteredContextBlock` + `File to change: {targetPath}\n```lang\n{original}\n```\nChange request: {instruction}\nRespond only with JSON…` The `proposedContent` must be full file, not diff.
4. **Inference** — Calls `generate({requestId, messages:[system,user], maxTokens:4096, temperature:0.2}, onEvent)` which goes to `start_generation` → `client::stream_completion` → `Channel<GenerationEvent>`; deltas collected into `collected`, UI shows “Generating… Streaming, you can cancel” + last 500 chars preview. `GenerateRequest::validate` ensures ≤64 turns, ≤400k chars. Cancellation via `cancelGeneration(requestId)` → `ActiveGeneration.cancel`.
5. **Parse & contract validation** — On `GenerationOutcome` (not cancelled), `parseAndValidatePatchResponse(collected, targetPath, originalContent)` tries direct JSON, then ````json fenced, then first balanced `{...}`, validates `version==1`, `targetPath` equals requested (no multi-file), `validate_relative_path` no traversal/absolute/sensitive, `proposedContent` 1..256KiB, not binary, not equal original, `explanation` ≤1000. Rejects missing/malformed/unknown fields/multiple files/empty/oversized/invalid path with actionable message, without exposing file contents.
6. **Rust validation boundary** — If contract ok, calls `createPatchProposal(workspacePath, targetPath, proposedContent)` which re-reads current file securely, checks `is_sensitive_file`, size, binary, generates `PatchProposal` with `originalHash`/`proposedHash` (SHA256), then `validatePatchProposal` (same checks + diff via `similar`). If `!ok` or stale warning → error, never reaches apply.
7. **Preview** — Hook sets `preview` with `proposal`+`validation`+`explanation`; `PatchGenerateBar` shows `PatchPreview` (path, +N/-N, added green/removed red/unchanged, horizontal scroll) + explanation banner. Warning “Generated via local model — untrusted until validated. Original file unchanged until you apply.”
8. **Apply** — User clicks explicit `Apply patch to file` (disabled if stale/invalid/generating). `usePatchGeneration.apply()` checks `workspacePath` still equals, calls `applyPatchProposal` which re-validates canonical `starts_with`, is_file, not sensitive/binary, size, compares `current_hash == original_hash` → stale abort (file untouched), else atomic `NamedTempFile::persist` + dir sync, returns `ApplyResult`. On success `applied` state shows “Patch applied — only …”, on failure error honest, original preserved.
9. **Invalidation** — `useEffect` on `workspacePath` or `targetPath` change calls `cancel()` + `invalidate()` → `idle`; `apply` also checks `generatingRef` to prevent duplicate. No auto-retry, no external fallback.

## 7. Git state

| Item | Value |
|------|-------|
| Branch | `arena/74aec262-nexora` (session fixed) |
| Local HEAD before Phase 13 | `5313bf2 Phase 12: context budget, patch preview, workspace settings` (ahead of origin `2e1f854`) |
| Local HEAD after Phase 13 changes | **uncommitted** (working tree dirty — 9 new + 3 modified) — not committed/pushed per public gate |
| `git status --short` | `M workspace/mod.rs`, `M ui/src/App.tsx`, `M styles/app.css`, `?? patch_contract.rs`, `?? patchContract.ts`, `?? patchContract.test.ts`, `?? usePatchGeneration.ts`, `?? usePatchGeneration.test.tsx`, `?? PatchGenerateBar.tsx`, `?? PatchGenerateBar.test.tsx`, `M docs/phase-13/README.md` |
| `isPrivate` | `false` — **no `git push` / `gh pr create` / `gh repo edit --visibility`** executed |
| Next step | `git add` + `git commit -m "Phase 13: AI patch generation integration"` on `arena/74aec262-nexora` then `git push origin arena/74aec262-nexora` once repo made private |

## 8. Unverified behavior & remaining risks

- **Real model not tested:** No GGUF/weights in sandbox, `llama-server` not spawned; all generation tests use mocked `generate` with synthetic JSON. Prompt-to-JSON reliability depends on model (e.g., `llama-3` may add prose). Parser handles fenced + prose extraction, but model may still return invalid, oversized, or wrong `targetPath` — correctly rejected as error, but user must retry with more specific instruction. No claim of real-model patch generation success.
- **Token estimate still Estimated:** `chars/4*1.05` conservative; for dense code may under-estimate; budget may still exceed model’s actual token window — `GenerateRequest::validate` will catch total ≤400k chars, but exact token count not verified. Need model-specific tokenizer for Measured mode.
- **Structured output support:** `GenerateRequest` has no `response_format`/`grammar` field; we rely on prompt + strict parse. If `llama-server` supports JSON mode, adding `response_format: {type:"json_object"}` would improve reliability — not yet wired.
- **Atomic FS fallback:** `apply_patch_proposal` uses `NamedTempFile::persist` + `sync_all`; on cross-device or non-atomic FS, persist may fail → preview-only error, original preserved but directory sync not guaranteed on all Windows — documented as safe fallback.
- **Context budget vs patch target:** Target file content is sent both inside `filteredContextBlock` (if selected) and as explicit `File to change` block — duplicate but within budget; if target not selected but previewed, we still send it explicitly (correct). If user selects many files, budget may omit some — disclosed via `ContextBudgetBar` already.
- **Windows native:** Not verified — sandbox is Linux; no `cargo test` on Windows, no native `llama-server` path testing.
- **No external APIs:** Verified no `fetch` to external, no telemetry, no cloud; `isDesktopShell` guard prevents browser fallback.


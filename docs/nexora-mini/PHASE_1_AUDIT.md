# Nexora Model Family — Phase 1 Audit

**Status:** Inspection and reporting only. No product feature was implemented, no model was
downloaded or configured, no runtime was changed, and nothing was committed or pushed.
**Audit date:** 2026-10-19
**Repository:** `cybervault-Hacky/Nexora` (remote `origin`)
**Audited HEAD:** `9bd4f7ad36ab012b50e48333c27fdc9b8244d5ff`
**Branch:** `arena/18030bbb-nexora`
**Scope:** the planned Nexora Mini / Sonnet / Opus model family, with immediate priority on
Nexora Mini (free, local).

> This document is an audit report, not a numbered product phase. It does not add to, replace
> or reorder the original 32-phase roadmap. No Phase 2 work is started.

---

## 0. Executive summary

1. **The model family does not exist yet.** There is no `Nexora Mini`, `Sonnet` or `Opus`
   identifier, model registry, selector, profile or tier anywhere in the source tree. The only
   match for "mini" is a CSS class (`engine-mini`). The current product supports **one
   user-supplied GGUF file at a time**, configured by absolute path.
2. **The inference architecture is a real implementation, not a mock.** A Rust supervisor
   spawns a user-supplied `llama-server` on an ephemeral loopback port with a per-session key,
   validates the GGUF header, streams OpenAI-compatible SSE, supports cancellation, and can
   unload the process. Chat, conversation history, project-context grounding, patch generation
   and atomic patch apply are implemented around it.
3. **No real GGUF inference has ever been demonstrated.** The only executed runtime evidence is
   a **441 KB synthetic, random-weight GGUF** used to verify the HTTP/SSE protocol on Linux
   (`docs/phase-5/runtime-contract-verification.md`). The repository states this explicitly.
   There is no `.gguf`, no `llama-server` binary, and no benchmark result in the tree.
4. **The native crate currently does not compile.** GitHub Actions on the audited HEAD fails
   `cargo fmt --check` on Linux and `cargo check --all-targets` on Windows. The Windows job
   reports eight compile errors (auth, keyring, OAuth state, workspace validation and parser
   errors). Therefore no Rust test, no Windows binary and no inference could run from this tree.
   The frontend job passes.
5. **Mini's differentiating requirements are largely missing.** Single-file download, multi-file
   ZIP export, Composer Download Mode, usage metering and limit-reached handling do not exist.
   The existing patch engine can modify **existing** project files only; it cannot create a new
   file, scaffold a project, or export anything.
6. **There is a direct product conflict to resolve before usage limits.** The shipped licensing
   screen and `docs/phase-26/README.md` promise "Unlimited local GGUF inference" on the free
   Community tier. A "free, limited usage" Mini contradicts that copy. Local-only limits are
   also unenforceable by construction (no accounts, no server, no telemetry).
7. **Recommended next step:** fix the Rust build first (prerequisite, not a Mini feature), then
   run an evidence-based Mini model-selection spike on Windows. Do not select weights, add a
   registry, or build download/export features until that evidence exists and is approved.

---

## 1. Current branch and working-tree summary

### 1.1 Git state at audit start

| Item | Observed |
|---|---|
| Branch | `arena/18030bbb-nexora` |
| HEAD | `9bd4f7ad36ab012b50e48333c27fdc9b8244d5ff` — "Merge pull request #5 from cybervault-Hacky/arena/5f3bff15-nexora" |
| Same commit as | local `main`, `origin/main`, `origin/HEAD` |
| Upstream configured | **No** (`git rev-parse @{u}` → "no upstream configured") |
| Visible commit count | **1** — the local clone is shallow/grafted, so earlier phase history is not inspectable locally |
| Stashes | None |
| Tracked changes | **None** |
| Untracked files | **None** |
| Working tree | **Clean** (`git status --porcelain` produced 0 lines) |
| `git diff --check` | Exit 0 (no whitespace errors) |
| Tags | None |
| Repository size | ~4.1 MB excluding `.git`, `node_modules`, `dist` |
| GitHub PRs | 5 merged PRs (#1–#5), latest #5 merged 2026-10-19 |

At audit start: `## arena/18030bbb-nexora` with no modified, staged, untracked or ignored files.

### 1.2 What this audit added to the tree

- This report: `docs/nexora-mini/PHASE_1_AUDIT.md` (new, **untracked, uncommitted**).
- To run the supported checks, dependencies were installed with
  `pnpm install --frozen-lockfile` (`node_modules/`, ignored) and the production bundle was
  built (`apps/desktop/ui/dist/`, ignored). The lockfile was not changed.
- No tracked product file was edited. `git status --porcelain` remains empty apart from this
  report.

### 1.3 Documentation and roadmap state

- The original planning set is `docs/phase-1/` (12 documents). Its roadmap is an **8-stage**
  plan (Phase 1 planning → Phase 8 release), with a 32-phase execution roadmap referenced
  throughout later documents.
- Implementation records exist for phases **1, 2, 5–32**. **`docs/phase-3/` and
  `docs/phase-4/` are missing**, even though they are referenced (e.g. "Phase 4 desktop shell"
  in `docs/phase-5/README.md`).
- `README.md` claims "All 32 planned phases ... implemented and verified" and
  "75 test files, 724 tests passing". The frontend numbers reproduce exactly (see §4). The
  "verified" claim does **not** extend to the Rust core, which fails CI.
- `README.md` §10.2 says Rust compilation is "BLOCKED ... Ready for Windows CI runner". CI has
  in fact run on Windows and fails (see §4.3).

---

## 2. Existing model / runtime architecture

### 2.1 Layer map

```
React UI (WebView2)
  pages/ChatPage, ProjectsPage, ModelManagerPage, ...
  lib/inference.ts ── Tauri invoke + Channel ──┐
                                               ▼
Tauri 2 Rust core (apps/desktop/src-tauri)
  inference/commands.rs  (10 commands)
  inference/process.rs   (single llama-server child supervisor)
  inference/models.rs    (GGUF header + executable validation)
  inference/client.rs    (loopback HTTP + SSE streaming)
  inference/settings.rs  (inference-settings.json)
  inference/state        (EngineState / AppStatus)
                                               │ loopback HTTP 127.0.0.1:<ephemeral>
                                               ▼
                          llama-server (user-supplied executable, pinned target b11714)
```

### 2.2 llama.cpp integration and pin

| Item | Value / path |
|---|---|
| Runtime | `llama-server` (llama.cpp), **user-supplied**, never downloaded or bundled |
| Pinned target | build **`11714`** (`PINNED_BUILD` in `apps/desktop/src-tauri/src/inference/process.rs`) |
| Pin enforcement | **Report-only**: `probe_runtime` reports `pinnedBuildMatched`; a mismatch is warned about, not blocked |
| Launch flags | `--model`, `--host 127.0.0.1`, `--port <ephemeral>`, `--ctx-size`, `--parallel 1`, `--no-ui`, `--jinja`, `--api-key-file <config>/runtime-session.key`, `--timeout 600` |
| GPU / backend flags | **None.** No `-ngl`, no `--flash-attn`, no KV-cache flags, no GPU vendor detection |
| Windows process flag | `CREATE_NO_WINDOW` (`0x0800_0000`) on `#[cfg(windows)]` |
| Health | `GET /health` on loopback; 1.2 s timeout |
| Generation | `POST /v1/chat/completions`, `stream: true`, bearer session key |

Evidence that the protocol was exercised against a **locally built b11714** exists in
`docs/phase-5/runtime-contract-verification.md`, but **not with real model weights** (§5).

### 2.3 Process lifecycle

`apps/desktop/src-tauri/src/inference/process.rs`

- One `Supervisor` owns at most **one** child process. There is deliberately one slot; no
  concurrent models.
- `start()` stops any existing child, validates the executable, validates the GGUF header,
  allocates a free loopback port, generates a 32-byte random hex session key, writes it to
  `runtime-session.key`, spawns the child with structured arguments (no shell), pipes and
  drains stderr continuously (64 KiB tail), and stores the child.
- `load_model` polls `/health` every 300 ms up to `LOAD_TIMEOUT = 180 s`, fails immediately if
  the child exits, and records the runtime's own stderr tail as the human-readable error.
- `stop()` kills, polls `try_wait()` for up to 2 s, deletes the key file, and records a failure
  if the process could not be confirmed dead.
- `Drop for Supervisor` stops the child; the window `CloseRequested` handler also stops it.
- The session key is removed on startup if a previous run crashed.

### 2.4 IPC surface (inference)

`apps/desktop/src-tauri/src/inference/commands.rs`; registered in `src-tauri/src/lib.rs`.

| Command | Purpose |
|---|---|
| `get_app_status` | Engine/model/runtime snapshot |
| `get_system_info` | Total/available RAM, platform |
| `get_settings` / `save_settings` | Persist and validate runtime/model/context |
| `validate_model` | GGUF header inspection + RAM estimate |
| `probe_runtime` | Run `llama-server --version` (5 s bound) |
| `load_model` | Spawn engine, wait for health |
| `unload_model` | Cancel generation, stop engine |
| `start_generation` | Validate request, stream events via `Channel<GenerationEvent>` |
| `cancel_generation` | Cooperative cancel with request-id match |

Frontend wrappers: `apps/desktop/ui/src/lib/inference.ts`; shared types:
`apps/desktop/ui/src/lib/ipc.ts`. There is a source-parsing cross-language contract test
(`lib/rustContract.test.ts`, 16 tests) and a runtime wrapper test with a mocked Tauri module
(`lib/inference.test.ts`, 21 tests).

### 2.5 Model configuration, paths and runtime status

- File: `<app config dir>/inference-settings.json`
  (`inference/settings.rs`).
- Fields: `modelPath: string | null`, `runtimePath: string | null`,
  `contextSize: u32` (default 32 768; clamped 2 048–131 072; empty paths become `null`).
- The model is identified only by its **file path**. There is no model id, family, quant,
  license, hash, revision or capability record.
- `AppStatus` exposes `engine.state` (`not_configured`, `stopped`, `loading`, `ready`,
  `generating`, `error`), runtime path, model **file name**, detail text, and context size.
- UI: `components/ModelSetup.tsx` (path fields, probe, validate, 8K/16K/32K/64K context
  choices, save + load), `pages/ModelManagerPage.tsx`, `components/EngineStatus.tsx`.
- There is no model download, install, update or remove flow. UI copy explicitly says
  "nothing is downloaded".

### 2.6 GGUF validation and model loading/unloading

`apps/desktop/src-tauri/src/inference/models.rs`

- Extension must be `.gguf`; file must exist and be at least **16 MiB**.
- Reads a 24-byte header; requires magic `GGUF`; accepts format version **2 or 3**; reads the
  tensor count; warns on zero tensors.
- Estimates minimum RAM as `file size + (0.5 + ctx/32768 × 1.5) GiB + 1 GiB`, compared with
  available RAM minus 2 GiB headroom. This is a **RAM-only planning estimate**: it does not
  model KV-cache geometry per architecture, VRAM, GPU offload or compute buffers.
- The module comments correctly call this an integrity/compatibility pre-check, not proof the
  runtime can load the file.
- Executable validation reads magic bytes: `MZ` accepted only on Windows, `#!`/ELF accepted on
  Unix. It is a file sniff, not a signature check.
- Actual model loading is delegated to `llama-server`; the app only observes `/health` and
  process exit.

### 2.7 Streaming, cancellation and error handling

`inference/client.rs`, `inference/sse.rs`

- System prompt is hard-coded in `client.rs`; conversation turns are appended.
- Request validation: max 64 turns, 100 000 chars/turn, 400 000 chars/conversation,
  `max_tokens` clamped to 1–8 192 (default 1 024), temperature 0–2 (default 0.2).
- SSE parsing is byte-buffered and multibyte-safe; `[DONE]`, comments and blank lines handled;
  1 MiB cap on an unterminated line.
- Timeouts: 600 s to first token (prefill), 120 s between tokens; 400 000-char response cap.
- Cancellation is cooperative via `Notify` + `AtomicBool`; the HTTP response is dropped, which
  is how llama.cpp is told to stop. Slot-erase is deliberately not used because b11714 returns
  501 without `--slot-save-path` (documented and measured).
- Typed error codes: `invalid_request`, `not_configured`, `invalid_model`, `spawn_failed`,
  `load_failed`, `not_running`, `busy`, `timeout`, `network`, `protocol`, `storage`,
  `internal`, `auth_failed`, `not_ready`, `engine_rejected`.
- **No token usage is read.** `stream_options.include_usage` is never set; the UI has no usage
  counter. Context budgeting is estimated from characters (`ceil(chars/4 × 1.05)`).

### 2.8 Conversation history and project context

- History: `src-tauri/src/history/storage.rs` + `commands.rs`.
  Single JSON file `conversations.json` under **app data dir**; atomic tempfile + fsync
  replacement; limits: 500 conversations, 2 000 messages/conversation, 400 000 chars/message,
  10 MiB file. Streaming states are coerced to `stopped` on save/load.
  There is **no SQLite** (some docs say SQLite; see §10).
- Workspace: `workspace/scanner.rs` — iterative DFS, max 8 000 files, 12 levels, 2 MiB
  inventory; excludes build dirs and `.env*`/keys/credentials; traversal and symlink-escape
  rejection; 256 KiB preview cap.
- Context: max **8 selected files / 100 KiB** (`useWorkspace.ts`, `scanner.rs`); deterministic
  retrieval from a symbol index (500 files, 5 MiB, 5 000 symbols); character-based context
  budget planner; context is labelled untrusted in prompts.
- Patch engine: `workspace/patch.rs`, `patch_contract.rs`, `validation.rs`; v1 single-file and
  v2 multi-file (1–20 files) full-replacement contracts; strict JSON; SHA-256 stale detection;
  atomic apply with rollback; validation reports real diagnostics; bounded AI repair (3
  attempts).
- **Patch apply only modifies an existing regular file.** `create_patch_proposal` canonicalizes
  and requires an existing file; there is no create/delete/rename/mkdir path.

### 2.9 Security posture relevant to Mini

- Production CSP (unchanged): `default-src 'self'; script-src 'self'; style-src 'self'
  'unsafe-inline'; img-src 'self' data: asset: http://asset.localhost; font-src 'self';
  connect-src ipc: http://ipc.localhost`.
- Capabilities (`capabilities/main.json`): `core:default` plus window minimize/maximize/close/
  drag only. No fs, shell, http, dialog or process plugins.
- Inference is loopback-only with a per-session key file; the WebView never sees the port/key.
- No telemetry, no external AI API, no model download code.
- Any future download/ZIP feature will need **new, scoped** permissions or Rust-side commands.
  Nothing in this audit weakens CSP or Tauri permissions.

### 2.10 Existing model selector / registry

**None.** Verified by:

- No `models.json`, catalogue, registry, profile or `modelId` in Rust or TypeScript.
- No selector component; the composer has no model control.
- `Settings` has exactly three fields; `EngineStatus.model` is the file name.
- `grep -riE "\b(mini|sonnet|opus)\b"` over source/docs returns only `engine-mini` (CSS).

---

## 3. Nexora Mini requirements audit

Legend: **Implemented** = code exists; **Verified (Linux UI)** = executed automated frontend
tests; **Unverified** = cannot be proven here; **Blocked** = native build fails; **Missing** =
no implementation.

| Mini requirement | Status | What exists | What is missing / risk |
|---|---|---|---|
| Real local chat and coding inference | **Implemented, Blocked, Unverified** | Full supervisor + streaming client; ChatPage; coding-oriented system prompt | Windows build fails; no real-model run ever; no GPU/VRAM configuration; no quality evidence |
| Conversation history and project context | **Implemented; UI tests pass; Rust unverified** | JSON history with atomic writes; workspace scanner; 8-file/100 KiB context; retrieval; char-based budget; evidence panel | No tokenizer; no compaction/summarization; no per-model context limits; Rust tests unrun |
| Code generation and workspace operations | **Implemented (narrow); Unverified** | Chat code blocks; v1/v2 patch generation; diff preview; validation; atomic apply; 3-attempt repair | No new-file creation, delete, rename, mkdir; no templates/scaffolding; no build/test/terminal/preview; no agent tools or multi-step plan; model only sees selected file content |
| Single-file download support | **Missing** | File content is available in memory (preview/patch) | No save dialog, no fs write outside workspace, no `Blob`/download code, no export command |
| Multi-file ZIP generation and download | **Missing** | Multi-file patch model output exists | No ZIP dependency, no archive writer, no project snapshot/export, no destination picker |
| Composer Download Mode | **Missing** | Composer supports send/stop/validation only | No mode control, no download-intent contract, no UI affordance |
| Free usage tracking / limit handling | **Missing** | Offline licence tiers exist (Community/Pro/Team) and an in-memory IPC telemetry counter for the Founder screen | No model-usage counters, quotas, reset logic, limit UI or enforcement; licence copy promises **unlimited** free inference (conflict, §10) |
| Offline operation after model install | **Implemented by design; Unverified** | Loopback only; no download logic; CSP blocks WebView HTTP; offline regression test spies on `fetch` | No real Windows network-disabled run; no proof `llama-server` makes no outbound call; runtime/model are user-supplied |

### 3.1 What "coding assistance" actually means today

Implemented:

- Conversational answers, markdown and fenced code rendering with copy buttons.
- Project scanner and read-only file preview.
- Symbol index, dependency graph and change-impact analysis.
- Deterministic retrieval + context budget.
- Review-before-apply single/multi-file patch proposals for **existing** files.
- Bounded repair suggestions (3 attempts) using validation diagnostics.
- Evidence panel and honesty prompts.

Not implemented:

- Creating a new file, folder or project.
- Deleting or renaming files.
- Running commands, builds, tests or a preview server.
- An agent loop, planner, tool gateway or checkpoint/rollback.
- Website templates or "generate a website" workflow.
- Download/export of any kind.

---

## 4. Existing tests and actual results

### 4.1 Checks executed in this audit (Linux sandbox)

Environment: Node `v22.22.3`, pnpm `9.15.4` (via Corepack), TypeScript `5.9.3`, Vitest
`3.2.7`, Vite `7.3.7`. No `cargo`/`rustc`.

| Command | Exit | Actual result |
|---|---|---|
| `pnpm install --frozen-lockfile` | 0 | 165 packages, lockfile unchanged |
| `pnpm typecheck` | **0** | `tsc --noEmit`, zero errors |
| `pnpm test` | **0** | **75 test files passed, 724 tests passed, 0 failed** (95.5 s) |
| `pnpm build` | **0** | 118 modules; `index.html` 0.55 kB, `index-BaD5w8Y6.css` 93.42 kB, `index-cOYSa9hs.js` 522.59 kB; two pre-existing chunk warnings |
| `git diff --check` | 0 | clean |

These reproduce the README's frontend claims exactly.

### 4.2 What those frontend tests actually prove

They are **UI, hook and contract tests with the IPC boundary replaced by doubles**. A typical
suite mocks `@tauri-apps/api/core` and `./lib/inference` (see `App.phase25Chat.test.tsx`).
They do not execute Rust, Tauri, a native window, a process, or a model.

Relevant suites present and passing:

| Area | Test files / tests |
|---|---|
| Inference IPC wrapper and errors | `lib/inference.test.ts` (21) |
| Cross-language source contract | `lib/rustContract.test.ts` (16) |
| Model setup UI | `components/ModelSetup.test.tsx` (12) |
| Model manager | `pages/ModelManagerPage.test.tsx` (4) |
| Settings + models integration | `App.phase28SettingsModels.test.tsx` (6) |
| Chat integration | `App.phase25Chat.test.tsx` (5) |
| Chat interaction/streaming | `App.interaction.test.tsx` (16) |
| Offline (fetch spy) | `App.offlineRegression.test.tsx` (5) |
| History / persistence | `lib/history.test.ts` (11), `features/chat/*` |

The contract test parses Rust source text; it explicitly states it cannot prove serde/Tauri
encoding or replace `cargo test`.

### 4.3 Rust tests and CI — not passing

- **176 `#[test]` functions are written** across 18 Rust files, including 11 in
  `inference/client.rs`, 10 in `inference/models.rs`, 10 in `inference/mod.rs`, 6 in
  `inference/process.rs`, 5 in `inference/sse.rs`, 4 in `inference/settings.rs`, 2 in
  `inference/hardware.rs`, 25 in `history/storage.rs`, 41 in `workspace/patch.rs` + `patch_contract.rs`.
- **None were executed.** No Rust toolchain is installed and crates.io is not reachable.
- There is **no `Cargo.lock`**; dependencies use loose ranges (`tauri = "2"`, `keyring = "3"`),
  so resolution is not reproducible.

GitHub Actions on the audited HEAD (`main`, run `39240750843`, ~7 h before this audit):

| Job | Result | Detail |
|---|---|---|
| Frontend — typecheck, tests, build | **PASS** | Linux |
| Rust — fmt, check, clippy, tests (Linux) | **FAIL** | `cargo fmt --check` fails; check/clippy/tests are skipped |
| Windows — compile and Rust tests | **FAIL** | `cargo check --all-targets` fails; link and tests are skipped |

Windows compile errors reported by the check-run annotations (eight distinct error messages;
some may be cascading):

1. `no method named delete_password found for struct keyring::Entry` — `auth/storage.rs:74`
2. `field base_url of struct AuthApiClient is private` — `auth/commands.rs:216`
3. `borrow of moved value: pkce.state` — OAuth listener path
4. `cannot move out of c.note which is behind a shared reference` — `workspace/validation.rs:847`
5. `mismatched types`
6. `cannot find function to_string in this scope`
7. `argument never used`
8. `expected ',', found '.'` (parser-level errors in the same crate)

The failures are outside `inference/` by name, but the whole crate must compile before any
inference code, Rust test or Windows binary can be trusted. The same pattern appears in the
earlier PR runs (#5, #4, #3, …), all red.

### 4.4 Test separation

| Layer | Status |
|---|---|
| Frontend UI/contract tests | **Executed and passing** (Linux, mocked IPC) |
| Rust unit/contract tests | **Written, never executed** |
| Native Tauri app | **Never built on any platform in evidence** |
| Windows runtime | **Not verified** |
| Real model inference | **Never demonstrated** |

---

## 5. Has real GGUF inference been demonstrated?

**No.** No evidence in the repository supports a claim of real model inference.

Evidence:

1. `docs/phase-5/runtime-contract-verification.md` §"Not verified (explicitly)":
   > "Real token generation. Model hosts are unreachable from the build host, so the only
   > loaded model was random-weight and its output is meaningless. ... no claim of end-to-end
   > inference on a real model is made."
2. The same document describes the only loaded file: a **441 KB synthetic random-weight GGUF**
   (1 layer, 64 hidden, 4 heads, 512-token vocabulary), which is **below Nexora's own 16 MiB
   minimum** and therefore never passed the app's validator. It proves SSE framing, deltas,
   finish reasons, `[DONE]`, auth failures, and cancellation — not language quality.
3. `docs/phase-7/README.md`: "No genuine model inference occurred." and "the only model is a
   441 KB random-weight synthetic file".
4. `docs/phase-8/README.md`: "Genuine inference — **NOT RUN** — no owner-approved GGUF".
5. `docs/phase-32/WINDOWS_VERIFICATION.md`: "**Real GGUF Model Inference** — BLOCKED".
6. The repository contains **no `.gguf` file, no `llama-server`/`.exe` binary**, and no
   recorded HPS/benchmark artefact (`find` over the tree).
7. The native application has never been built, because the crate does not compile (§4.3).

**Conclusion:** protocol-level integration is partially verified on Linux against a synthetic
model. Real local chat, coding, patch generation and stop-mid-generation with genuine weights
remain **unproven**.

---

## 6. What remains unverified on Windows

Nothing native has been demonstrated. The list that must be closed before Mini can be called
working:

1. `cargo fmt`, `cargo check --all-targets`, `cargo clippy -D warnings`, `cargo test`,
   `cargo build` — currently failing at the first two.
2. `pnpm tauri dev` launches a real WebView2 window; CSP is applied at runtime.
3. `probe_runtime` against a real `llama-server.exe` built from or comparable to b11714;
   `--version` parsing from stderr; pinned-build warning behaviour.
4. Process supervision: `CREATE_NO_WINDOW`, ephemeral port, key-file path/permissions,
   `/health` transitions 503 → 200, load timeout, child exit detection, no orphan process on
   window close, bounded kill.
5. `GlobalMemoryStatusEx` returns correct total/available RAM.
6. GGUF validation against real Windows paths, sizes and files; the 16 MiB floor.
7. Real model load, streaming, cancellation mid-stream, next request after cancel, unload,
   reload, and crash recovery — with genuine weights.
8. Performance and fit: TTFT, prompt/decode tok/s, RAM/VRAM, 8K/16K/32K context, CPU-only vs
   GPU, thermal/power behaviour.
9. Offline operation with the network adapter disabled, including proof of zero outbound
   connections.
10. Windows path handling (`C:\...`), backslashes in workspace paths, long paths.
11. Keyring/DPAPI auth flows (currently also compile-broken).
12. NSIS packaging, install/uninstall, non-admin install, WebView2 dependency.
13. Any future export/download feature (dialog, destination write, ZIP) — not yet built.

---

## 7. Recommended Mini model-selection criteria and evidence still needed

No model has been selected. No weights were downloaded in this phase.

### 7.1 Hard gates (must pass before scoring)

| Gate | Requirement | Evidence needed |
|---|---|---|
| **G-1 Licence** | Commercial use, redistribution and modification permitted; notices can be satisfied; no non-production/acceptable-use trap | Exact model repo + quant repo licence text, pinned revision, legal review; attribution plan |
| **G-2 GGUF + runtime** | A GGUF exists and its architecture is supported by the exact pinned llama.cpp build | `llama-server --help`/load test on Windows; tokenizer/chat-template check |
| **G-3 Memory fit** | Weights + KV + compute fit the chosen Windows floor with ≥ 1 GiB headroom | Measured RAM/VRAM at 8K/16K/32K on reference machines |
| **G-4 Usable context** | At least 8K usable; 16K/32K preferred for project context | Measured prefill/decode and quality at depth |
| **G-5 Coding quality** | Beats a stated baseline on Nexora's own coding tasks, not vendor claims | NCE subset results (task success, patch apply, repair cycles) |
| **G-6 Structured output** | Produces the strict v1/v2 patch JSON reliably | Measured parse/validation success rate; target ≥ 98% (Phase-1 gate) |
| **G-7 Windows performance** | Usable TTFT and decode speed on representative CPUs | Benchmarks with/without GPU, 8 GB/16 GB RAM classes |
| **G-8 Integrity** | Pinned file + SHA-256 + source; no silent updates | Manifest entry and verified hash |

### 7.2 Mini-specific criteria

Because Mini is the free tier, it will likely be the **smallest** model and must run on the
weakest supported Windows machines:

- **Class:** 1.5–4B parameters at Q4, with a 7B Q4 "high" option only if 16 GB RAM is the
  floor. 7B Q4 is ~4.7 GB before KV; it is not a safe default for 8 GB machines.
- **CPU-first viability:** AVX2 CPUs, no discrete GPU; measure, do not assume.
- **Download/size budget:** must fit an owner-approved free download size; no bundling.
- **Latency over raw quality:** a smaller model that answers in seconds is better than a 7B
  that takes minutes on CPU, provided patch JSON remains reliable.
- **Footprint:** file size + KV + runtime overhead; disk and RAM both matter.
- **Scope fit:** chat, coding Q&A, patch proposals for existing files. It must not be expected
  to scaffold whole websites until that feature exists.

Candidate **classes** to evaluate (not selections): 1.5B–4B Apache-2.0 code models (for example
the Qwen2.5-Coder family), with the repository's already-documented
`Qwen2.5-Coder-7B-Instruct Q4_K_M` (4.68 GB, Apache-2.0, 32K, revision `c03e6d35`) as the
7B baseline. The repo's Phase-2 shortlist (Qwen3.8-27B, gpt-oss-20b, Mellum, etc.) is aimed at
16–24 GB developer machines, not a free Mini tier, and its facts were not independently
verifiable in this sandbox (Hugging Face is not reachable). Treat all Phase-2 model facts as
repository claims to re-verify, not as audit findings.

### 7.3 Evidence still needed before selection

1. Owner decisions: **minimum Windows hardware** (RAM/CPU/GPU), **maximum free download size**,
   and whether Mini is CPU-only or GPU-accelerated.
2. A licence review of the exact repos and quantisers.
3. A Windows benchmark run with a fixed protocol (TTFT, prefill/decode tok/s, peak RAM/VRAM,
   load time, 8K/16K/32K) on at least two machine classes.
4. An NCE/coding subset with pass rate, patch-contract validity, repair cycles, and
   false-success rate.
5. A long-chat and cancellation soak; OOM and malformed-GGUF failure tests.
6. A recorded manifest (source, revision, file, SHA-256, quant, licence, context, RAM estimate).
7. A written recommendation with the losing candidates' results, not just the winner's.

**Do not** choose a model from vendor benchmarks, parameter count alone, or this report.

---

## 8. What can be reused for future model switching (Sonnet / Opus later)

### 8.1 Reusable without redesign

- `process::Supervisor` lifecycle, ephemeral port, key handling, stderr capture, bounded stop.
- `client::stream_chat`, SSE parser, cancellation semantics, typed errors.
- GGUF header validation and executable sniffing.
- `SettingsStore` atomic JSON persistence pattern.
- `EngineState` / `AppStatus` state machine and UI status chips.
- IPC wrappers, `Channel` streaming, contract test pattern.
- Conversation history, workspace scanner, context budget, symbol index, retrieval.
- Patch contract/validation/apply engine and review UI.

### 8.2 Needs a thin abstraction (not present today)

| Gap | Why it matters for switching |
|---|---|
| No model registry/profile | Switching needs an id, display name, path, quant, licence, context, size, hash, capabilities |
| `Settings` has only a path | No stable identity across moved files; no per-model settings |
| No capability descriptor | Sonnet/Opus may need different context, template, tool support, hardware gates |
| No hardware fit per profile | Current estimate is a blunt RAM formula; no VRAM/GPU tiers |
| One process slot only | Switching = unload + load; acceptable, but status/UX must show it |
| No prompt/template per model | System prompt is hard-coded in `client.rs` |
| No benchmark/eval harness | Cannot prove a switch is an improvement |
| No `InferenceProvider` trait | Phase-1 proposed it; current code calls llama.cpp directly |

### 8.3 Explicitly out of scope for the Mini workstream

- Sonnet/Opus implementation, paid tiers, payments, accounts, entitlements.
- Cloud inference, external APIs, telemetry, Founder Control Centre.
- Redesigning the frontend.
- Weakening CSP or Tauri capabilities.

The existing auth and licensing code is pre-existing roadmap work. It is **not** a model-tier
system and is not wired to inference. The auth Rust code is part of the current compile
failure; the licensing module is not among the reported errors.
This audit neither extends nor repairs it.

---

## 9. Dependency-aware implementation plan for the next Mini phase

Sequence matters. Steps 0 and 1 are prerequisites; do not start UI work before them.

### Step 0 — Restore a buildable, testable native baseline (blocking)

**Why first:** no Mini feature can be verified while the crate does not compile and no Rust
test runs.

- Fix the eight Windows compile errors and the Linux formatting failures. Keep the changes
  minimal and unrelated to Mini features (auth/keyring, OAuth state move, `workspace/validation`
  test ownership, parser errors).
- Add/commit a `Cargo.lock` or pin exact dependency versions; decide the keyring API version.
- Get CI green: frontend, Linux `fmt/check/clippy/test`, Windows `check/build/test`.
- **Exit gate:** all four jobs green on the branch; `cargo test` actually executed and passing;
  no product behaviour changed.
- **Risk:** scope creep into auth repair. Time-box it; it is a prerequisite, not a feature.

### Step 1 — Mini model-selection spike (evidence only)

**Depends on:** Step 0 (a runnable app) and owner decisions on hardware floor + download budget.

- Shortlist 2–3 candidate GGUF files (1.5–4B Q4, plus one 7B Q4 baseline).
- Licence review per candidate; pin revisions; record SHA-256.
- Run the fixed Windows benchmark protocol and NCE subset.
- Produce a selection memo with all results, including failures.
- **Exit gate:** one recommended Mini model, approved by the owner, with evidence and a
  manifest record. **No weights are bundled or downloaded by the app.**
- **Do not:** pick a model in this phase; add a downloader.

### Step 2 — Model profile/registry (smallest useful abstraction)

**Depends on:** Step 1.

- Add a `ModelProfile` record (id, display name, model path, quant, context, licence notice,
  SHA-256, size, RAM estimate, capabilities) and a `modelId` in settings.
- Seed exactly **one** profile: the approved Mini model. Keep the user-supplied file path.
- Show the profile name in `EngineStatus`/model manager; keep one process slot.
- Reuse `SettingsStore`, `Supervisor`, `validate_model`; do not duplicate any of them.
- **Exit gate:** selecting the Mini profile and loading it works; switching between two paths
  (unload → load) is honest about state.
- **Explicitly not now:** Sonnet/Opus entries, entitlements, model downloads.

### Step 3 — Real-inference acceptance on Windows

**Depends on:** Steps 0–2.

- Execute the full §6 runbook with genuine weights: load, chat, patch, stop, unload, reload,
  offline, orphan-process check.
- Record TTFT/tok/s/RAM and failure modes; attach evidence to the phase report.
- Fix any defects found before building export features on top.
- **Exit gate:** real chat + coding + patch generation demonstrated with evidence; no false
  claims where evidence is missing.

### Step 4 — Export plumbing: single file + ZIP

**Depends on:** Step 3 (real generated content) but can be built against fixtures.

- Add **Rust-side** export commands with an explicit user-chosen destination. Prefer a scoped
  dialog capability or a Rust command that receives an absolute destination; do **not** add
  broad fs permissions to the WebView and do **not** weaken CSP.
- Single-file export: write the current file/patch content to the chosen path.
- Multi-file ZIP: build an archive from an explicit, validated file list; reject traversal,
  symlinks, sensitive files and oversized entries; never archive the whole disk or `.git`.
- Add tests for path safety, empty/oversized archives, and overwrite behaviour.
- **Exit gate:** both exports work on Windows, only to user-approved destinations, with no
  capability widening beyond the minimum.
- **Reuse:** workspace scanner for the file list, patch validation for content, existing
  path-safety helpers. Do not build a second scanner.

### Step 5 — Composer Download Mode

**Depends on:** Step 4.

- Add an explicit mode control to the composer (Chat vs Download), with clear labels.
- Define the download contract: one file or a project archive; where filenames come from; what
  happens when the model returns multiple files or invalid JSON.
- Reuse the patch contract/parser where possible; a download must not silently apply changes to
  the workspace.
- **Exit gate:** user can request and receive a single file and a ZIP; errors are honest;
  no filesystem write happens without confirmation.
- **UI:** minimal addition; no frontend redesign.

### Step 6 — Free usage tracking and limit-reached handling (policy first)

**Depends on:** Step 3 (real usage semantics) and a product decision.

- Resolve the contradiction with the shipped "Unlimited local GGUF inference" copy
  (`ui/src/lib/licensing.ts`, `docs/phase-26/README.md`) **before** writing any meter.
- Decide what counts (messages, tokens, generations, downloads), the limit values, the reset
  period, and the stance on local tampering.
- Implement local-only counters in the existing settings/history storage; no accounts, no
  telemetry, no server. Show remaining usage and a clear limit-reached state that does not fake
  success.
- **Exit gate:** limits are visible, honest, reset correctly, and cannot corrupt history or
  engine state. Document that local limits are not tamper-proof.
- **If the owner keeps "unlimited":** skip this step entirely; do not invent a limit.

### Step 7 — Offline + Windows end-to-end verification

**Depends on:** Steps 3–6.

- Network-disabled Windows run: chat, patch, downloads, limits, history; verify zero outbound
  connections.
- Installer smoke test (non-admin NSIS), uninstall, no orphan processes.
- Record results and any caveats.

### Step 8 — Documentation and evidence close-out

- Update the audit/selection report with measured results, not claims.
- Record known limitations and the exact model manifest.
- Do **not** update the 32-phase roadmap or claim phase completion.

### 9.1 Dependency graph

```
Step 0 (build/CI) ──► Step 1 (model evidence) ──► Step 2 (profile/registry)
                                                   │
                                                   ▼
                                        Step 3 (real inference)
                                         │                 │
                                         ▼                 ▼
                              Step 4 (export)      Step 6 (usage policy)
                                         │                 │
                                         ▼                 │
                              Step 5 (composer mode)       │
                                         └──────► Step 7 (offline/E2E) ◄──┘
                                                           │
                                                           ▼
                                                   Step 8 (report)
```

### 9.2 What must not be duplicated

- Do not add a second inference client, supervisor, stream parser or error type.
- Do not add a second workspace scanner, path validator or sensitive-file list.
- Do not add a second history store or context budget planner.
- Do not add a second patch parser/diff engine.
- Do not add a model downloader in the Mini phase; the user supplies the file.

---

## 10. Risks, conflicts and decisions required

| ID | Finding | Impact | Required decision |
|---|---|---|---|
| A-01 | Windows `cargo check` fails with 8 errors; Linux fmt fails | Blocks every native test and Mini verification | Fix before any Mini feature |
| A-02 | No `Cargo.lock`; loose dependency ranges | Non-reproducible builds; likely cause of keyring API mismatch | Pin and commit a lockfile |
| A-03 | No real GGUF inference evidence anywhere | Cannot claim Mini works | Windows benchmark + acceptance run |
| A-04 | "Unlimited local inference" is shipped copy for Community | Direct conflict with "free, limited usage" | Product/legal decision; update copy only if approved |
| A-05 | Local usage limits are unenforceable without accounts/server | Users can edit local state | Decide honor-system vs no limits; no telemetry/accounts permitted here |
| A-06 | Patch engine cannot create files or scaffold projects | "Basic website generation" is not supported today | Scope Mini as chat + patch of existing files, or plan scaffolding separately |
| A-07 | No download/ZIP infrastructure or permissions | New security surface | Approve scoped export design before implementation |
| A-08 | Memory estimate ignores VRAM/KV geometry; no GPU flags | Model may be mis-sized on Windows | Per-profile hardware fit + measured benchmarks |
| A-09 | Docs claim SQLite, token counting, 32-phase verification | Mismatch with code/CI | Correct docs when reporting results; do not build on false claims |
| A-10 | Missing `docs/phase-3` and `phase-4` | Roadmap continuity gap | Do not invent history; note the gap |
| A-11 | Auth code is entangled with the compile failure | "Fix build" may touch out-of-scope systems | Explicit approval for a minimal compile-only fix |

Documentation drift found (reported, not fixed):

- `README.md` §2.2 claims token counting; no usage/token counting is implemented.
- `README.md` §10.2 says Rust compilation is "Ready for Windows CI runner"; CI is red.
- `docs/phase-25/AI_CHAT.md` says SQLite; storage is a JSON file.
- `apps/desktop/README.md` says Rust tests 48 (Linux) / 45 (Windows); there are 176 test
  functions written and none executed.
- `docs/phase-32` says the tree is "uncommitted"; the audited tree is committed.
- `docs/phase-3` and `docs/phase-4` do not exist.

---

## 11. Method and limits of this audit

Read-only inspection plus safe, local checks:

- `git status`, `git log`, `git branch -a`, `git diff`, `git diff --check`, `git rev-list`,
  `git remote -v`, `gh pr list`, `gh run view`.
- Full source reading of `inference/`, `history/`, `workspace/`, `lib.rs`, key UI modules,
  Tauri config and capabilities.
- Documentation reading across `docs/phase-1`, `phase-2`, `phase-5`–`phase-32`.
- `pnpm install --frozen-lockfile`, `pnpm typecheck`, `pnpm test`, `pnpm build`.
- No `cargo` command could run; no model, runtime or weight was downloaded; no server was
  started; no file outside this report was written.

Limitations:

- The clone is shallow/grafted (one visible commit), so earlier branch history cannot be
  audited locally.
- Hugging Face and model hosts are unreachable from this sandbox; model facts in `docs/phase-2`
  were not independently verified.
- GitHub Actions logs are not downloadable here; failure details come from check-run
  annotations and job step conclusions.
- Frontend tests replace IPC with doubles by design; passing them is not evidence of native or
  model behaviour.

---

## Appendix A — Key file map

| Concern | Path |
|---|---|
| Supervisor / process lifecycle | `apps/desktop/src-tauri/src/inference/process.rs` |
| GGUF + executable validation | `apps/desktop/src-tauri/src/inference/models.rs` |
| Streaming client / SSE | `apps/desktop/src-tauri/src/inference/client.rs`, `sse.rs` |
| Settings | `apps/desktop/src-tauri/src/inference/settings.rs` |
| Engine state / status | `apps/desktop/src-tauri/src/inference/mod.rs` |
| IPC commands | `apps/desktop/src-tauri/src/inference/commands.rs`, `src/lib.rs` |
| Hardware probe | `apps/desktop/src-tauri/src/inference/hardware.rs` |
| Typed errors | `apps/desktop/src-tauri/src/inference/error.rs` |
| Frontend IPC wrappers/types | `apps/desktop/ui/src/lib/inference.ts`, `lib/ipc.ts` |
| Model setup / manager UI | `ui/src/components/ModelSetup.tsx`, `pages/ModelManagerPage.tsx` |
| Chat / history | `ui/src/pages/ChatPage.tsx`, `features/chat/*`, `src-tauri/src/history/*` |
| Workspace / context | `src-tauri/src/workspace/scanner.rs`, `ui/src/features/workspace/*` |
| Patch contract/apply | `src-tauri/src/workspace/patch.rs`, `patch_contract.rs`, `validation.rs` |
| Security config | `apps/desktop/src-tauri/tauri.conf.json`, `capabilities/main.json` |
| CI | `.github/workflows/ci.yml` |
| Runtime evidence | `docs/phase-5/runtime-contract-verification.md`, `docs/phase-7/README.md` |
| Model research (unverified here) | `docs/phase-2/model-evaluation.md` |

## Appendix B — Exact audit commands and outcomes

```
git status --porcelain -b          → ## arena/18030bbb-nexora (clean)
git rev-parse HEAD                 → 9bd4f7ad36ab012b50e48333c27fdc9b8244d5ff
git rev-list --count HEAD          → 1 (grafted)
git stash list                     → empty
git diff --check                   → exit 0
pnpm install --frozen-lockfile     → exit 0, 165 packages
pnpm typecheck                     → exit 0
pnpm test                          → exit 0, 75 files / 724 tests passed
pnpm build                         → exit 0, 118 modules
cargo/rustc                        → not installed
gh run view 39240750843            → frontend PASS; Linux Rust FAIL (fmt); Windows FAIL (cargo check)
```

**End of audit. No Phase 2 work has started. Waiting for explicit approval.**

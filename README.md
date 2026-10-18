# Nexora — Local-First AI Coding Studio for Windows

**Nexora is a Windows desktop application that lets you code with a Large Language Model running entirely on your own machine.** It combines a conversational workspace, a read-only codebase scanner, a bounded code-intelligence layer, and a review-before-apply patch workflow — with no paid external AI API required for core use.

> **Primary handover guide:** this root `README.md` is the single authoritative README for the project. `apps/desktop/README.md` remains as a package-level supplement. `docs/phase-1/` and `docs/phase-2/` contain the original planning research; per-phase `docs/phase-*/README.md` files are historical implementation logs now summarized here (see §12). See §3 for what is actually implemented and §11 for what is partial/planned/unverified.

---

## 1. Nexora Overview

* **What it is:** a Tauri 2 + React desktop shell whose Rust core supervises a user-supplied `llama-server` process and streams chat completions into a persistent conversation. The same workspace is used for code-aware features: scanning a folder, selecting files as read-only context, budgeting that context against the model's window, generating a single-file patch via the local model, and validating that patch before you apply it.
* **Purpose:** give individual developers, students, and small teams a premium conversational workspace that can understand an existing codebase without uploading it, propose changes locally, and keep every filesystem or process action under user control.
* **Target users:** Windows developers who want a local-first alternative to cloud coding assistants and who already have (or can obtain) a GGUF model.
* **Main goals (from `docs/phase-1/`):** local-first inference, bounded coding agent, multi-file website/app builder support, premium UI, verifiable security/privacy, offline-capable within disclosed limits.

## 2. Product Type

* **Windows desktop application — not a website or mobile app.** Delivered as a Tauri 2 application (`apps/desktop/src-tauri`, `tauri.conf.json` NSIS `currentUser`, 1280×840 window, custom titlebar, WebView2 on Windows). The `apps/desktop/ui` Vite preview (`http://localhost:1420`, `pnpm dev`) exists for fast iteration, but local inference (`llama-server`) only runs inside the Tauri desktop build. The repository ships no hosted site and no mobile target.

## 3. Core Features (implemented as of Phase 18)

* **Native window & conversation shell** — Tauri 2 window (`tauri.conf.json` `frontendDist ../ui/dist`), TitleBar/Sidebar/HistoryList/EmptyState/ModelSetup, collapsible sidebar, `Ctrl+B/L/N`, `EmptyState` prompt picks, `Composer` with IME-aware Enter/Shift+Enter, 350 ms submission guard, scroll-follow (`stickToBottomRef`), reduced-motion & ARIA.
* **Local inference with user-supplied runtime + model** — Rust `inference/` Supervisor spawns `llama-server` on `127.0.0.1` ephemeral port with per-session 32-byte key (`runtime-session.key` 0600, passed via `--api-key-file`), args `--model --host --port --ctx-size --parallel 1 --no-ui --jinja --api-key-file --timeout`, `CREATE_NO_WINDOW` on Windows. Streaming via `reqwest` + `sse.rs` byte-safe framing, `Channel<GenerationEvent::Started|Delta>` + `GenerationOutcome {cancelled}`, one generation at a time (`BUSY_ERROR`), `cancel_generation(requestId)` wakes the wait and drops the stream. Engine states: `not_configured|loading|ready|generating|error|stopped` plus `unknown` when IPC unreadable — never guessed.
* **Conversation persistence & history** — `history/storage.rs` atomic `conversations.json` in `app_data_dir` (`tempfile` + `fsync`), ordered IPC, race guards (`requestId+sessionRef+conversationId`), `Sidebar` history switcher with rename/delete, `usePersistentConversation` handles streaming/cancellation/workspace-switch invalidation.
* **Read-only workspace & codebase understanding** — `workspace/scanner.rs` iterative DFS `MAX_DEPTH 12, MAX_FILES 8000, MAX_DIRS 8000, MAX_INVENTORY_BYTES 2 MiB, MAX_PREVIEW_BYTES 256 KiB, MAX_CONTEXT_* 100 KiB/8 files, MAX_PATH_CHARS 4096`, excludes `node_modules/.git/target/dist/__pycache__/.next/build/coverage` etc + `.gitignore` lines, rejects `..` via `Component::ParentDir`, `canonicalize_root + starts_with` for symlinks, binary via NUL/utf8, posix relative. `useWorkspace` provides `filteredEntries/search/toggleContext/clearContext`, `contextFiles {path,content,size}`, `contextBlock` (fenced, “untrusted project data”), `preview` (read-only), `WorkspacePanel` + `FilePreview`. Settings in `workspace/settings.rs` (`workspace-settings.json`), manual path entry supported.
* **Context budget** — `features/workspace/contextBudget.ts` `estimateTokens = ceil(chars/4 *1.05)` (conservative, labelled `Estimated`; `measureTokens` hook exists for future tokenizer), reserve `SYSTEM_OVERHEAD 200 + maxTokens 1024` from `contextSize` (default 32768, clamp 2048–131072), priority `prompt never dropped → selectedFiles (user order) → recent history newest-first`, never slice inside a fence, warnings for omitted. `ContextBudgetBar` shows used/available, included/omitted.
* **Patch review (non-mutating)** — `workspace/patch.rs + patch_contract.rs + lib/patchContract.ts` contract `{version:1,targetPath,proposedContent,explanation?}` `deny_unknown_fields`, validates traversal/sensitive/binary/size/stale, `create_patch_proposal/validate_patch_proposal/apply_patch_proposal` atomic (`tempfile` + `fsync`), diff via `similar` (≤256 KiB), `PatchEditor` + `PatchPreview` (add/removed/unchanged gutters, truncated marker), explicit **Apply** required, never auto-apply.
* **Patch generation integration** — `usePatchGeneration` validates `validateInstruction ≤2000` chars, checks engine `ready|generating`, workspace selected, traversal/sensitive/binary/size, then `systemPrompt: “You are Nexora … edit exactly one file … untrusted … Respond ONLY JSON version N”` + `userPrompt` = `filteredContextBlock` + fenced `File to change: path` + `Change request:` + JSON contract, `generate({requestId,messages,maxTokens 4096,temperature 0.2})` streaming, collected truncated at 500 chars in UI, `parseAndValidatePatchResponse` strict then Rust `create/validate`, preview/validation/cancel.
* **Code intelligence (Phase 14)** — `workspace/symbol_index.rs + lib/symbolIndex.ts` bounded `MAX_INDEXED_FILES 500, MAX_INDEXED_BYTES 5 MiB, MAX_FILE_SIZE 256 KiB, MAX_PARSE_MS 2000, MAX_SYMBOLS_PER_FILE 200, MAX_SYMBOLS_TOTAL 5000`, regex for `rust (.rs: fn/struct/enum/trait/type/mod/const/use)`, `ts/tsx (function/class/interface/type/enum/import/export/arrow)`, `js/jsx (function/class/import/export)`, `py (def/class/import)`, others `kind: file` → `unsupported`, `Symbol {name,kind,path,startLine,endLine,detail?}` (`startLine` via `\\n` count), `SymbolIndex {version,root,symbols,fileCount,truncated,unsupported,parseErrors,generatedAt}`, commands `get_symbol_index/refresh_symbol_index/search_symbols/clear_symbol_index`, `CodeIntelligencePanel` (stats, search exact→prefix→substring→path, limit 50).
* **Deterministic retrieval & grounded prompt (Phase 14)** — `features/workspace/retrieval.ts` `rankSymbols`, `retrieveRelevantFiles({query,selectedFiles,symbolIndex,maxFiles 8,maxBytes 100 KiB})` selected first, then ranked symbols, dedupe, sensitive-filtered, budget-aware; `buildGroundedContext` fences with language hints; `App.tsx` `groundedContextBlock = groundedHeader + manifest + base` where header carries `GROUNDED — indexed: … Graph: … Verified paths/lines only — never invent … Retrieval … Supported languages … cycles via verified only`; fallback `no index — answer only from selected files`.
* **Validation & AI self-correction (Phase 15)** — `workspace/validation.rs` `validate_patch_detailed` bounded (3 s timeout, cancels on new request/workspace switch), per-check `{label,status: pass|fail|notRun|unsupported|inconclusive, diagnostics[]}`, sensitive/traversal/size/binary/stale already in patch, no network, reuses `tempfile`. `useValidation` + `ValidationPanel` (overall badge `live/passed/failed/notRun/unsupported/inconclusive`, checks/diagnostics, collapsible raw), `usePatchRepair` (`explain/repair` via local model, `maxAttempts`, revised proposal not auto-applied).
* **Deep intelligence (Phase 16)** — `workspace/analysis.rs` bounded `MAX_GRAPH_FILES 500, MAX_GRAPH_BYTES 5 MiB, MAX_EDGES 2000`, `extractTsImports/Rs/Py` via regex, `resolveRelative` via `existing_set` (eligible files), `DependencyEdge {from,to,raw,line,verified,kind,note}`, `FileNode {path,imports,importedBy}`, `AnalysisGraph {version,root,nodes,edges,truncated,unsupported,cycles,generationTime}`, cycles via verified-only DFS (max 10 cycles×10 length, sorted/dedup). Commands `get_dependency_graph/refresh_dependency_graph/get_file_relationships/get_change_impact/clear_analysis_graph`. `retrieveRelevantFilesSmart` extends retrieval with verified dependencies + dependents of selected + dependents of queried symbol (`Graph-aware: added N via verified imports`). `AnalysisPanel` (tabs Graph / File / Impact, verified green vs unresolved amber, confidence `Verified/Inferred`, sensitive excluded).
* **Evidence-first grounded responses (Phase 17)** — `lib/evidence.ts` bounded `EvidenceReport {entries≤20 {path,symbol?,line?,category verified|inferred|unavailable,source,detail≤120}, limitations {truncated,unsupported,parseErrors,graphTruncated,graphUnsupported,graphCycles,retrievalOmitted,retrievalWarnings≤5,budgetOmitted,budgetWarnings≤5,omittedPaths≤10,unsupportedLanguages≤8,indexUnavailable,graphUnavailable}, summary, hasVerified/hasInferred/hasUnavailable/isPartial/isUnsupported/verificationPerformed}` sensitive-excluded, no contents in logs, `formatEvidenceForPrompt` manifest (≤8 entries in prompt, `Limitations:` + `Instructions: separate observations (cite path:line) from hypotheses Hypothesis: / recommendations Suggestion:; never invent files/symbols/tests/builds/validation/line numbers; static analysis != runtime; if not grounded say Not grounded + what checked then ask clarification; Verification not performed note; general coding explanations allowed without workspace evidence`), `getPatchMissingEvidence(target,…)` → `missing/warnings` (target omitted, partial context, sensitive). `App.tsx` memo `evidenceReport` + `evidenceManifest` fed into `groundedContextBlock`; `EvidencePanel` concise chips (green `Evidence available` when `hasVerified`, amber `Partial context` when `isPartial`, yellow `Inferred relationship` when `hasInferred`, rose `Unsupported analysis` when `isUnsupported`, grey `Verification not performed` when `!verificationPerformed`, accent `Response incomplete` when streaming) + expandable manifest in explorer and near Composer; evidence details remain opaque for readability. Patch missing warning `patch-generate__missing` above `PatchGenerateBar` when required source not retrieved.

* **Premium desktop UI (Phase 18)** — refined the existing graphite/violet design with selective glass on the title bar, sidebar, search controls, and composer; technical/code/evidence/analysis surfaces remain opaque. The workspace path filter and indexed-symbol search are clearer and accessible while keeping their existing scopes, operations, and data access. Also refines spacing, sidebar collapse, control alignment, short motion, empty/status states, and responsive layouts with reduced-motion/transparency and solid-surface fallbacks. This is presentation-only: Tauri integration, inference, evidence, analysis, persistence, and explicit patch-apply safeguards remain unchanged; existing icon assets were retained.

## 4. Technology Stack (as present in repo)

| Layer | Choice | Where pinned |
|-------|--------|--------------|
| Shell | Tauri 2 (Rust) | `apps/desktop/src-tauri/tauri.conf.json` `productName Nexora 0.1.0`, `src-tauri/Cargo.toml` `tauri = { version = "2" }`, `rust-toolchain.toml` channel `1.93.0` + `rustfmt/clippy` |
| UI | React 19.3.1 + TypeScript 5.9.3 | `apps/desktop/package.json` `react 19.3.1`, `typescript 5.9.3` |
| Bundler | Vite 7.3.7 | `vite.config.ts` `root ui, outDir dist, target chrome105`, `@vitejs/plugin-react 5.2.0` |
| Package manager | pnpm 9.15.4 (workspace) | `package.json` `packageManager pnpm@9.15.4`, `pnpm-workspace.yaml` `apps/*` |
| Styling | Hand-written CSS + design tokens, no UI framework | `ui/src/styles/tokens.css + app.css` `var(--bg-base)` etc |
| Rust deps | serde 1, serde_json 1, reqwest 0.12 (json/stream), tokio 1 (sync/time/macros/rt), futures-util 0.3, getrandom 0.2, sha2 0.10, similar 2.7, regex 1, tempfile 3, windows-sys 0.59 (cfg windows) | `Cargo.toml` |
| Runtime client | `reqwest` loopback HTTP + byte-safe SSE `inference/sse.rs` | `inference/client.rs` `stream_completion` |
| Model runtime | **Not bundled** — user-supplied `llama-server` pinned `llama.cpp b11714` (see `docs/phase-2/`) | `inference/models.rs` header check, `--version` probe |
| Tests | Vitest 3.2.7 + jsdom 26.1.0 + Testing Library 10.4.2/16.3.3 + user-event 14.6.8 (frontend, 500 tests); `cargo test` (Rust 48 Linux + 45 Windows via CI) | `vitest.config.ts` `environment jsdom, setupFiles ui/src/test/setup.ts` |

No vector DB, no embeddings, no external AI APIs, no payment/subscription code.

## 5. Architecture

```
apps/desktop/
  ui/  — React SPA (Vite root ui/, dist → src-tauri frontendDist)
    App.tsx — composes workspace + analysis + evidence + grounded prompt + composer
    components/ — TitleBar, Sidebar/HistoryList, ModelSetup, WorkspacePanel, FilePreview,
                  CodeIntelligencePanel, RetrievalBar, ContextBudgetBar, AnalysisPanel,
                  EvidencePanel (Phase 17), PatchEditor/PatchPreview/PatchGenerateBar,
                  ValidationPanel, MessageList/CodeBlock, Composer
    features/
      chat/ — chat.ts reducers, useConversation/usePersistentConversation (stream+cancellation)
      workspace/ — useWorkspace (inventory+context), contextBudget, retrieval (+smart),
                   useAnalysis/useImpact, usePatch/usePatchGeneration/usePatchRepair/useValidation
    lib/ — ipc (typed), inference (Supervisor), symbolIndex/analysis (typed),
           evidence (bounded report), patch/patchContract, validation, buildConfig, etc.
    styles/ — tokens.css + app.css (no heavyweight framework)
  src-tauri/ — Tauri 2 crate (nexora_lib + nexora bin)
    src/lib.rs — Builder + ManagedState (WorkspaceState, SymbolIndexState, AnalysisState, EngineState)
    src/inference/ — settings.rs (validated JSON), hardware.rs, models.rs (GGUF+exe checks),
                    process.rs (spawn + 32-byte key + stderr drain 64 KiB + probes),
                    client.rs (loopback streaming), sse.rs, error.rs, commands.rs (10 commands)
    src/workspace/ — scanner.rs (scanner+limits+is_sensitive+canonicalize), commands.rs (8 cmds),
                    symbol_index.rs (regex, 500/5 MiB), analysis.rs (graph+relationships+impact),
                    patch.rs/patch_contract.rs (single-file contract, atomic apply),
                    validation.rs (bounded 3 s, per-check diagnostics), settings.rs
    src/history/ — storage.rs (atomic conversations.json), commands.rs
    capabilities/main.json — least-privilege window permissions (minimize/toggle-maximize/close/drag)
    tauri.conf.json — window 1280×840→960×640, CSP `default-src 'self'`, bundle NSIS currentUser
```

* **Local AI inference:** UI `generate({requestId,messages,maxTokens,temperature})` → Tauri command → `Supervisor` validates `≤64 turns / 400k chars` → spawns `llama-server` once (`inference/settings.json` survives restarts) → loopback `POST /v1/chat/completions` SSE → `Channel<GenerationEvent>` back to UI → single `assistant` message appended, stale dropped via `requestId+sessionRef+conversationId`. Cancellation via `cancel_generation`, stop keeps partial text `Stopped`.
* **Workspace analysis:** UI selects files → `planContextBudget` fits prompt+files+history against `contextSize` → `filteredContextBlock` (only included) → `retrieveRelevantFiles(Smart)` (selected first + ranked symbols + verified graph deps) → `CodeIntelligencePanel/AnalysisPanel/EvidencePanel` read `SymbolIndex/AnalysisGraph` cached per root, cleared on `closeWorkspace`/switch.
* **Evidence-grounded responses:** every chat `contextBlock` is `groundedHeader (indexed fileCount/symbols, graph stats, verified vs inferred legend, retrieval summary) + evidenceManifest (up to 8 entries verified/inferred/unavailable + Limitations + reliability instructions) + filteredContextBlock (fenced, “untrusted project data”)` — attached to last user turn only for the model request, not persisted.
* **Patch review & validation:** `propose` → edit `proposedContent` locally → `create_patch_proposal` (traversal/sensitive/binary/size) → `validate_patch_proposal` (diff) + `validate_patch_detailed` (bounded checks, per-diagnostic) → `PatchPreview` + `ValidationPanel` → explicit **Apply** → `apply_patch_proposal` atomic; `repair` path via `usePatchRepair` streams local-model `explain/repair` without auto-applying revision.

All filesystem/child-process operations are via Rust, `canonicalize_root + starts_with` for traversal, `is_sensitive_file` for `.env*/*.pem/*.key/credentials.json`, `starts_with` never `contains`, `MAX_*` bounds everywhere, retrieved files treated as untrusted text (never executed).

## 6. Repository Structure (actual)

```
Nexora/
  README.md                         ← this primary handover guide (you're here)
  package.json / pnpm-workspace.yaml / pnpm-lock.yaml
  pnpm-lock.yaml / rust-toolchain.toml / .editorconfig / .gitignore
  .github/workflows/ci.yml          ← frontend + rust (linux+windows) pins SHA, no bundling
  resources/icons/nexora-icon.png   ← app icon source
  apps/desktop/
    README.md                       ← package supplement (Tauri+UI specifics)
    vite.config.ts / vitest.config.ts / tsconfig.json
    ui/  index.html, src/ (App.tsx, main.tsx, components/, features/, lib/, styles/, test/)
         dist/  — production output (ignored, generated by pnpm build, embedded at cargo compile)
    src-tauri/  Cargo.toml, build.rs, tauri.conf.json, capabilities/main.json,
                icons/ (32/128/ico), src/ (lib.rs, main.rs, inference/, workspace/, history/)
  docs/
    phase-1/  README + 01–12 planning docs (scope, UX, arch, runtime, workflow, site, security, test, build, repo, hardware, roadmap)
    phase-2/  README + 8 research docs (runtime/model evaluation, hardware matrix, benchmark, integration, acquisition, sources)
    phase-5/  README + runtime-contract-verification
    phase-6/  README
    phase-7/  README
    phase-8/  README
    phase-9/  README
    phase-10/ README  (history switcher details)
    phase-11/ README  (workspace foundation)
    phase-12/ README  (context budget + patch preview)
    phase-13/ README  (patch generation)
    phase-14/ README  (code intelligence)
    phase-15/ README  (validation + repair)
    phase-16/ README  (deep intelligence)
    phase-17/ README  (evidence-first)
    phase-18/ README  (premium desktop UI transformation)
```

Generated/ignored: `node_modules/`, `dist/`, `coverage/`, `apps/desktop/src-tauri/target/`, `gen/`, `.vite/`, `*.log`. None are tracked.

## 7. Installation and Development Setup

### Prerequisites (Windows — per `apps/desktop/README.md` + `tauri.conf.json` + `rust-toolchain.toml`)

1. **Node.js 22 LTS** and **pnpm 9** (`corepack enable pnpm` — `packageManager pnpm@9.15.4`).
2. **Rust** pinned channel `1.93.0` via `rustup` (`rust-toolchain.toml` `components = ["rustfmt","clippy"]`).
3. **Visual Studio 2022 Build Tools** — “Desktop development with C++” + Windows 10/11 SDK.
4. **WebView2 Evergreen Runtime** (preinstalled on Windows 11).
5. **A `llama-server` build** pinned `llama.cpp b11714` (`docs/phase-2/runtime-evaluation.md`) **and a GGUF model file** — Nexora does not ship, download, or install either.

Tauri docs: <https://v2.tauri.app/start/prerequisites/>

### First run (from repo root)

```bash
pnpm install                 # workspace, frozen-lockfile in CI
pnpm tauri dev               # native window (starts Vite on http://localhost:1420 internally)
```

For fast UI-only iteration without Rust:

```bash
pnpm dev                     # Vite only, preview host 0.0.0.0:1420 (allowedHosts true for containers)
```

Browser preview will show “Desktop app required” and keep sending disabled — inference needs Tauri.

## 8. Build, Typecheck, and Test Commands (real scripts)

From repo root (`package.json`):

| Command | What it runs |
|---------|--------------|
| `pnpm install` | `pnpm --filter @nexora/desktop` deps via `pnpm-lock.yaml` |
| `pnpm dev` | `vite` on `http://localhost:1420` (`apps/desktop/vite.config.ts`) |
| `pnpm tauri dev` | `tauri dev` (builds Rust + starts Vite) |
| `pnpm typecheck` | `tsc --noEmit` (`apps/desktop/tsconfig.json` strict, bundler) |
| `pnpm test` | `vitest run` (`jsdom`, setup `ui/src/test/setup.ts`, `ui/src/**/*.test.{ts,tsx}`, `restoreMocks: true`) |
| `pnpm build` | `tsc --noEmit && vite build` → `apps/desktop/ui/dist` (`emptyOutDir true`) |
| `pnpm check` | `typecheck && test && build` (convenience) |

Direct desktop package (`apps/desktop`):

```bash
pnpm --filter @nexora/desktop typecheck
pnpm --filter @nexora/desktop test
pnpm --filter @nexora/desktop build
```

Rust (must build UI first — `generate_context!` embeds `ui/dist`):

```bash
pnpm build && cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml --all-targets
pnpm build && cargo fmt --all -- --check
pnpm build && cargo clippy --manifest-path apps/desktop/src-tauri/Cargo.toml --all-targets -- -D warnings
pnpm build && cargo test  --manifest-path apps/desktop/src-tauri/Cargo.toml
cargo build --manifest-path apps/desktop/src-tauri/Cargo.toml        # links Windows exe
pnpm tauri build             # NSIS installer — must run on Windows with prereqs above; not executed in CI
```

CI (`.github/workflows/ci.yml`, SHA-pinned actions, `contents: read`): `frontend` (typecheck/tests/build on ubuntu-24.04), `rust` (fmt/check/clippy/test on linux), `windows` (check + link + tests on windows-2022). No bundling step — `tauri build` deliberately absent.

**Verified 2026-10-18 on branch `arena/74aec262-nexora` HEAD `f032318`:**

* `pnpm typecheck` **PASS** (tsc)
* `pnpm test` **PASS** — 44 files, **497 tests** (Phases 10–17 inclusive; new `lib/evidence.test.ts` 18 + `EvidencePanel.test.tsx` 11)
* `pnpm build` **PASS** — `90 modules, dist/assets/index-*.js 419 kB`
* `cargo fmt/check/clippy/test` **BLOCKED** in this sandbox (`cargo: command not found`, no Rust toolchain nor registry) — see §13. Not a new failure; same as Phases 10–16 sandbox. CI on Linux/Windows is the place these gates pass.

**Phase 18 verification, 2026-10-18, local working tree on `arena/555ee411-nexora` (base HEAD `6ea0dbd9784db6f1e353fb87be84686cc12ae46e`):**

* `pnpm check` **PASS** — typecheck; 44 test files / **500 tests**; production build (90 modules, CSS 67.38 kB / 10.61 kB gzip, JS 420.00 kB / 122.43 kB gzip).
* Build reports the same two Vite dynamic/static import chunking warnings seen at the pre-change baseline; no new warning was introduced.
* Rust/Tauri/Windows/local-model checks **not run**: `cargo` and `rustc` are unavailable, this runner is Linux, and no native desktop/display was available for runtime inspection.
* Vite browser UI preview is running on port 1420; HTTP smoke requests for the app entry, UI modules, and stylesheet returned 200, but agent-side visual inspection is not available. The preview is not the Tauri runtime, and generation remains disabled outside the desktop shell.

## 9. AI Model Setup

* **You bring both files.** Nexora never ships weights or `llama-server`, never downloads them, and never claims a model works until the Rust checks succeed.
* **Runtime executable** — absolute path to `llama-server.exe` (typed, no file picker). `Check` runs `llama-server --version`, shows first line, labels non-`b11714` as *unverified* not incompatible. Launch args are structured, no shell. Stored keys never appear in UI or logs.
* **Model file** — absolute path to `*.gguf` (typed). `Check` reads GGUF magic `\u0000GGUF` / version 2–3 / tensor count, requires `.gguf` extension and plausible size, estimates memory from file size + context window, never uploads.
* **Context window** — UI 8K–64K, core clamp 2048–131072 (`inference/settings.rs`), persisted to `<app config dir>/inference-settings.json` on `Save and load model`, then Supervisor starts engine on random `127.0.0.1` port + 32-byte key, waits ≤180 s for `/health`. Reload restarts process so changed size takes effect. Failure shows core error + last 6 non-empty lines of 64 KiB stderr, then engine is `stopped` not half-alive. `Stop engine` card cancels generation + kills child + refreshes status.
* **Limitations today:** chat `maxTokens 1024` (patch generation 4096) + `temperature 0.2` fixed in `App.tsx`/`usePatchGeneration`; no tokenizer wired (`estimateTokens` conservative 4 chars/token +5% margin); no embeddings/vector DB; no automatic model download; no fine-tuning; `hardware.rs` physical memory detection is `cfg(windows)`-only elsewhere fallback.

## 10. Security and Privacy

* **Local-first, loopback-only.** `llama-server` binds `127.0.0.1` ephemeral port, no LAN/CORS relaxation, no control endpoint to webview. Prompts are validated (`≤64 turns, ≤400k chars`) and streamed as text-only; no prompt/model output is executed as code, path, or command. Key is per-launch random, `getrandom 0.2` + `0600` file, never in command line, never logged.
* **Child hardening.** No interpolated shell; `CREATE_NO_WINDOW` (windows), kill+reap on `unload/close/Drop`; stderr drained continuously (child never blocks), only last 64 KiB kept in RAM, only last 6 lines surface in errors; `similar` diff truncated; no log file; no telemetry.
* **Workspace.** `workspace/scanner.rs` enforces `canonicalize_root + starts_with` against symlink traversal, `Component::ParentDir` rejection, `MAX_*` bounds (files/dirs/bytes/depth/path), `is_sensitive_file` excludes `.env* / *.pem / *.key / id_rsa / credentials.json` from inventory, retrieval, evidence, and graph; oversized (>256 KiB) / binary (NUL) / unreadable excluded; `read_workspace_file` re-checks sensitive+size+binary+workspace-root. Evidence report stores only `path/symbol/line/category/detail`, never file contents in the manifest/logs.
* **Tauri surface.** 10 inference commands + ~8 workspace/history commands, none can browse arbitrary FS, spawn shell, or reach network beyond owned loopback. `capabilities/main.json` allowlist: `minimize/toggle-maximize/close/startDragging` only. CSP: `default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: asset:` — no remote fonts/images/scripts. In browser without Tauri, UI shows `Desktop app required` and disables generation (no simulated reply).
* **No unsupported privacy guarantees.** Offline inference does not mean downloads/updates are offline; model download is manual; no claim of network isolation beyond loopback.

## 11. Current Feature Status

| Area | Status | Notes |
|------|--------|-------|
| Native window, titlebar, sidebar/history, composer | **Implemented** (Phases 3–10) | Verified via Vitest + manual; Windows Snap Layouts hover not available with custom chrome (`decorations false`) — known limitation. |
| Conversation persistence (atomic, ordered, race-guarded) | **Implemented** (Phase 10) | `conversations.json` atomic; `history/storage.rs`. |
| Read-only workspace scanner + context selection | **Implemented** (Phase 11) | DFS bounded, preview, 8 files/100 KiB context. |
| Context budget planner + bar | **Implemented** (Phase 12) | Estimated tokens, never slice fence, warnings. No measured tokenizer yet — **partial**. |
| Patch preview (diff) + atomic apply | **Implemented** (Phase 12) | Single-file contract, stale/size/binary/sensitive checks. |
| Patch generation via local model | **Implemented** (Phase 13) | JSON contract, streaming, strict parse, validation-gated preview. |
| Code intelligence (symbol index + retrieval) | **Implemented** (Phase 14) | Regex for rs/ts/js/py, 500/5 MiB, unsupported marked. Not a language server — **partial** for other langs. |
| Validation & AI repair (detailed checks) | **Implemented** (Phase 15) | Bounded 3 s, per-check diagnostics, `explain/repair` streamed, not auto-applied. |
| Deep intelligence (dependency graph, relationships, impact) | **Implemented** (Phase 16) | Regex imports, verified vs unresolved, cycles via verified only, smart retrieval. |
| Evidence-first grounded responses | **Implemented** (Phase 17) | Bounded manifest, chips, prompt hardening, patch missing warning. |
| Premium desktop UI / interaction polish | **Implemented** (Phase 18) | Graphite/violet design tokens, focused composer, scroll-safe technical explorer, clearer validation/evidence surfaces, responsive states, reduced-motion/transparency support. Glass remains selective; no backend or workflow changes. |
| Multi-file patch proposal | **Planned** (future phase beyond 18 in old roadmap, not in original 18) | Current contract is single-file only. |
| Cloud fallback / embeddings / vector DB | **Not planned** — local-only | Never added. |
| Monetisation / subscriptions | **Not planned** | Never added. |
| Rust/Windowing real-model verification | **Unverified** in this sandbox | Requires local Rust toolchain + Windows + `llama-server b11714` + GGUF; CI on Linux/Windows covers compile/lint/tests but not inference. |

## 12. Phase History (14–18)

* **Phase 14 — Code intelligence & project-aware analysis** (`a15b304`, 2026-10-18) — bounded symbol index (`symbol_index.rs`, regex, 500/5 MiB), project overview stats, symbol search (exact→prefix→substring), deterministic retrieval (selected first + ranked), grounded `CodeIntelligencePanel + RetrievalBar` + `groundedContextBlock` header. Limits Truman truncated/unsupported, sensitive excluded. Verification: `pnpm typecheck PASS`, `pnpm test PASS` (new 373-range), `pnpm build PASS`, `cargo BLOCKED` (no toolchain). Limitation: regex not tree-sitter, others `unsupported`.
* **Phase 15 — AI self-correction & validation** (`c2a2a71`, 2026-10-18) — bounded `validation.rs` (fmt/clippy-like checks, per-diagnostic), `useValidation/ValidationPanel` (NotRun/Unsupported honest), `usePatchRepair` (`explain/repair` via local model streamed, cancellable, maxAttempts, revised not auto-applied). All via existing `generate`. Verification: typecheck PASS, tests + new `ValidationPanel/useValidation/usePatchRepair` PASS, build PASS, cargo BLOCKED. Limitation: no network checks, 3 s timeout.
* **Phase 16 — Advanced deep codebase intelligence** (`0187aea`, 2026-10-18) — bounded `analysis.rs` (500/5 MiB/2000 edges, ts/rs/py regex imports, verified vs unresolved via `existing_set`, cycles verified-only DFS), `get_dependency_graph/file_relationships/change_impact`, `retrieveRelevantFilesSmart` (adds verified deps/dependents), `AnalysisPanel` (Graph/File/Impact tabs, Verified green vs Inferred amber). Tests: `analysis.test.ts + useAnalysis 13 + useImpact 10 + AnalysisPanel 10 + retrieval.smart 7` (all deterministic mocks). Verification: typecheck PASS, tests 44 files, cargo BLOCKED. Limitation: no dynamic imports, inferred symbol-reference search (len>2, 50 refs, 120-char snippet).
* **Phase 17 — Evidence-first AI & grounded responses** (`f032318`, 2026-10-18, this phase) — bounded `lib/evidence.ts` (`EvidenceReport` 20 entries + limitations), `formatEvidenceForPrompt` (≤8 in prompt + reliability instructions: separate observations/Hypothesis:/Suggestion:, never invent paths/symbols/tests/builds/validation/lines, static analysis ≠ runtime, Not grounded + ask clarification, Verification not performed until actual report, note truncated uncertainty, general explanations allowed), `EvidencePanel` (chips near composer+explorer, reuses tokens; evidence details remain opaque for readability), `getPatchMissingEvidence` warning (`patch-generate__missing`) when target/dependency not retrieved, sensitive excluded. Tests: `evidence.test.ts 18 + EvidencePanel.test.tsx 11`, total **497 passing**, typecheck PASS, build PASS, cargo still BLOCKED (no toolchain in runner; no Rust changes, so no regression). Limitation: prompt manifest capped to avoid budget overflow.
* **Phase 18 — Premium desktop UI transformation** (2026-10-18; frontend-only, local working tree) — refined the existing token system/shell, selective glass, shared path/symbol search controls and truthful empty states, control spacing, shortcut copy, brief motion, opaque technical surfaces, and reduced-motion/transparency fallbacks. Search scope/data access and all backend/workflow safeguards remain unchanged; existing icon assets were retained and no dependencies were added. `pnpm check`: typecheck PASS, tests 44 files / 500 tests PASS, production build PASS (two pre-existing Vite chunking warnings). Rust/Tauri/Windows/local-model checks remain unavailable in this Linux sandbox. Preview HTTP smoke checks passed; agent-side visual/native runtime inspection was not possible. No commit, push, PR, or release was performed. Details: `docs/phase-18/README.md`.

Earlier phases summarized in `docs/phase-1/README.md` (planning index), `docs/phase-2/README.md` (research), `docs/phase-5/README.md` (runtime contract), `docs/phase-6/7/8/9/README.md` (verification, code-aware rendering) — see those folders for full historical logs.

## 13. Known Limitations

* **Rust toolchain not available in this sandbox** — `cargo fmt/check/clippy/test` cannot be executed here; CI on `ubuntu-24.04` and `windows-2022` is the verification path. This has been **BLOCKED** since Phase 10 and remains so; Phase 18 is frontend/CSS-only and changes no Rust source.
* **Windows & real model inference not verified here** — `cargo build` links Windows exe, `tauri build` NSIS installer, and `llama-server b11714 + GGUF` stream require a Windows machine with Build Tools + WebView2 — not exercised in CI beyond compile.
* **Workspace analysis is regex-bounded, not a language server** — `ts/tsx/rs/js/jsx/py` via regex; `.json/.md/.toml/.yaml/.css/.html/.sh/.go/.java` are `unsupported` (counted, not parsed). Dynamic imports, macros, re-exports beyond regex are missed; `referencedBy` is substring search (≥3 chars, 50 refs). Treat `inferred` as suggestion until verified.
* **Graph is verified-only for edges** — `unresolved` means `external` or `not found` via `existing_set`; no npm/cargo resolution, no transitive container analysis.
* **Context budget is estimated** — `chars/4 +5%` not a real tokenizer; may over/under-estimate dense code; second guard is `MAX_CONTEXT_*` 100 KiB/8 files. History oldest-first trimmed, `prompt` never dropped.
* **No whole-repo dump** — retrieval and evidence are bounded (8 files, 20 entries) so answers saying “Not grounded in the provided files” are correct when context incomplete — add files to context or enlarge budget.
* **Sensitive files excluded** — `.env*`, `*.pem`, `*.key`, `credentials.json` etc are never indexed, retrieved, graphed, or evidenced; `Patch` refuses them (sensitive error), `getPatchMissingEvidence` flags them.
* **No whole-history persistence across machines** — conversations are local `conversations.json`; closing window loses unsaved? Actually it persists locally, not synced.
* **Browser preview is not the desktop app** — `isDesktopShell()` false disables inference, shows `Desktop app required`; no simulated replies.

## 14. Phase 18 Completion — Premium Desktop UI Refinement

* **Scope preserved:** UI/UX only. No new search backend/functionality, prompt changes, Rust/Tauri IPC changes, cloud inference, telemetry, or monetization. Existing local inference, code intelligence, evidence, validation, persistence, streaming/cancellation, and patch-apply safeguards remain unchanged.
* **Design:** graphite/charcoal foundation, restrained violet action/focus accent, shared surface/status/motion tokens, selective glass on title bar/sidebar/search controls/composer; opaque explorer and technical/code/evidence surfaces; simplified nested cards and empty-state decoration.
* **Search:** project filter remains local, case-insensitive substring matching on inventory paths only; indexed symbol search keeps the existing operation and 20-result limit. Clear actions, accessible names, counts/no-match feedback, submitted-query no-results, and stale-request handling were refined without changing data access.
* **Accessibility and responsiveness:** keyboard-focusable conversation transcript; preserved shortcuts including visible Esc-to-stop help; visible focus and named search/clear controls; compact layouts through existing breakpoints; reduced-motion and reduced-transparency styles; opaque fallbacks if backdrop filters are unavailable.
* **Assets:** retained the existing Nexora aperture icon and all configured Tauri PNG/ICO assets; no icon or packaging configuration changes were needed.
* **Verification and limitations:** see `docs/phase-18/README.md` for exact commands/results, baseline comparison, branch/commit discrepancy, and Windows/runtime verification still required.

## 15. Contribution and Git Workflow

* **Current Phase 18 checkout:** `arena/555ee411-nexora` at base HEAD `6ea0dbd9784db6f1e353fb87be84686cc12ae46e`. The phase brief expected `arena/74aec262-nexora` at `d662588`; the mismatch was preserved without switching or resetting. This Arena session is fixed to its assigned branch — never switch/create/push other branches.
* **Local only:** commits stay local. `isPrivate: false` (public) via `gh repo view` — do **not** push, do not create PR, do not change visibility, do not publish/release, do not delete branches/tags/history, do not run `git reset --hard` / `git clean -fd`. Reports are capped ~128 MB / 10k files; keep large datasets out of Git per `.gitignore`.
* **Safe dev loop:** `pnpm install` → `pnpm typecheck` → `pnpm test` (deterministic mocks, not real model) → `pnpm build` → `cargo` gates (where available — recheck; currently BLOCKED in sandbox, run locally before ship). Keep `pnpm-lock.yaml` exact pins (`buildConfig.test.ts` enforces).
* **No secrets:** never commit `.env`/keys/tokens (`.gitignore` covers `.env*`, `*.pem`, `credentials.json`), never log prompt/key/model output.
* **Files are saved automatically** after each tool turn — no manual `git commit` required to preserve work, but explicit commits are kept for history.

## 16. Release Status

* **Current status: not released, not published.** Repository is public (`cybervault-Hacky/Nexora`, `isPrivate: false`) but no installer has been built or uploaded here — `src-tauri/target` not present, `ui/dist` is ignored build output, no GitHub Release. CI builds on Linux/Windows for verification only; bundling/`tauri build` is deliberately absent from CI.
* **Publishing requires explicit founder approval.** Do not push commits, do not create PR, do not change `isPrivate`, do not publish builds/releases, do not delete `origin` tags/commits. When approved, package on Windows: `pnpm build && cargo build` then `pnpm tauri build` → NSIS per-user installer in `src-tauri/target/release/bundle/nsis/`. Keep `update notices` per `docs/phase-2/model-acquisition-policy.md`.

---

*Phase 18 UI refinement verification — 2026-10-18 — session branch `arena/555ee411-nexora`, base HEAD `6ea0dbd9784db6f1e353fb87be84686cc12ae46e`; `pnpm check` PASS (44 files / 500 tests, typecheck and build); Rust/Tauri/Windows checks unavailable. Browser preview HTTP smoke passed; agent-side visual inspection was unavailable. The phase brief named `arena/74aec262-nexora` / `d662588`; the actual checkout was preserved without switching branches. No dependencies were added; no commit, push, PR, release, or visibility change occurred.*

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

## 3. Core Features (implemented as of Phase 22)

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
* **Page architecture & separate-page navigation (Phase 19)** — the app shell (TitleBar, Sidebar with primary nav + `aria-current="page"`, global engine notices) renders one page at a time via a dependency-free hash router (`lib/router.ts`: `#/`, `#/chat`, `#/projects`, `#/code`, `#/models`, `#/settings`, honest not-found fallback). AI Chat and Projects are genuinely separate views (`pages/ChatPage.tsx` vs `pages/ProjectsPage.tsx`); symbol/graph tools have their own Code Intelligence page; the existing `ModelSetup` flow is the Local Models page; Settings shows real persisted preferences and read-only engine config. All feature hooks stay mounted in `App.tsx`, so conversations, drafts, workspace, and patch state survive page switches; back/forward hash history works and no route reloads the app. No auth, glassmorphism, or animation system was added (later phases).
* **Welcome & onboarding entry flow (Phase 20)** — the app now opens on a dedicated Welcome screen (`#/`, alias `#/welcome`) outside the workspace sidebar: Nexora brand lockup, concise headline and local-first copy limited to verifiable claims, a restrained decorative product preview (non-interactive, `aria-hidden`), and a primary **Continue** action into `#/onboarding` — an honest transitional screen reserving the slot for Phase 21's sign-in (no fake form), with working **Back to Welcome** and **Open the workspace** (`#/home`, Home's canonical path since Phase 20). Title-bar window chrome is kept on the flow; `Ctrl+B` is scoped to pages that have a sidebar. No onboarding persistence, no dependencies, no auth.
* **Sign-in interface (Phase 21)** — dedicated Sign In page at `#/signin` (Welcome → Continue → Onboarding → Continue to Sign in) with labelled email/password fields (`autocomplete=email`/`current-password`), accessible password-visibility toggle, real client-side validation (errors beside fields via `aria-invalid`/`aria-describedby`, password never cleared on error), loading/disabled submit states with repeat-submission blocking, and an honest terminal status — *"Sign-in is not connected yet. Account authentication will be enabled in a later phase."* — never a success claim, session, or credential transmission/storage/logging. Google sign-in entry point (recognisable inline four-colour G) explains that integration arrives with the authentication backend; `#/signup` and `#/recover-password` hold honest Phase 22 placeholders. No OAuth, secrets, or auth backend.
* **Create account, verification, recovery & reset interfaces (Phase 22)** — real frontend pages replacing the P21 placeholders: `#/signup` (full name/email/password/confirm with `lib/formValidation.ts` rules: trimmed name, syntactic email, **≥ 8-character** new passwords per NIST SP 800-63B's lower bound with no composition rules, matching confirmation; per-field `aria-invalid`/`aria-describedby` errors, focus-first-invalid, values retained on failure, dual visibility toggles, repeat-submission guard); `#/verify-email` (honest verification-pending experience — no code, no token, no resend); `#/recover-password` (validated email → *"Password recovery is not connected yet. No reset email has been sent."*); `#/reset-password` (the future recovery-email link target — no token handling, *"Password reset is not connected yet. No password has been changed."*). Valid submissions never claim success, create accounts, send email, or authenticate; passwords clear after completed submissions; nothing is stored, logged, or transmitted (regression-tested).

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
    App.tsx — Phase 19 shell: feature hooks + router outlet (one page at a time)
    pages/ — WelcomePage, OnboardingPage (Phase 20), SignInPage (Phase 21),
             CreateAccountPage, VerifyEmailPage, RecoverPasswordPage, ResetPasswordPage (Phase 22),
             HomePage, ChatPage, ProjectsPage, CodeIntelligencePage, ModelManagerPage,
             SettingsPage, NotFoundPage, PageHeader
    components/ — TitleBar, Sidebar/HistoryList + nav, RouterLink, WelcomePreview (Phase 20),
                  GoogleMark (Phase 21),
                  ModelSetup, WorkspacePanel, FilePreview, CodeIntelligencePanel, RetrievalBar,
                  ContextBudgetBar, AnalysisPanel, EvidencePanel (Phase 17),
                  PatchEditor/PatchPreview/PatchGenerateBar, ValidationPanel,
                  MessageList/CodeBlock, Composer
    features/
      chat/ — chat.ts reducers, useConversation/usePersistentConversation (stream+cancellation)
      workspace/ — useWorkspace (inventory+context), contextBudget, retrieval (+smart),
                   useAnalysis/useImpact, usePatch/usePatchGeneration/usePatchRepair/useValidation
    lib/ — ipc (typed), inference (Supervisor), router (hash router, Phase 19),
           formValidation (Phase 22), symbolIndex/analysis (typed), evidence (bounded report),
           patch/patchContract, validation, buildConfig, etc.
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
  services/
    nexora-auth/  standalone Rust/Axum service foundation (Phase 23A, health-only
                  scaffold — NOT part of the Tauri process or the pnpm workspace;
                  never auto-starts; no authentication yet) + Phase 23B schema
                  (migrations/: users, oauth_accounts) and src/password.rs
                  (Argon2id hashing; no endpoints, no DB connection yet)
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
    phase-19/ README  (UI architecture & separate-page navigation)
    phase-20/ README  (welcome & onboarding page)
    phase-21/ README  (sign-in interface & frontend interactions)
    phase-22/ README  (create account & password recovery interfaces)
    phase-23/ README  (auth backend audit & security architecture plan)
    phase-23a/ README (secure Rust auth backend foundation — health-only scaffold)
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

**Phase 19 verification, 2026-10-18, working tree on `arena/1008499f-nexora` (base HEAD `128781618917fd15d2923aeb8eec04e9a5e90e98`):**

* `pnpm check` **PASS** — typecheck; 46 test files / **518 tests** (baseline 44/500; +`lib/router.test.ts` 7 + `App.navigation.test.tsx` 11, no test removed); production build (100 modules, CSS 73.78 kB / 11.59 kB gzip, JS 435.73 kB / 126.38 kB gzip, same two pre-existing Vite chunking warnings).
* Navigation suite covers: every sidebar item reaches its page, `aria-current` active state, AI Chat ⇄ Projects separation (no explorer on chat, no composer/transcript on projects), draft preserved across page switches, hash navigation without document reload, graceful unknown-route fallback, real quick-action links.
* Rust/Tauri/Windows checks **not run** (no `cargo`/`rustc`, Linux runner) — Phase 19 changes no Rust source. Browser-preview HTTP smoke for the shell, router, and all seven page modules returned 200; agent-side visual inspection unavailable.

**Phase 20 verification, 2026-10-18, working tree on `arena/1008499f-nexora` (base HEAD `128781618917fd15d2923aeb8eec04e9a5e90e98`, Phase 19 work preserved uncommitted):**

* `pnpm check` **PASS** — typecheck; 47 test files / **529 tests** (baseline 46/518; +`App.welcome.test.tsx` 9 and 2 router tests, no test removed); production build (103 modules, CSS 80.64 kB / 12.66 kB gzip, JS 441.82 kB / 127.86 kB gzip, same two pre-existing Vite chunk warnings).
* `git diff --check` **PASS**. Vite HTTP smoke for the flow modules returned 200 (pipeline only).
* Welcome suite covers: entry screen without the workspace shell, Continue → honest onboarding-next (no fake sign-in inputs), Back → Welcome, Open the workspace → `#/home`, direct load/refresh of `#/onboarding`, graceful unknown routes from the flow, Tab+Enter on Continue, workspace navigation intact.
* Rust/Tauri/Windows checks **not run** (no toolchain; no Rust changes).

**Phase 21 verification, 2026-10-18, working tree on `arena/1008499f-nexora` (base HEAD `128781618917fd15d2923aeb8eec04e9a5e90e98`, Phases 19–20 preserved uncommitted):**

* `pnpm check` **PASS** — typecheck; 48 test files / **540 tests** (baseline 47/529; +`App.signin.test.tsx` 11, no test removed); production build (106 modules, CSS 83.52 kB / 13.08 kB gzip, JS 449.65 kB / 130.20 kB gzip, same two pre-existing Vite chunk warnings).
* `git diff --check` **PASS**. Vite HTTP smoke for the sign-in modules returned 200 (pipeline only).
* Sign-in suite covers: rendering with labelled fields/autocomplete, validation errors with accessible association, invalid-email handling with password retention, accessible visibility toggle, keyboard submission with the honest not-connected status (no success claim), loading/disabled + blocked repeat submissions, credentials never stored or logged, Google informational status, recovery/create-account placeholder navigation, Back + Welcome/onboarding integration, unknown-route fallback, workspace navigation intact.
* Rust/Tauri/Windows checks **not run** (no toolchain; no Rust changes).

**Phase 22 verification, 2026-10-18, working tree on `arena/1008499f-nexora` (base HEAD `128781618917fd15d2923aeb8eec04e9a5e90e98`, Phases 19–21 preserved uncommitted):**

* `pnpm check` **PASS** — typecheck; 50 test files / **558 tests** (baseline 48/540; +`App.signup.test.tsx` 9 and `App.recovery.test.tsx` 9, no test removed; one P21 navigation test's heading assertions updated to the real pages it now reaches); production build (110 modules, CSS 83.66 kB / 13.10 kB gzip, JS 462.60 kB / 132.44 kB gzip, same two pre-existing Vite chunk warnings).
* `git diff --check` **PASS**. Vite HTTP smoke for the four pages + `formValidation.ts` returned 200 (pipeline only).
* Suites cover: create-account rendering/labels, required/invalid-email/min-length/mismatch validation with accessible error association, value retention on failure, both visibility toggles, keyboard submission with honest status + post-submit password clearing, loading/disabled + blocked repeat submissions, no credentials in storage/console/network (spied), verification-pending honesty (no code/token/resend) with working Back, recovery validation + honest status with no false claims and no network, reset page (no token field) + validation + honest status, Sign In ⇄ account-page navigation, browser history back, direct route loads, unknown-route fallback, welcome/workspace flow intact.
* Rust/Tauri/Windows checks **not run** (no toolchain; no Rust changes).

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
| Separate-page UI architecture & navigation | **Implemented** (Phase 19) | Hash-router shell with Home/AI Chat/Projects/Code Intelligence/Local Models/Settings + not-found fallback; state preserved across pages; no new dependency, no backend changes. |
| Welcome & onboarding entry flow | **Implemented** (Phase 20) | Welcome screen at `#/` with Continue → `#/onboarding` (honest Phase 21 sign-in slot) → workspace at `#/home`; flow renders outside the sidebar; no auth, no persistence. |
| Sign-in interface (frontend only) | **Implemented** (Phase 21) | `#/signin` with validation, visibility toggle, loading/disabled states, honest not-connected status; Google entry point informational; `#/signup` + `#/recover-password` are Phase 22 placeholders. No authentication backend. |
| Create account / verification / recovery / reset interfaces | **Implemented** (Phase 22) | `#/signup`, `#/verify-email`, `#/recover-password`, `#/reset-password` with local validation (8-char new-password floor), honest not-connected statuses, no credentials stored/logged/sent. Backend remains future work. |
| Auth service (standalone) | **Registration, verification, login, sessions, recovery & Google OAuth exchange implemented, unverified** (Phase 23A–23G) | `services/nexora-auth`: isolated Rust/Axum package, loopback bind by default. 23C adds `POST /v1/auth/register` (enumeration-safe 202), `/v1/auth/verify-email`, `/v1/auth/resend-verification` with sha256-at-rest single-use tokens (migration 3), honest email-provider abstraction (unconfigured = no mail, never claimed). 23D adds `POST /v1/auth/login` (generic `invalid_credentials`, timing-equalized), `/v1/auth/logout` (204 family revocation), `/v1/auth/refresh` (rotation + family reuse detection), `GET /v1/auth/me`, sha256-at-rest sessions with absolute+idle expiry (migration 4), and DB-backed `login_attempts` rate-limit windows (no lockout). 23E adds `POST /v1/auth/recovery-request` (enumeration-safe always-202), `/v1/auth/reset-password` (single-use sha256-at-rest tokens; transactional consume + Argon2id update + all-sessions revocation), and `/v1/auth/password-change` (migration 5). 23F adds `POST /v1/auth/google/exchange` (server-side PKCE exchange, hashed single-use state, plan §2.5 linking rules keyed by Google `sub`, migration 6). 23G hardens it: redacting `Debug`s + log scrub layer, real OIDC ID-token signature verification (RS256 vs Google JWKS + iss/aud/exp), exponential rate-limit backoff, password-change audit lines, and a secret-leakage test suite. Desktop IPC wiring is a later plan step. Rust sources **not compiler-verified** (sandbox: no cargo/crates.io/PostgreSQL). |
| Multi-file patch proposal | **Implemented** (roadmap continuation — Phase-9 deferred item, not a new product phase) | Contract v2 `files[]` (1–20 files), `create/validate/apply_patch_batch` with **atomic all-or-nothing apply** + rollback; batch preview UI on the generation path. Single-file v1 contract untouched. Details: `docs/multi-file-patch/README.md`. |
| Cloud fallback / embeddings / vector DB | **Not planned** — local-only | Never added. |
| Monetisation / subscriptions | **Not planned** | Never added. |
| Rust/Windowing real-model verification | **Unverified** in this sandbox | Requires local Rust toolchain + Windows + `llama-server b11714` + GGUF; CI on Linux/Windows covers compile/lint/tests but not inference. |

## 12. Phase History (14–23G + roadmap continuations)

* **Phase 14 — Code intelligence & project-aware analysis** (`a15b304`, 2026-10-18) — bounded symbol index (`symbol_index.rs`, regex, 500/5 MiB), project overview stats, symbol search (exact→prefix→substring), deterministic retrieval (selected first + ranked), grounded `CodeIntelligencePanel + RetrievalBar` + `groundedContextBlock` header. Limits Truman truncated/unsupported, sensitive excluded. Verification: `pnpm typecheck PASS`, `pnpm test PASS` (new 373-range), `pnpm build PASS`, `cargo BLOCKED` (no toolchain). Limitation: regex not tree-sitter, others `unsupported`.
* **Phase 15 — AI self-correction & validation** (`c2a2a71`, 2026-10-18) — bounded `validation.rs` (fmt/clippy-like checks, per-diagnostic), `useValidation/ValidationPanel` (NotRun/Unsupported honest), `usePatchRepair` (`explain/repair` via local model streamed, cancellable, maxAttempts, revised not auto-applied). All via existing `generate`. Verification: typecheck PASS, tests + new `ValidationPanel/useValidation/usePatchRepair` PASS, build PASS, cargo BLOCKED. Limitation: no network checks, 3 s timeout.
* **Phase 16 — Advanced deep codebase intelligence** (`0187aea`, 2026-10-18) — bounded `analysis.rs` (500/5 MiB/2000 edges, ts/rs/py regex imports, verified vs unresolved via `existing_set`, cycles verified-only DFS), `get_dependency_graph/file_relationships/change_impact`, `retrieveRelevantFilesSmart` (adds verified deps/dependents), `AnalysisPanel` (Graph/File/Impact tabs, Verified green vs Inferred amber). Tests: `analysis.test.ts + useAnalysis 13 + useImpact 10 + AnalysisPanel 10 + retrieval.smart 7` (all deterministic mocks). Verification: typecheck PASS, tests 44 files, cargo BLOCKED. Limitation: no dynamic imports, inferred symbol-reference search (len>2, 50 refs, 120-char snippet).
* **Phase 17 — Evidence-first AI & grounded responses** (`f032318`, 2026-10-18, this phase) — bounded `lib/evidence.ts` (`EvidenceReport` 20 entries + limitations), `formatEvidenceForPrompt` (≤8 in prompt + reliability instructions: separate observations/Hypothesis:/Suggestion:, never invent paths/symbols/tests/builds/validation/lines, static analysis ≠ runtime, Not grounded + ask clarification, Verification not performed until actual report, note truncated uncertainty, general explanations allowed), `EvidencePanel` (chips near composer+explorer, reuses tokens; evidence details remain opaque for readability), `getPatchMissingEvidence` warning (`patch-generate__missing`) when target/dependency not retrieved, sensitive excluded. Tests: `evidence.test.ts 18 + EvidencePanel.test.tsx 11`, total **497 passing**, typecheck PASS, build PASS, cargo still BLOCKED (no toolchain in runner; no Rust changes, so no regression). Limitation: prompt manifest capped to avoid budget overflow.
* **Phase 18 — Premium desktop UI transformation** (2026-10-18; frontend-only, local working tree) — refined the existing token system/shell, selective glass, shared path/symbol search controls and truthful empty states, control spacing, shortcut copy, brief motion, opaque technical surfaces, and reduced-motion/transparency fallbacks. Search scope/data access and all backend/workflow safeguards remain unchanged; existing icon assets were retained and no dependencies were added. `pnpm check`: typecheck PASS, tests 44 files / 500 tests PASS, production build PASS (two pre-existing Vite chunking warnings). Rust/Tauri/Windows/local-model checks remain unavailable in this Linux sandbox. Preview HTTP smoke checks passed; agent-side visual/native runtime inspection was not possible. No commit, push, PR, or release was performed. Details: `docs/phase-18/README.md`.
* **Phase 19 — UI architecture & separate-page navigation** (2026-10-18, frontend-only) — dependency-free hash router (`lib/router.ts` + `RouterLink`), seven `pages/` views (Home dashboard, AI Chat, Projects with patch workflow, Code Intelligence, Local Models, Local Model Manager reused `ModelSetup`, Settings with real prefs, not-found fallback); `App.tsx` keeps all feature hooks so chat/project state survives switches; sidebar primary nav with `aria-current`, TitleBar page label; Ctrl+N/L/B and Esc preserved. `pnpm check`: typecheck PASS, tests **46 files / 518 tests** PASS (none removed), build PASS (100 modules, two pre-existing chunk warnings). No dependencies added; no Rust/Tauri changes; no auth/payment; glass/animation systems left for Phases 29/30. Details: `docs/phase-19/README.md`.
* **Phase 20 — Welcome & onboarding page** (2026-10-18, frontend-only) — dedicated Welcome screen at `#/` (brand lockup, verifiable local-first copy, decorative non-interactive product preview, primary **Continue**) → `#/onboarding` honest transitional screen reserving Phase 21's sign-in slot (no fake form) with **Back to Welcome** and **Open the workspace** (`#/home`); flow renders outside the workspace sidebar with title-bar chrome retained; `Ctrl+B` scoped to sidebar pages. `pnpm check`: typecheck PASS, tests **47 files / 529 tests** PASS (none removed), build PASS (103 modules, two pre-existing chunk warnings); `git diff --check` PASS. No dependencies, no Rust/Tauri changes, no auth/persistence. Details: `docs/phase-20/README.md`.
* **Phase 21 — Sign-in interface & frontend interactions** (2026-10-18, frontend-only) — dedicated Sign In page at `#/signin` (Welcome → Onboarding → Continue to Sign in): labelled email/password fields with `autocomplete`, accessible password-visibility toggle, client-side validation (field errors with `aria-invalid`/`aria-describedby`, password never cleared), loading/disabled submit with repeat-submission guard, honest terminal status *"Sign-in is not connected yet…"* (never success/session/navigation), Google entry point (inline four-colour G) with informational status, `#/signup` + `#/recover-password` honest Phase 22 placeholders, Back to onboarding. Credentials never sent/stored/logged (regression-tested); no OAuth/secrets/CSP changes. `pnpm check`: typecheck PASS, tests **48 files / 540 tests** PASS (none removed), build PASS (106 modules, two pre-existing chunk warnings); `git diff --check` PASS. Details: `docs/phase-21/README.md`.
* **Phase 22 — Create account & password recovery interfaces** (2026-10-18, frontend-only) — real pages replacing the P21 placeholders: `#/signup` (name/email/password/confirm; `lib/formValidation.ts`: trimmed name, syntactic email, **≥8-char** new passwords (NIST SP 800-63B floor, no composition rules), matching confirmation; accessible per-field errors, value retention, dual toggles, repeat-submission guard; honest *"Account creation is not connected yet. Your details have not been submitted."* with passwords cleared post-submit), `#/verify-email` (verification-pending: no code/token/resend, working Back), `#/recover-password` (*"Password recovery is not connected yet. No reset email has been sent."*), `#/reset-password` (future email-link target, no token handling, *"…No password has been changed."*). Nothing stored/logged/transmitted (spied tests); PlaceholderPage removed as superseded. `pnpm check`: typecheck PASS, tests **50 files / 558 tests** PASS (none removed), build PASS (110 modules, two pre-existing chunk warnings); `git diff --check` PASS. Details: `docs/phase-22/README.md`.
* **Phase 23 — Auth backend audit & security architecture plan** (2026-10-18, docs-only) — read-only audit (no auth backend exists; 38 Tauri commands are inference/history/workspace only; auth pages have zero transport) + full architecture plan: axum+Postgres+argon2id API with Rust-held keyring sessions, Google OIDC+PKCE, DB entities, API contracts, env inventory, threat model, test/deploy strategy. `docs/phase-23/README.md`.
* **Phase 23A — Secure Rust auth backend foundation** (2026-10-18) — standalone `services/nexora-auth` Rust/Axum package (deliberately outside any Cargo workspace and the pnpm workspace; `apps/desktop/src-tauri` untouched): `GET /health` returning exactly `{"service":"nexora-auth","status":"ok"}` (service health only — claims nothing about auth/db/email/OAuth), predictable `{"error":{code,message}}` 404/405 envelopes, 4 KiB body limit, loopback bind by default (`127.0.0.1:8787`) with validated `NEXORA_AUTH_ALLOW_NON_LOOPBACK=true` opt-in for anything else, secret-free config errors. 18 Rust tests written (9 config unit + 6 HTTP-surface + 3 bind); frontend unchanged (558 tests still green). Rust checks not executable in this sandbox (no toolchain/crates.io egress) — documented honestly. Details: `docs/phase-23a/README.md`.
* **Phase 23B — Database foundation & password security** (2026-10-18, unverified-Rust caveat) — `services/nexora-auth` gains SQLx-convention migrations (`users`: uuid PK, normalized-email CHECK+UNIQUE, nullable Argon2id PHC `password_hash`, status enum CHECK; `oauth_accounts`: separate linking table, UNIQUE(provider, subject), no provider tokens) with documented forward-recovery reversals, plus `src/password.rs` (Argon2id m=19456 KiB/t=2/p=1/32-byte, per-hash OsRng salt, PHC strings, generic verification failures, 10 unit tests) and `NEXORA_AUTH_DATABASE_URL` validation with redacted `Debug`/never-echoed errors. 3 real-PostgreSQL integration tests written `#[ignore]`d (no mock substitution). **Verification honesty: `cargo` absent (exit 127) + crates.io TLS-blocked (exit 35) — nothing compiled or ran; frontend regression 50/558 still green.** 23C blocked until a toolchain verifies 23A+23B. Details: `docs/phase-23b/README.md`.

Earlier phases summarized in `docs/phase-1/README.md` (planning index), `docs/phase-2/README.md` (research), `docs/phase-5/README.md` (runtime contract), `docs/phase-6/7/8/9/README.md` (verification, code-aware rendering) — see those folders for full historical logs.

## 13. Known Limitations

* **Rust toolchain not available in this sandbox** — `cargo fmt/check/clippy/test` cannot be executed here; CI on `ubuntu-24.04` and `windows-2022` is the verification path. This has been **BLOCKED** since Phase 10 and remains so; Phase 18 was frontend/CSS-only and Phase 19 is frontend architecture-only — neither changes Rust source.
* **Windows & real model inference not verified here** — `cargo build` links Windows exe, `tauri build` NSIS installer, and `llama-server b11714 + GGUF` stream require a Windows machine with Build Tools + WebView2 — not exercised in CI beyond compile.
* **Workspace analysis is regex-bounded, not a language server** — `ts/tsx/rs/js/jsx/py` via regex; `.json/.md/.toml/.yaml/.css/.html/.sh/.go/.java` are `unsupported` (counted, not parsed). Dynamic imports, macros, re-exports beyond regex are missed; `referencedBy` is substring search (≥3 chars, 50 refs). Treat `inferred` as suggestion until verified.
* **Graph is verified-only for edges** — `unresolved` means `external` or `not found` via `existing_set`; no npm/cargo resolution, no transitive container analysis.
* **Context budget is estimated** — `chars/4 +5%` not a real tokenizer; may over/under-estimate dense code; second guard is `MAX_CONTEXT_*` 100 KiB/8 files. History oldest-first trimmed, `prompt` never dropped.
* **No whole-repo dump** — retrieval and evidence are bounded (8 files, 20 entries) so answers saying “Not grounded in the provided files” are correct when context incomplete — add files to context or enlarge budget.
* **Sensitive files excluded** — `.env*`, `*.pem`, `*.key`, `credentials.json` etc are never indexed, retrieved, graphed, or evidenced; `Patch` refuses them (sensitive error), `getPatchMissingEvidence` flags them.
* **No whole-history persistence across machines** — conversations are local `conversations.json`; closing window loses unsaved? Actually it persists locally, not synced.
* **Browser preview is not the desktop app** — `isDesktopShell()` false disables inference, shows `Desktop app required`; no simulated replies.

## 14. Phase 22 Completion — Create Account & Password Recovery

* **Scope preserved:** account-flow frontend only. No real authentication, email delivery/verification, reset-token issuance or validation, sessions, identity provider, or account database (Phase 23+); no Google OAuth, payments, glassmorphism (29), animation system (30), or workspace redesigns. All uncommitted Phase 19–21 work was preserved and built upon.
* **Pages:** `#/signup` (full name, email, password, confirm — with documented **≥ 8-character** new-password floor (NIST SP 800-63B lower bound; no composition rules), matching confirmation, accessible per-field errors, focus-first-invalid, values retained on failure, dual visibility toggles, loading/disabled states, repeat-submission guard); `#/verify-email` (honest verification-pending experience — no code, token, or resend, working Back to Sign In); `#/recover-password` (validated email → *"Password recovery is not connected yet. No reset email has been sent."*); `#/reset-password` (the future recovery-email link target; no token handling; *"Password reset is not connected yet. No password has been changed."*).
* **Honesty & security:** no form claims success, creates an account, sends email, or authenticates; passwords are cleared after completed submissions (never on validation errors); nothing is stored, logged, or transmitted — covered by spied storage/console/network regression tests. `PlaceholderPage` was removed as superseded by the real pages.
* **Verification and limitations:** `pnpm check` PASS (typecheck, 50 files / 558 tests, production build with the two pre-existing Vite chunk warnings); `git diff --check` PASS. Exact commands and Windows/manual limitations are recorded in `docs/phase-22/README.md`.

## 15. Phase 21 Completion — Sign In Page & Frontend Interactions

* **Scope preserved:** sign-in interface frontend only. No create-account/verification/reset flows (Phase 22), no authentication backend/OAuth/sessions/identity provider (Phase 23), no glassmorphism (29), animation system (30), payments, or workspace redesigns. All uncommitted Phase 19 + 20 work was preserved and built upon.
* **Sign-in page (`#/signin`):** Nexora branding and step strip consistent with Welcome/Onboarding; heading *"Welcome back"*; labelled email (`type=email`, `autocomplete=email`) and password (`type=password`, `autocomplete=current-password`) fields; accessible Show/Hide password toggle (`aria-pressed`/`aria-controls`); primary Sign in with loading (`Signing in…`, `aria-busy`) and disabled states; **Forgot password?** → `#/recover-password` and **Create account** → `#/signup` (honest Phase 22 placeholders); Back to onboarding; recognisable four-colour Google "G" entry point (inline SVG).
* **Frontend behaviour:** real client-side validation with per-field errors beside inputs (`role="alert"`, `aria-invalid`, `aria-describedby`), focus management, password never cleared on error, keyboard submission, and a repeat-submission guard while the local submission state is active. Valid submission reports *"Sign-in is not connected yet. Account authentication will be enabled in a later phase."* — never a success claim, never a session, never navigation into an authenticated area. Google click reports that integration arrives with the authentication backend.
* **Security:** credentials are never transmitted, stored, or logged (regression-tested against storage writes and console output); no hardcoded credentials, no API/OAuth secrets, no CSP or Tauri capability changes, no fake authenticated state.
* **Verification and limitations:** `pnpm check` PASS (typecheck, 48 files / 540 tests, production build with the two pre-existing Vite chunk warnings); `git diff --check` PASS. Exact commands and Windows/manual limitations are recorded in `docs/phase-21/README.md`.

## 16. Phase 20 Completion — Welcome & Onboarding Page

* **Scope preserved:** entry-flow frontend only. No Rust/Tauri IPC, inference, workspace, chat, patch, or settings changes; no sign-in/create-account/OAuth/session/auth backend (Phases 21–23); no payments/licensing; no glassmorphism (Phase 29) or animation/aurora system (Phase 30); no redesign of the workspace pages. All uncommitted Phase 19 work was preserved and built upon.
* **Welcome screen:** `#/` (alias `#/welcome`) outside the workspace sidebar — Nexora brand lockup, headline *"Code with an AI that runs on your machine."*, concrete capability list, and a subtle local-first note limited to verifiable claims (llama.cpp + user-supplied GGUF; prompts processed locally, not sent to a cloud service). The product preview is a decorative, `aria-hidden`, non-interactive composition made from existing tokens and the local `ApertureMark`.
* **Continue/Back:** **Continue** → `#/onboarding`, the dedicated slot for Phase 21's sign-in (labelled placeholder — no fake form or dead controls), with **Back to Welcome** and **Open the workspace** → `#/home`. Direct load/refresh of either route renders normally; unknown routes keep the graceful fallback. Onboarding completion is intentionally not persisted.
* **Shell/keyboard:** title-bar window chrome retained on the flow; sidebar and its toggle hidden (`showSidebarToggle`); `Ctrl+B` scoped to pages with a sidebar; `Ctrl+N`/`Ctrl+L` unchanged; workspace state and local AI init untouched (hooks stay mounted in `App.tsx`).
* **Verification and limitations:** `pnpm check` PASS (typecheck, 47 files / 529 tests, production build with the two pre-existing Vite chunk warnings); `git diff --check` PASS. Exact commands and Windows/manual limitations are recorded in `docs/phase-20/README.md`.

## 17. Phase 19 Completion — UI Architecture & Separate-Page Navigation

* **Scope preserved:** frontend architecture only. No Rust/Tauri IPC, inference, persistence, retrieval, evidence, validation, or patch-safeguard changes; no auth/account/OAuth (later phases); no payment/licensing. Glassmorphism (Phase 29) and animation/welcome systems (Phase 30) intentionally not started.
* **Architecture:** dependency-free hash router (`lib/router.ts`) with `#/`, `#/chat`, `#/projects`, `#/code`, `#/models`, `#/settings` and an honest not-found fallback; one page component per feature under `ui/src/pages/`; all feature hooks stay mounted in `App.tsx` so chat/project state survives page switches; in-app links share `components/RouterLink.tsx`.
* **Separation:** AI Chat and Projects are distinct views with distinct content areas (chat: transcript/composer/prompt-grounding tools; projects: workspace explorer/file preview/patch workflow). Code Intelligence (symbols/graph), Local Models (the existing `ModelSetup` + engine controls), Settings (real persisted prefs, read-only engine config, shortcuts), and a Home dashboard round out the pages. No feature's full interface is embedded inside another.
* **Navigation:** sidebar primary nav with active styling and `aria-current="page"`; TitleBar shows the active page; Ctrl+N/L/B and Esc-to-stop preserved (Ctrl+N/L route to the chat page first); browser back/forward work through hash history; page switches never reload the document; no dead links or buttons.
* **Verification and limitations:** `pnpm check` PASS (typecheck, 46 files / 518 tests, production build with the two pre-existing Vite chunk warnings). Exact commands, the test-route updates, and Windows/native limitations are recorded in `docs/phase-19/README.md`.

## 18. Phase 18 Completion — Premium Desktop UI Refinement

* **Scope preserved:** UI/UX only. No new search backend/functionality, prompt changes, Rust/Tauri IPC changes, cloud inference, telemetry, or monetization. Existing local inference, code intelligence, evidence, validation, persistence, streaming/cancellation, and patch-apply safeguards remain unchanged.
* **Design:** graphite/charcoal foundation, restrained violet action/focus accent, shared surface/status/motion tokens, selective glass on title bar/sidebar/search controls/composer; opaque explorer and technical/code/evidence surfaces; simplified nested cards and empty-state decoration.
* **Search:** project filter remains local, case-insensitive substring matching on inventory paths only; indexed symbol search keeps the existing operation and 20-result limit. Clear actions, accessible names, counts/no-match feedback, submitted-query no-results, and stale-request handling were refined without changing data access.
* **Accessibility and responsiveness:** keyboard-focusable conversation transcript; preserved shortcuts including visible Esc-to-stop help; visible focus and named search/clear controls; compact layouts through existing breakpoints; reduced-motion and reduced-transparency styles; opaque fallbacks if backdrop filters are unavailable.
* **Assets:** retained the existing Nexora aperture icon and all configured Tauri PNG/ICO assets; no icon or packaging configuration changes were needed.
* **Verification and limitations:** see `docs/phase-18/README.md` for exact commands/results, baseline comparison, branch/commit discrepancy, and Windows/runtime verification still required.

## 19. Contribution and Git Workflow

* **Current Phase 23G checkout:** `arena/1008499f-nexora` at base HEAD `128781618917fd15d2923aeb8eec04e9a5e90e98` (post PR #3), with Phases 19–23G + the multi-file patch proposal uncommitted in the working tree. [Correction (Phase 23D): this line still said “Phase 23B / 19–23B” after 23C landed — updated to the actual tree.] Phase 18 ran on `arena/555ee411-nexora` at `6ea0dbd9` (its brief's branch mismatch was preserved without switching). This Arena session is fixed to its assigned branch — never switch/create/push other branches.
* **Local only:** commits stay local. `isPrivate: false` (public) via `gh repo view` — do **not** push, do not create PR, do not change visibility, do not publish/release, do not delete branches/tags/history, do not run `git reset --hard` / `git clean -fd`. Reports are capped ~128 MB / 10k files; keep large datasets out of Git per `.gitignore`.
* **Safe dev loop:** `pnpm install` → `pnpm typecheck` → `pnpm test` (deterministic mocks, not real model) → `pnpm build` → `cargo` gates (where available — recheck; currently BLOCKED in sandbox, run locally before ship). Keep `pnpm-lock.yaml` exact pins (`buildConfig.test.ts` enforces).
* **No secrets:** never commit `.env`/keys/tokens (`.gitignore` covers `.env*`, `*.pem`, `credentials.json`), never log prompt/key/model output.
* **Files are saved automatically** after each tool turn — no manual `git commit` required to preserve work, but explicit commits are kept for history.

## 20. Release Status

* **Current status: not released, not published.** Repository is public (`cybervault-Hacky/Nexora`, `isPrivate: false`) but no installer has been built or uploaded here — `src-tauri/target` not present, `ui/dist` is ignored build output, no GitHub Release. CI builds on Linux/Windows for verification only; bundling/`tauri build` is deliberately absent from CI.
* **Publishing requires explicit founder approval.** Do not push commits, do not create PR, do not change `isPrivate`, do not publish builds/releases, do not delete `origin` tags/commits. When approved, package on Windows: `pnpm build && cargo build` then `pnpm tauri build` → NSIS per-user installer in `src-tauri/target/release/bundle/nsis/`. Keep `update notices` per `docs/phase-2/model-acquisition-policy.md`.

---

*Phase 18 UI refinement verification — 2026-10-18 — session branch `arena/555ee411-nexora`, base HEAD `6ea0dbd9784db6f1e353fb87be84686cc12ae46e`; `pnpm check` PASS (44 files / 500 tests, typecheck and build); Rust/Tauri/Windows checks unavailable. Browser preview HTTP smoke passed; agent-side visual inspection was unavailable. The phase brief named `arena/74aec262-nexora` / `d662588`; the actual checkout was preserved without switching branches. No dependencies were added; no commit, push, PR, release, or visibility change occurred.*

*Phase 19 UI architecture verification — 2026-10-18 — session branch `arena/1008499f-nexora`, base HEAD `128781618917fd15d2923aeb8eec04e9a5e90e98`; `pnpm check` PASS (46 files / 518 tests, typecheck and production build; two pre-existing Vite chunk warnings). Hash-router page architecture with separate AI Chat / Projects / Code Intelligence / Local Models / Settings / Home views; navigation suite passes. Rust/Tauri/Windows checks unavailable (no Rust changes). No dependencies were added; no commit, push, PR, release, or visibility change occurred.*

*Phase 20 welcome & onboarding verification — 2026-10-18 — session branch `arena/1008499f-nexora`, base HEAD `128781618917fd15d2923aeb8eec04e9a5e90e98` (Phase 19 tree preserved); `pnpm check` PASS (47 files / 529 tests, typecheck and production build; two pre-existing Vite chunk warnings) and `git diff --check` PASS. Welcome screen at `#/` with Continue → `#/onboarding` (honest Phase 21 slot) → workspace at `#/home`; no auth or persistence added. Rust/Tauri/Windows checks unavailable (no Rust changes). No dependencies were added; no commit, push, PR, release, or visibility change occurred.*

*Phase 21 sign-in interface verification — 2026-10-18 — session branch `arena/1008499f-nexora`, base HEAD `128781618917fd15d2923aeb8eec04e9a5e90e98` (Phases 19–20 preserved); `pnpm check` PASS (48 files / 540 tests, typecheck and production build; two pre-existing Vite chunk warnings) and `git diff --check` PASS. Sign In at `#/signin` with real client-side validation, accessible visibility toggle, honest not-connected status (no success claims), Google informational entry point, and `#/signup`/`#/recover-password` Phase 22 placeholders. Credentials are never sent, stored, or logged. Rust/Tauri/Windows checks unavailable (no Rust changes). No dependencies were added; no commit, push, PR, release, or visibility change occurred.*

*Phase 22 account interfaces verification — 2026-10-18 — session branch `arena/1008499f-nexora`, base HEAD `128781618917fd15d2923aeb8eec04e9a5e90e98` (Phases 19–21 preserved); `pnpm check` PASS (50 files / 558 tests, typecheck and production build; two pre-existing Vite chunk warnings) and `git diff --check` PASS. `#/signup`, `#/verify-email`, `#/recover-password`, `#/reset-password` with local validation (8-char new-password floor) and honest not-connected statuses; nothing stored, logged, or transmitted (spied tests). Rust/Tauri/Windows checks unavailable (no Rust changes). No dependencies were added; no commit, push, PR, release, or visibility change occurred.*

*Phase 23A auth service foundation verification — 2026-10-18 — session branch `arena/1008499f-nexora`, base HEAD `128781618917fd15d2923aeb8eec04e9a5e90e98` (Phases 19–23 preserved); `pnpm typecheck` + `pnpm test` PASS (50 files / 558 tests, app untouched) and `git diff --check` PASS. New standalone `services/nexora-auth` (Axum 0.8) with health-only surface, loopback-default binding, and 18 written Rust tests (recounted at the Phase 23B verification gate); `cargo test/fmt/check` NOT runnable in this sandbox (no Rust toolchain, no crates.io egress) — reported as not run, not as passes. `pnpm build` skipped to preserve `dist/` as instructed. No commit, push, PR, release, or visibility change occurred.*

*Phase 23B database foundation & password security verification — 2026-10-18 — session branch `arena/1008499f-nexora`, base HEAD `128781618917fd15d2923aeb8eec04e9a5e90e98` (Phases 19–23A preserved); frontend regression `pnpm typecheck` + `pnpm test` PASS (50 files / 558 tests) and `git diff --check` PASS. Added `users`/`oauth_accounts` SQLx migrations and Argon2id `password.rs` (+ `NEXORA_AUTH_DATABASE_URL` validation) with 13 written unit tests (10 password + 3 config) and 3 `#[ignore]`d real-PostgreSQL integration tests. **Rust verification gate FAILED: `cargo` absent (exit 127), crates.io TLS-blocked (exit 35), PostgreSQL absent — no Rust code compiled or ran and no database test executed; Phase 23C is blocked until a toolchain verifies 23A+23B.** Nothing committed, pushed, published, or released. Phase 23B verification gate (2026-10-18): re-confirmed **BLOCKED/UNVERIFIED** with the same exact exit codes (cargo/rustc/rustfmt 127; crates.io/index/static 35; PostgreSQL absent); frontend regression re-run green (50/558); no source changes made; test counts recounted and prior docs corrected (23A: 18 written tests, not 21; 23B: 13 unit tests, not 14).

*Roadmap continuation — multi-file patch proposal (2026-10-18, NOT a new product phase; Phase 23C remains gated behind the BLOCKED auth verification) — the Phase-9 deferred item from the feature roadmap: contract v2 `files[]` (1–20 files, unique case-insensitive paths, all v1 per-file safety rules) mirrored in `patchContract.ts`/`patch_contract.rs`; `create/validate/apply_patch_batch` with **atomic all-or-nothing apply** (whole-batch validation + stale refusal before any write; failed batches roll back files already replaced) + 12 written Rust tests (NOT RUN — no toolchain, same blocker); new IPC commands registered; batch preview/apply-all UI (`PatchPreview` `files` prop, `previewBatch`/`appliedBatch` states). `pnpm typecheck` PASS; `pnpm test` **50 files / 569 tests** PASS (+11, none removed); `git diff --check` PASS. Single-file v1 contract untouched. Details: `docs/multi-file-patch/README.md`. Nothing committed, pushed, published, or released.

*Phase 23C registration & email verification (2026-10-18, UNVERIFIED-Rust caveat) — plan sequence step 2 in `services/nexora-auth`: `POST /v1/auth/register {name,email,password}` → 202 `{status:"verification_pending"}` (byte-identical for duplicate emails — enumeration defence; UNIQUE+23505 handled under concurrency), `POST /v1/auth/verify-email {token}` (32-byte OsRng token, **sha256-at-rest only**, single-use+expiry+revocation in SQL, transactional consume→verify), `POST /v1/auth/resend-verification {email}` (revokes prior live tokens). New migration `20261018000003_create_email_verification_tokens.sql`; `src/email.rs` provider trait with honest `UnconfiguredEmailProvider` (**no email claimed when none sent**; SMTP vars rejected loudly); config `NEXORA_AUTH_VERIFICATION_TTL_SECONDS`/`EMAIL_*` validated at startup, values never printed. Frontend **unchanged** — honest disconnected state preserved (no secure transport exists; CSP + non-auto-start service). Tests: 33 written (10 registration units, 2 email, 4 config, 8 HTTP no-DB, 9 real-PG `#[ignore]`d) — **NONE RUN** (cargo 127 / crates.io 35 / PostgreSQL absent); `pnpm typecheck` + `pnpm test` 50/569 PASS; `git diff --check` PASS. Details: `docs/phase-23c/README.md`. Nothing committed, pushed, published, or released.*

*Phase 23D login & secure sessions (2026-10-18, UNVERIFIED-Rust caveat) — plan sequence step 3 in `services/nexora-auth`: `POST /v1/auth/login {email,password}` (generic `invalid_credentials` for unknown accounts and wrong passwords with dummy-Argon2id timing equalization; `email_unverified` for correct-password-unverified accounts; locked/deleted refused), `POST /v1/auth/logout` (204, whole-family revocation), `POST /v1/auth/refresh` (in-family rotation with family reuse detection), `GET /v1/auth/me`. Sessions: 32-byte OsRng tokens **sha256-hashed at rest** (`migrations/20261018000004_create_sessions_and_login_attempts.sql`), absolute+idle expiry, throttled `last_used_at` that cannot revive revoked rows, tokens never logged/placed in URLs. DB-backed `login_attempts` windows rate-limit login/register/resend (multi-instance-safe, failures-only for login, `Retry-After`, no permanent lockout). Frontend **byte-unchanged** (secure desktop transport still absent — integration documented as blocked; local AI stays login-free). Tests: 35 written (5 session, 6 login, 4 rate-limit, 3 config, 7 HTTP no-DB, 10 real-PG `#[ignore]`) — **NONE RUN** (cargo 127 / crates.io 35 / PostgreSQL absent); `pnpm typecheck` + `pnpm test` 50/569 PASS; `git diff --check` PASS. Details: `docs/phase-23d/README.md`. Nothing committed, pushed, published, or released.*

*Phase 23E password recovery & reset (2026-10-18, UNVERIFIED-Rust caveat) — plan sequence step 4 in `services/nexora-auth`: `POST /v1/auth/recovery-request {email}` (always-202 enumeration-safe; 32-byte OsRng reset tokens **sha256-at-rest**, single-use, TTL 60 min default, supersession of outstanding tokens, minted only for active accounts), `POST /v1/auth/reset-password {token,password}` (transactional single-use consume + Argon2id update + **all-sessions/families revocation** + reset/verification-token invalidation; generic `invalid_or_expired_token` for every token state), `POST /v1/auth/password-change` (plan step-4 contract; revokes other session families). Concurrency hardening: login/create_session now verify+insert under the account row lock (`FOR UPDATE`) so a racing login can never keep a stale session after a password change. New migration `20261018000005_create_password_reset_tokens.sql`; envs `NEXORA_AUTH_RESET_TTL_SECONDS`/`NEXORA_AUTH_RATE_LIMIT_RECOVERY`; DB-backed recovery windows (429 + `Retry-After`, no lockout). Frontend **byte-unchanged** (recovery/reset UI stays honest; secure transport still absent — integration blocked; no delivery claimed when email unconfigured). Tests: 26 written (6 recovery units, 2 config, 8 HTTP no-DB, 10 real-PG `#[ignore]`) — **NONE RUN** (cargo 127 / crates.io 35 / PostgreSQL absent); `pnpm typecheck` + `pnpm test` 50/569 PASS; `git diff --check` PASS. Details: `docs/phase-23e/README.md`. Nothing committed, pushed, published, or released.*

*Phase 23F Google OAuth exchange (2026-10-18, UNVERIFIED-Rust caveat) — plan §9 sequence step 5 (“Google OAuth (state/PKCE/loopback + linking rules; wire the Google button)”) backend half in `services/nexora-auth`: `POST /v1/auth/google/exchange {code, codeVerifier, state, redirectUri}` → `200 {user, session}` | `409 email_conflict` (plan §6). Server-side PKCE code exchange via a `GoogleProvider` seam (`HttpGoogleProvider` → Google's token endpoint; secret-less and secret-bearing client types both supported — the client secret stays in the server secret store; `UnconfiguredGoogleProvider` answers honest 503 and invents nothing); hashed **single-use state** with 10-min window (migration 6 `google_oauth_states`); exact loopback redirect-URI match; optional OIDC nonce validation; plan §2.5 linking rules keyed by immutable Google `sub` (verified-match auto-link; unverified match → 409 guided message, no auto-link; unverified provider email refused; OAuth-only accounts with NULL `password_hash`); sessions identical to password login; `google` rate windows (429 + `Retry-After`). Frontend **byte-unchanged** — Google-button/IPC/keyring wiring documented as BLOCKED (no secure transport). New deps `reqwest`+`base64` (justified in Cargo.toml; resolution unverified). Tests: 29 written (8 oauth units, 3 config, 6 HTTP no-DB, 12 real-PG `#[ignore]`) — **NONE RUN** (cargo 127 / crates.io 35 / PostgreSQL absent); `pnpm typecheck` + `pnpm test` 50/569 PASS; `git diff --check` PASS. Details: `docs/phase-23f/README.md`. Nothing committed, pushed, published, or released.*


*Phase 23G authentication security hardening (2026-10-19, UNVERIFIED-Rust caveat) — plan §9 sequence step 6 (“Rate limiting + abuse hardening + secret-leakage test suite”) in `services/nexora-auth`: redacting `Debug` impls on all 9 secret-bearing request/message/session/claims types; new `src/scrub.rs` log-scrub layer (plan §4 list: password/token/authorization/cookie/code_verifier + equivalents) with metadata-only `password_change` audit lines; **real OIDC ID-token signature verification** replacing payload-only decoding (RS256 vs Google JWKS with rotation handling + iss/aud/exp checks, `jsonwebtoken` replaces `base64`; unverified payloads never trusted — no fake verification); exponential rate-limit backoff (doubling Retry-After per exhausted window, capped 24 h — no indefinite lockout); forwarded headers remain untrusted (plan defines no trusted-proxy config; keys from the socket peer only). New suites: `tests/secret_leakage.rs` (sentinels vs Debug/errors/scrub/HTTP envelopes), `tests/validator_props.rs` (property tests on validators), `tests/hardening_db.rs` (real-PG backoff/boundary/proxy-header, `#[ignore]`). Frontend **byte-unchanged**. Tests: 21 new (178 crate total written) — **NONE RUN** (cargo 127 / crates.io 35 / PostgreSQL absent); `pnpm typecheck` + `pnpm test` 50/569 PASS; `git diff --check` PASS. Details: `docs/phase-23g/README.md`. Nothing committed, pushed, published, or released.*
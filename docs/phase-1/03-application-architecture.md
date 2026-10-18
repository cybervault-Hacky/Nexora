# 03 — Proposed Application Architecture

**Status:** Proposed for owner approval  
**Related:** [02-ux-ui-specification.md](02-ux-ui-specification.md), [04-ai-runtime-and-model-strategy.md](04-ai-runtime-and-model-strategy.md), [05-coding-agent-workflow.md](05-coding-agent-workflow.md), [07-security-and-privacy.md](07-security-and-privacy.md)

---

## 1. Architectural goals

1. **Windows-native reliability** with correct DPI, window management, process containment, and installer behaviour.
2. **A hard security boundary** between the rendered UI, the agent policy engine, the model, and the operating system.
3. **Responsiveness under load:** streaming inference must never freeze typing, scrolling, or panel transitions.
4. **Process isolation:** model crashes, build crashes, and preview-server crashes must not take down the application.
5. **Evidence integrity:** every check, diff, and tool result is persisted and tied to file hashes so the UI cannot show stale success.
6. **Maintainability:** clear crate/package boundaries, typed IPC, no duplicated privilege checks.
7. **Local-first data:** projects are plain folders; app data is local, exportable, and deletable.
8. **Reversible actions:** checkpoints before mutation, atomic writes, safe shutdown.

---

## 2. Desktop framework decision (D-01)

### 2.1 Options considered

| Criterion | Tauri 2 (Rust + WebView2) | Electron (Node + Chromium) | .NET WinUI 3 / WPF + WebView2 | Qt/C++ |
|-----------|---------------------------|----------------------------|-------------------------------|--------|
| Installer size (typical) | ~5–20 MB shell, runtime packs separate | ~120–250 MB | ~50–120 MB + .NET runtime | ~40–100 MB |
| Idle RAM (shell/UI) | ~40–100 MB | ~150–400 MB | ~80–150 MB | ~50–120 MB |
| Cold start | Sub-second shell | 1–2 s typical | Fast native shell | Fast native shell |
| Rendering consistency on Windows | WebView2 (Chromium) | Bundled Chromium | WebView2 | Native |
| Security model | Capability-scoped IPC, Rust core | Requires hardening (contextIsolation, sandbox, no nodeIntegration) | Strong native boundary | Strong native boundary |
| Native Windows APIs | Excellent via `windows` crate | Via native modules / N-API | Excellent | Excellent |
| UI ecosystem for premium web-like design | Excellent (React/CSS) | Excellent | Weaker for glass/motion-heavy UI | Custom, expensive |
| Dev velocity for AI/agent logic | Rust + TS | TS/Node (fastest) | C# | C++ |
| Team skill assumption | Rust + TS | TS | C# + XAML | C++/Qt |
| Auto-update / installer | Tauri updater, NSIS | electron-builder, mature | MSIX/WiX | Custom |
| Multi-window / child webviews | Supported | Mature | Supported | Supported |
| Risk | Rust ramp-up; WebView2 quirks | Size/RAM/startup; security hardening burden | UI polish and web-tooling friction | Cost, licensing, velocity |

### 2.2 Recommendation

**Use Tauri 2 with a Rust core and a React 19 + TypeScript UI on Windows WebView2.**

**Advantages**

- Small installer and low idle memory leave more RAM/VRAM headroom for local models.
- The Rust core is the natural home for process supervision, filesystem containment, policy enforcement, checkpointing, and model management — exactly the operations that must not live in JavaScript.
- Capability-scoped IPC is a better default than Electron's broad main-process authority.
- WebView2 is Chromium-based and present on Windows 11; because Nexora targets Windows only, the classic Tauri "different webview on every OS" problem does not apply.
- Rust produces a small, signed, predictable binary and makes a future headless agent CLI straightforward.

**Disadvantages and mitigations**

| Risk | Mitigation |
|------|------------|
| Rust development velocity | Keep crates small and well-typed; generate TS types from Rust; spike the hardest parts in Phase 2 before committing |
| WebView2 `backdrop-filter` performance | Blur budget, opaque fallbacks, measurement on Intel Iris Xe class hardware |
| WebView2 accessibility quirks | NVDA audit in Phase 2/3; native fallback controls for critical paths |
| IPC streaming overhead | Use Tauri channels with batched deltas (20–30 Hz), not one event per token |
| WebView2 runtime missing (old Windows 10) | Evergreen bootstrapper in installer; Windows 11 target |
| Sidecar process management complexity | One supervisor module with a tested state machine and job objects |
| Debugging across Rust/JS | Structured tracing, IPC inspector, replayable event logs |

**Why not Electron:** the product already consumes several GB of RAM for a local model. Adding 200–400 MB of Chromium and a 150–250 MB installer is a poor trade for a premium, fast Windows app. Electron's Node ecosystem is attractive, but the security-critical work would still need a separate privileged service to be credible. Electron remains the **fallback** if the Phase 2 Tauri spike fails performance, accessibility, or IME gates.

**Why not .NET/WinUI 3:** excellent native integration, but the glass-heavy, motion-rich, web-tooling-centric UI (code editor, diff, preview, terminal) would be slower to build and harder to make visually consistent. A WebView2-heavy hybrid reintroduces most Tauri trade-offs without the Rust security core.

**Why not Qt/C++:** strongest native performance, but highest cost and slowest iteration for an AI product whose UI and prompts will change frequently.

**Reversal conditions:** Phase 2 spike fails any of: 60 fps panel transitions with glass on Intel Iris Xe; NVDA can operate the composer, sidebar, and dialogs; IME composition is correct; 10k-message conversation scrolls at 60 fps; streaming 30 Hz does not affect typing latency; sidecar supervision meets crash/restart gates. If two or more fail, re-evaluate Electron.

---

## 3. Process model and trust boundaries

```
┌─────────────────────────────────────────────────────────────────────────────┐
│ Windows user session                                                        │
│                                                                             │
│  ┌──────────────────────────────┐   typed IPC (commands/channels)           │
│  │ Nexora.exe (Tauri host)       │◄──────────────────────────────┐          │
│  │ Rust core                     │                               │          │
│  │  · policy engine              │                     ┌─────────┴───────┐  │
│  │  · agent orchestrator         │                     │ WebView2        │  │
│  │  · model supervisor           │                     │ React UI        │  │
│  │  · checkpoint/store           │                     │ (no Node, CSP)  │  │
│  │  · tool runtime               │                     └─────────────────┘  │
│  └───────┬───────────┬───────────┘                                          │
│          │           │                                                      │
│  ┌───────▼───────┐ ┌─▼──────────────────────┐  ┌────────────────────────┐  │
│  │ llama-server  │ │ Job objects             │  │ Preview iframe          │  │
│  │ 127.0.0.1     │ │  · node/npm/build/tests │  │ localhost dev server    │  │
│  │ token, no UI  │ │  · dev server           │  │ sandboxed, no IPC       │  │
│  └───────────────┘ │  · msedge --headless    │  └────────────────────────┘  │
│                    └─────────────────────────┘                              │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │ Local stores: SQLite · blobs · checkpoints · logs · models · vault   │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Trust zones

| Zone | Trust level | Can do | Cannot do |
|------|-------------|--------|-----------|
| **Z0 — UI renderer** | Untrusted for privilege | Render state, send user intents, edit text | Access filesystem, spawn processes, read model handles, call arbitrary IPC |
| **Z1 — Rust core** | Trusted application code | Enforce policy, read/write allowed paths, spawn contained children, talk to inference | Bypass user consent for scoped actions |
| **Z2 — Model output** | Untrusted input | Propose text, tool calls, patches | Execute anything directly; its text is data |
| **Z3 — Tool children** | Least privilege | Run one command in a job object with scoped access | Escape job, access network unless granted, persist after kill |
| **Z4 — Inference server** | Local service | Serve model on loopback with bearer token | Bind externally, expose web UI, enable experimental agent tools |
| **Z5 — Preview content** | Untrusted web content | Render in a sandboxed iframe from localhost | Access Nexora IPC, open arbitrary windows, read app files |
| **Z6 — Network** | Explicitly gated | Model downloads, dependency installs, update checks | Silent prompt/file upload, background telemetry |

### Process rules

- **Single instance:** a named mutex ensures one Nexora per user session; a second launch focuses the existing window.
- **Renderer:** `contextIsolation` equivalent is inherent; no Node integration; strict CSP; navigation blocked to non-app URLs; remote content only in the sandboxed preview iframe.
- **Inference:** child process, own job object, loopback bind, random port, bearer token written to a user-only file with restrictive ACL, `--no-webui`, no `--tools`/`--agent` experimental flags, killed with the job on exit.
- **Tools:** each command runs in its own job object with `KILL_ON_JOB_CLOSE`, memory/CPU limits where compatible, restricted token, sanitized environment, no inherited secrets.
- **Preview servers:** own job object; loopback bind enforced by flags (`--host 127.0.0.1` or framework equivalent); never exposed to LAN.
- **Browser checks:** Edge headless via CDP in a temporary profile, job-contained, loopback only.

---

## 4. Module structure and dependency rules

```
apps/desktop/            Tauri shell, command registration, window lifecycle
crates/
  nexora-core/           domain types, errors, config, time, IDs
  nexora-protocol/       IPC DTOs, JSON Schemas, TS type export
  nexora-store/          SQLite, migrations, repositories, blobs, checkpoints
  nexora-inference/      runtime packs, model registry, supervisor, HTTP client
  nexora-context/        repo scan, ignore rules, tree-sitter map, search, index
  nexora-tools/          filesystem, patch, process, search, git-read, preview, browser
  nexora-policy/         permissions, path guard, command classifier, redaction
  nexora-security/       secret scanning, manifest verification, Windows containment
  nexora-agent/          state machine, planner, budgets, verification, reporting
  nexora-diagnostics/    logging, crash capture, bundle export
ui/                      React application, design system, feature modules
resources/               templates, model manifests, icons, runtime manifests
```

**Dependency direction (acyclic):**

```
ui ──► protocol
apps/desktop ──► agent, inference, store, tools, diagnostics, protocol
agent ──► tools (interfaces), policy, context, store, inference (client), protocol, core
tools ──► policy, security, store (blob), core
inference ──► store, security, core
context ──► core, store
store ──► core
policy ──► core
security ──► core
protocol ──► core
core ──► (no internal crates)
```

**Rules enforced in CI:**

- `nexora-core` must not depend on any other Nexora crate.
- `nexora-policy` and `nexora-security` must not depend on `nexora-agent` or `ui`.
- Only `apps/desktop` may register Tauri commands.
- No crate may call `std::process::Command` directly; all spawning goes through `nexora-tools::process`.
- No crate may read arbitrary paths; all filesystem access goes through `nexora-policy::PathGuard`.
- UI code may not contain privilege logic; it may only render decisions and request actions.

---

## 5. IPC protocol

### 5.1 Design

- **Commands** are request/response, typed, validated, and explicitly registered. No dynamic method names from the UI.
- **Channels/events** carry high-frequency streams: model tokens, task step updates, log lines, download progress, permission requests.
- **Schemas** are generated from Rust (`serde` + `schemars`) and exported to TypeScript (`ts-rs` or `specta`). A protocol version is exchanged at startup; mismatches are rejected with a clear error.
- **Idempotency:** mutating commands accept an idempotency key so retries after a UI restart do not duplicate writes.
- **Errors:** stable code + human message + optional details + retryable flag + correlation ID. The UI never parses error strings.

### 5.2 Representative command set

| Command | Purpose | Privilege |
|---------|---------|-----------|
| `app.get_state` | Bootstrap UI state, versions, feature flags | none |
| `project.open` | Register a folder, validate path, start indexing | read |
| `project.create` | Scaffold from template | write |
| `conversation.create/send/stop` | Conversation lifecycle and generation | model |
| `model.list/install/remove/load` | Model management | network/disk |
| `task.get/cancel/approve` | Task state and approvals | policy |
| `permission.respond` | Grant/deny a pending request | user |
| `checkpoint.list/restore` | Inspect and restore snapshots | write |
| `preview.start/stop/status` | Dev server lifecycle | process/network |
| `editor.read_file/write_file` | User-initiated file edits (separate from agent tool) | write |
| `diagnostics.export` | Build a local bundle | read/log |
| `settings.get/set` | Validated settings | config |

### 5.3 Representative event streams

| Stream | Payload | Rate control |
|--------|---------|--------------|
| `generation.delta` | message id, text delta, sequence | batched 20–30 Hz, coalesced |
| `generation.status` | state, step, elapsed, tokens | on change, max 5 Hz |
| `task.step` | step id, state, title, detail | on change |
| `task.tool_call` | tool, args summary, status, duration | on change |
| `download.progress` | model id, bytes, total, stage | 4 Hz |
| `log.line` | level, target, redacted message | 10 Hz, ring buffer |
| `permission.request` | request id, scope, action, preview | on demand |

### 5.4 IPC security rules

1. Every command validates its arguments against a schema and a policy check; validation failure is a typed error, never a panic.
2. Paths are canonicalized and checked against the project root and explicit grants; symlink and junction escapes are rejected.
3. The UI cannot pass a raw command string, environment map, or executable path to the process tool.
4. File contents streamed to the UI are size-capped and typed; binary content is never injected into DOM as HTML.
5. Event payloads are treated as untrusted by the UI for rendering (escape by default).
6. Permission responses are bound to a request ID and expire; a delayed grant cannot authorize a different action.

---

## 6. Request and agent execution sequence

```
UI                Core/Policy            Inference           Tools              Store
│  send message      │                      │                  │                  │
├───────────────────►│ build context        │                  │                  │
│                    ├─ retrieve files ─────┼──────────────────┼─────────────────►│
│                    ├─ budget tokens       │                  │                  │
│                    ├─ start run ──────────┼──────────────────┼─────────────────►│
│                    ├─ stream request ────►│                  │                  │
│                    │◄──── delta ──────────┤                  │                  │
│◄── batched delta ──┤                      │                  │                  │
│                    │◄──── tool call ──────┤                  │                  │
│                    ├─ validate schema     │                  │                  │
│                    ├─ policy decision     │                  │                  │
│◄── permission ─────┤ (if required)        │                  │                  │
│───────────────────►│                      │                  │                  │
│                    ├─ checkpoint ─────────┼──────────────────┼─────────────────►│
│                    ├─ execute tool ───────┼─────────────────►│                  │
│                    │◄──── result ─────────┼──────────────────┤                  │
│                    ├─ append tool result ─┼──────────────────┼─────────────────►│
│                    ├─ continue generation ►│                  │                  │
│                    ├─ run verification ───┼─────────────────►│                  │
│                    ├─ persist evidence ───┼──────────────────┼─────────────────►│
│◄── final report ───┤                      │                  │                  │
```

Key invariants:

- One active generation per conversation; one mutating run per project.
- A tool call is persisted **before** execution and updated after; a crash leaves an auditable "unknown/aborted" state.
- File writes are atomic and checkpointed; a failed batch leaves the project unchanged or fully restorable.
- Verification results are bound to the post-change file hash; any later edit marks them stale.

---

## 7. Storage architecture

### 7.1 Locations

| Data | Default location | User-configurable | Notes |
|------|------------------|-------------------|-------|
| App + runtimes | `%LOCALAPPDATA%\Programs\Nexora` | Installer choice | Per-user, no admin |
| App data | `%LOCALAPPDATA%\Nexora` | No | DB, logs, cache |
| Models | `%LOCALAPPDATA%\Nexora\models` | Yes | Large, relocatable |
| Checkpoints | `%LOCALAPPDATA%\Nexora\checkpoints` | Yes | Never in project |
| Logs | `%LOCALAPPDATA%\Nexora\logs` | No | Rotated, redacted |
| Temp | `%LOCALAPPDATA%\Nexora\tmp` | No | Cleaned on start |
| Secrets | Windows Credential Manager (DPAPI) | No | Never SQLite/logs |

Project-local metadata is limited to a small `.nexora/project.json` (project id, settings overrides, ignore rules) and optionally `.nexora/` ignore entries. It contains no secrets and no conversation history. Users may delete it without breaking the project.

### 7.2 SQLite schema (proposed)

```
schema_migrations(version, applied_at)
projects(id, name, root_path, created_at, last_opened_at, trust_state, settings_json)
conversations(id, project_id, title, created_at, updated_at, archived, model_id, summary)
messages(id, conversation_id, role, content_json, created_at, token_count, status, parent_id)
runs(id, conversation_id, project_id, title, state, started_at, ended_at, budget_json, error_code)
steps(id, run_id, ordinal, kind, title, state, started_at, ended_at, detail_json)
tool_calls(id, run_id, step_id, tool, args_summary_json, result_summary, exit_code,
           started_at, ended_at, decision, correlation_id)
file_changes(id, run_id, path, change_type, before_hash, after_hash, additions, deletions, accepted)
checkpoints(id, project_id, run_id, label, manifest_path, created_at, pinned, size_bytes)
checks(id, run_id, kind, command, status, exit_code, output_excerpt, evidence_path,
       created_at, file_hash)
models(id, manifest_id, version, path, quant, size_bytes, sha256, installed_at, status)
permissions(id, project_id, scope, decision, granted_at, expires_at, last_used_at)
audit_log(id, ts, actor, action, target, decision, detail_json)   -- append-only
settings(key, value_json)
attachments(id, message_id, path, mime, size, sha256)
```

- SQLite with `WAL`, `foreign_keys=ON`, `busy_timeout=5000`, `synchronous=NORMAL`.
- One writer connection owned by a dedicated database thread; readers use a small pool. Long queries are bounded and cancellable.
- Migrations are forward-only, transactional, and tested against a copy of the previous release database.
- FTS5 virtual table for conversation search; a separate per-project index database for code search and symbols so it can be rebuilt without touching user data.
- Daily integrity check (`PRAGMA quick_check`) and rolling backups (7 days). Corrupt DB → restore latest backup, preserve the corrupt file for diagnostics, notify the user.
- No secrets, tokens, or full file contents in the main DB. Message content is stored because it is the conversation; attachments reference content-addressed blobs.

### 7.3 Checkpoints

- Content-addressed blob store (`sha256`), compressed with zstd for text, copied/hardlinked for large files.
- Manifest per checkpoint: list of `{path, hash, mode, size, mtime}`, plus run id, label, timestamp, and the set of changed files.
- Ignore `.git`, `node_modules`, common build outputs, and files > 20 MB by default; configurable. Never silently skip a file the agent changed — if a changed file is excluded, record it and force a full copy.
- Restore is atomic per file (temp + rename) and verified by hash; if a file changed outside Nexora since the checkpoint, require confirmation and offer a side-by-side diff.
- Retention: last 20 checkpoints per project and 30 days, whichever is larger; pinned checkpoints exempt. Disk usage visible in Settings.
- Alternative considered: a shadow Git repository. It offers excellent diff/rollback semantics but adds Git object overhead, LFS/binary complexity, and potential confusion with the user's own repository. Revisit in Phase 2; the content-addressed design is simpler and framework-agnostic.

### 7.4 Secrets

- API keys for optional future providers, if any, live in Windows Credential Manager via the `keyring` crate (DPAPI-backed).
- The app never stores project secrets. It detects and excludes common secret files and values from context by default.
- Logs and diagnostics pass through a redaction layer before writing.
- Export/delete flows explicitly include or exclude secrets; there is no "export everything including secrets" shortcut.

---

## 8. Inference supervisor

### 8.1 State machine

```
NotInstalled ──install/verify──► Installed ──load──► Starting ──health──► Ready
      ▲                              ▲                 │                   │
      │                              │                 │ timeout/crash     │ stop
      └──────── remove ◄─────────────┴─────────────────┴────► Crashed ◄────┘
                                                               │ restart (≤3)
                                                               ▼
                                                            Degraded
```

- **NotInstalled:** no verified runtime pack.
- **Installed:** runtime and model files verified, process not running.
- **Starting:** process spawned, auth token loaded, waiting for `/health` with a 120 s timeout (larger models allowed more, with progress UI).
- **Ready:** health OK, `/props` reports the expected model/context; first warm-up request optional.
- **Crashed:** process exited unexpectedly; capture exit code and last 200 log lines; offer restart with reduced settings.
- **Degraded:** running with CPU fallback, reduced context, or failed GPU initialization; UI explains why.
- **Stopped:** user stopped or app is shutting down.

### 8.2 Launch contract

| Setting | Default | Notes |
|---------|---------|-------|
| host | `127.0.0.1` | Never `0.0.0.0` |
| port | ephemeral chosen by app, verified free | Passed explicitly; no fixed public port |
| api key | 256-bit random per install, stored in user-only file | `--api-key-file`, never argv |
| web UI | disabled | `--no-webui` where supported |
| server agent/tools | disabled | Do not enable experimental file/shell tools |
| parallel slots | 1 | Predictable memory and cancellation |
| context | chosen by hardware profile | User override with warnings |
| GPU layers | auto from VRAM probe | Fallback to fewer layers/CPU |
| KV cache | Q8_0 default | Q4_0 optional for tight VRAM |
| flash attention | auto | Enabled when supported |
| logs | app-owned file, size-capped | Redacted on export |

Startup sequence: verify runtime hash → verify model hash → choose config → write token file with ACL → spawn in job object → poll health with backoff → validate `/props` → mark ready. Shutdown: cancel requests → graceful terminate (2 s) → kill job tree → delete token file.

### 8.3 Failure recovery

| Failure | Detection | Recovery |
|---------|-----------|----------|
| Model file corrupt | Hash/header check before spawn | Reject, offer re-download |
| GPU OOM at load | Exit code/log pattern | Reduce GPU layers, context, KV precision; then CPU |
| GPU OOM during generation | Runtime error/stream close | Reduce context or layers, reload, inform user |
| Driver reset | Health failure, log pattern | Restart; if repeated, CPU fallback |
| Port in use | Bind failure | Choose a new port and retry once |
| Auth failure | 401 | Regenerate token, restart |
| Hang | No health/stream progress within timeout | Kill job, restart once, report |
| Repeated crashes | ≥ 3 in 10 min | Stop auto-restart, offer safe mode |

All recovery decisions are logged and visible in Settings → Diagnostics.

---

## 9. Tool runtime architecture

- **Single gateway:** `ToolRuntime::execute(ToolCall, RunContext)` is the only path to privileged work. It validates the schema, consults the policy engine, creates a checkpoint when required, executes, caps output, and records the result.
- **Process tool:** executable allowlist + subcommand policy; argv arrays; no shell; sanitized environment; working directory fixed to the project root; timeout (default 120 s, builds/tests up to 15 min); output cap (256 KB per stream, with full log to a capped file); job-object kill; stdin closed; no PTY for agent commands.
- **Filesystem tool:** `PathGuard` canonicalizes and checks roots, rejects symlink/junction escapes, enforces file size limits, detects binary content, uses atomic writes with newline/encoding preservation, and maintains a per-run write ledger to prevent the same file being written twice in one batch without re-reading.
- **Patch tool:** applies validated unified diffs or search/replace blocks with expected hashes; rejects ambiguous matches; dry-run produces the diff shown in the UI; failed application leaves the file unchanged.
- **Search tool:** ripgrep-style bounded search inside the project; ignores binary files and configured directories; caps results and context lines.
- **Preview tool:** starts/stops dev servers through the preview manager; validates loopback binding; captures readiness, logs, and console events.
- **Browser tool:** Edge headless via CDP; navigation restricted to loopback URLs; screenshots and accessibility scans stored as evidence.
- **Git tool:** read-only `status`, `diff`, `log`, `show` at 1.0; no `commit`, `push`, `reset`, or `checkout`.
- **Checkpoint tool:** creates/restores snapshots; restore is itself a privileged action requiring confirmation.

Detailed tool schemas and permissions are in [05-coding-agent-workflow.md](05-coding-agent-workflow.md). Containment implementation is in [07-security-and-privacy.md](07-security-and-privacy.md).

---

## 10. Preview and dev-server manager

- **Port allocation:** pick a free port in the ephemeral range, bind-check, pass it to the framework; verify the listening process belongs to our job before connecting.
- **Readiness:** wait for the port, then HTTP `GET /` with a short timeout; accept 2xx/3xx/4xx (a 404 on `/` may still mean the server is up), reject connection errors; also watch stdout for the framework's ready line.
- **Binding enforcement:** Vite `--host 127.0.0.1`, Next `-H 127.0.0.1`, Astro `--host 127.0.0.1`; if a framework ignores the flag, detect a non-loopback listener and stop it.
- **Lifecycle:** one preview per project; starting a second stops the first after confirmation; stop kills the job tree; app exit stops all.
- **Crash handling:** capture last 100 lines; show error plus "Restart" and "Open log"; do not auto-restart more than twice.
- **Isolation:** the preview renders in a sandboxed iframe with `allow-scripts allow-same-origin allow-forms` only; no top navigation, no popups; external links open in the system browser.
- **Screenshots:** Edge headless with a temp profile, viewport presets, device scale 1, waits for network idle or a 3 s cap, masks dynamic regions if configured.

---

## 11. Front-end architecture

### 11.1 Stack

| Concern | Choice | Rationale | Alternatives |
|---------|--------|-----------|--------------|
| UI framework | React 19 + TypeScript (strict) | Mature ecosystem, concurrent rendering, hiring pool | Svelte, Solid — smaller ecosystem for complex IDE surfaces |
| Build | Vite | Fast HMR, simple config | Webpack, Turbopack (not needed) |
| Styling | CSS variables + Tailwind CSS + component CSS for glass | Token discipline, no runtime CSS-in-JS cost | CSS Modules only; styled-components (runtime cost) |
| Primitives | Radix UI | Accessible dialogs, menus, popovers, tooltips | Headless UI, custom (risk) |
| Motion | Motion (Framer Motion successor) | Declarative, reduced-motion support | CSS-only; react-spring |
| Editor | CodeMirror 6 | Smaller/faster than Monaco, excellent diff/merge, tree-shakeable | Monaco (heavier, IDE features not needed at 1.0) |
| Terminal | xterm.js + portable-pty (Rust) | Proven ConPTY integration | Custom terminal (no) |
| State | Zustand per domain + event reducers | Minimal boilerplate, fine-grained subscriptions | Redux Toolkit (heavier), Jotai |
| Lists | TanStack Virtual | Virtualized messages/trees | react-window |
| Icons | Curated Lucide base + custom brand marks | Consistent stroke system, ISC licence | Mixed icon packs (rejected) |
| i18n | Externalized strings, ICU messages | Future localization | Hard-coded copy |

### 11.2 Rendering boundaries

- Shell (title bar, sidebar, status bar) — stable, rarely re-renders.
- Conversation list — virtualized; only the active streaming message subscribes to deltas.
- Composer — isolated; token counting debounced; never re-rendered by message stream.
- Task panel — event-driven, throttled to 5 Hz.
- Terminal — xterm instance outside React's render path; React only mounts it.
- Editor — CodeMirror instance owned by a controller; React passes documents and commands.
- Preview — iframe; React only controls URL, viewport, and toolbar.

### 11.3 State ownership

| State | Owner | Persistence |
|-------|-------|-------------|
| Projects, conversations, messages | Rust/SQLite (authoritative) | Durable |
| Active run, steps, checks | Rust (events) | Durable |
| UI layout, panel sizes, theme | UI (local) | `settings` table |
| Draft input | UI | Local draft table, debounced |
| Model download progress | Rust (events) | Durable checkpoint |
| Permission prompts | Rust (request) → UI (response) | Audit log |
| Preview URL/status | Rust | Session |

The UI is a projection. It may optimistically show a sent message and panel states, but it reconciles from persisted events after reconnect or restart.

### 11.4 Error boundaries

- One boundary per major panel; a crashed panel shows a contained error card with reload, not a blank window.
- Global boundary captures render errors, reports locally, and preserves the last good state.
- IPC errors are mapped to user-facing messages by code; unknown codes show a generic message plus a correlation ID.

---

## 12. Startup, shutdown, and crash recovery

### 12.1 Startup budget

| Stage | Target | Notes |
|-------|--------|-------|
| Single-instance + config load | < 100 ms | No network |
| Logging + DB open/migrate | < 300 ms | Integrity check async |
| Window + first paint | < 800 ms | Skeleton shell |
| Restore last project/conversation | < 1.5 s total | Lazy file tree |
| Interactive (NFR-P-01) | ≤ 2.5 s p95 cold | No model load |
| Model load | On demand | Never blocks shell |
| Update check | 10 s delay, if enabled | Cancellable |
| Indexing | Background, throttled | Pausable |

### 12.2 Shutdown

1. Stop accepting new commands.
2. Cancel active generation and tool calls; checkpoint any completed work.
3. Stop preview servers and browser checks.
4. Stop inference gracefully, then kill the job tree.
5. Flush database WAL; close connections.
6. Flush logs; delete temp files and the inference token file.

### 12.3 Crash recovery

- On launch, detect an unclean shutdown marker.
- Scan for runs in `running` state and mark them `interrupted`; preserve steps and tool calls.
- Offer "Resume task" only if the task journal has a safe continuation point; otherwise offer "Review changes" and "Restore checkpoint".
- Never auto-delete partial files; the checkpoint and write ledger determine what changed.
- Renderer crash: WebView2 reloads; UI rehydrates from the core; the run continues if the core is alive, otherwise it is marked interrupted.

---

## 13. Concurrency and cancellation

- Tokio multi-thread runtime for I/O and supervision; CPU-heavy work (hashing, parsing, tokenization) on a bounded blocking pool.
- Per-project async mutex for mutating runs; per-file advisory locks for edits; read operations use snapshots.
- One inference request at a time by default (`--parallel 1`); a queue shows position and estimated wait. Multiple projects share one loaded model; switching models drains the queue.
- Every long operation takes a `CancellationToken`; cancellation propagates to HTTP streams, child processes (job terminate), file operations (between chunks), and indexing.
- Bounded channels: token deltas may coalesce; terminal output is buffered to a capped ring; audit events are never dropped (backpressure instead).
- Timeouts: health 120 s, generation idle 120 s, command 120 s default, build/test 15 min, browser navigation 30 s. All configurable within safe bounds.
- A "panic switch" cancels everything and kills all job objects immediately.

---

## 14. Observability and diagnostics

- Structured logging with `tracing`: timestamp, level, target, span id, run id, project id (hashed path where sensitive), message.
- JSONL logs, 10 MB × 5 rotation, local only. A redaction layer removes tokens, keys, `.env` values, and configured patterns.
- Separate append-only `audit_log` in SQLite for privileged actions: actor (user/agent/system), action, target, decision, result, correlation id.
- Performance spans for startup, IPC, context build, inference first token, tool execution, verification, and checkpoint.
- Diagnostics bundle (zip): app/runtime versions, hardware report, settings with secrets removed, recent logs, task summaries, optional selected files, and a manifest. The user previews the contents before export. No automatic upload.
- Crash handling: panic hook writes a local report with a backtrace and recent spans; WebView2 process failure is captured; no prompt/file content is included.
- A hidden developer panel (behind a setting) shows IPC traffic, model requests, and timings for support.

---

## 15. Performance architecture

| Concern | Design |
|---------|--------|
| Streaming | 20–30 Hz batched deltas over a channel; UI appends to a buffer and flushes on `requestAnimationFrame` |
| Message list | Virtualized above 200 messages; stable keys; memoized message components |
| Composer | Isolated subscriptions; debounced token count; cached auto-grow metrics |
| File tree | Lazy loading; virtualized; watcher events debounced 150 ms and batched |
| Editor | CodeMirror viewport rendering; large files read-only; no full-document diff above 2 MB |
| Preview | Iframe compositing; no React re-render on page navigation; screenshot work off the UI thread |
| Model | Separate process; VRAM budget; idle unload; no model work on the UI thread |
| DB | Writer thread; short transactions; indexes on foreign keys and timestamps; FTS for search |
| Startup | Lazy panels, async integrity check, no model load, no synchronous network |
| Memory | Bounded caches; image downscaling; log ring buffers; explicit limits on retained tool output |

Budgets are enforced by automated performance tests in CI on a reference machine and by telemetry-free local benchmarks.

---

## 16. Architectural trade-offs

| Decision | Benefit | Cost | Revisit if |
|----------|---------|------|------------|
| Tauri + Rust core | Security, size, memory, native control | Rust velocity; WebView2 dependency | Spike fails gates |
| Single Rust process for core | Simple IPC, shared state | One crash domain for core features | Core instability |
| Separate inference process | Crash isolation, GPU memory control | Startup latency, supervision complexity | Never (keep) |
| SQLite + blob store | Simple, local, portable | Manual concurrency and migration discipline | Multi-user sync added |
| Content-addressed checkpoints | Framework-agnostic, reliable | Disk usage, no Git semantics | Shadow Git proves better |
| Sandboxed iframe preview | Strong isolation, simple | Limited DOM inspection across origin | Need deeper automation (use CDP) |
| No cloud dependency | Privacy, offline | No remote fallback for weak hardware | Product permits BYO endpoint |
| TypeScript generated from Rust | Contract safety | Build step, tooling | Never (keep) |
| One active mutating run | Safe, predictable | Less parallelism | Multi-agent orchestration added |

---

## 17. Architecture acceptance criteria

- [ ] Stack decision approved, with Phase 2 spike gates defined.
- [ ] Trust zones and process rules accepted.
- [ ] Crate boundaries and dependency rules accepted.
- [ ] IPC command/event model and security rules accepted.
- [ ] Storage schema, checkpoint design, and secret handling accepted.
- [ ] Inference supervisor lifecycle and failure matrix accepted.
- [ ] Preview isolation model accepted.
- [ ] Startup/shutdown/crash recovery behaviour accepted.
- [ ] Performance budgets mapped to tests.
- [ ] No implementation begins before Phase 2 authorization.

---

## 18. Sources and assumptions

Framework size/startup/memory ranges are industry-typical comparisons, not measured on Nexora hardware; they will be verified in the Phase 2 spike. Key references:

- Tauri 2 documentation — https://v2.tauri.app/
- Tauri security/capabilities — https://v2.tauri.app/security/
- Electron security checklist — https://www.electronjs.org/docs/latest/tutorial/security
- WebView2 documentation — https://learn.microsoft.com/microsoft-edge/webview2/
- Windows Job Objects — https://learn.microsoft.com/windows/win32/procthread/job-objects
- Windows AppContainer isolation — https://learn.microsoft.com/windows/win32/secauthz/appcontainer-isolation
- llama.cpp server README — https://github.com/ggml-org/llama.cpp/blob/master/tools/server/README.md
- SQLite WAL — https://sqlite.org/wal.html
- CodeMirror 6 — https://codemirror.net/
- xterm.js — https://xtermjs.org/
- portable-pty — https://github.com/wez/wezterm/tree/main/pty
- Radix UI — https://www.radix-ui.com/

Access date: 2026-10-17. All versions and capabilities must be re-verified at the start of Phase 2.

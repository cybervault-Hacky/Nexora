# Nexora Desktop

The Tauri 2 desktop client for Nexora. Its source includes a native-window shell,
React interface, workspace analysis, and explicit patch generation/review/application
paths, plus a Rust core designed to supervise a separately supplied `llama-server`
process. These source features are not evidence of verified Windows behavior. The
installer configuration includes neither a runtime executable nor model weights.
Approved model profiles are acquired separately after an explicit user action; the
source checks expected size and SHA-256 before accepting an artifact, but no genuine
model download or inference has been verified.

The current verification environment has no Cargo toolchain, so no native Rust
compile or test was run here. [`.github/workflows/ci.yml`](../../.github/workflows/ci.yml)
defines Linux and Windows Rust/packaging jobs, but those jobs were not triggered for
Phase 18 or 19. The native window, Windows installer, and real model behavior remain
unverified. See [Phase 18](../../docs/PHASE_18_INTEGRATION_VERIFICATION.md) and
[Phase 19](../../docs/PHASE_19_WINDOWS_PACKAGING.md).

## Stack

| Layer | Choice |
|---|---|
| Shell | Tauri 2 (Rust) |
| UI | React 19 + TypeScript 5.9 |
| Bundler | Vite 7 |
| Package manager | pnpm 9 (workspace) |
| Styling | Hand-written CSS with design tokens, no UI framework |
| Runtime client | `reqwest` against loopback HTTP, SSE parsing in Rust |
| Tests | Vitest + jsdom + React Testing Library; `cargo test` for Rust units |

## Prerequisites (Windows)

1. **Node.js 22.22.3** and the pinned **pnpm 9.15.4** (`corepack enable pnpm`).
2. **Rust 1.93.0** via [rustup](https://rustup.rs). The exact toolchain and
   `rustfmt`/`clippy` components are pinned in `rust-toolchain.toml`.
3. **Visual Studio 2022 Build Tools** with "Desktop development with C++" and
   the Windows 10/11 SDK; run from an x64 developer shell where `cl.exe`,
   `link.exe`, and `rc.exe` are available.
4. **WebView2 Evergreen runtime**. NSIS uses Tauri's silent download
   bootstrapper when the runtime is absent, so installation may require internet
   access.
5. A compatible **`llama-server.exe` runtime** supplied/managed separately; the
   installer does not bundle or acquire it. The approved Nexora Mini/Sonnet GGUF
   weights are acquired separately through the explicit in-app download flow or
   selected from a local file, then verified against the approved SHA-256
   metadata. Neither model is included in the installer.

Tauri's full prerequisite list is at <https://v2.tauri.app/start/prerequisites/>.

## Commands

Run from the repository root unless noted.

```bash
pnpm install --frozen-lockfile  # exact workspace dependencies
pnpm dev                        # Vite only, http://localhost:1420
pnpm tauri dev                  # native window (runs Vite automatically)
pnpm typecheck                  # tsc --noEmit
pnpm test                       # Vitest: unit, hook, and DOM interaction tests
pnpm test:installer-contract    # metadata, icon, frontend, and payload assertions
pnpm build                      # typecheck + production UI build + security/installer scans
```

`cargo check` or `cargo clippy` run before `pnpm build` will fail: Tauri's
`generate_context!` embeds `ui/dist` at compile time. Build the UI first.

```bash
pnpm build && cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml
```

For a Windows x64 production installer, run the guarded packaging and smoke scripts
on a native Windows host from an x64 Visual Studio developer shell. The smoke
script refuses an existing Nexora install/shortcut registration or non-empty
roaming/local AppData; it uses a disposable Unicode/space test path under
`LOCALAPPDATA` that cannot overlap the default install or product-data roots. It
removes only content-matching test sentinels and an unchanged installer copy;
unexpected or modified test files are left for manual review:

```powershell
.\scripts\windows\build-installer.ps1
.\scripts\windows\smoke-verify.ps1
```

The build script checks Node.js 22.22.3, pnpm 9.15.4, Rust/Cargo 1.93.0,
the x64 MSVC target, and required compiler tools; it installs the frozen lockfile,
runs frontend typecheck/tests and installer-contract tests, then explicitly runs
`pnpm build` **before** Cargo formatting, Clippy, or Rust tests because Tauri's
native macro embeds `ui/dist`. The final Tauri packaging command runs the
configured frontend/security/contract build again. Existing NSIS `.exe` outputs
or `SHA256SUMS.txt` are never reused or overwritten: archive them outside the
bundle directory before another packaging run. The script requires exactly one
fresh setup executable and creates the SHA-256 manifest with create-new semantics.
Tauri 2.12.5 pins NSIS 3.13 and checks upstream toolchain/package hashes with
SHA-1 (source-level behavior, not an installer signature or provenance
attestation). The script enforces production settings and rejects configured
backend-secret environment names. It skips signing and reports an unsigned
installer unless a current-user Personal-store certificate with Code Signing EKU
and a matching thumbprint is configured; it checks validity, verifies
Authenticode, and requires the resulting signer thumbprint to match. PFX password
variables are rejected to avoid command-line exposure. Node/Rust/package versions
are pinned, but the hosted Windows/SDK image and WebView2 Evergreen bootstrapper
are external build/runtime inputs; the manifest identifies actual installer
bytes, not a complete reproducible-build attestation. The script does not publish
or distribute artifacts.

## Google sign-in packaging

Set the public installed-app `GOOGLE_OAUTH_CLIENT_ID` in the environment before
running the Windows packaging script. The Tauri build script embeds that public ID
in the native binary so a packaged app launched from the Start menu or Dock does
not depend on its parent shell environment; a runtime environment value can
still override it. The backend must be configured with the matching Google
client ID and, only if its OAuth client type requires one, the client secret.
Never put a client secret in the desktop build. Source configuration is designed to disable Google
sign-in when the public ID is missing or a placeholder; live OAuth and backend
behavior remain unverified. Signed-out local chat is intended to remain available.

## Connecting a model

The first screen (and the sidebar) reports the real engine state. With no model
configured, Nexora shows a setup panel instead of an empty chat:

1. **Runtime executable** — full path to `llama-server.exe`.
   `Check` runs `llama-server --version` and shows the first line it printed.
   A build that does not report the pinned number (llama.cpp b11714) is
   labelled as unverified rather than assumed compatible.
2. **Model file (.gguf)** — choose an approved Nexora Mini/Sonnet download in
   the model manager (started only by explicit user action) or select a local
   GGUF file. Downloads are published only after the expected size and SHA-256
   match the approved registry metadata. `Check` reads the header (magic,
   version 2–3, tensor count), requires the `.gguf` extension and a plausible
   size, and estimates memory need. The file is never uploaded anywhere.
3. **Context window** — 8K–64K in the UI (the core clamps to 2 048–131 072).
4. **Save and load model** — persists the choice to
   `<app config dir>/inference-settings.json`, starts the engine on a random
   loopback port with a random per-session key, and waits up to 180 s for
   `/health` to report success.

There is no file picker and no dialog permission: a path is only ever set by
deliberate typing. Reloading always restarts the process, so a changed model or
context size really takes effect.

If the model fails to load, the panel shows the core's own error plus the last
lines of engine output, and the engine is stopped rather than left half-alive.
Once a model is loaded, the sidebar engine card offers **Stop engine**, which
cancels any active generation, terminates the child process, and refreshes the
real status.

## Keyboard

| Shortcut | Action |
|---|---|
| `Enter` | Send (ignored during IME composition) |
| `Shift + Enter` | New line |
| `Esc` | Stop generation while streaming; otherwise dismiss a composer error |
| `Ctrl + L` | Focus the composer |
| `Ctrl + N` | New chat (cancels any in-flight request) |
| `Ctrl + B` | Collapse or expand the sidebar |

## Layout

```
apps/desktop/
├─ ui/
│  ├─ index.html
│  └─ src/
│     ├─ App.tsx                      shell composition, routing, shortcuts
│     ├─ components/                  title bar, sidebar, setup, messages, composer
│     ├─ features/chat/               chat model, conversation reducers, streaming hook
│     ├─ lib/                         typed IPC, inference client, status hook, scroll
│     ├─ test/                        jsdom primitives and status fixtures
│     └─ styles/                      tokens and application CSS
├─ src-tauri/
│  ├─ src/
│  │  ├─ lib.rs                       builder, managed state, 10 commands
│  │  └─ inference/
│  │     ├─ mod.rs                    state, request validation, status, cancel
│  │     ├─ commands.rs               the Tauri command surface
│  │     ├─ process.rs                child-process supervisor, key file, probes
│  │     ├─ client.rs                 streaming SSE client, HTTP error mapping
│  │     ├─ sse.rs                    byte-safe line framing
│  │     ├─ models.rs                 GGUF header + runtime executable checks
│  │     ├─ hardware.rs               physical/available memory
│  │     ├─ settings.rs               validated JSON settings
│  │     └─ error.rs                  typed, serialisable failures
│  ├─ capabilities/main.json          least-privilege window permissions
│  ├─ icons/                          generated desktop icons
│  └─ tauri.conf.json                 window, CSP, bundle configuration
├─ vite.config.ts
└─ vitest.config.ts
```

## Conversation behaviour

- Messages are created once with a stable id and an explicit `role` and state
  (`streaming`, `complete`, `stopped`, `error`).
- The composer sends a typed request over Tauri IPC. Text arrives through a
  `Channel<GenerationEvent>` and is appended to the single assistant message.
- Only one generation may run at a time. A second send while streaming is
  refused with a typed `busy` error; it is never queued.
- **Stop** calls `cancel_generation` with the request id. The Rust client wakes
  from its wait and drops the HTTP stream; llama.cpp notices the closed
  connection and stops the task (`POST /slots/0?action=erase` is **not** used —
  b11714 rejects slot actions unless `--slot-save-path` is set). The promise
  resolves with `cancelled: true`. The partial text stays visible
  and is labelled `Stopped`.
- A submission guard rejects a second submit inside 350 ms. Each new chat
  rotates an internal conversation id so a late token or completion from the
  previous conversation is dropped rather than appended.
- Errors keep whatever text arrived, mark the reply `Failed`, and show the
  core's message in a banner. An empty failed reply is removed instead of
  leaving a blank bubble.
- Conversation history is persisted by the Tauri core as `conversations.json` in
  `app_data_dir()`. Closing the window stops the live session but does not clear
  saved conversations. The smoke test seeds disposable history/settings/model
  storage sentinels and checks that install, same-build reinstall, uninstall,
  and reinstall-after-uninstall preserve them; it does not claim a prior-version
  upgrade test.

## Security posture

- The engine listens on `127.0.0.1` only, on an ephemeral port chosen at spawn.
  There is no LAN binding, no CORS relaxation, and no control endpoint exposed
  to the webview.
- The API key is 32 random bytes generated per launch, written to
  `runtime-session.key` in the app config directory (0600 on Unix), passed to
  `llama-server` with `--api-key-file` — never on the command line, never to
  the UI.
- The runtime is launched with structured arguments only
  (`--model`, `--host`, `--port`, `--ctx-size`, `--parallel 1`, `--no-ui`,
  `--jinja`, `--api-key-file`, `--timeout`), no shell, and no interpolated
  argument string. On Windows the child is created with `CREATE_NO_WINDOW` and
  is killed and reaped on unload, on window close, and on `Drop`.
- Engine stderr is drained continuously (so the child can never block on a
  full pipe); only the last 64 KiB is retained in memory and only the last six
  non-empty lines reach an error message. Nexora writes no log file and never
  logs prompt text or the key.
- Prompt and model output are treated as untrusted text. They are rendered as
  text only — never executed, never used to build a path or a command.
- At this checkout, `src-tauri/src/lib.rs` registers **86 application IPC
  command handlers** spanning local inference, model acquisition, history,
  workspace/file operations, patching/export, account/auth, and licensing.
  This is a broad trusted-webview API surface and must be reviewed separately
  from `capabilities/main.json`, which lists Tauri/plugin and window permissions.
  No generic shell-execution command is registered. The UI has no general
  browser HTTP access under the production CSP; the Rust core makes the defined
  HTTPS account/OAuth and explicit model-download requests, while inference
  talks only to the supervised loopback runtime. These are source/configuration
  findings, not native-runtime verification.
- CSP allows only local scripts, local styles, and Tauri IPC. There are no
  remote fonts, images, or scripts. The only window permissions are minimize,
  toggle-maximize, close, and start-dragging.
- In a plain browser (no Tauri), the UI states that the desktop app is required
  and disables sending. It never simulates a reply.

## Window chrome

The title bar is custom (`decorations: false`). Drag regions are dedicated
empty elements, so the sidebar toggle and window buttons remain clickable, and
double-click-to-maximize is delegated to Tauri's drag-region handling instead
of a manual handler.

**Known Windows limitation:** custom caption buttons do not participate in
Windows 11 Snap Layouts (the hover flyout on the maximize button), because that
requires answering `WM_NCHITTEST` with `HTMAXBUTTON`, which Tauri does not
expose. If Snap Layouts is a requirement, switch `decorations` to `true`,
remove the custom controls, and keep the in-app header. This needs an owner
decision and a Windows machine to verify.

## Tests

The current frontend suite runs **873 Vitest tests across 88 files** (jsdom plus a node-environment configuration suite):

- request validation, message factories, and turn reduction
- submission guard, conversation isolation, and message reducers
- streaming lifecycle: deltas, completion, stop, failure, stale-drop, duplicate
  submit, and new-chat cancellation, with the IPC boundary replaced by a
  scripted double (labelled as such in the test file)
- setup panel: memory reporting, runtime probe success/failure, model warnings,
  save-and-load, load failure, and absence of any success claim
- shell routing: browser preview, no model, loading, ready, and unreadable IPC
- composer disabled/counter/error/stop states and keyboard behaviour
- scroll-follow logic
- code rendering: fence and inline parsing, language labels, copy success and
  failure, streaming fences, and untrusted markup staying text
- IPC runtime: invoke names and argument objects, channel pass-through, typed
  and transport-level rejection normalisation, and closed-failure status helpers
- build configuration: Vite/Tauri port and output wiring, bundle icons, capability
  allowlist, CSP, and the pinned Rust and pnpm versions
- cross-language contract: serde field names, optional/nullable mapping, enum
  encodings, constructor integrity, command registration, invocation, and argument
  keys, compared against the real Rust sources (source-derived, not a serde
  execution; mutation-tested)

Rust unit tests live beside the code (`cargo test`): SSE framing and
classification, delta extraction, GGUF header validation, executable magic,
settings normalisation, memory parsing, request validation, the `--version`
line (captured from a real build, printed on stderr), and the mapping from HTTP
rejections to UI error codes. They run in CI on Linux and Windows; they cannot
run in the development sandbox, which has no Rust toolchain.

## Known limitations

- The Rust crate, the child-process launch, streaming, and cancellation have
  **not** been compiled or run here; no Rust toolchain is available in this
  container. They are unverified, not proven.
- No genuine model inference has been performed. No weights or binaries are
  committed or bundled. Approved model artifacts can be downloaded only after an
  explicit user action; no real model download or inference has been performed
  here. A runtime binary *was* built from the pinned source and its HTTP surface
  exercised on Linux; that
  proves the contract, not generation. Linux runs are not Windows proof.
- Only the documented llama.cpp `llama-server` interface is targeted
  (`/health`, `/v1/chat/completions` with SSE). Arbitrary GGUF files are not
  claimed to work: architecture support is proven only when the engine loads
  them.
- Memory figures shown in setup are estimates from file size and context size,
  clearly labelled as such. No throughput measurement is displayed or claimed.
- Conversation history, inference settings, and the runtime session key are
  stored through Tauri's app data/config directories; on Windows these resolve
  under `%APPDATA%\com.nexora.desktop`. Approved model acquisition defaults to
  `app_data_dir()/models`. Tauri's `app_local_data_dir()` uses the separate
  `%LOCALAPPDATA%\com.nexora.desktop` path. All user data is separate from the
  per-user NSIS install root.
- The pinned Tauri NSIS template exposes an explicit app-data deletion choice
  during interactive uninstall and skips data deletion during an upgrade. The
  smoke script invokes silent uninstall without requesting deletion and checks
  roaming/local/model-storage sentinels; it also checks that WebView2 Evergreen
  remains installed. These are authored checks, not Windows results.
- Windows packaging (NSIS), native window startup, and these data/WebView2
  lifecycle behaviors have **not** been verified in this environment.

## Step 3 packaging verification (2026-10-24)

Evidence is deliberately separated by type:

| Evidence type | Result |
|---|---|
| Source/configuration inspection | Node `22.22.3`, pnpm `9.15.4`, and Rust/Cargo `1.93.0` match `package.json`, CI, and `rust-toolchain.toml`. The guarded build now creates `ui/dist` with `pnpm build` before native Cargo checks/tests, refuses existing installer `.exe`/manifest outputs instead of reusing or overwriting them, and writes its SHA-256 manifest with create-new semantics. The smoke harness rejects install paths overlapping the standard install/data roots and leaves modified sentinels or unexpected files for review. `.github/workflows/ci.yml` configures the hosted `windows-2022` job's x64 Visual Studio environment via the runner's `vswhere`/`VsDevCmd.bat`; that workflow was not run. |
| Automated checks on Linux | `PATH="/tmp/nexora-pnpm-bin:$PATH" corepack pnpm check` **PASS**: TypeScript, **88 Vitest files / 873 tests**, installer-contract **17/17**, frontend production build, config-security scan, and production frontend contract scan. `node scripts/check-installer-contract.mjs` reported Nexora `0.1.0`, `com.nexora.desktop`, NSIS, **4 files / 720,456 bytes**, with no model/runtime/credential payload. Vite retained non-fatal warnings for `theme-init.js`, mixed static/dynamic imports, and a 590.21 kB minified chunk. `git diff --check` passed. |
| PowerShell / native Windows | **NOT VERIFIED.** This host is Linux x86_64; PowerShell, Rust/Cargo, MSVC/Windows SDK, NSIS, and SignTool are absent. Neither `.ps1` script was parsed or executed. No installer, SHA-256 artifact manifest, signature result, or Windows install/launch/uninstall evidence exists. |
| Windows acceptance | **NOT RUN.** `gh run list --branch arena/9ea88704-nexora` returned no runs. The existing hosted workflow is configuration, not an allocated runner result; this session cannot push the local worktree or trigger a run against these unpushed edits. There is no authorized Windows 10/11 x64 machine available here. No valid previous supported build was supplied, so same-build reinstall is not an upgrade; offline/missing-model runtime behavior also remains untested on Windows. |

The Linux checks and authored smoke tests are **not** Windows acceptance. Do not infer successful packaging, signing, clean installation, shortcut launch, WebView2 bootstrap, version upgrade, uninstall/data retention, offline behavior, or model inference from them.

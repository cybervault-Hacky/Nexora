# Nexora Desktop

The Tauri 2 desktop client for Nexora. This is the Phase 5 feature set — a
native window shell, a polished React interface, and a Rust core that supervises
a user-supplied `llama-server` process and streams its output into the
conversation — carried through the Phase 6–7 verification work. It still contains **no agent
tools, no project modification, and no model or runtime downloads**.

The Rust crate cannot be compiled in the development sandbox (no toolchain, no
reachable crate registry). [`.github/workflows/ci.yml`](../../.github/workflows/ci.yml)
compiles it, runs clippy with warnings denied, and runs its unit tests on Linux
(48) and Windows (45). Nothing has verified the window, the installer, or a real
model run. See [`docs/phase-8/`](../../docs/phase-8/README.md).

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

1. **Node.js 22 LTS** and **pnpm 9** (`corepack enable pnpm`).
2. **Rust stable** via [rustup](https://rustup.rs). The pin is in
   `rust-toolchain.toml` at the repository root.
3. **Visual Studio 2022 Build Tools** with "Desktop development with C++" and
   the Windows 10/11 SDK.
4. **WebView2 Evergreen runtime** (preinstalled on Windows 11).
5. **A llama.cpp `llama-server` build** pinned to the version recorded in
   [`docs/phase-2/`](../../docs/phase-2/README.md), and **a GGUF model file**.
   Nexora does not ship, download, or install either one.

Tauri's full prerequisite list is at <https://v2.tauri.app/start/prerequisites/>.

## Commands

Run from the repository root unless noted.

```bash
pnpm install                 # workspace dependencies
pnpm dev                     # Vite only, http://localhost:1420
pnpm tauri dev               # native window (runs Vite automatically)
pnpm typecheck               # tsc --noEmit
pnpm test                    # Vitest: unit, hook, and DOM interaction tests
pnpm build                   # typecheck + production UI build
pnpm tauri build             # Windows installer (NSIS, per-user)
```

`cargo check` or `cargo clippy` run before `pnpm build` will fail: Tauri's
`generate_context!` embeds `ui/dist` at compile time. Build the UI first.

```bash
pnpm build && cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml
```

`pnpm tauri build` must be run on Windows with the prerequisites above.

## Connecting a model

The first screen (and the sidebar) reports the real engine state. With no model
configured, Nexora shows a setup panel instead of an empty chat:

1. **Runtime executable** — full path to `llama-server.exe`.
   `Check` runs `llama-server --version` and shows the first line it printed.
   A build that does not report the pinned number (llama.cpp b11714) is
   labelled as unverified rather than assumed compatible.
2. **Model file (.gguf)** — full path to a GGUF file you already have.
   `Check` reads the header (magic, version 2–3, tensor count), requires the
   `.gguf` extension and a plausible size, and estimates memory need. The file
   is never uploaded anywhere.
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
- Nothing is persisted. Closing the window loses the session, by design.

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
- Exactly ten commands are exposed. None can browse the filesystem, run a
  shell, or reach the network beyond the loopback endpoint the core owns.
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

148 Vitest tests run in jsdom (plus a node-environment configuration suite):

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
  committed, and nothing is downloaded automatically. A runtime binary *was*
  built from the pinned source and its HTTP surface exercised on Linux; that
  proves the contract, not generation. Linux runs are not Windows proof.
- Only the documented llama.cpp `llama-server` interface is targeted
  (`/health`, `/v1/chat/completions` with SSE). Arbitrary GGUF files are not
  claimed to work: architecture support is proven only when the engine loads
  them.
- Memory figures shown in setup are estimates from file size and context size,
  clearly labelled as such. No throughput measurement is displayed or claimed.
- Chat history lives in React state only; it is not persisted.
- Windows packaging (NSIS) and the native window have **not** been verified in
  this environment.

# Nexora Desktop

The Tauri 2 desktop shell for Nexora. This is the Phase 4 build: a real native
window, a polished React interface, and one narrow Rust command. It does **not**
contain model inference, agent tools, or project modification yet.

## Stack

| Layer | Choice |
|---|---|
| Shell | Tauri 2 (Rust) |
| UI | React 19 + TypeScript 5.9 |
| Bundler | Vite 7 |
| Package manager | pnpm 9 (workspace) |
| Styling | Hand-written CSS with design tokens, no UI framework |
| Tests | Vitest + jsdom + React Testing Library |

## Prerequisites (Windows)

1. **Node.js 22 LTS** and **pnpm 9** (`corepack enable pnpm`).
2. **Rust stable** via [rustup](https://rustup.rs). The pin is in
   `rust-toolchain.toml` at the repository root.
3. **Visual Studio 2022 Build Tools** with "Desktop development with C++" and
   the Windows 10/11 SDK.
4. **WebView2 Evergreen runtime** (preinstalled on Windows 11).

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
```

`pnpm tauri build` must be run on Windows with the prerequisites above.

## Keyboard

| Shortcut | Action |
|---|---|
| `Enter` | Send (ignored during IME composition) |
| `Shift + Enter` | New line |
| `Esc` | Dismiss a composer error |
| `Ctrl + L` | Focus the composer |
| `Ctrl + N` | New chat |
| `Ctrl + B` | Collapse or expand the sidebar |

## Layout

```
apps/desktop/
├─ ui/
│  ├─ index.html
│  └─ src/
│     ├─ App.tsx                      shell composition and shortcut wiring
│     ├─ components/                  title bar, sidebar, messages, composer
│     ├─ features/chat/               chat logic, conversation state, tests
│     ├─ lib/                         typed IPC, status hook, scroll, window
│     ├─ test/setup.ts                jsdom primitives for Vitest
│     └─ styles/                      tokens and application CSS
├─ src-tauri/
│  ├─ src/commands.rs                 read-only status command
│  ├─ capabilities/main.json          least-privilege window permissions
│  ├─ icons/                          generated desktop icons
│  └─ tauri.conf.json                 window, CSP, bundle configuration
├─ vite.config.ts
└─ vitest.config.ts
```

## Conversation behaviour

- Messages are created once with a stable id and an explicit `role`.
- A submission guard rejects a second submit inside 350 ms, so a double-click
  or key repeat cannot duplicate a turn.
- Each new chat rotates an internal conversation id. A turn produced for the
  previous conversation is dropped rather than appended to the new one.
- Nothing is persisted. Closing the window loses the session, by design.

## Security posture

- The webview never receives a runtime port, API key, or HTTP client.
- Exactly one command is exposed: `get_app_status`, which returns the app
  version and a static engine description. It cannot read files, spawn
  processes, or execute commands.
- CSP allows only local scripts, local styles, and Tauri IPC. There are no
  remote fonts, images, or scripts.
- The only window permissions are minimize, toggle-maximize, close, and
  start-dragging. No shell, filesystem, or dialog permissions are granted.
- The interface labels every assistant reply as **scaffold** text. It never
  claims that inference works.

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

49 tests run in jsdom:

- chat validation and scaffold labelling
- submission guard and conversation isolation
- scroll-follow logic
- composer disabled, counter, and error states
- full interaction: send with Enter, Shift+Enter, empty submit, new chat,
  sidebar collapse, honest engine status, absent model controls

## Known limitations

- No inference engine, model manager, or model files.
- Chat history lives in React state only; it is not persisted.
- The browser preview hides native window controls; `pnpm tauri dev` shows them.
- Windows packaging (NSIS) and the native window have **not** been verified in
  this environment.

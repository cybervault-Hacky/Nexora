# Nexora Desktop

The Tauri 2 desktop shell for Nexora. This is the Phase 3 foundation: a real
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
pnpm test                    # Vitest unit tests
pnpm build                   # typecheck + production UI build
pnpm tauri build             # Windows installer (NSIS, per-user)
```

`pnpm tauri build` must be run on Windows with the prerequisites above. It was
not executed in the Linux research environment because no Rust toolchain or
bundler toolchain was available there.

## Layout

```
apps/desktop/
├─ ui/
│  ├─ index.html
│  └─ src/
│     ├─ App.tsx                 shell composition and session state
│     ├─ components/             title bar, sidebar, messages, composer
│     ├─ features/chat/          pure chat logic + unit tests
│     ├─ lib/                    typed IPC and window controls
│     └─ styles/                 tokens and application CSS
├─ src-tauri/
│  ├─ src/commands.rs            read-only status command
│  ├─ capabilities/main.json     least-privilege window permissions
│  ├─ icons/                     generated desktop icons
│  └─ tauri.conf.json            window, CSP, bundle configuration
├─ vite.config.ts
└─ vitest.config.ts
```

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

## Known limitations

- No inference engine, model manager, or model files.
- Chat history lives in React state only; it is not persisted.
- The browser preview hides native window controls; `pnpm tauri dev` shows them.
- Windows packaging (NSIS) and the native window have **not** been verified in
  this environment.

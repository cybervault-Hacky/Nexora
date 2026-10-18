# Phase 19 — UI Architecture & Separate-Page Navigation

**Status:** Implemented in the local working tree; uncommitted.

**Scope:** Frontend architecture only. No Rust/native, Tauri configuration, dependency, or lockfile changes. No auth, account, OAuth, payment, or licensing work (later roadmap phases). The Glassmorphism System (Phase 29) and the Animation System / welcome text / background effects (Phase 30) were deliberately **not** implemented.

**Checkout:** `arena/1008499f-nexora`, based on `128781618917fd15d2923aeb8eec04e9a5e90e98` (main, after PR #3 merged the Phase 18 work). The working tree was clean before this phase; nothing pre-existing was overwritten, reset, or reverted.

## Audit (before implementation)

- pnpm 9 workspace with one app, `apps/desktop`: Tauri 2 + React 19.3 + TypeScript 5.9 + Vite 7. **No routing library installed** (dependencies: `@tauri-apps/api`, `diff`, `react`, `react-dom`).
- `App.tsx` was a 1,049-line monolith: the explorer column (WorkspacePanel, FilePreview, CodeIntelligencePanel, RetrievalBar, ContextBudgetBar, EvidencePanel, AnalysisPanel, patch workflow) rendered **beside** the chat column (header, notices, messages, context bar, composer) in one view — the exact chat/projects mixing this phase removes.
- Baseline `pnpm check` (before changes): typecheck PASS, **44 files / 500 tests PASS**, production build PASS (90 modules, two pre-existing Vite chunking warnings).
- Rust/Tauri commands (`scan_workspace`, `search_symbols`, `get_dependency_graph`, inference Supervisor, history storage, settings, patch create/validate/apply) were inventoried via their TS bindings and left untouched.

## Implementation plan (executed)

1. Dependency-free hash router (`lib/router.ts`) instead of adding react-router.
2. One page component per feature under `ui/src/pages/`, each with its own content area.
3. App shell keeps all feature hooks mounted at the top level so chat/project state survives page switches; pages receive state as props.
4. Sidebar primary navigation with `aria-current="page"`; TitleBar shows the active page name.
5. Existing tests routed to their pages; new router + navigation tests.
6. Phase docs and root README updated only where necessary.

## Routing

Hash-based, same-document routing (`#/`, `#/chat`, `#/projects`, `#/code`, `#/models`, `#/settings`), chosen deliberately:

- works in the packaged Tauri webview (`tauri.localhost`), the Vite browser preview, and jsdom without server rewrite rules;
- needs **no new dependency**;
- switching pages never reloads the app (single `index.html`, no document navigation);
- browser back/forward work through normal hash history; `useHashRoute` subscribes via `useSyncExternalStore` to `hashchange`/`popstate`;
- unknown hashes (`#/no-such-page`) render a graceful *Page not found* view with a working *Return to Home* link — no dead ends.

In-app links use `components/RouterLink.tsx` (real `<a href="#/…">` anchors that navigate explicitly so the Tauri webview, browser preview, and tests behave identically; modifier-clicks are left to the browser).

## Pages (each with its own content area)

| Route | Page | Contents (all pre-existing panels, redistributed — none duplicated, none removed) |
|---|---|---|
| `#/` | `pages/HomePage.tsx` | Dashboard: welcome header, quick actions (new chat / projects / code intelligence / local models), honest status cards (engine, project, saved conversations) from real state only. |
| `#/chat` | `pages/ChatPage.tsx` | Conversation header (rename/delete/save chips), chat notices (runtime/save/load errors), transcript + empty/setup/error states, retrieval bar, context budget bar, context strip, composer-side evidence, Composer. |
| `#/projects` | `pages/ProjectsPage.tsx` | Master–detail layout: WorkspacePanel (path search, context selection) + read-only FilePreview, evidence panel, patch-missing warning, PatchGenerateBar, and the full manual patch workflow (PatchEditor → PatchPreview → ValidationPanel → repair/revised) with explicit Apply. |
| `#/code` | `pages/CodeIntelligencePage.tsx` | CodeIntelligencePanel (symbol search) + AnalysisPanel (graph/file/impact), bounded-index summary line. |
| `#/models` | `pages/ModelManagerPage.tsx` | Local Model Manager: the existing `ModelSetup` flow (runtime probe, GGUF validate, load) + live `EngineStatus` card with unload/refresh. Browser preview shows an honest desktop-only notice instead of fake controls. |
| `#/settings` | `pages/SettingsPage.tsx` | Real persisted interface preference (sidebar collapse, same store as Ctrl+B), current/last workspace path (read), engine config read-only (runtime/model/context size), keyboard shortcuts, about. No invented settings. |
| fallback | `pages/NotFoundPage.tsx` | Honest not-found with the requested path and a way back. |

Shared shell (kept in `App.tsx` + `components/`): TitleBar (page name in the context slot), Sidebar (brand, primary nav with active state + `aria-current="page"`, New chat, history, engine status), and the global engine notices (status-read failure with Retry, unload failure). Shared `pages/PageHeader.tsx` gives every page the same eyebrow → `h1` → copy hierarchy.

## Architecture decisions

- **All feature hooks stay in `App.tsx`** (`usePersistentConversation`, `useWorkspace`, `usePatch`, `useValidation`, `usePatchRepair`, symbol index / dependency graph effects, context budget, retrieval, evidence, grounded prompt). Pages are presentational receivers of state/callbacks. Switching pages therefore preserves the conversation, draft, stream, workspace, preview, and patch state — nothing remounts or refetches.
- **Chat and Projects are genuinely separate views**: on Chat there is no project explorer; on Projects there is no transcript or composer (covered by tests). The only cross-links are honest navigation (e.g. the chat empty state links to the Local Models page instead of embedding `ModelSetup` twice).
- **Keyboard shortcuts preserved:** `Ctrl+N` (new chat — now also navigates to the chat page), `Ctrl+L` (focus composer — navigates to chat first, then focuses), `Ctrl+B` (sidebar), `Esc` (stop generation, in Composer). The single window `keydown` listener is registered/unregistered exactly as before (regression-tested).
- **No dead controls:** every button/link on the new pages performs a real action (navigate, toggle + persist, unload, refresh) or is clearly labelled read-only text.
- **No fabricated data:** dashboard counts, paths, index stats, and engine state all come from existing hooks/IPC. Unimplemented surfaces (browser-preview inference) show honest explanatory states.

## Visual scope

Presentation uses the existing graphite/charcoal tokens and restrained violet accents (`.nav-item`, `.page*`, `.home-*`, `.projects-layout`, `.settings-*` — all built from `--bg-*`, `--text-*`, `--accent-*`, `--radius-*`, `--motion-*` tokens). No new glassmorphism, aurora/animated backgrounds, or welcome/sign-in flows — reserved for Phases 29/30 and the auth phases. The existing universal `prefers-reduced-motion` / `prefers-reduced-transparency` / `@supports not (backdrop-filter)` rules automatically cover the new classes. Responsive rules added for the projects master–detail layout at 900px, matching the existing breakpoint set.

## Files changed

| File | Purpose |
|---|---|
| `apps/desktop/ui/src/lib/router.ts` | New dependency-free hash router (routes, `parseRoute`, `routeHref`, `useHashRoute`, `navigateTo`). |
| `apps/desktop/ui/src/lib/router.test.ts` | New: route parsing, hrefs, navigation no-op behavior. |
| `apps/desktop/ui/src/components/RouterLink.tsx` | New: deterministic in-app anchor used by shell and pages. |
| `apps/desktop/ui/src/pages/*.tsx` (7 pages + `PageHeader.tsx`) | New page components extracted from the `App.tsx` monolith. |
| `apps/desktop/ui/src/App.tsx` | Shell refactor: keeps all feature hooks/computations, renders router outlet + global notices; 1,049 → ~430 lines of orchestration. |
| `apps/desktop/ui/src/components/Sidebar.tsx` | Primary navigation with `aria-current="page"` and active styling; history/engine footer unchanged. |
| `apps/desktop/ui/src/components/TitleBar.tsx` | Context slot now shows the active page name (`pageLabel`). |
| `apps/desktop/ui/src/components/Icon.tsx` | Added `home`, `message`, `code`, `chip`, `settings` icons (same inline-SVG style). |
| `apps/desktop/ui/src/styles/app.css` | Phase 19 section: nav, page frames, dashboard, projects layout, settings; token-based, no new visual system. |
| `apps/desktop/ui/src/App.test.tsx`, `App.interaction.test.tsx` | Suites start on `#/chat` (their surface); the "shows setup" case now follows the real chat → Local Models link to find `ModelSetup`. No assertion removed. |
| `apps/desktop/ui/src/App.navigation.test.tsx` | New: home default, nav → correct pages, active state, chat/projects separation, hash navigation without reload, draft preserved across pages, not-found fallback, quick actions. |
| `README.md` | Status/features/tree/notes updated for Phase 19. |
| `docs/phase-19/README.md` | This document. |

No changes under `apps/desktop/src-tauri/`, `package.json`, `pnpm-lock.yaml`, `tauri.conf.json`, or icon assets.

## Verification (executed after the changes)

```text
pnpm check   # = pnpm typecheck && pnpm test && pnpm build
```

| Check | Result |
|---|---|
| `pnpm typecheck` (`tsc --noEmit`) | **PASS** |
| `pnpm test` (`vitest run`) | **PASS — 46 files / 518 tests** (baseline 44/500; +2 files, +18 tests, none removed) |
| `pnpm build` (`tsc --noEmit && vite build`) | **PASS** — 100 modules; CSS 73.78 kB / 11.59 kB gzip; JS 435.73 kB / 126.38 kB gzip. Same two pre-existing Vite dynamic/static import warnings (`@tauri-apps/api/window.js`, `lib/patch.ts`) as baseline. |

Browser-preview smoke (Vite dev server on port 1420): `/`, `/src/main.tsx`, `/src/App.tsx`, `/src/lib/router.ts`, all seven page modules, `RouterLink.tsx`, `Sidebar.tsx`, `/src/styles/app.css` returned **HTTP 200**. This confirms the dev/module pipeline serves the new architecture; it is not visual inspection and not a Tauri runtime test.

Navigation behavior covered by the new Vitest suite: every sidebar item renders its page; `aria-current="page"` tracks the active route; Chat shows no explorer and Projects shows no composer/transcript; drafts survive page switches; hash hrefs (`#/chat`) switch pages without a document load; unknown routes fall back gracefully; Home quick actions and the not-found link navigate for real; Ctrl+N/L/B keep working from any page (routing to AI Chat before focusing the composer).

## Known limitations and manual verification (Windows)

This Linux sandbox has no `cargo`/`rustc` and no browser/display: Rust checks, Windows packaging, native window behavior, and agent-side visual inspection were **not** possible (unchanged from Phase 18). Before shipping, verify on Windows:

1. `cargo fmt --all -- --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, and the Tauri Windows build (no Rust source changed; CI remains the Rust verification path).
2. Native window at 960×640 minimum and 1280×840 default: sidebar collapse rail with nav icons, page switching, back/forward via hash history, deep links like `app://…/#/projects`.
3. With a real llama.cpp + GGUF model: model load/unload from Local Models, streaming chat, patch generate/validate/apply from Projects, symbol search and graph on Code Intelligence — all untouched workflows that were not runtime-exercised here.
4. Keyboard-only pass: tab order through sidebar nav, `aria-current` announcement, Ctrl+N/L/B, Esc-to-stop, focus visibility, reduced-motion.

## Git

All work is local and uncommitted on `arena/1008499f-nexora`. No commit, push, pull request, release, visibility change, branch switch, reset, clean, or history rewrite was performed.

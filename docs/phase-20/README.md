# Phase 20 — Welcome & Onboarding Page

**Status:** Implemented in the local working tree; uncommitted.

**Scope:** The welcome/onboarding entry flow only — one polished Welcome screen, one honest onboarding-next screen, and the route/shell wiring around them. No authentication of any kind (Phase 21+), no glassmorphism system (Phase 29), no animation/aurora/text-reveal system (Phase 30), and no redesign of the existing workspace pages (Home, AI Chat, Projects, Code Intelligence, Local Models, Settings are untouched content-wise).

**Checkout:** `arena/1008499f-nexora`, based on `128781618917fd15d2923aeb8eec04e9a5e90e98`. The working tree held the full uncommitted Phase 19 implementation at the start of this phase; all of it was preserved and built upon — nothing was reset, reverted, or overwritten.

## Audit (before implementation)

- Branch `arena/1008499f-nexora`, working tree = Phase 19 (uncommitted): hash router (`lib/router.ts`), `pages/` (Home, Chat, Projects, Code Intelligence, Local Models, Settings, NotFound), `App.tsx` shell + feature hooks, sidebar nav with `aria-current`, token-based CSS.
- Baseline `pnpm check` **before any change**: typecheck PASS, **46 files / 518 tests PASS**, production build PASS (100 modules).
- Route map at baseline: `#/` and `#/home` → Home; `#/chat`, `#/projects`, `#/code`, `#/models`, `#/settings`; unknown → NotFound. No routing library; no welcome/onboarding code existed.
- Existing assets reused: `ApertureMark` (local SVG brand mark), `Icon`, `Button` classes, design tokens (`--bg-*`, `--text-*`, `--accent-*`, `--radius-*`, `--motion-*`), `panel-enter` keyframes (already reduced-motion-safe via the universal override).

## What was built

### Routes (same dependency-free hash router)

| Route | Page | Role |
|---|---|---|
| `#/` (alias `#/welcome`) | `pages/WelcomePage.tsx` | New app entry: the welcome screen. |
| `#/onboarding` | `pages/OnboardingPage.tsx` | Onboarding-next: the dedicated slot where Phase 21's sign-in page will live. |
| `#/home` | `pages/HomePage.tsx` | Home dashboard (unchanged; now the canonical Home path since `#/` is the Welcome entry). |
| others | unchanged | AI Chat, Projects, Code Intelligence, Local Models, Settings, not-found fallback. |

`parseRoute` maps the empty/root hash to `welcome`; `routeHref("home")` is now `#/home`; `navigateTo` no-ops via parsed-id comparison (so `#/` ↔ `#/welcome` do not stack history). Unknown routes still fall back to the graceful not-found page with a working *Return to Home* link.

### Welcome screen

- Brand lockup (ApertureMark + `nexora`), headline *"Code with an AI that runs on your machine."*, and concise supporting copy describing the real product: conversational workspace, read-only code intelligence, review-before-apply patch flow, powered by a llama.cpp runtime and GGUF model the user supplies.
- Primary **Continue** action → `#/onboarding`.
- Restrained capability list (AI Chat / Projects & patches / Code intelligence) — descriptions only, no fake controls.
- Subtle secondary note on local-first operation, limited to verifiable claims from the existing docs: inference runs on this PC via llama.cpp + a user-supplied GGUF model; prompts are processed locally and not sent to a cloud service. No performance, download, or absolute-privacy guarantees were added.
- A restrained product preview (`components/WelcomePreview.tsx`): a decorative composition of abstract window chrome, message rows, and a code block built from the same tokens. It is `aria-hidden`/`role="presentation"`, contains **no interactive elements**, and shows no fabricated content — every shape is a plain visual placeholder.

### Onboarding-next screen (Phase 21 slot)

- Explicitly designed as the future sign-in page's home: a step strip (Welcome · Next step with `aria-current="step"`), headline *"Sign-in is the next step."*, and honest copy stating that account sign-in is not part of this build.
- A reserved, clearly labelled slot (`role="note"`, "Upcoming sign-in") where Phase 21's form will render — **no fake form, no password fields, no disabled pretend-submit**.
- Working **Back to Welcome** and an honest **Open the workspace** → `#/home`, so the flow is never a dead end. Direct load/refresh of `#/onboarding` renders this screen normally.

### Shell and architecture

- The flow renders **outside the workspace sidebar**: `isFlowRoute()` in `lib/router.ts`; `App.tsx` shows the TitleBar (window chrome: drag/minimize/maximize/close still work in Tauri) but hides the sidebar and its toggle (`TitleBar` gained `showSidebarToggle`), and skips the engine notices. `app--flow` collapses the shell grid to a single column.
- `App.tsx` keeps all Phase 19 feature hooks mounted unchanged (local AI init, workspace state, chat state unaffected). Pages stay presentational (`WelcomePage({ onContinue })`, `OnboardingPage({ onBack, onEnterWorkspace })`); no monolith growth, no new dependency, no persistence of onboarding state (revisiting `#/welcome` at any time works — completion is deliberately not stored).
- Keyboard: `Ctrl+N`/`Ctrl+L` keep their Phase 19 global behavior; `Ctrl+B` is now scoped to pages where a sidebar actually exists (it never fires on the flow, so the hidden sidebar state can't be toggled invisibly).

### Visual direction

Graphite/charcoal surfaces and restrained violet accents via existing tokens only. The composition relies on proportion, spacing, quiet borders, and one accent line — no new colors, no gradients beyond the existing accent wash, no glassmorphism (Phase 29). Entrance motion reuses the existing `panel-enter` 3px rise (with a 70 ms stagger on the preview); the universal `prefers-reduced-motion` / `prefers-reduced-transparency` / no-backdrop-filter rules already cover it — Phase 30 animation work was not started. Responsive: the two-column welcome layout stacks at 1020 px, preview rail compresses at 560 px, actions go full-width; `min-width: 0` throughout avoids horizontal overflow; the stage scrolls vertically at short heights.

## Accessibility

- Semantic structure: one `h1` per screen, real `<button>` actions, a labelled capability list (`aria-label="What Nexora includes"`), `role="note"` for the reserved sign-in slot, `aria-current="step"` on the active onboarding step.
- Keyboard: the Continue button is the first stop on Welcome (verified by test with `userEvent.tab()` + `{Enter}`); all actions are native buttons with visible `:focus-visible` outlines from the shared button styles.
- The decorative preview is fully hidden from assistive technology; the hidden sidebar toggle spacer is `aria-hidden` and not focusable (it exists only for title-bar alignment).
- Text uses the existing contrast-tuned tokens (`--text-primary/secondary/tertiary` on `--bg-base/panel`); no font sizes below the existing 11 px floor.

## Files changed (this phase)

| File | Change |
|---|---|
| `apps/desktop/ui/src/lib/router.ts` | `welcome` + `onboarding` routes, root hash → welcome, `isFlowRoute`, parsed-id `navigateTo` no-op. |
| `apps/desktop/ui/src/lib/router.test.ts` | Updated + 2 new tests (flow-route classification, root-alias no-op). |
| `apps/desktop/ui/src/pages/WelcomePage.tsx` | New welcome screen. |
| `apps/desktop/ui/src/pages/OnboardingPage.tsx` | New onboarding-next transitional screen. |
| `apps/desktop/ui/src/components/WelcomePreview.tsx` | New decorative product composition (non-interactive). |
| `apps/desktop/ui/src/components/TitleBar.tsx` | `showSidebarToggle` prop (hidden spacer on the flow). |
| `apps/desktop/ui/src/App.tsx` | Flow branch (no sidebar/notices), Ctrl+B scoping. |
| `apps/desktop/ui/src/styles/app.css` | Phase 20 section: flow layout, welcome, preview composition, onboarding card, responsive rules (tokens only). |
| `apps/desktop/ui/src/App.welcome.test.tsx` | New suite (9 tests). |
| `apps/desktop/ui/src/App.navigation.test.tsx` | Start at `#/home` (Home's canonical path); not-found returns to `#/home`. No assertion removed. |
| `README.md` | Entry flow, features, tree, status, phase notes, verification. |
| `docs/phase-20/README.md` | This document. |

No changes under `apps/desktop/src-tauri/`, `package.json`, `pnpm-lock.yaml`, or `tauri.conf.json`. No Phase 19 file was rewritten beyond the listed routing/shell touch-points.

## Tests and actual results

```text
pnpm check        # = pnpm typecheck && pnpm test && pnpm build
git diff --check
```

| Check | Result |
|---|---|
| `pnpm typecheck` | **PASS** |
| `pnpm test` | **PASS — 47 files / 529 tests** (baseline 46/518; +`App.welcome.test.tsx` 9, `router.test.ts` 7→9; **no test removed**) |
| `pnpm build` | **PASS** — 103 modules; CSS 80.64 kB / 12.66 kB gzip; JS 441.82 kB / 127.86 kB gzip (same two pre-existing Vite chunking warnings as baseline). |
| `git diff --check` | **PASS** |
| Vite dev-server smoke (port 1420) | `/`, `App.tsx`, `router.ts`, `WelcomePage`, `OnboardingPage`, `WelcomePreview`, `app.css` all **HTTP 200** (pipeline check only — not visual inspection). |

Welcome suite coverage: entry renders Welcome without the workspace shell; honest local-first copy present; sidebar toggle hidden; **Continue** → `#/onboarding` with honest placeholder (asserts no password/text inputs exist); **Back** → `#/welcome`; **Open the workspace** → `#/home` with navigation intact; direct load/refresh of `#/onboarding` works; unknown routes stay graceful from the flow; Tab + Enter reach and activate Continue; workspace pages and their navigation remain intact.

## Known limitations and later phases

- No sign-in, create-account, verification, password recovery, OAuth, sessions, or account persistence — **Phases 21–23**. The onboarding slot is a labelled placeholder only.
- No glassmorphism system (**Phase 29**), no Living Aurora / text-reveal / page-transition animation system (**Phase 30**). Motion here is the existing `panel-enter` micro-entrance only.
- Onboarding completion is intentionally **not persisted** (per brief): the app opens on Welcome every launch until a later phase decides otherwise and documents the need.
- `cargo`/Windows/Tauri runtime checks remain unavailable in this Linux sandbox (no Rust source changed). Manual Windows verification: title-bar controls on the flow screens, narrow-window layout at 960×640 and display scaling, keyboard-only pass, and reduced-motion rendering.

## Git

All work is local and uncommitted on `arena/1008499f-nexora`, layered on top of the preserved Phase 19 working tree. No commit, push, pull request, release, visibility change, branch switch, reset, clean, or history rewrite was performed. Phase 21 was **not** started.

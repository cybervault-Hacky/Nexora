# Phase 18 — Premium Desktop UI Refinement

**Status:** Implemented in the local working tree; unpublished and uncommitted.

**Scope:** Refinement of the existing Phase 18 React/CSS experience only. No new phase, backend capability, search service, product workflow, inference mode, or data access was added.

**Checkout:** Work remains on `arena/555ee411-nexora`, based on `6ea0dbd9784db6f1e353fb87be84686cc12ae46e`. The earlier brief named `arena/74aec262-nexora` / `d662588`; the actual Arena checkout was preserved without switching, resetting, cleaning, or rewriting history.

## Audit and constraints

The root `README.md` and actual UI components/styles were inspected before editing. Nexora is an existing Tauri 2 Windows desktop app with React 19.3.1, TypeScript 5.9.3, Vite 7.3.7, pnpm 9.15.4, hand-written CSS tokens, and no UI framework. Its native configuration remains 1280×840 with a 960×640 minimum and a custom title bar. The explorer already held workspace files, previews, symbol search, retrieval, budget, evidence, analysis, patch generation/review, and validation; conversation, local inference, persistence, streaming/cancellation, and explicit patch application were preserved.

The initial Phase 18 implementation began from a clean tree. This refinement continued on that existing local Phase 18 working tree; prior work was preserved. Search behavior was read in `WorkspacePanel.tsx`, `useWorkspace.ts`, and `CodeIntelligencePanel.tsx` before styling or changing its empty/error states.

## Refinements

- **Selective glass, not a glass rewrite:** added subdued translucent surfaces to the custom title bar, collapsible sidebar, shared search controls, and composer. The large scrolling workspace explorer and code, evidence, analysis, patch, and validation details stay opaque for long-form readability. Glass uses a shared 18px blur/saturation token, fine borders, a quiet inset highlight, and low shadows; no oversized glow, decorative gradient, or glass overlay was added to technical text.
- **Solid alternatives:** `prefers-reduced-transparency` switches the glass tokens to solid theme surfaces and disables blur; `@supports not (backdrop-filter: blur(1px))` supplies the same opaque fallback. The standard dark theme remains legible when blur is unavailable.
- **Purposeful motion:** reused 140ms/190ms timing tokens for small hover/focus feedback, sidebar collapse, search/control response, and brief message, notice, file-preview, evidence, and analysis entry. `prefers-reduced-motion` disables transitions and animations. There are no new perpetual decorative animations.
- **Less visual noise:** removed low-value ambient/section gradients, reduced redundant empty-state decoration and repeated path/count text, flattened nested stat cards, and aligned compact controls. Security/evidence limitations, errors, unsupported states, validation results, and explicit patch safeguards remain visible.
- **Shell and layout:** kept the project explorer bounded and scrollable; refined sidebar collapse and title-bar/conversation hierarchy; retained the existing narrow-window reflow, keyboard shortcuts, button actions, and focus treatments. The existing file preview close button and other compact controls retain visible, accessible targets.
- **Shortcut helper:** restored the visible `Esc`-to-stop hint next to the composer controls. The existing cancel behavior and `Esc` shortcut were not changed.

## Search audit and behavior

The two existing searches have different scopes and remain separate:

1. **Project paths** — live, case-insensitive substring matching over the already-loaded `inventory.entries` paths. It does not search file contents or perform additional filesystem access. The field now has a named search landmark and path-specific input name, a clear action, file/folder counts, and a live “No paths match” state. Its example path hint covers Windows and Unix separators.
2. **Indexed symbols** — retains the existing `searchSymbols(workspacePath, query.trim(), 20)` operation, with Enter and the Search button as submission methods. No backend, index, result limit, suggestions, filters, or data access changed. The field has a consistent clear action; no-results feedback appears only after a search is submitted, previous results/errors clear when editing, and a request-sequence guard prevents an older in-flight search from replacing newer results.

Focused component tests cover the path filter/clear/no-match states, symbol-search submission/clear/no-results behavior, stale-result protection, empty history-preview count de-duplication, and the composer controls. No test was removed.

## Accessibility, readability, and responsiveness

- The transcript remains a labelled, keyboard-focusable region. Search controls have accessible landmark/input names; clear controls have explicit accessible names; result/status feedback uses live status semantics. Icon-only actions retain names/tooltips.
- Existing keyboard shortcuts, focus-visible states, disabled/error/loading distinctions, and action targets were preserved. Security/evidence caveats and validation statuses remain textual, not color-only.
- Core text token contrast was checked on the dark base/panel/elevated surfaces: primary 16.96:1 / 16.08:1 / 15.17:1; secondary 9.69:1 / 9.19:1 / 8.67:1; tertiary 6.34:1 / 6.02:1 / 5.68:1. This is not a component-level WCAG certification and does not cover all translucent overlays or OS scaling. No CSS font-size declaration is below 11px.
- Existing breakpoints cover wide desktop, 1100px, 900px, 760px, and short-height cases; the native minimum size is unchanged. The explorer retains its own scroll area when the preview stacks.

## Files changed

| File | Purpose |
|---|---|
| `apps/desktop/ui/src/styles/tokens.css` | Shared graphite/violet, semantic, spacing, elevation, glass, and motion tokens. |
| `apps/desktop/ui/src/styles/app.css` | Selective glass and solid fallbacks; shared search styling; shell, controls, surfaces, motion, focus, and reduced-preference rules. |
| `apps/desktop/ui/src/App.tsx` | Existing shell/transcript accessibility and hierarchy refinements; no workflow changes. |
| `apps/desktop/ui/src/components/WorkspacePanel.tsx` | Accessible path search, clear action, counts, no-match feedback, and clearer path hint. |
| `apps/desktop/ui/src/components/CodeIntelligencePanel.tsx` | Existing symbol search states, clear/no-result feedback, stale-request protection, and compact control presentation. |
| `apps/desktop/ui/src/components/HistoryList.tsx` | Avoid repeating the message count when the history preview is empty. |
| `apps/desktop/ui/src/components/AnalysisPanel.tsx` | Reduce repeated root-path presentation while retaining the path as a title. |
| `apps/desktop/ui/src/App.test.tsx`, `apps/desktop/ui/src/components/WorkspacePanel.test.tsx`, `apps/desktop/ui/src/components/CodeIntelligencePanel.test.tsx`, `apps/desktop/ui/src/components/HistoryList.test.tsx` | Accessibility, search, stale-request, and empty-preview regression coverage. |
| `README.md` | Keep the project status and verification summary accurate. |
| `docs/phase-18/README.md` | Record this refinement and actual checks/limitations. |

No Rust/native, Tauri configuration, package/dependency, or lockfile changes were made. Existing Nexora icon assets remain unchanged.

## Verification

Executed after the UI and test changes:

```text
pnpm check
```

**Result: PASS** — TypeScript check passed; Vitest passed all **44 files / 500 tests**; production build passed (90 modules). Build assets: CSS 67.38 kB / 10.61 kB gzip; JS 420.00 kB / 122.43 kB gzip. Vite emitted the same two existing dynamic/static import chunking warnings for `@tauri-apps/api/window.js` and `ui/src/lib/patch.ts`; these were present at the earlier baseline.

`git diff --check` **PASS**. Preview HTTP smoke checks also **PASS** on port 1420: `/`, `/src/main.tsx`, `/src/App.tsx`, the WorkspacePanel/CodeIntelligencePanel/HistoryList/Composer/AnalysisPanel modules, and `/src/styles/app.css` all returned HTTP 200. These requests confirm the dev server/module pipeline responds; they are not visual inspection or a Tauri runtime test.

## Limitations and manual Windows verification

This Linux environment has no `cargo` or `rustc`; Rust formatting/checks/tests and Windows packaging were not run. No browser executable or display server is available in the agent environment, so agent-side visual inspection, screenshots, OS scaling tests, and native title-bar interaction checks were not possible. The Vite preview is available for manual browser review, but is not the Tauri desktop runtime; inference remains unavailable outside the desktop shell. No real `llama-server`/GGUF session was exercised.

Before shipping, verify on Windows with the pinned Rust toolchain, WebView2, Visual Studio C++ build tools, supported `llama-server`, and a user-supplied GGUF model:

1. `cargo fmt --all -- --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, and the Tauri Windows build/package.
2. Native window sizing at 960×640 minimum, 1280×840 default, wide/maximized, Windows display scaling (including 150–200%), plus title-bar drag/minimize/maximize/close.
3. Explorer scrolling, workspace open/close/refresh, file preview, symbol/graph panels, evidence and budget warnings, patch preview/validation, and explicit Apply flow at narrow and wide layouts.
4. Keyboard-only navigation, screen-reader labels, native confirmation dismissal, focus visibility, and reduced-motion/reduced-transparency preferences.
5. Local model setup, streaming/cancellation, history persistence, workspace safety, and patch-apply behavior with a real local model; these workflows were not runtime-tested by this UI-only refinement.

## Publishing and Git

All work remains local and uncommitted on `arena/555ee411-nexora`, based on `6ea0dbd9784db6f1e353fb87be84686cc12ae46e`. No commit, push, pull request, release, installer publication, visibility change, branch switch, reset, clean, or history rewrite was performed.

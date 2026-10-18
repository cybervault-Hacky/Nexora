# Phase 21 — Sign In Page & Frontend Interactions

**Status:** Implemented in the local working tree; uncommitted.

**Scope:** The sign-in **interface** and its safe frontend behaviour only. No real authentication: no accounts, sessions, identity provider, database schema, create-account flow, email verification, password-reset backend, or Google OAuth (Phases 22–23). No glassmorphism (Phase 29), no animation/aurora system (Phase 30), no workspace redesigns.

**Checkout:** `arena/1008499f-nexora`, based on `128781618917fd15d2923aeb8eec04e9a5e90e98`. The working tree held the full uncommitted Phase 19 + Phase 20 implementations at the start of this phase; all of it was preserved and built upon — nothing was reset, reverted, or overwritten.

## Audit (before implementation)

- Branch `arena/1008499f-nexora`; working tree = Phases 19–20 (uncommitted): hash router (`lib/router.ts` with `welcome`/`onboarding` flow routes), `WelcomePage`, `OnboardingPage` (reserved sign-in slot), `App.tsx` flow/workspace branch, `.field`/`.field__input` form styles (used by `ModelSetup`), `panel-enter` motion, `GoogleMark` not yet present.
- Baseline `pnpm check` **before any change**: typecheck PASS, **47 files / 529 tests PASS**, production build PASS (103 modules).
- Adapted from the actual code: reused the existing `.field`/`.field__label`/`.field__input`/`.field__row` styles and the `onboarding__card` layout rather than inventing a parallel form system.

## Routes and flow

| Route | Page | Role |
|---|---|---|
| `#/signin` | `pages/SignInPage.tsx` | The Phase 21 sign-in interface. |
| `#/signup` | `pages/PlaceholderPage.tsx` | Honest Phase 22 create-account placeholder. |
| `#/recover-password` | `pages/PlaceholderPage.tsx` | Honest Phase 22 password-recovery placeholder. |

Flow (all in the pre-workspace flow shell, no sidebar): Welcome (`#/`) → Continue → Onboarding (`#/onboarding`) → **Continue to Sign in** (`#/signin`) → Back to onboarding / Forgot password? (`#/recover-password`) / Create account (`#/signup`). Unknown routes keep the graceful fallback. `#/home` remains the workspace entry; the Welcome entry flow is unchanged.

**OnboardingPage update (required by the brief):** its forward action is now **Continue to Sign in** (primary) while **Back to Welcome** and **Open the workspace** are preserved; the reserved-slot copy now states honestly that the sign-in screen is ready but interface-only. `App.welcome.test.tsx` (Phase 20) passes unmodified.

## Sign-in page contents

- Nexora brand lockup (same `ApertureMark` + `nexora` lockup as Welcome/Onboarding), step strip with `aria-current="step"` on *Sign in*, heading **"Welcome back"**, brief supporting copy.
- **Email** (`type="email"`, `autoComplete="email"`, `inputMode="email"`, labelled *"Email address"*) and **Password** (`type="password"`, `autoComplete="current-password"`, labelled *"Password"*) with accessible labels, `aria-invalid`, and `aria-describedby` error association.
- **Password visibility toggle** — icon button with state-dependent accessible name (*Show password* / *Hide password*) plus `aria-pressed` and `aria-controls`; never loses the typed value.
- Primary **Sign in** submit button with loading (`Signing in…`, `aria-busy`) and disabled states.
- **Forgot password?** and **Create account** links to the Phase 22 placeholder routes; **Back to onboarding** action.
- **Sign in with Google** button with the recognisable four-colour Google "G" (`components/GoogleMark.tsx`, inline SVG — no network asset, no unrelated brand styling).

## Frontend behaviour (real, client-side, local only)

- Validation on submit: email present and reasonable format (`local@domain.tld` shape — no deliverability claims), password non-empty. Errors render beside their field (`role="alert"`), focus moves to the first invalid field, and **no entered value — including the password — is cleared** on validation failure. Editing a field clears its error.
- Keyboard: native form submission (Enter), all controls tab-reachable with visible `:focus-visible` outlines; the layout works at narrow widths (footer stacks ≤560 px) and inside the Tauri window.
- Repeat submissions are blocked while the local submission state is active (guard + disabled button). The 400 ms local submission beat exercises the real loading/disabled button states; it performs no network access — and the terminal status is always the honest one.
- **Valid submission never claims success.** It shows: *"Sign-in is not connected yet. Account authentication will be enabled in a later phase."* — no session, no navigation into an authenticated area, no fabricated account state. Google sign-in click shows: *"Google sign-in will be connected with the authentication backend in a later phase."* — no OAuth endpoint, no account picker, no client secrets.

## Security boundaries

- **Credentials are never transmitted, stored, or logged.** There is no fetch/invoke with the values; a regression test asserts they never appear in `localStorage`/`sessionStorage` writes or console output.
- No password in URL parameters, preferences, or any persisted store; no hardcoded credentials; no API/OAuth secrets. `GoogleMark` is display-only.
- No changes to the Tauri capabilities allowlist or the CSP (`default-src 'self'; …`) — the page renders offline with zero new network surface.
- The UI explicitly communicates that authentication is not connected; nothing in this phase claims security or completeness of authentication.

## Accessibility

One `h1` per screen; labelled inputs via `htmlFor`/`id`; `aria-invalid` + `aria-describedby` for field errors; `role="alert"` on errors and `role="status"`/`aria-live="polite"` for the honest info statuses; `aria-busy` on the loading button; toggle button with dynamic accessible name + `aria-pressed`; decorative divider/`GoogleMark` hidden from assistive tech; text on graphite surfaces uses the contrast-tuned theme tokens; no font sizes below the existing 11 px floor. Small transitions reuse `--motion-fast` and the existing `panel-enter` entrance, covered by the universal `prefers-reduced-motion` override.

## Files changed (this phase)

| File | Change |
|---|---|
| `apps/desktop/ui/src/lib/router.ts` | `signin`, `signup`, `recover` routes; flow-route list extended. |
| `apps/desktop/ui/src/lib/router.test.ts` | Updated route map/flow-route assertions. |
| `apps/desktop/ui/src/pages/SignInPage.tsx` | New sign-in interface with local validation and honest statuses. |
| `apps/desktop/ui/src/pages/PlaceholderPage.tsx` | New shared honest placeholder (Phase 22 routes). |
| `apps/desktop/ui/src/pages/OnboardingPage.tsx` | Forward action → *Continue to Sign in* (welcome + workspace actions preserved); slot copy refreshed. |
| `apps/desktop/ui/src/components/GoogleMark.tsx` | New inline four-colour Google "G" mark. |
| `apps/desktop/ui/src/components/Icon.tsx` | Added `eye` / `eye-off` icons. |
| `apps/desktop/ui/src/App.tsx` | Flow branch routes the three new pages. |
| `apps/desktop/ui/src/styles/app.css` | Phase 21 section: sign-in card, field error/invalid states, visibility toggle, Google button, divider, footer (existing tokens + Google identity colours on the Google button only). |
| `apps/desktop/ui/src/App.signin.test.tsx` | New suite (11 tests). |
| `README.md` | Entry flow, features, tree, status, phase notes, verification. |
| `docs/phase-21/README.md` | This document. |

No changes under `apps/desktop/src-tauri/`, `package.json`, `pnpm-lock.yaml`, or `tauri.conf.json`. No Phase 19/20 test was removed or weakened.

## Tests and actual results

```text
pnpm check        # = pnpm typecheck && pnpm test && pnpm build
git diff --check
```

| Check | Result |
|---|---|
| `pnpm typecheck` | **PASS** (one test-file typing fix applied mid-run: `HTMLElement` → `HTMLButtonElement` cast) |
| `pnpm test` | **PASS — 48 files / 540 tests** (baseline 47/529; +`App.signin.test.tsx` 11; **no test removed**) |
| `pnpm build` | **PASS** — 106 modules; CSS 83.52 kB / 13.08 kB gzip; JS 449.65 kB / 130.20 kB gzip (same two pre-existing Vite chunking warnings as baseline). |
| `git diff --check` | **PASS** |
| Vite dev-server smoke (port 1420) | `SignInPage`, `PlaceholderPage`, `GoogleMark`, `router.ts`, `app.css` all **HTTP 200** (pipeline check only). |

Sign-in suite coverage: page rendering with labelled fields/actions and autocomplete attributes; required-field errors with accessible association; invalid-email handling with password retention; accessible password-visibility toggle; keyboard submission with the honest not-connected status and no success claim; loading/disabled state and blocked repeat submissions (single status); credentials never stored or logged; Google informational status without navigation; recovery/create-account placeholder navigation with working Back; Welcome → onboarding → Sign in → Back integration; unknown-route fallback and intact workspace navigation.

## Known limitations and later phases

- **Phase 22:** real create-account form, email verification, password recovery UI backed by real flows (routes `#/signup`, `#/recover-password` are honest placeholders today).
- **Phase 23:** authentication backend, sessions, identity provider, database schema, and real Google OAuth (the button and form currently end in honest "not connected" statuses).
- The 400 ms local submission beat is a UI-state exercise only; when the backend exists, the same states will wrap the real request.
- `cargo`/Windows/Tauri runtime checks remain unavailable in this Linux sandbox (no Rust source changed). Manual Windows verification: form focus order and screen-reader announcements, WebView2 autofill behaviour, narrow-window layout at 960×640 and display scaling, reduced-motion rendering.

## Git

All work is local and uncommitted on `arena/1008499f-nexora`, layered on top of the preserved Phase 19 + Phase 20 working tree. No commit, push, pull request, release, visibility change, branch switch, reset, clean, or history rewrite was performed. Phase 22 was **not** started.

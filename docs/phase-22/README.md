# Phase 22 — Create Account & Password Recovery

**Status:** Implemented in the local working tree; uncommitted.

**Scope:** Frontend pages, validation, route transitions, and honest status messaging for account creation, email verification (pending experience), password recovery, and password reset. **No real authentication backend**: no account is created, no email is sent, no password is reset, no session appears. No Google OAuth, no Developer/Founder centre, no payments, no glassmorphism (29), no animation system (30), no workspace redesigns.

**Checkout:** `arena/1008499f-nexora`, based on `128781618917fd15d2923aeb8eec04e9a5e90e98`. The working tree held the full uncommitted Phase 19 + 20 + 21 implementations at the start of this phase; all of it was preserved and built upon — nothing was reset, reverted, or overwritten.

## Audit (before implementation)

- Branch `arena/1008499f-nexora`; working tree = Phases 19–21 (uncommitted). Inspected `lib/router.ts`, `SignInPage.tsx`, `PlaceholderPage.tsx` (the P22 stand-in used by `App.tsx` for `#/signup` and `#/recover-password`), `App.tsx` flow branch, shared `.field`/`.signin__*` form styles, and the test setup.
- Baseline `pnpm check` **before any change**: typecheck PASS, **48 files / 540 tests PASS**, production build PASS (106 modules).
- Adapted to actual conventions: pages stay self-contained in `ui/src/pages/`, forms reuse `.field`/`.field__input`/`.signin__*` classes and the `onboarding__card` shell, statuses use the Phase 21 `role="status"` pattern, validation messages match Phase 21's wording where the fields overlap.

## Pages and routes

| Route | Page | Role |
|---|---|---|
| `#/signup` | `pages/CreateAccountPage.tsx` | Real create-account interface (replaces the Phase 21 placeholder). |
| `#/verify-email` | `pages/VerifyEmailPage.tsx` | Dedicated verification-pending experience (new route). |
| `#/recover-password` | `pages/RecoverPasswordPage.tsx` | Real recovery interface (replaces the Phase 21 placeholder). |
| `#/reset-password` | `pages/ResetPasswordPage.tsx` | Password-reset interface — the future recovery-email link target (new route). |

`pages/PlaceholderPage.tsx` was **removed**: its only two usances were the `#/signup`/`#/recover-password` stand-ins that this phase replaces. No other component referenced it.

### Route decision: `#/reset-password`

Implemented because the future authentication backend's recovery emails will need a concrete frontend target (`#/reset-password?token=…`), and having the page now makes that flow explicit and testable. It is reachable by direct route today; it **does not read, accept, or validate tokens** (that requires the backend) and says so on the page. No in-app link pretends that reset emails already exist.

## Form fields and validation rules

Shared rules live in `lib/formValidation.ts` (purely local; nothing is sent):

| Field | Rule | Error messages |
|---|---|---|
| Full name (`#/signup`) | non-empty after trimming | `Enter your full name.` |
| Email (all pages) | non-empty + reasonable `local@domain.tld` shape (no deliverability claims) | `Enter your email address.` / `Enter a valid email address.` |
| New password (`#/signup`, `#/reset-password`) | non-empty and **≥ 8 characters** (`PASSWORD_MIN_LENGTH = 8`) | `Enter your password.` / `Use at least 8 characters.` |
| Password confirmation | non-empty and matches the password | `Re-enter your password.` / `Passwords do not match.` |
| Sign-in password (`#/signin`, unchanged) | presence only — sign-in never enforces length on existing passwords | `Enter your password.` |

**Minimum length choice:** 8 characters — the NIST SP 800-63B lower bound for user-chosen passwords. No composition rules (no forced symbols/case/digits), per NIST guidance that composition rules have no clear security justification. The floor is documented here and shown as field help (`Use at least 8 characters.`).

Behaviour (all forms): errors render beside their field with `role="alert"` and are wired via `aria-invalid` + `aria-describedby`; submission focuses the first invalid field in page order; **validation failures never clear what was typed** (including passwords); editing a field clears its error; Enter submits; the local submission state (button `disabled` + `aria-busy` + loading label, 400 ms beat) blocks repeated submissions; native `autocomplete` attributes (`name`, `email`, `new-password`) are set.

## Honest submission behaviour

No form ever reports success. After a **valid** client-side submission each page shows its terminal status and stays on the page:

- `#/signup` → *"Account creation is not connected yet. Your details have not been submitted."* — no local/fake user record, no authenticated state, no navigation, no email claim. Password + confirmation fields are cleared once the submission completes (name/email are kept for editing); nothing is cleared on validation errors.
- `#/recover-password` → *"Password recovery is not connected yet. No reset email has been sent."* — the email is never transmitted; nothing claims an account exists or does not exist.
- `#/reset-password` → *"Password reset is not connected yet. No password has been changed."*

## Email verification experience (`#/verify-email`)

A dedicated verification-pending page for the future flow, reachable from the create-account status ("About email verification") and by direct route. It states that new accounts will need to verify email once account creation is connected, and makes its non-functional status explicit: **no email has been sent, there is no code to enter, no token to redeem, and nothing to resend**. It contains no code input, no resend button, and no fabricated token; *Back to Sign In* works.

## Security boundaries

- **Nothing is transmitted, stored, or logged.** Regression tests spy on `fetch` and `Storage.prototype.setItem` around submissions and on `console.log`/`console.error`, asserting that passwords, emails, and names never appear (the app may store unrelated workspace preferences; only the entered values are forbidden).
- Passwords are not kept in component state after a completed submission (cleared + visibility toggles reset); validation errors never destroy input.
- No hardcoded credentials, no API/OAuth secrets, no token handling; no changes to Tauri capabilities or the CSP; no fake authenticated state anywhere. The UI consistently communicates that no real account or recovery operation has occurred.

## Accessibility

One `h1` per screen; every input has a real `<label>`; `aria-invalid`/`aria-describedby` error association with `role="alert"` announcements; `role="status"`/`aria-live="polite"` for terminal statuses; `aria-busy` loading buttons; per-field visibility toggles with distinct accessible names (`Show password` / `Show password confirmation` / `Show new password`…) plus `aria-pressed`/`aria-controls`; step strip and notes use `aria-current="step"` and `role="note"`; decorative marks are `aria-hidden`; the layout reuses the Phase 21 card that stacks at narrow widths (≤560 px footer stack) and respects reduced-motion via the universal override.

## Files changed (this phase)

| File | Change |
|---|---|
| `apps/desktop/ui/src/lib/formValidation.ts` | New shared local validators + documented `PASSWORD_MIN_LENGTH`. |
| `apps/desktop/ui/src/pages/CreateAccountPage.tsx` | New create-account interface. |
| `apps/desktop/ui/src/pages/VerifyEmailPage.tsx` | New verification-pending experience. |
| `apps/desktop/ui/src/pages/RecoverPasswordPage.tsx` | New recovery interface. |
| `apps/desktop/ui/src/pages/ResetPasswordPage.tsx` | New reset interface (future email-link target). |
| `apps/desktop/ui/src/pages/PlaceholderPage.tsx` | **Removed** — superseded by the two real pages it stood in for. |
| `apps/desktop/ui/src/pages/SignInPage.tsx` | Uses shared `validateEmail` (message-identical); non-empty password rule kept local. |
| `apps/desktop/ui/src/lib/router.ts` | `verify` (`/verify-email`), `reset` (`/reset-password`); flow list extended. |
| `apps/desktop/ui/src/lib/router.test.ts` | New route map / flow-route / href assertions. |
| `apps/desktop/ui/src/App.tsx` | Flow branch routes the four pages. |
| `apps/desktop/ui/src/styles/app.css` | Small Phase 22 additions (status-next line, links label). |
| `apps/desktop/ui/src/App.signup.test.tsx` | New suite (9 tests). |
| `apps/desktop/ui/src/App.recovery.test.tsx` | New suite (9 tests). |
| `apps/desktop/ui/src/App.signin.test.tsx` | One navigation test's two heading assertions updated to the real pages it now reaches (coverage kept; all other P21 tests untouched). |
| `README.md` | Entry flow, features, tree, status, phase notes, verification. |
| `docs/phase-22/README.md` | This document. |

No changes under `apps/desktop/src-tauri/`, `package.json`, `pnpm-lock.yaml`, or `tauri.conf.json`.

## Tests and actual results

```text
pnpm check        # = pnpm typecheck && pnpm test && pnpm build
git diff --check
```

| Check | Result |
|---|---|
| `pnpm typecheck` | **PASS** |
| `pnpm test` | **PASS — 50 files / 558 tests** (baseline 48/540; +`App.signup.test.tsx` 9, `App.recovery.test.tsx` 9; **no test removed**) |
| `pnpm build` | **PASS** — 110 modules; CSS 83.66 kB / 13.10 kB gzip; JS 462.60 kB / 132.44 kB gzip (same two pre-existing Vite chunking warnings as baseline). |
| `git diff --check` | **PASS** |
| Vite dev-server smoke (port 1420) | All four pages + `formValidation.ts` + `app.css` **HTTP 200** (pipeline check only). |

Coverage: create-account rendering/labels; required-field, invalid-email, min-length, and mismatch validation with accessible association; value retention on failure; both visibility toggles; keyboard submission with the honest status and post-submission password clearing; loading/disabled state with blocked repeat submissions; no credentials in storage/console/network; verification-pending navigation and its no-code/no-token/no-resend honesty with working Back; recovery rendering, validation, honest status with no false email-sent/account claims and no network; reset page rendering (no token field), validation, honest status; Sign In ⇄ Create Account/Recovery navigation; browser history back; direct route loads; unknown-route fallback; welcome entry and workspace navigation intact; Phase 21 sign-in suite otherwise untouched.

## Known limitations and remaining backend requirements (Phase 23+)

- Real account creation, email delivery + verification, recovery/reset-token issuance and validation, sessions, identity provider, and the account database all remain **unimplemented** — every terminal status says so.
- The 400 ms local submission beat is a UI-state exercise only; when the backend exists, the same states will wrap the real requests.
- `cargo`/Windows/Tauri runtime checks remain unavailable in this Linux sandbox (no Rust source changed). Manual Windows verification: screen-reader announcements per field, WebView2 autofill (`name`/`email`/`new-password`), narrow-window layout at 960×640 and scaling, reduced-motion rendering.

## Git

All work is local and uncommitted on `arena/1008499f-nexora`, layered on top of the preserved Phase 19–21 working tree. No commit, push, pull request, release, visibility change, branch switch, reset, clean, or history rewrite was performed. Phase 23 was **not** started.

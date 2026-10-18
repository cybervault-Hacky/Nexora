# Phase 23F — Google OAuth (state/PKCE/loopback + linking rules)

**Status:** Implemented in the local working tree; uncommitted. **No Rust code has been compiled or executed** (environment re-verified unchanged this phase: `cargo`/`rustc`/`rustfmt` exit 127, crates.io TLS-blocked exit 35, PostgreSQL absent). The frontend Google-button wiring (plan step 5's client half) is **blocked** — see §5. The next roadmap step was not started.

**Exact roadmap item implemented:** `docs/phase-23/README.md` §9 sequence step 5 — *"Google OAuth (state/PKCE/loopback + linking rules; wire the Google button)"* — the next unimplemented step after 23E completed step 4 (recovery + reset + password-change). Specification: §2.5 (planned behaviour: PKCE S256, state/nonce, redirect-URI exact match, server-side code exchange, linking rules), §6 (`POST /v1/auth/google/exchange {code, codeVerifier, state, redirectUri}` → `200 {user, session}` | `409 email_conflict`; "Server holds client secret"), §4 (OAuth threat rows), §5 (`oauth_accounts` keyed by `provider_subject`), §7 (`GOOGLE_OAUTH_*` env inventory; `google` rate-limit scope; test list "Google state/nonce/PKCE validation, redirect-URI mismatch, code-replay, linking rules (verified match, unverified reject, conflict 409)"). **Not included:** step 6 (hardening/secret-leakage suite) and step 7 (Windows verification pass), any other auth work.

## 1. Exchange — `POST /v1/auth/google/exchange` (plan §6)

Service (`src/oauth.rs`), in order — invalid input never burns state or reaches Google:
1. **Shape validation:** authorization code present; PKCE `code_verifier` per RFC 7636 (43–128 unreserved); `state` 32–128 URL-safe chars (128-bit desktop generator); `nonce` optional ≤256 (contract clarification below).
2. **Redirect-URI exact match** against the configured registered loopback (`http://127.0.0.1:<port>/callback` shape enforced at config time too). Any mismatch = hard generic `invalid_or_expired_token` — the provider is never reached.
3. **State one-time-ness (plan §2.5 "mirrored (hashed) server-side; single-use, 10-min TTL"):** sha256(state) recorded in `google_oauth_states`; replays refused (23505 → generic failure); rows pruned past the 10-minute window (a replay past that window already fails at Google: codes are single-use and short-lived). State freshness/equality is enforced in the desktop process (client half, blocked).
4. **Provider exchange** via the `GoogleProvider` seam. `HttpGoogleProvider` performs the **real server-side exchange** against Google's documented token endpoint (`https://oauth2.googleapis.com/token`) with the optional client secret — plan §2.5: the exchange is server-side for both client types (unresolved decision #3 solved by supporting secret-less desktop clients AND secret-bearing clients; the secret lives only in the server secret store). Claims are read from the ID token received directly from that endpoint over TLS (OIDC core permits skipping signature validation at this hop; **JWKS verification is step-6 hardening**). `UnconfiguredGoogleProvider` answers honestly (`dependency_unavailable` 503) — **no identity is ever invented**, exactly like `EmailProvider`.
5. **Nonce** (plan §2.5/§7): when the request supplies a nonce, the ID token must carry the same claim — mismatch/missing ⇒ hard generic failure. *(Contract clarification: the plan's §6 request abbreviates the body as `{code, codeVerifier, state, redirectUri}` while §2.5/§7 require nonce validation; the desktop generates the nonce with the authorization request, so the exchange accepts an optional `nonce` field — the same style of documented clarification used for the session token placement in 23D.)*
6. **Claims:** `sub` required (≤255, the immutable linking key); **`email_verified` from Google is the only basis for trusting the email** — unverified provider emails are refused outright (403 `email_unverified`), never linked, never created.
7. **Linking rules (plan §2.5/§7), under one transaction with the account row lock (23E discipline):**
   - `oauth_accounts (provider='google', provider_subject=sub)` found → sign in (active only; locked/deleted ⇒ generic `invalid_credentials`).
   - else verified + active account with the **exactly matching normalized email** → **auto-link** + sign in (the verified-match rule).
   - else account exists but unverified ⇒ **`409 email_conflict`** with the plan's guided message ("An account with this email already exists. Sign in with your password to link Google.") — proof of existing-account control required first, **no auto-link**.
   - else **create** an OAuth-only account (`password_hash` NULL, `email_verified_at` = now() on the Google-verified email) + link + sign in. `ON CONFLICT` races (email or subject) resolve to the existing row.
8. **Session:** same as password login — 32-byte `OsRng` token, sha256 at rest, absolute TTL — returned `200 {user, session:{token, expiresAt}}` under the 23D token-placement rules (trusted desktop client only, never the WebView, never logged).

Rate limiting: plan §4 `google/exchange` windows (scope `google`, per-IP, DB-backed `login_attempts`, 429 + `Retry-After`, no lockout). Provider/identity failures collapse into two generic codes (`invalid_or_expired_token`, `email_unverified`) plus the plan's `email_conflict` — no account existence, database, provider, or credential detail ever leaks.

## 2. Database & configuration

- **Migration `20261018000006_create_google_oauth_states.sql`** (forward-only; nothing prior touched; documented forward-recovery): `google_oauth_states(state_hash bytea PRIMARY KEY, seen_at)` + prune index. `oauth_accounts` (23B) already matches plan §5 — unchanged. No plaintext states/tokens stored.
- New envs (validated all-or-nothing; values never printed; `Debug` redacts id/secret): `NEXORA_AUTH_GOOGLE_CLIENT_ID` + `NEXORA_AUTH_GOOGLE_REDIRECT_URI` (must be the exact loopback callback shape) configure the flow; `NEXORA_AUTH_GOOGLE_CLIENT_SECRET` optional (server secret store only). Nothing set = honest 503; partial configuration = loud startup error ("(Values withheld.)").
- **Two new dependencies** (`Cargo.toml`, justified inline): `reqwest` (rustls+json — the server-side code exchange is this phase's core contract) and `base64` (standard base64url decode of the ID-token payload; no hand-rolled codecs). No new database, framework, or runtime.

## 3. Tests — written vs actually run

| Suite | Written | Ran? | Result |
|---|---|---|---|
| Frontend `pnpm typecheck` / `pnpm test` | (existing) | **YES** | **PASS — 50 files / 569 tests** (app byte-unchanged) |
| `git diff --check` | — | **YES** | **PASS** |
| `oauth.rs` units (8) | YES | **NO** | cargo absent (127) + crates.io blocked (35) |
| `config.rs` new units (3; 24 total) | YES | **NO** | same |
| `tests/oauth_http.rs` (6, no DB) | YES | **NO** | same — intended first in CI |
| `tests/oauth_db.rs` (12, **real PostgreSQL**, `#[ignore]`) | YES | **NO** | PostgreSQL absent; `NEXORA_AUTH_TEST_DATABASE_URL=… cargo test --test oauth_db -- --ignored` |
| 23C–23E suites (unchanged; regression in `oauth_db.rs`) | (74) | **NO** | same blocker |
| `cargo fmt` / `cargo check` | — | **NO** | toolchain absent |

Coverage intent: state single-use + replay generic failure; redirect mismatch refused before the provider (panic-on-reach provider proves it); verified-match auto-link + session; unverified-match 409 with no link; unverified provider email refused with zero writes; OAuth-only account creation (NULL password, verified email, `sub` keying); same-sub-≠-email sign-in; locked linked accounts refused; nonce mismatch/missing/match; provider refuse vs unconfigured distinction; `google` rate window; 23C–23E regressions. **The Google network hop itself is the one injected edge** (no network, no credentials here) — the tests exercise real database/logic paths with a `GoogleIdentity` seam, exactly like the email `CaptureProvider` pattern; the live exchange is compile-and-config verified only.

## 4. Frontend status — Google button wiring BLOCKED

Plan step 5's client half ("wire the Google button" via `auth_google_begin` IPC → system browser → loopback listener) is desktop-crate work (Tauri `shell:open` scope, keyring, IPC) behind the same **absent secure transport** as 23C–23E: the WebView CSP stays closed to this service, the service is not auto-started, and privileged material must never cross into the React frontend (plan §2.4). `SignInPage.tsx` keeps its honest informational Google entry (byte-unchanged); nothing claims Google sign-in works. Local AI/projects/settings remain login-free.

## 5. Known limitations / follow-ups (existing plan)

- **Compiler-unverified blocker (23B–23F):** `cargo fmt --check`, `cargo check --all-targets`, `cargo test`, then the `--ignored` DB suites on a disposable PostgreSQL must pass before reliance; `reqwest`/`base64` resolution is likewise unverified (no crates.io access, no `Cargo.lock`).
- Live Google end-to-end (real client, real consent screen) is untested by definition here; ID-token **JWKS signature verification**, log-scrubbing middleware, and the secret-leakage/fuzz suites are step-6 hardening.
- Desktop IPC + keyring + Google-button wiring (plan step 5 client half) — blocked (§4).

## 6. Git

Local and uncommitted on `arena/1008499f-nexora`, layered on the preserved Phase 19–23E + multi-file-patch tree. No commit, push, merge, release, visibility change, reset, clean, or history rewrite occurred. The next roadmap step was **not** started.

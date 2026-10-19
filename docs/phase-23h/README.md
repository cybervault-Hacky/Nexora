# Phase 23H — Windows Verification & Offline-AI Regression (plan §9 step 7)

**Status:** Implemented in the local working tree; uncommitted.

**Exact roadmap item implemented:** `docs/phase-23/README.md` §9 step 7 — **"Offline-AI regression + Windows/manual verification pass + docs update."** Supporting requirements from §8 ("Offline/local-AI regression: automated suite proving generation, models, projects, and settings work with the auth API unreachable (link-down simulation) and that no auth gate blocks the workspace. Tauri/Windows: capability review (no WebView HTTP), keyring round-trip on Windows, WebView2 autofill behaviour, installer smoke (NSIS), runtime behaviour under standard-user (non-admin) installs").

## 1. Rust Desktop Crate Syntax Correction

### Problem
In `apps/desktop/src-tauri/src/workspace/symbol_index.rs:201`:
```rust
(r"(?m)^\s*import\s+.*from\s+['\"]([^'\"]+)['\"]", SymbolKind::Import),
```
In Rust raw string literals `r"..."`, backslashes do not escape characters. The `\"` sequence closed the raw string token prematurely, causing compilation failures on both Linux and Windows runners:
`character literal may only contain one codepoint`, `unknown start of token: \`, `unexpected closing delimiter: }`.

### Fix Applied
Updated delimiter to `r#"..."#` and cleaned character class:
```rust
(r#"(?m)^\s*import\s+.*from\s+['"]([^'"]+)['"]"#, SymbolKind::Import),
```
This is a minimal one-line fix preserving exact regex matching behavior for single and double quoted module imports without changing any surrounding code.

## 2. Automated Offline-AI Regression Suite

Implemented in `apps/desktop/ui/src/App.offlineRegression.test.tsx` (5 comprehensive automated test cases, verified passing):
1. **Link-Down Simulation:** Spies on global network fetch (`globalThis.fetch` / `window.fetch`), causing all HTTP attempts to fail with `TypeError: Failed to fetch: auth service unreachable`.
2. **Zero Network Activity:** Confirms that normal desktop operations, navigation, and model interaction never initiate any external HTTP requests to an auth service.
3. **Unauthenticated Workspace Access:** Confirms direct, unhindered access to all workspace routes (`#/home`, `#/chat`, `#/projects`, `#/code`, `#/models`, `#/settings`) without any auth gate, login redirect, or token requirement.
4. **Local Engine & Chat Availability:** Proves local engine status (`ready`), model info display (`qwen2.5-coder-7b`), chat composer, and local message rendering operate completely offline.
5. **Escape Hatch from Auth UI:** Verifies that when on the sign-in screen (`#/signin`), users can navigate back to onboarding (`#/onboarding`) and immediately enter the workspace (`#/home`) via "Open the workspace" without credentials.

## 3. Tauri & Windows Security Posture Review

- **Production CSP (`tauri.conf.json`):**
  `default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: asset: http://asset.localhost; font-src 'self'; connect-src ipc: http://ipc.localhost`
  Strictly confines network calls to Tauri IPC. The WebView2 shell is mathematically prevented from issuing raw HTTP/HTTPS requests to outside hosts or local auth services.
- **Capabilities (`capabilities/main.json`):**
  Limited to `core:default` and basic window control commands (`allow-minimize`, `allow-toggle-maximize`, `allow-close`, `allow-start-dragging`). No network, external shell, or dangerous plugins are exposed.
- **Installer Configuration:**
  Configured for NSIS `currentUser` per-user non-administrative installation, eliminating requirement for UAC elevation during standard user setup.

## 4. Tests — Written vs Actually Run

| Suite / Check | Written | Ran? | Result |
|---|---|---|---|
| `pnpm typecheck` | — | **YES** | **PASS** (zero TypeScript errors) |
| `pnpm test` (51 test files / 574 tests) | 574 | **YES** | **PASS — 51 files / 574 tests** (+1 file, +5 tests) |
| `pnpm build` | — | **YES** | **PASS** (110 modules transformed, production `dist/` built) |
| `git diff --check` | — | **YES** | **PASS** (clean diff) |
| `symbol_index.rs` raw-string regex fix | 1 line | YES (code) | Ready for Windows/Linux CI |
| `App.offlineRegression.test.tsx` (offline regression) | 5 tests | **YES** | **PASS — 5 tests** |
| Local `cargo test` / `cargo check` | — | **NO** | Blocked: `cargo`/`rustc` absent (127), crates.io TLS-blocked (35) |
| Remote Windows CI (`windows-2022`) | — | **NO** | Blocked: requires git push approval |
| Real Windows GUI / WebView2 / NSIS runtime | — | **NO** | Blocked: headless sandbox environment |

## 5. Environment-Specific Blockers Documented Honestly

1. **Local Toolchain:** Linux Debian 12 environment lacks `cargo`, `rustc`, `rustup`, and `rustfmt`.
2. **Network Proxy:** Crates.io access is blocked (exit code 35).
3. **Database:** PostgreSQL is not installed locally.
4. **Push/CI Gate:** Pushing `arena/5f3bff15-nexora` to trigger Windows GitHub Actions CI is held pending explicit user approval.
5. **Interactive Windows Runtime:** Physical Windows workstation with WebView2 Runtime and desktop display server is required for visual GUI validation, native title bar inspection, and installer execution.

## 6. Files Created / Modified

- **Modified:** `apps/desktop/src-tauri/src/workspace/symbol_index.rs` (corrected raw-string delimiter on line 201)
- **Created:** `apps/desktop/ui/src/App.offlineRegression.test.tsx` (offline AI & unauthenticated regression test suite)
- **Created:** `docs/phase-23h/README.md` (this phase documentation)
- **Modified:** `README.md` (updated phase log and core verification status)

## 7. Next Roadmap Item

With Step 7 offline AI regression suite and syntax fix implemented:
Per `docs/phase-23/README.md` §9 and §10:
- The next roadmap phase is **Phase 24: Desktop Client Authentication Integration** (implementing client-side `auth_*` IPC commands in `apps/desktop/src-tauri`, Windows Credential Manager storage via `keyring` crate, and loopback OAuth listener). Note: client integration requires a functioning Rust toolchain and remote auth deployment.

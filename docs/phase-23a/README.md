# Phase 23A — Secure Rust Auth Backend Foundation

**Status:** Implemented in the local working tree; uncommitted. **This phase builds a health-only service scaffold — it contains no authentication of any kind.**

**Scope:** The first isolated backend foundation for the future authentication service proposed in `docs/phase-23/README.md`. No registration/sign-in/out/recovery, no email or verification, no Google OAuth, no sessions/JWT/tokens/credential storage, no PostgreSQL/SQLx/migrations, no frontend wiring, no automatic server startup, no deployment automation. Nothing in the desktop app changed.

**Checkout:** `arena/1008499f-nexora`, base HEAD `128781618917fd15d2923aeb8eec04e9a5e90e98`. The working tree held the uncommitted Phase 19–23 work at the start of this phase; all of it was preserved (verified before and after).

## 1. Service location and responsibility

```
services/nexora-auth/          ← NEW standalone Rust package (binary + library)
  Cargo.toml                   ← own manifest; intentionally NOT a Cargo
                                 workspace member and outside the pnpm
                                 workspace (pnpm-workspace.yaml covers apps/* only)
  src/lib.rs                   ← library root (app, config modules)
  src/app.rs                   ← router + GET /health + 404/405 envelopes + body limit
  src/config.rs                ← validated bind configuration (loopback default)
  src/main.rs                  ← manual-run binary: config → bind → serve → Ctrl+C shutdown
  tests/http_surface.rs        ← in-process router tests
  tests/bind_behaviour.rs      ← loopback-default / non-loopback-restriction tests
```

**Responsibility (today):** run as a standalone HTTP process that answers `GET /health` and nothing else. It exists so future authentication work (Phase 23 plan) has a home that is **separate from the Tauri desktop process** — per the architecture boundary, no network server is embedded in `apps/desktop/src-tauri`, and `apps/desktop/src-tauri/Cargo.toml` + its `Cargo.lock` are byte-untouched. The service **never starts with Nexora**; the desktop frontend is **not connected** to it; there is no reference to it anywhere in `apps/`. **[Correction (Phase 23B): `apps/desktop/src-tauri/Cargo.toml` is byte-untouched as stated; `Cargo.lock` does not exist in the repository (no Rust build has run here), so the claim about it was inaccurate.]**

## 2. Build and test commands

```bash
cd services/nexora-auth
cargo build            # compile the service
cargo test             # unit tests (config) + integration tests (HTTP surface, bind behaviour)
cargo fmt --check      # formatting
cargo clippy           # lints (optional but recommended)
cargo run              # start locally (loopback-only by default)
```

> **Sandbox note:** this Phase 23A runner has no Rust toolchain (`cargo`/`rustc`/`rustup` absent) and no crates.io egress (allowlist: github/npm/pypi only), so `cargo build/test/fmt/clippy` **could not be executed here**. The checks run and their outcomes are listed in the phase report and §9; the Rust commands above are the exact ones to run where a toolchain exists. No Rust check result is claimed without having run it.

## 3. Configuration variables and defaults

| Variable | Default | Meaning | Validation |
|---|---|---|---|
| `NEXORA_AUTH_BIND` | `127.0.0.1:8787` | `IP:PORT` (IPv6 as `[::1]:8787`) the service listens on | Must parse as `SocketAddr`; port must be 1–65535 (port 0 rejected). Unset/empty ⇒ default. Invalid values are rejected with an actionable error that names the variable and expected format but **never echoes the provided value**. |
| `NEXORA_AUTH_ALLOW_NON_LOOPBACK` | `false` | Explicit opt-in required before any non-loopback bind | Must be exactly `true` or `false`. Anything else is rejected. A non-loopback `NEXORA_AUTH_BIND` without `=true` is refused with an explanation. |

No other configuration exists. **No `.env` files, credentials, addresses, or secret values are hardcoded, read, or logged.** (An `.env.example` is deliberately not added; the table above is the full inventory.) Startup output prints the resolved bind address only.

## 4. Local-only development binding

Default bind is **`127.0.0.1:8787`** — loopback only, so a normal `cargo run` is invisible to the LAN and the internet. Binding `0.0.0.0`, a LAN IP, or `[::]` is **refused** unless `NEXORA_AUTH_ALLOW_NON_LOOPBACK=true` is set explicitly (validated), for deliberate firewalled deployments only. Tests assert both behaviours.

## 5. Security boundaries and known limitations

- **Health ≠ authentication, and health ≠ production readiness.** `GET /health` returns exactly `{"service":"nexora-auth","status":"ok"}` — it reports that the process is up and routing works. It does **not** claim that authentication, a database, email delivery, or OAuth is operational (none exist). Production readiness additionally requires everything in `docs/phase-23/README.md` (TLS, hardening, rate limits, secrets management, monitoring, …) — none of which this scaffold provides.
- **Error safety:** 404/405 use a fixed envelope `{"error":{"code":…,"message":…}}` with generic messages. No stack traces, filesystem paths, environment values, or secrets appear in any response (asserted by tests).
- **Request-size limit:** `DefaultBodyLimit` caps bodies at 4 KiB (`MAX_REQUEST_BODY_BYTES`) as defence-in-depth; nothing reads bodies.
- **No CORS** is configured (and no `tower-http` dependency is pulled in). Because the WebView is not integrated with this service, none is needed; **future desktop integration must be designed separately** (and the existing Tauri CSP `connect-src ipc: http://ipc.localhost` — untouched — would need an explicit, reviewed change if the WebView ever talks to it directly).
- **Tauri surface unchanged:** `tauri.conf.json` CSP and `capabilities/main.json` were not modified or weakened.
- **No `unsafe` Rust** anywhere in the crate.
- **Dependency minimalism:** `axum` (HTTP framework required by the approved plan), `tokio` (async runtime: serve loop, TCP, Ctrl+C shutdown), `serde` + `serde_json` (typed deterministic JSON), dev-only `tower` (`oneshot` in-process tests) and `http-body-util` (read test bodies). No auth, JWT, crypto, database, email, or OAuth dependency is present — by design.
- **Startup/ops:** no environment contents or secret values are printed; no logging framework yet (stderr status lines only); no deployment automation, containers, or CI changes.

## 6. Why there are no authentication endpoints

The Phase 23 plan keeps registration, sign-in, verification, recovery, OAuth, sessions, and storage behind separately approved implementation steps. Adding placeholder login/registration endpoints now would create fake, unauthenticated "auth" surface — the exact failure mode Phases 20–22 refused in the UI. A health-only scaffold is the honest minimal foundation: it proves the runtime, router, config validation, and test harness without pretending to authenticate anyone.

## 7. Adding PostgreSQL and authentication later (without coupling to local AI)

Future phases (23B+) add database/auth dependencies **inside this crate only** (`services/nexora-auth`), following `docs/phase-23/README.md` §5–§7 (tables, API contracts). The desktop app will reach the service through **Rust-side IPC commands** in a later, separately approved step — never by letting the WebView call the network (the CSP already forbids it) and never by starting this service from Tauri. Local AI inference (`apps/desktop/src-tauri/src/inference/`, llama.cpp, local JSON stores) remains fully independent: no code path in this crate touches models, projects, or settings, and auth will remain optional for offline AI use (plan §2.6).

## 8. Running locally without exposing it to the network

```bash
cd services/nexora-auth
cargo run
# → nexora-auth: listening on http://127.0.0.1:8787 (service health only; no authentication yet)
curl http://127.0.0.1:8787/health
# → {"service":"nexora-auth","status":"ok"}
```

That's the entire exposure: loopback-only, one route. To stop: Ctrl+C (graceful shutdown). Do **not** set `NEXORA_AUTH_ALLOW_NON_LOOPBACK=true` on a shared network.

## 9. Verification (actual outcomes — see the phase report for the full table)

| Check | Outcome |
|---|---|
| `pnpm typecheck` | PASS (frontend untouched but re-verified) |
| `pnpm test` | PASS — 50 files / 558 tests (no test added/removed/changed for the app) |
| `git diff --check` | PASS |
| `cargo test` (targeted: 9 config + 6 HTTP + 3 bind tests written) | **NOT RUN — no Rust toolchain and no crates.io egress in this sandbox** | **[Recount (Phase 23B verification gate): the file contains 9 config tests; the earlier "12 config / 21 total" claim was an overcount — actual 18 written tests.]** |
| `cargo fmt --check` | **NOT RUN — same reason** |
| `cargo check`/`cargo build` | **NOT RUN — same reason** |
| `pnpm build` | **NOT RUN** — it overwrites `dist/`; the existing `dist/` is preserved as instructed (frontend is unchanged this phase) |

## 10. Git

All work is local and uncommitted on `arena/1008499f-nexora`, layered on the preserved Phase 19–23 tree. No commit, push, PR, release, visibility change, branch switch, reset, clean, or history rewrite occurred. Phase 23B was **not** started.

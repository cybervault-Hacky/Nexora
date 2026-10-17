# Phase 8 — engineering hardening, CI verification, Windows build readiness

**Scope delivered:** two process-lifecycle defects fixed in the Rust core, two new
frontend/contract test suites plus lifecycle regressions, a reproducibility pin for
the Rust toolchain, and a three-job CI workflow. **No Windows build, no Rust
compilation, and no real inference were executed.** The environment is unchanged
from Phase 7 and is re-measured below.

This document is evidence, not the deliverable. The deliverable is the code,
configuration and tests listed in §5.

## 1. Repository state

| Item | Observed at the start of Phase 8 |
|---|---|
| Branch | `arena/00bd2df7-nexora` |
| HEAD | `b4a7921` (`b4a792117ddcfa947afa76b1f4efc113751dae73`) |
| Working tree | clean |
| Remote | `https://github.com/cybervault-Hacky/Nexora.git` (private), branch ref at the same commit |
| History | Phase 1–7 commits present; nothing reset, rewritten, or force-pushed |

The sandbox did not retain its local Git metadata between phases: the working tree
was intact but the branch ref was back at the initial commit. It was recovered by
fetching the remote branch, extracting `FETCH_HEAD` with `git archive`, comparing
every file against the working tree (identical), then re-pointing the branch ref
and re-running `git reset --mixed`. No file was re-authored and no commit was
recreated.

## 2. Environment recheck (one attempt, not repeated)

| Probe | Result |
|---|---|
| OS | Debian 12 (bookworm), Linux 6.1.158+, x86_64 |
| CPU / RAM | 2 vCPU / 3.8 GiB |
| `rustc`, `cargo`, `rustfmt`, `clippy-driver` | absent; no `~/.cargo`, `~/.rustup`, `/opt/rust` |
| `static.rust-lang.org`, `index.crates.io` | HTTP `000` (one request each) |
| Windows / MSVC / Windows SDK / WebView2 | absent |
| `pkg-config` | absent (so no Linux Tauri build either) |
| Node / pnpm | v22.22.3 / 9.15.4 (from `packageManager`) |
| Model weights | none; the only GGUF is a 441 KB random-weight synthetic file |

Consequence: `cargo fmt/check/clippy/test` and every Windows step are **NOT RUN**.
They are not reported as passing anywhere, including in the workflow file.

## 3. Audit findings

The audit read the real sources rather than trusting earlier reports.

### Defects fixed

**D1 — `Supervisor::stop()` could block forever while holding the supervisor mutex.**
`stop()` called `child.kill()` (ignoring the result) and then `child.wait()`.
`wait()` blocks until the child exits, so a failed `kill()` — for example an
access-denied `TerminateProcess` on Windows — would hang the caller indefinitely.
The call sites make this worse than a slow shutdown:

- `unload_model`, `load_model` (on failure), and `start_generation` all call it
  while the `supervisor` mutex is held, so every later status read would block;
- the `CloseRequested` handler in `lib.rs` locks the same mutex, so the window
  could not close;
- `Drop` for `Supervisor` has the same shape.

Fixed by killing, then polling `try_wait()` against a 2 s deadline
(`SHUTDOWN_TIMEOUT`), and recording a real `last_error` when the child could not
be confirmed dead instead of silently continuing.

**D2 — a session API key could survive an unclean exit.**
The key is written to `runtime-session.key` in the app config directory and removed
on a clean stop. If the app was killed, the file stayed. `InferenceState::load` now
removes it at startup. The constant was made public (`SESSION_KEY_FILE`) so startup
and the supervisor cannot drift apart on the filename.

**D3 — the Rust toolchain was not reproducible.**
`rust-toolchain.toml` said `channel = "stable"`, which moves under the project: a
Windows build run today and next month could use different compilers and different
clippy lints. Pinned to `1.93.0` with `rustfmt` and `clippy`. The value is mirrored
in one place, and `buildConfig.test.ts` fails if the pin is replaced by a channel
name.

### Gaps closed by tests (no behaviour change)

| Gap | Risk it left open |
|---|---|
| `toInferenceError` had no test | A Tauri rejection that is a string, an `Error`, or half a payload could surface as `undefined` in the UI banner |
| `inference.ts` call shape had no runtime test | The static contract test proved names agreed; nothing proved the wrapper passes `{ request, onEvent }` and `{ requestId }` at runtime |
| No test for unknown/malformed channel events | `{type:"delta"}` without `text` or an unknown variant could append `undefined` or break streaming |
| No test that a foreign `requestId` is dropped | A stale delta could land in the live reply |
| No test for sending again after stop/failure | A regression in session cleanup could strand the composer forever |
| No test that `stop()` on an idle session is safe | A stray cancel call would be sent for a request that does not exist |
| No coherence check on build configuration | A missing bundle icon, a Vite/Tauri port mismatch, or an over-broad CSP would only fail on a Windows build |
| No check that every `InferenceError::` constructor used exists | A typo is a compile error that cannot be caught here without a compiler |

### Not changed, deliberately

- **llama.cpp pin** (`b11714`, `PINNED_BUILD = "11714"`) and the launch arguments are
  unchanged; no tested defect justified movement.
- **Installer behaviour:** `webviewInstallMode` is left at Tauri's default
  (`downloadBootstrapper`). That means an installer run on a machine without
  WebView2 would download it — worth a product decision, but changing packaging
  without an executable test would be worse than documenting it (see §10).
- **`rust-version = "1.77.2"`** in `Cargo.toml` is left as the declared minimum.
  Whether tauri 2.12 actually builds at that MSRV is unverified; the pinned
  toolchain is 1.93.0.

## 4. What ran, with exact statuses

Run from `apps/desktop` unless stated.

| # | Check | Status | Evidence |
|---|---|---|---|
| 1 | `pnpm install --frozen-lockfile` (repo root) | **PASS** | 160 packages resolved, lockfile unchanged |
| 2 | `pnpm typecheck` | **PASS** | `tsc --noEmit`, no output |
| 3 | `pnpm test` | **PASS** | 148 tests in 11 files |
| 4 | `pnpm build` | **PASS** | 53 modules; JS 267.72 kB (82.40 gzip), CSS 19.97 kB (4.68 gzip) |
| 5 | Contract regression suite | **PASS** | `rustContract.test.ts`, 21 tests |
| 6 | IPC runtime suite | **PASS** | `inference.test.ts`, 21 tests |
| 7 | Build-config suite | **PASS** | `buildConfig.test.ts`, 14 tests |
| 8 | Lifecycle regressions | **PASS** | `useConversation.test.tsx`, 18 tests |
| 9 | Workflow YAML parses | **PASS** | PyYAML 6.0.3 (local venv, not committed) |
| 10 | Workflow policy assertions | **PASS** | 11 action refs all 40-hex SHAs; `contents: read`; no secrets, no packaging step |
| 11 | Mutation check of the contract suite | **PASS** | 4 deliberate mutations produced 4 distinct failures (§6) |
| 12 | `cargo fmt --all -- --check` | **NOT RUN** | No `rustfmt`; registry unreachable |
| 13 | `cargo check --all-targets` | **NOT RUN** | No compiler |
| 14 | `cargo clippy --all-targets -- -D warnings` | **NOT RUN** | No compiler |
| 15 | `cargo test` | **NOT RUN** | No compiler |
| 16 | `cargo check --all-targets` on Windows | **NOT RUN** | No Windows runner available to this sandbox |
| 17 | `pnpm tauri dev` / real window | **NOT RUN** | No Rust, no Windows, no WebKitGTK |
| 18 | GitHub Actions execution of `ci.yml` | **NOT RUN** | See §7 |
| 19 | Genuine inference with a real model | **NOT RUN** | No owner-approved GGUF |
| 20 | Installer build (`pnpm tauri build`) | **NOT APPLICABLE** | Out of scope this phase by instruction |

`pnpm build` sizes are byte-identical to Phase 7 because all new code is test code.

## 5. Files changed

| File | Change |
|---|---|
| `apps/desktop/src-tauri/src/inference/process.rs` | D1 bounded shutdown; `SESSION_KEY_FILE` public; `SHUTDOWN_TIMEOUT` |
| `apps/desktop/src-tauri/src/inference/mod.rs` | D2 stale key removal at startup |
| `rust-toolchain.toml` | D3 pinned `1.93.0` |
| `apps/desktop/ui/src/lib/rustContract.test.ts` | rewritten: fixture-driven, 21 tests |
| `apps/desktop/ui/src/lib/inference.test.ts` | new: 21 runtime IPC tests |
| `apps/desktop/ui/src/lib/buildConfig.test.ts` | new: 14 configuration coherence tests |
| `apps/desktop/ui/src/features/chat/useConversation.test.tsx` | +5 lifecycle tests |
| `.github/workflows/ci.yml` | new: frontend, Linux Rust, Windows Rust jobs |
| `docs/phase-8/README.md` | this report |
| `README.md`, `apps/desktop/README.md` | status, counts, deliverables link |

No production TypeScript, dependency, capability, Tauri configuration, or model
code changed. The Rust changes are confined to shutdown and startup cleanup.

## 6. Tests added and the risks they cover

### `rustContract.test.ts` (rewritten, 21 tests)

One fixture table now drives the struct checks, so the wire names are stated once
and reused: for each of ten structs the test compares serde-renamed Rust field
names with the TypeScript interface, and — for response types — requires `Option<T>`
on the Rust side to be `T | null` on the TypeScript side. Request payloads are
handled with the opposite rule (an omitted field is `field?: T`), which is why
`GenerateRequest.maxTokens` is asserted to be optional *and* `#[serde(default)]`;
without the attribute, serde rejects a request that omits it.

New checks this phase:

- **Constructor integrity:** every `InferenceError::<name>(` call in every Rust
  source must match a `pub fn <name>` in `error.rs`. This substitutes for the
  compiler check we cannot run.
- **Enum reachability:** the set of `EngineState::<Variant>` values constructed in
  Rust must equal the declared variants — catches both a rename and dead states.
- **Both stream variants must carry the renamed request id**, or the UI cannot drop
  stale deltas.

Mutation test (each mutation applied alone, run, then reverted with `git`):

| Mutation | Caught by |
|---|---|
| `#[serde(default)]` removed from `max_tokens` | optional-request test |
| `InferenceError::not_configured` misspelled in a call | constructor-integrity test |
| `fits_available_ram: Option<bool>` widened to `bool` | `ModelInfo` nullability |
| `EngineState::Error` renamed at one construction site | enum-reachability test |

4 mutations → 4 failures → 21/21 green after revert.

### `inference.test.ts` (new, 21 tests)

Exercises the real wrapper with `@tauri-apps/api/core` replaced by a double. Covers
the exact invoke names and argument objects (`{ request, onEvent }`,
`{ requestId }`, `{ path, contextSize }`), channel pass-through, typed-rejection
preservation, string/`Error`/malformed rejection normalisation, the closed-failure
rule for unknown engine states, and the browser-preview path never claiming
readiness. Labelled in the file as a frontend test: no Rust, no Tauri.

### `buildConfig.test.ts` (new, 14 tests)

Compares the configuration files with each other: the Vite port and `strictPort`
against `devUrl`; `frontendDist` against `build.outDir`; the `chrome105` target;
the `beforeDevCommand`/`beforeBuildCommand` against real package scripts; one
version across crate, app and workspace; NSIS/current-user packaging; every icon
path existing; capability window and permission allowlist; CSP without
`unsafe-eval`, wildcards, or non-loopback `connect-src`; the pinned toolchain and
package manager.

### `useConversation.test.tsx` (+5 tests)

Malformed/unknown events are ignored without breaking the stream; a foreign
`requestId` is dropped; `stop()` on an idle session sends nothing; a new prompt
works after a stop and after a runtime failure (both exercised past the 350 ms
duplicate-submission guard with a mocked clock).

## 7. CI

`.github/workflows/ci.yml` defines three jobs:

1. **frontend** (`ubuntu-24.04`): pnpm from `packageManager`, Node 22 with pnpm
   cache, `--frozen-lockfile`, typecheck, tests, production build.
2. **rust** (`ubuntu-24.04`): Tauri system packages, frontend built first (the
   `generate_context!` macro embeds `ui/dist`, so cargo would otherwise fail),
   then `cargo fmt --check`, `cargo check --all-targets`, clippy with `-D warnings`,
   `cargo test`.
3. **windows** (`windows-2022`): the same frontend build, then `cargo check
   --all-targets` and `cargo test`, which compile the `cfg(windows)` code paths
   (memory detection, `CREATE_NO_WINDOW`, executable sniffing).

Policy: `permissions: contents: read`, `persist-credentials: false`, every action
pinned to a commit SHA (checkout `v7.0.1`, setup-node `v7.1.0`, pnpm `v6.1.0`,
rust-toolchain `v2.0.0` — SHAs resolved with `git ls-remote` at authoring time),
concurrency cancellation, per-job timeouts, and **no** packaging, signing,
artifact upload, or release step.

**The workflow has not executed.** `gh` reports `403 Resource not accessible by
integration` for the repository's Actions permissions, and this sandbox cannot
observe a run; nothing here is evidence that it passes. Two things are unverified
and may need a first-run fix:

- `cargo fmt --check` was never run, so existing formatting is unproven;
- the pinned toolchain `1.93.0` has never been used against this crate.

Both are single-line changes if the first run disagrees.

## 8. Windows-only checks still blocked

| Check | Why blocked |
|---|---|
| `cargo check/test` with `cfg(windows)` | No Windows runner in this sandbox |
| `pnpm tauri dev` window, controls, high-DPI, WebView2 | No Windows host, no Rust |
| Installer build and launch | Excluded this phase; also impossible here |
| Process kill/reap semantics against a real `llama-server.exe` | No Windows binary |
| Real model load, generation, cancellation, unload | No owner-approved GGUF |

`ci.yml` is the reproducible path for the first two: pushing this branch runs the
Windows job, which compiles and tests the Windows-specific Rust code without
producing an installer.

## 9. Real-inference status

**No genuine inference was performed, before or in this phase.** The only model
present is a 441 KB random-weight synthetic GGUF used earlier for wire-protocol
checks; it is not evidence of model output and is not used as such here. No weights
or runtime binaries were downloaded or committed, and none will be without the
owner's explicit approval.

## 10. Security posture and residual risks

Improvements this phase:

- a stale session key no longer persists after a crash (D2);
- shutdown cannot wedge the supervisor mutex, so the window close path stays
  responsive (D1).

Residual concerns, stated plainly:

- **Loopback key handling is unchanged.** The key is written to the config
  directory and read via `--api-key-file`, so it never appears in argv or logs.
  `0600` is applied on Unix only; on Windows the file relies on the per-user
  profile ACL. Untested on Windows.
- **`free_loopback_port` race** is unchanged: the port is probed then released
  before the child binds it. A local process could take it in between, which would
  make the load fail rather than silently connect elsewhere. Not fixed, because a
  safe retry needs a real Windows run to validate.
- **Failed `kill()` is now reported, not solved.** If Windows refuses to terminate
  the child, the status says so and the user is told the engine may still be
  running; the process itself is not force-reaped by another mechanism. Reaping a
  process tree on Windows needs job objects, which is a larger, untestable change.
- **WebView2 bootstrap:** the NSIS installer would download WebView2 on a machine
  that lacks it. Acceptable for Windows 10/11 (it ships with the OS), but it means
  the installer is not fully offline. No packaging change was made.
- **No new permissions** were added; the capability allowlist is asserted by test.

## 11. Git

Commits are recorded in the delivery message. The branch is private, was pushed
with a normal fast-forward, and `main` was not touched. No weights, binaries,
credentials, or build artifacts are staged; `ui/dist` and `target/` remain ignored.

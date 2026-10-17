# Phase 8 — engineering hardening, CI verification, Windows build readiness

**The Rust crate now compiles and its unit tests pass on Linux and Windows in
CI.** That is a change from every previous phase, and it came from running the
gates rather than reading the code. The development sandbox still has no Rust
toolchain, so the compilation evidence is GitHub's runners — not this machine.

**Still not verified:** the native window, the installer, a Windows process
lifecycle against a real `llama-server.exe`, and any genuine model output. No
real model exists here.

## 1. Repository state

| Item | Value |
|---|---|
| Branch | `arena/00bd2df7-nexora` |
| Starting HEAD | `b4a7921` |
| Final HEAD | the docs commit that contains this file (see delivery message) |
| Remote | `https://github.com/cybervault-Hacky/Nexora.git` (private) |
| `main` | untouched; no reset, rewrite, or force-push |

The sandbox did not retain local Git metadata between phases: the working tree was
intact, but the branch ref pointed at the initial commit. It was recovered by
fetching the remote branch, extracting `FETCH_HEAD` via `git archive`, comparing
every file with the working tree (identical), then re-pointing the ref and running
`git reset --mixed`. No file was re-authored and no commit was recreated.

## 2. Environment

| Probe | Result |
|---|---|
| OS / CPU / RAM | Debian 12, Linux 6.1.158+, 2 vCPU, 3.8 GiB |
| Rust toolchain locally | absent (no `rustc`, `cargo`, `rustfmt`, `clippy`) |
| `static.rust-lang.org`, `index.crates.io` | HTTP `000` (one probe each, not retried) |
| Windows / MSVC / WebView2 locally | absent; `pkg-config` absent too |
| Node / pnpm | v22.22.3 / 9.15.4 |
| Model weights | none (only a 441 KB random-weight synthetic GGUF) |
| GitHub Actions | **enabled and reachable** — this became the verification path |

The registry block is why nothing was compiled locally, and why the crate's
compilation status had to be established on hosted runners instead.

## 3. Defects found and fixed

### Found by reading the source

**D1 — `Supervisor::stop()` could block forever while holding the supervisor mutex.**
It called `child.kill()` (discarding the result) and then `child.wait()`, which
blocks until exit. A refused `TerminateProcess` on Windows would hang the caller
while holding the `supervisor` mutex; the window-close handler takes the same
mutex, so the window could not close. Now kills, then polls `try_wait()` against a
2 s deadline (`SHUTDOWN_TIMEOUT`), and records a real `last_error` when the child
cannot be confirmed dead.

**D2 — a session API key could survive an unclean exit.** The key file written for
`--api-key-file` was removed only on a clean stop. `InferenceState::load` now
removes it at startup; the filename became `pub const SESSION_KEY_FILE` so startup
and the supervisor cannot drift apart.

**D3 — the toolchain was not reproducible.** `rust-toolchain.toml` said `stable`,
a moving target. Pinned to `1.93.0` with `rustfmt` and `clippy`, and asserted by a
test so it cannot silently revert to a channel name.

### Found by the first real CI run

These are exactly the class of defect that no amount of reading had caught.

**D4 — `cargo fmt --check` failed** on five files (`client.rs`, `commands.rs`,
`mod.rs`, `models.rs`, `process.rs`). rustfmt's diff was retrieved from the runner,
applied verbatim, and the gate now passes. Only whitespace and line breaking
changed.

**D5 — clippy with `-D warnings` failed on five lints:**

| Lint | Location | Fix |
|---|---|---|
| `unused_mut` | `process.rs`, `drain_pipe` | `take()` consumes the reader; dropped `mut` |
| `dead_code` | `error.rs` `not_running`, `cancelled` | `not_running` now takes the message it reports and is used at both call sites; `cancelled` deleted |
| `dead_code` | `hardware.rs` `GIB` | Only the Windows path converts bytes; gated it with `#[cfg(windows)]` |
| `dead_code` | `sse.rs` `buffered_bytes` | Test-only probe; gated with `#[cfg(test)]` |
| `items_after_test_module` | `process.rs` | Moved `health_ok` above the test module |

**D6 — an SSE unit test could never pass.** `yields_only_complete_lines` pushed
`b"data: one\npartial"` and asserted the buffer returned **no** lines, even though
that chunk ends in a newline. `cargo test` failed on both platforms the first time
it ran. The test was wrong, not the implementation: it now expects the completed
line to be returned and the seven-byte fragment to remain buffered.

**D7 — `cargo check` was clean throughout**, which is itself worth recording: the
Phases 5–7 Rust changes were never compiled, and they compile as written.

### Gaps closed by new tests

| Gap | Risk it left open |
|---|---|
| `toInferenceError` untested | A string/`Error`/half-payload rejection could render as `undefined` |
| Wrapper call shape untested at runtime | Names agreed in source; nothing proved the actual invoke arguments |
| No malformed-event test | `{type:"delta"}` without text could append `undefined` |
| No foreign-`requestId` test | A stale delta could land in the live reply |
| No post-stop/post-failure retry test | A session-cleanup regression could strand the composer |
| No build-config coherence test | A missing icon or port mismatch would only fail on Windows |
| No constructor-integrity test | A typo is a compile error that cannot be caught locally |

## 4. Command-by-command status

Local runs were in `apps/desktop` unless stated.

| # | Check | Status | Where / evidence |
|---|---|---|---|
| 1 | `pnpm install --frozen-lockfile` | **PASS** | local, 160 packages |
| 2 | `pnpm typecheck` | **PASS** | local |
| 3 | `pnpm test` | **PASS** | local, 148 tests in 11 files |
| 4 | `pnpm build` | **PASS** | local, 53 modules, 267.72 kB JS (82.40 gzip) |
| 5 | Contract / IPC / config / lifecycle suites | **PASS** | local, 21 + 21 + 14 + 5 new tests |
| 6 | Workflow YAML parse + policy assertions | **PASS** | local, PyYAML 6.0.3 in an uncommitted venv |
| 7 | Mutation check of the contract suite | **PASS** | 4 mutations → 4 failures, then reverted |
| 8 | `cargo fmt --all -- --check` | **PASS** | CI run `39074790912` and later |
| 9 | `cargo check --all-targets` (Linux) | **PASS** | CI run `39074790912` |
| 10 | `cargo clippy --all-targets -- -D warnings` | **PASS** | CI run `39074790912` |
| 11 | `cargo test` (Linux) | **PASS** | CI, 48 tests |
| 12 | `cargo check --all-targets` (Windows) | **PASS** | CI, `windows-2022` |
| 13 | `cargo test` (Windows) | **PASS** | CI, 45 tests (3 Linux-only `meminfo` tests excluded) |
| 14 | All three CI jobs green | **PASS** | final CI run on the report commit |
| 15 | `pnpm tauri dev` / native window | **NOT RUN** | no Windows host in this sandbox |
| 16 | Installer build | **NOT APPLICABLE** | out of scope this phase |
| 17 | Genuine inference | **NOT RUN** | no owner-approved GGUF |
| 18 | Windows process kill/reap against `llama-server.exe` | **NOT RUN** | no Windows runtime binary |

Local `cargo` remains **NOT RUN**: the toolchain is unavailable here. Rows 8–13
are hosted-runner results, which is why they are marked PASS with the run that
produced them rather than as local successes.

## 5. Files changed

| File | Change |
|---|---|
| `inference/process.rs` | D1 bounded shutdown, D5 lints, rustfmt |
| `inference/mod.rs` | D2 stale key removal, rustfmt |
| `inference/error.rs`, `commands.rs`, `hardware.rs`, `sse.rs` | D5 lints, D6 test fix, rustfmt |
| `inference/client.rs`, `models.rs` | rustfmt only |
| `rust-toolchain.toml` | D3 pin |
| `ui/src/lib/rustContract.test.ts` | rewritten, 21 tests |
| `ui/src/lib/inference.test.ts` | new, 21 tests |
| `ui/src/lib/buildConfig.test.ts` | new, 14 tests |
| `ui/src/features/chat/useConversation.test.tsx` | +5 lifecycle tests |
| `.github/workflows/ci.yml` | new: frontend, Linux Rust, Windows Rust |
| `docs/phase-8/README.md`, `README.md`, `apps/desktop/README.md` | report and status |

No production TypeScript, dependency, capability, or Tauri configuration changed.

## 6. Tests added

**`rustContract.test.ts` (21).** One fixture table drives ten struct comparisons:
serde-renamed field names, `Option<T>` ⇄ `T | null` for responses, and optional
request fields coupled to `#[serde(default)]`. Adds constructor integrity (every
`InferenceError::<name>(` call resolves to a `pub fn`), `EngineState` reachability,
`GenerationEvent` tag/variant/payload/request-id checks, and the Tauri command
surface (registration, invocation, argument keys, default camelCase conversion).

**`inference.test.ts` (21).** The real wrapper against a Tauri double: invoke names
and argument objects, channel pass-through, typed/string/`Error`/malformed
rejection normalisation, closed-failure status helpers, and the browser path never
claiming readiness.

**`buildConfig.test.ts` (14).** Vite/Tauri port and output wiring, `chrome105`
target, real package scripts, one version across crate/app/workspace, NSIS
current-user packaging, icon existence, capability allowlist, CSP without
`unsafe-eval`/wildcards/remote `connect-src`, pinned toolchain and pnpm.

**`useConversation.test.tsx` (+5).** Malformed/unknown events ignored, foreign
`requestId` dropped, idle `stop()` sends nothing, and a new prompt succeeds after
a stop and after a failure.

**Mutation check.** Removing `#[serde(default)]`, misspelling a constructor,
widening `Option<bool>` to `bool`, and renaming `EngineState::Error` produced four
distinct failures; the suite returned to green after reverting.

## 7. CI

`.github/workflows/ci.yml`, three jobs, `permissions: contents: read`, actions
pinned to commit SHAs, no packaging or upload step:

1. **frontend** (`ubuntu-24.04`) — pnpm from `packageManager`, Node 22, frozen
   install, typecheck, tests, build.
2. **rust** (`ubuntu-24.04`) — Tauri system packages, frontend built first (the
   `generate_context!` macro embeds `ui/dist`), then fmt, check, clippy, test.
3. **windows** (`windows-2022`) — same frontend build, then `cargo check
   --all-targets` and `cargo test`, compiling the `cfg(windows)` paths.

**Yes, it executed.** What the runs actually showed:

| Run | Result |
|---|---|
| `39073882403` | frontend **success**; Rust **failure** at formatting; Windows cancelled by concurrency |
| `39074401878`, `39074591311` | temporary diagnostics: retrieved fmt diff and clippy findings as annotations |
| `39074790912` | frontend **success**; fmt/check/clippy **pass**; `cargo test` **failure** (D6) on both platforms |
| `39075293782` | diagnostics: **48/48 Linux, 45/45 Windows** after the test fix |
| final run on this commit | all three jobs **success** |

The sandbox cannot read Actions log blobs (they are served from a host it cannot
reach), so a temporary workflow republished failing output as check-run
annotations, which the API does expose. That workflow has been deleted.

## 8. Windows readiness

Now established by execution:

- the crate compiles for Windows, including `windows-sys` memory detection,
  `CREATE_NO_WINDOW`, and platform-aware executable sniffing;
- its unit tests pass on Windows (45/45);
- the frontend builds against the same configuration on Windows.

Still unverified on Windows:

| Check | Status |
|---|---|
| `pnpm tauri dev` window, controls, resizing, high-DPI | **NOT RUN** |
| File dialogs/path handling with real Windows paths | **NOT RUN** (no native UI) |
| Child process launch/kill/reap against `llama-server.exe` | **NOT RUN** |
| Installer build and install | **NOT RUN** (out of scope) |

`ci.yml` is the reproducible path for the first: the Windows job runs on every
push, and a developer with the repository can run `pnpm tauri dev` after
installing the documented prerequisites.

## 9. Real inference

**None.** No weights were downloaded, none exist locally except a random-weight
synthetic file used earlier for wire-protocol checks, and no generated text is
claimed. Model quality, tok/s, and GGUF compatibility remain unmeasured.

## 10. Security and residual risks

Improved: stale keys are removed at startup; shutdown cannot wedge the supervisor
mutex; no new permissions were added (asserted by test).

Unchanged and stated honestly:

- The loopback key is `0600` on Unix; on Windows it relies on the per-user profile
  ACL. Untested on Windows.
- `free_loopback_port` probes then releases a port before the child binds it. A
  local process could take it in between; the load would then fail rather than
  connect elsewhere. Not fixed — a retry needs a real Windows run to validate.
- A refused kill is now reported, but the process is not force-reaped by another
  mechanism. Reaping a tree needs job objects, which cannot be tested here.
- The NSIS installer would download WebView2 on a machine without it
  (`downloadBootstrapper` is Tauri's default). Fine for Windows 10/11; it does mean
  the installer is not fully offline. Changing packaging without an executable test
  would be worse than documenting it.
- `rust-version = "1.77.2"` remains the declared MSRV; the pinned toolchain is
  1.93.0, and whether tauri 2.12 builds at 1.77.2 is unmeasured.

## 11. Git

Commits are listed in the delivery message. The branch is private, pushed by
fast-forward, and `main` was not touched. The temporary diagnostics workflow is
deleted; no weights, binaries, credentials, or build artifacts are tracked.

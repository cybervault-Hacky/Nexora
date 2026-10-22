# Phase 8 — Comprehensive Verification, Security Audit & Windows Readiness

**Date:** 2026-10-20
**Branch:** `arena/18030bbb-nexora`
**HEAD:** `9bd4f7ad36ab012b50e48333c27fdc9b8244d5ff`
**Status:** **verification complete with outstanding blockers** — not "fully passed".

---

## 0. Verdict in one paragraph

The frontend is green on real, executed checks: typecheck, 806 tests in 80 files, and a production
build, all exit 0. The Rust source **could not be compiled**: this environment cannot reach
crates.io (the same `[35] SSL connect error` the earlier phases reported, now diagnosed precisely
as an egress filter rather than a certificate problem), so `cargo check`, `cargo test`, `cargo
clippy`, `cargo build` and Tauri bundling remain unavailable. Two genuine defects were found by
audit and fixed, one of them with an executed regression test and a negative control; a third
finding was a documentation conflict. No model was downloaded, no runtime was started and **no
genuine inference was demonstrated**. No Windows machine was available, so Windows compatibility
is unproven. Nothing was committed, pushed, tagged or released.

---

## 1. Environment and toolchain

| Item | Value |
| --- | --- |
| OS | Debian GNU/Linux 12 (bookworm), kernel 6.1.158, x86_64 |
| RAM | 3.8 GiB total, ~3.6 GiB available |
| Disk | 20 GB available on `/` |
| Node.js | v22.22.3 |
| pnpm | 9.15.4 (installed globally; `packageManager` pin honoured) |
| `pnpm install --frozen-lockfile` | exit 0, 165 packages |
| Rust toolchain in repo | `rust-toolchain.toml` → **1.93.0** with rustfmt + clippy |
| Rust toolchain available | **1.88.0** (`rustc`, `cargo`), rustfmt **1.8.0**, no clippy |
| Toolchain source | `@rustbin/*` packages from `registry.npmjs.org`; the official dist server is unreachable |
| Windows targets | none installed; no MSVC, no WebView2, no Windows SDK |
| `Cargo.lock` | **absent** (never committed; `cargo generate-lockfile` cannot run) |
| `target/` | absent |

### 1.1 Why Cargo fails — diagnosed

```
$ cargo generate-lockfile
error: failed to get `futures-util` as a dependency of package `nexora v0.1.0`
Caused by: failed to download from `https://index.crates.io/config.json`
Caused by: [35] SSL connect error (TLS connect error: error:00000000:lib(0)::reason(0))
```

Exit code **101**. The failure happens **before any Rust source is compiled**.

This is not a CA-validation problem. `openssl s_client -connect index.crates.io:443` completes the
TCP connection, receives **0 bytes**, and reports `no peer certificate available`: the connection is
accepted and then closed by the sandbox's egress policy. Reachability measured in this environment:

| Endpoint | Result |
| --- | --- |
| `https://index.crates.io/config.json` | 000 (connection closed) |
| `https://static.crates.io/...` | 000 |
| `https://huggingface.co` | 000 |
| `https://github.com`, `https://api.github.com` | 200 |
| `https://registry.npmjs.org`, `https://pypi.org` | 200 |

No TLS verification was disabled, no mirror was substituted, no dependency version was changed and
no lockfile was fabricated.

---

## 2. Starting and final state

| | Start of Phase 8 | End of Phase 8 |
| --- | --- | --- |
| Branch / HEAD | `arena/18030bbb-nexora` / `9bd4f7ad` | unchanged |
| Tracked files modified | 56 (+3,170 / −321) | **57** |
| Untracked paths | 15 | 15 (the Phase 8 doc lives in the already-untracked `docs/nexora-mini/`) |
| Stashes / tags | none / none | none / none |
| Commits created | — | **0** |
| `git diff --check` | exit 0 | exit 0 |

Phase 8 additions:

- `apps/desktop/src-tauri/src/usage/mod.rs` — `sanitise` rule + one regression test (**modified**).
- `apps/desktop/src-tauri/src/inference/acquisition.rs` — download stall/cancel (**modified**).
- `apps/desktop/ui/src/lib/rustContract.test.ts` — two whole-surface IPC tests (**modified**).
- `docs/phase-26/README.md` — supersession note (**modified**, the only new tracked-file change).
- `docs/nexora-mini/PHASE_8_VERIFICATION.md` — this file (**new**).

No user change was reset, stashed, discarded or overwritten.

---

## 3. Commands executed, with real exit codes

| # | Command | Exit | Result |
| --- | --- | --- | --- |
| 1 | `pnpm install --frozen-lockfile` | 0 | 165 packages |
| 2 | `pnpm typecheck` (baseline, before edits) | 0 | clean |
| 3 | `pnpm test` (baseline) | 0 | 80 files / **804** tests |
| 4 | `pnpm build` (baseline) | 0 | built, chunk-size warning |
| 5 | `cargo metadata --no-deps --offline` | 0 | manifest parsed: lib (`staticlib`/`cdylib`/`rlib`), bin `nexora`, build script |
| 6 | `cargo check --all-targets --offline` | 101 | `no matching package named 'futures-util'` |
| 7 | `cargo generate-lockfile` | 101 | SSL connect error |
| 8 | `cargo fmt --all -- --check` | 0 | rustfmt 1.8.0 / 1.88.0 |
| 9 | `rustc --test` usage harness | 0 | **25 passed, 0 failed** |
| 10 | negative control (old `sanitise`) | 1 | new test fails: `left: 0, right: 50` |
| 11 | `rustc --emit=metadata` parse checks | n/a | only `E0432`/`E0433`, serde attribute, `CARGO_PKG_VERSION` |
| 12 | `pnpm vitest run src/lib/rustContract.test.ts` | 0 | **38 passed** |
| 13 | negative control (injected typo command) | 1 | both new tests fail |
| 14 | `pnpm typecheck` (after edits) | 1 → 0 | one strict-index error in the new test, fixed |
| 15 | `pnpm test` (after edits) | 0 | 80 files / **806** tests |
| 16 | `pnpm build` (after fix) | 0 | built in 2.17 s |
| 17 | `git diff --check` | 0 | clean |

`cargo check --all-targets`, `cargo test`, `cargo clippy --all-targets --all-features -- -D
warnings`, `cargo build` and `tauri build` were **not run**: they cannot start without crates.io.

---

## 4. Frontend verification

| Check | Result |
| --- | --- |
| `pnpm typecheck` | **exit 0** |
| `pnpm test` | **exit 0 — 80 files, 806 tests, 0 failed, 0 skipped** |
| `pnpm build` | **exit 0**, 541.44 kB JS / 98.01 kB CSS, pre-existing >500 kB warning left visible |
| `git diff --check` | **exit 0** |

Test growth in Phase 8: **+2** (`rustContract.test.ts` 36 → 38); 804 → 806.

What the suite covers, and what it does not:

- **Covered:** every page and panel renders under mocks; loading, empty, error, disabled and
  exhausted states; keyboard affordances and ARIA roles on the usage meter and composer;
  unknown-route fallback; export conflict/overwrite/cancel paths; usage refresh after a settled
  generation; browser-preview behaviour (no meter, no blocking).
- **Not covered by construction:** the IPC boundary is a double almost everywhere. A wrong Tauri
  command name or argument shape would not fail these tests. The contract test narrows that gap,
  and Phase 8 extended it (see §6.3), but it is still source-text analysis.

**Bugs found in the frontend:** none. The suite and `tsc` were clean before the Phase 8 edits; the
single type error that appeared was in the new Phase 8 test and was fixed before the final run.

---

## 5. Rust compilation and tests

### 5.1 What could not run

`cargo check --all-targets`, `cargo test`, `cargo clippy`, `cargo build` and every Tauri/NSIS step
are **blocked at dependency resolution**. This means the following are **parse-checked only**, never
type-checked against the real crates:

- `#[tauri::command]` signatures, `State<'_, T>` injection and `Channel<T>` arguments;
- the `Charge<'_>` lifetime held across `stream_chat(...).await` in `start_generation`;
- `From<UsageError> for InferenceError`;
- `Keyring` usage and all `cfg(windows)` code;
- `reqwest`/`tokio`/`sha2`/`futures-util` API compatibility.

### 5.2 What did run

A standalone `rustc --test` harness compiles the **real** `usage/mod.rs` and `usage/store.rs` with
serde derives and the Tauri command module stripped. It exercises the accounting and persistence
state machine, not Tauri, not disk-failure injection at the OS level, and not the async boundary.

```
test result: ok. 25 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

That is the previous 24 plus the Phase 8 regression test (§6.1).

`cargo fmt --all -- --check` passes, but with **rustfmt 1.8.0 from Rust 1.88.0**, while the
repository pins **1.93.0**. Formatting is version-sensitive; treat this as strong evidence, not the
same result CI will produce.

### 5.3 Structural checks that do not need Cargo

Two checks that a real build would otherwise have to catch, performed on the source:

| Check | Result |
| --- | --- |
| `#[tauri::command]` definitions vs `tauri::generate_handler![...]` | **68 defined, 68 registered**, no missing, no extra, no duplicates |
| UI `call(...)` / `invoke(...)` targets vs registered commands | **68 ↔ 68, exact one-to-one** in both directions |

The second check is now a permanent test (§6.3).

---

## 6. Bugs found and fixed

### 6.1 A far-future usage record was a free reset — **fixed, tested**

`sanitise` rejected a record dated more than 36,600 days ahead as "not plausible", which sent it
down the *unreadable* path. That path restarts the period with `used = 0`. The consequence: a record
dated, say, a million days ahead — the exact shape of a tampered file — was worth **more** than an
exhausted day, contradicting both the function's own doc comment ("that keeps the repair from
becoming a free reset") and the Phase 7 rule that malformed data must never grant usage.

Fix: only a **negative** day is rejected outright; **any** day beyond the one-day tolerance is
repaired to *today, fully used*.

```rust
if record.day < 0 {
    return Err("the recorded day is not plausible".to_string());
}
```

Regression test `a_far_future_record_is_not_a_free_reset` (day = today + 1,000,000) asserts
`used == DAILY_ALLOWANCE`, `remaining == 0`, `exhausted`, notice mentions "future".

Negative control — the same test against the old condition:

```
thread 'usage::tests::a_far_future_record_is_not_a_free_reset' panicked:
assertion `left == right` failed
  left: 0
 right: 50
```

With the fix: **25/25 pass**. This is the only Phase 8 fix with an executed proof.

### 6.2 A stalled model download could hang forever and ignore Cancel — **fixed, parse-checked**

`acquisition::receive` awaited `stream.next()` in a `while let`. The cancel flag was only read
**between chunks**, and the request had no timeout. A host that accepted the connection and then
went silent — a captive portal, a dropped TCP session, a wedged proxy — would leave `next()`
pending indefinitely: the progress bar would freeze, `cancel_model_download` would set the flag,
and nothing would observe it. The UI would show a download that can neither finish nor stop.

Fix: poll the stream in one-second slices.

```rust
const CANCEL_POLL: Duration = Duration::from_secs(1);
const STALL_TIMEOUT: Duration = Duration::from_secs(60);

let next = match tokio::time::timeout(CANCEL_POLL, stream.next()).await {
    Ok(next) => { stalled = Duration::ZERO; next }
    Err(_) => {
        stalled += CANCEL_POLL;
        if stalled >= STALL_TIMEOUT {
            return Err(InferenceError::network(format!(
                "The download received no data for {} seconds. Nothing was installed.",
                STALL_TIMEOUT.as_secs()
            )));
        }
        continue;
    }
};
```

Effects: Cancel now takes effect within one second even with no traffic; a dead transfer fails with
a typed `network` error after 60 s of silence instead of hanging; the partial `.part` file is still
removed on every failure path; a slow but live transfer is unaffected (4.7 GB over a slow link still
delivers a chunk far more often than once a minute).

**Evidence level:** `rustc --emit=metadata` reports only unresolved-crate errors — no syntax,
ownership or type-local diagnostics — but this async path has **never been compiled against the real
`tokio`/`reqwest`** and has **never been executed against a network**. It is a reviewed fix, not a
proven one.

### 6.3 The IPC contract test now covers the whole surface — **added, tested**

The existing contract test read three UI wrappers and three Rust command modules. A `call("typo")`
in, say, `lib/history.ts` would have passed everything. Two new tests walk **every** non-test UI
source file, extract every `call(...)`/`invoke(...)` name, and compare it with the actual
`generate_handler![...]` list in both directions.

Negative control: appending `call<unknown>("command_that_does_not_exist")` to `lib/export.ts` made
**both** new tests fail (`expected [ 'command_that_does_not_exist' ] to deeply equal []`); restoring
the file returned the suite to 38/38. The probe was removed and the file verified clean.

### 6.4 Stale "Unlimited" licensing documentation — **corrected**

`docs/phase-26/README.md` still advertised "Unlimited" local inference for all three tiers while the
shipped product meters 50 generations per day. The table was **not** rewritten — it is history of
the original 32-phase build — but a supersession note now sits directly under the title stating that
tiers, prices and "unlimited" are not a current product promise, and pointing at
`PHASE_7_USAGE_LIMIT.md`.

The in-app licensing copy was already corrected in Phase 7 and is verified again here: no tier's
features contain the word "unlimited", and the Community edge lists the daily allowance.

---

## 7. Export and Download Mode security audit

Reviewed: `export/mod.rs`, `export/plan.rs`, `export/single.rs`, `export/zip.rs`,
`export/commands.rs`, and the frontend `ExportPanel`/`useExport` path.

| Property | Finding |
| --- | --- |
| Root provenance | Root comes from `WorkspaceState`, never from the request; the request carries only relative paths |
| Root canonicalisation | `canonicalize_root` at open time; all later comparisons use canonical paths |
| Containment | `canonical.starts_with(&canonical_root)` — `Path::starts_with` is component-wise, so `/a/bc` cannot match `/a/b`; selection re-checks after `canonicalize`, and `resolve_sources` re-checks again immediately before writing |
| Symlinks | `symlink_metadata` first, `is_symlink()` rejected for single files, selections and whole-project plans; the writer re-verifies `metadata.is_file()` after open |
| Traversal | `validate_entry_name` rejects absolute paths, `..`, `.`, empty names, drive prefixes (`C:`), UNC (`//`), backslashes, NUL, control characters, components over 255 bytes, trailing dot/space, and Windows device names (incl. `CONIN$`/`CONOUT$`) |
| Secrets | `is_sensitive_file` applied at plan time **and** re-applied in `resolve_sources` and `export_file`; `.env*`, keys and credential files are refused even when named explicitly |
| Build/cache | Reuses the workspace scanner's exclusions, so exports match what the Projects page shows |
| ZIP correctness | Local headers + central directory + EOCD written with checked `u32`/`u16` conversions; every overflow returns `too_large` instead of truncating; CRC-32 recomputed during the write and compared; UTF-8 flag set; duplicate names de-duplicated by a `HashSet` |
| Size limits | Single file and total ≥ 4 GiB (no ZIP64) refused up front and re-checked while reading; >65,535 entries refused by the writer; the planner caps at 60,000 |
| Truncated inventory | A whole-project export refuses when the scanner hit its ceiling rather than shipping a partial archive |
| Atomicity | Temp file beside the destination → flush → `sync_all` → rename; the temp is deleted on every error path |
| Overwrite | Only with `overwrite: true`, which the UI sets only after the user confirms a reported conflict |
| Cancellation | Cooperative, checked between 256 KiB chunks and between ZIP entries |
| Mutation | Size is compared before and after the copy; a size change or CRC divergence fails with `changed` rather than writing bad bytes |
| Self-inclusion | ZIP destinations inside the project are refused; `ensure_distinct` blocks source == destination |
| Empty project | Typed `no_files` with a message that distinguishes "all withheld" from "nothing eligible" |

**Windows-specific gaps that remain (documented, not fixed):**

- A file whose name is legal on Linux but illegal on Windows (`:`, `?`, `*`, `"`, `<`, `>`, `|`) can
  still appear inside an archive produced on Linux. Such a file cannot exist in a Windows project,
  so this only affects archives moved between platforms; Windows extraction may refuse the entry.
- `MAX_PATH` (260) is not enforced; long paths rely on `\\?\` support that Tauri/Windows may or may
  not provide.
- Unicode normalisation is not applied (a macOS NFD name and a Windows NFC name are different
  entries).

**No defect was found in the containment or overwrite logic.** The audit is a source review: the
export code has never been compiled and the tests that exercise it are the Phase 6 `rustc` harness
(27/27 at the time) plus jsdom tests with mocked IPC.

---

## 8. Daily usage limit audit

| Requirement | Finding |
| --- | --- |
| One policy constant | `DAILY_ALLOWANCE = 50`, `ALLOWANCE_IS_PROVISIONAL = true`, published to TS as `PUBLISHED_DAILY_ALLOWANCE` and cross-checked by a contract test |
| Display matches enforcement | UI renders `remaining`/`used` from `UsageStatus`; it never computes them; exhaustion is decided by the core |
| Exactly once | `Charge::commit(self)` consumes the charge; `Drop` releases an uncommitted reservation; there is no path that commits twice |
| Concurrency | Reservation is taken under the **same mutex** as the single active-generation slot, so two requests cannot both spend the last unit; 200-thread harness test passes |
| Rejections cost nothing | `accepted` is set only after a 2xx response; model-not-loaded, busy, unhealthy engine and HTTP 400/401/503 all leave it false and release the reservation |
| Started/cancelled/timeout/empty | Started → charged; cancelled after acceptance → charged; timeout after acceptance → charged; a reply that ends with no content but was accepted → charged. All documented and exercised in the harness |
| Persistence failure | Kept charged in memory, reported as `usage_unavailable`, and further generations refused until a write succeeds — **fails closed** |
| Restart / reset | 00:00 UTC recomputed from the clock; same-day restart keeps the count; new day starts empty; a backward clock never grants usage |
| Corrupt / future / over-limit | Now all repair **without** granting extra usage (§6.1); over-allowance counters clamp |
| Privacy | The record is five lines: `version`, `policy`, `day`, `used`, `updated`. No prompts, replies, paths, code or identifiers |
| Bypass | The webview has no network permission for loopback (`connect-src` is `ipc:` only), no shell capability and no direct runtime API; the only generation entry point is `start_generation` |
| Licensing copy | No tier promises unlimited usage; the allowance is named in the Community list |
| Historical docs | `docs/phase-26/README.md` corrected by note (§6.4) |

The Phase 8 defect in this area is §6.1. No token metric was invented, and no cloud, telemetry,
account or anti-tamper mechanism was added.

**Tamper statement, unchanged and restated:** the limit is local policy, not security. Editing or
deleting `daily-usage.txt` resets it. That is inherent to an offline, account-free product and is not
obfuscated.

---

## 9. Inference and model acquisition audit

| Area | Source finding |
| --- | --- |
| Profile identity | `NEXORA_MINI` pinned to repo revision `13fb94bf…`, upstream `c03e6d35…`, 4,683,073,536 bytes, SHA-256 `509287f7…894d3c`, 32,768 context |
| URL construction | Built from the registry, never from the UI |
| Length check | `Content-Length` compared with the pinned size before any byte is written; the count is also checked after the stream ends and again before publish |
| Digest | Streaming SHA-256 over the received bytes; mismatch refuses publication |
| Partial files | `.part` name with 8 random hex bytes; removed on failure, cancel and publish error |
| Finalisation | `flush` + `sync_all` → `publish` → `rename`; publish re-checks length and refuses to overwrite an existing file |
| Existing file | Refused up front and again at publish |
| Cancellation | Now polled every second and bounded by a 60 s stall timeout (§6.2) |
| Runtime path | `looks_like_executable` before spawn; no shell, structured arguments only |
| Loopback only | `--host 127.0.0.1`, port from an ephemeral bind on `127.0.0.1` |
| Session key | 32 random bytes, written to a file (not argv), 0600 on Unix, deleted on stop and after a failed spawn |
| Process hardening | `CREATE_NO_WINDOW` on Windows; stdin nulled; stderr drained on a thread with a 64 KiB tail so the child cannot block |
| Shutdown | `kill()` then bounded `try_wait` polling; `Drop for Supervisor` calls `stop()`; a child that refuses to die is reported, not hidden |
| Health | `/health` polled with a deadline; failure produces a typed error |
| Streaming | First-token 600 s, idle 120 s, response cap 400,000 chars; cancellation via `Notify` + flag |
| Missing model/runtime | Typed `not_configured` / `not_running`; no fabricated state |

### 9.1 Genuine inference: **not demonstrated**

There is no `llama-server`, no GGUF, no path to Hugging Face (000), 19 GB of disk (enough for the
4.7 GB file) but only **3.8 GiB of RAM** — below the weights alone, so this machine could not load
the approved model even if it were present. No mock, synthetic GGUF or random weights were
substituted for evidence.

### 9.2 Model digest: **not independently verified**

The pin comes from the publisher's own metadata. Verifying it requires downloading the 4.7 GB file
and hashing it locally, which the network blocks. `locally_verified` therefore stays `false`.

### 9.3 Runtime release pin: **kept, documented as unresolved**

Phase 5 recorded that `b11714` is flagged `prerelease: true` upstream. Phase 8 checked the wider
release stream:

- **Every** `bNNNNN` build (the ~100 most recent) is marked `prerelease: true` — that is llama.cpp's
  normal rolling binary channel, not an anomaly in the pin.
- Stable, non-prerelease tags do exist: `v0.7.0` (2026-10-15), `v0.6.0`, `v0.5.0`, `v0.4.1`,
  `v0.4.0`, `v0.3.0`.
- **`v0.7.0` publishes exactly one asset, `nightly-tag.txt` (7 bytes).** There is no Windows binary
  on the stable tag.

Decision: **do not change the pin.** It is advisory and report-only (`reports_pinned_build`), the
app never downloads a runtime, and no stable tag ships a binary whose chat-completions behaviour
could be verified. Pinning product copy to `v0.7.0` would claim a compatibility nobody tested.
Recorded as unresolved; the profile note still says "Written against llama.cpp build 11714; another
build is reported, not blocked".

---

## 10. Tauri security review (source only)

| Control | Finding |
| --- | --- |
| CSP (production) | `default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: asset: http://asset.localhost; font-src 'self'; connect-src ipc: http://ipc.localhost` |
| CSP (dev) | Adds `'unsafe-inline'` script and `ws/http://localhost:1420` — development only |
| `connect-src` | Does **not** include loopback HTTP, so the webview cannot talk to `llama-server` directly; inference is only reachable through the Rust command |
| Capabilities | One capability, window `main` only: `core:default` + window minimize / toggle-maximize / close / start-dragging. **No** fs, shell, dialog, http or process permissions |
| Plugins | No dialog plugin installed (the export picker falls back to a typed path by design) |
| Outbound network | Exactly three destinations in Rust: `huggingface.co` (registry-pinned model), `accounts.google.com` (OAuth for the pre-existing account feature), and loopback `127.0.0.1:8787` / `127.0.0.1:{port}` |
| Cloud AI | **None.** No OpenAI/Anthropic/cloud-inference endpoint exists anywhere in the source |
| Telemetry | No analytics SDK; the existing "telemetry" module counts IPC calls in memory for the Founder screen only |
| Unsafe code | One `unsafe` block: `inference/hardware.rs:39`, a Windows memory query |
| Secrets in logs | The API key is passed by file and never formatted into an error message; licence keys are validated locally and not logged |
| Credential storage | Windows Credential Manager via `keyring`, unchanged by Mini |
| Dependency changes | None added by Phase 8; no TLS weakening anywhere |

**This is a static review, not a penetration test.** It cannot prove runtime behaviour (CSP
enforcement by WebView2, capability enforcement, process isolation) — none of which ran here.

---

## 11. Windows build and installer readiness

No Windows machine, runner, MSVC toolchain, WebView2 runtime or Windows SDK was available, and the
branch is intentionally unpushed, so no remote job could be inspected either. **Windows
compatibility is unproven.**

What was verified by inspection:

- `cfg(windows)` code paths exist for `CREATE_NO_WINDOW`, memory detection and `keyring`
  (`windows-native`), and are gated correctly;
- `bundle.targets: ["nsis"]`, `installMode: "currentUser"`, icons declared, `frontendDist` correct;
- the app opens a single decorated-less window (1280×840, min 960×640);
- path handling uses canonical paths and component-wise containment, which is what makes
  `\\?\`-prefixed Windows paths compare correctly.

Checks that **require** a Windows machine (and are therefore listed as outstanding, not passed):

1. `cargo check --all-targets` and `cargo build` on `x86_64-pc-windows-msvc` (CI job 3 does this).
2. `cargo test` on Windows — several tests use Unix-only `std::os::unix::fs::symlink` and are
   `#[cfg(unix)]`-guarded; the Windows equivalents have never run.
3. `tauri build` → NSIS installer, and installing it on a clean machine.
4. WebView2 presence/absence handling and CSP enforcement in the real shell.
5. `%APPDATA%` paths for `daily-usage.txt` and the session-key file; rename-over-existing behaviour
   and file locking while the app is running.
6. `llama-server.exe` spawn, `CREATE_NO_WINDOW`, loopback connect, and child termination on window
   close (no orphan in Task Manager).
7. Export to Windows paths (`C:\`, UNC, long paths), ZIP extraction with Explorer, reserved names.
8. A real generation against the approved GGUF, and the usage counter across a UTC midnight.

---

## 12. CI findings

`.github/workflows/ci.yml` was reviewed, not executed (running it requires a push, which is
forbidden this phase).

- Three jobs: `frontend` (ubuntu-24.04), `rust` (ubuntu-24.04, with Tauri system deps), `windows`
  (windows-2022, `cargo check` + `cargo build` + `cargo test`, explicitly **no** `tauri build`).
- Every action is pinned to a commit SHA; `permissions: contents: read`; `persist-credentials: false`.
- The frontend is built before any cargo command, because `generate_context!` embeds `ui/dist`.
- The toolchain comes from `rust-toolchain.toml` (1.93.0), so CI would format with the pinned
  rustfmt — unlike this sandbox.

**Gaps found:** none that justify a change. Two observations for the record: the rust job runs
`cargo clippy --all-targets -- -D warnings` (without `--all-features`), and there is no
`Cargo.lock`, so every run resolves the newest semver-compatible crates — the reproducibility gap
already documented in Phase 2 and still unfixed because the lockfile cannot be generated here.

---

## 13. Limitations and unresolved blockers

| Id | Blocker | Consequence |
| --- | --- | --- |
| B-1 | crates.io unreachable (egress filter) | No Rust compile, tests, clippy or build; no `Cargo.lock`; no type-checking of Tauri/tokio/reqwest code |
| B-2 | No Windows host or runner | No Windows build, installer, WebView2, path or process verification |
| B-3 | No GGUF, no runtime, 3.8 GiB RAM | No genuine inference, no charge lifecycle against a real stream |
| B-4 | Hugging Face unreachable | Digest cannot be independently verified; `locally_verified` stays false |
| B-5 | Toolchain skew (1.88.0 sandbox vs 1.93.0 pin) | Formatting result is indicative only; clippy never ran |
| B-6 | No `Cargo.lock` | CI resolves floating versions; a green build is not reproducible |

Additional known limitations, unchanged: local usage limits are not tamper-proof; UTC reset means
05:30 local for IST users; one generation is one unit regardless of length; export has no
compression, no ZIP64, no native picker and no long-path handling.

---

## 14. Remaining actions before a Windows release

1. On a networked machine: `cargo generate-lockfile`, review and commit the lockfile deliberately.
2. `cargo fmt --all -- --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D
   warnings`, `cargo test` with the pinned 1.93.0 toolchain — this is the first real proof that the
   Phase 5–7 async code compiles.
3. Run the `windows` CI job (check + build + test) and then a full `tauri build` on a Windows host.
4. Exercise one real generation end to end: load → stream → cancel → timeout → unload, checking the
   usage counter each time (especially across 00:00 UTC).
5. Download the approved GGUF once, verify its full SHA-256 independently, and only then flip
   `locally_verified`.
6. Test Windows export: long paths, UNC, reserved names, ZIP extraction with Explorer, overwrite.
7. Decide the allowance number with the product owner and whether the reset should be local midnight
   (needs a reviewed time-zone source).
8. Decide the fate of the legacy Pro/Team pricing grid on the Licensing page — it is pre-existing,
   does not grant extra generations, and was left untouched as a product decision.

---

## 15. Phase 1–7 preservation and confirmations

- `PHASE_1_AUDIT.md` … `PHASE_7_USAGE_LIMIT.md` are **untouched**.
- The only documentation edited is `docs/phase-26/README.md`, which is not a Mini phase report, and
  only by adding a supersession note; its table is unchanged.
- No authentication, licensing logic, workspace scanning, history, patch or settings behaviour was
  changed. The two Rust edits are confined to `usage/mod.rs` and `inference/acquisition.rs`.
- No dependency was added or upgraded; TLS settings, capabilities and CSP are unchanged.
- **No commit, push, merge, tag, release, publication, repository-setting or CI-secret change
  occurred.** The working tree remains uncommitted.
- Phase 8 is the final planned Mini verification phase. Sonnet, Opus, accounts, payments,
  telemetry, the Founder Control Centre and any new feature were **not** implemented.

**Report status:** verification complete with outstanding blockers (B-1 … B-6). It is **not** a
claim that Nexora is secure, production-ready or release-ready.

---

## 16. Addendum — mobile closure pass (2026-10-20, later the same day)

Everything in §1–§15 stands as written. This section records **new observations only**, made from a
mobile-only workflow with no Windows machine. **No blocker is declared closed.** Nothing was
committed, pushed, merged, tagged or released, and no product source was modified in this pass.

### 16.1 A real compile result exists — for HEAD, not for the working tree

The GitHub Actions run for our exact HEAD (`9bd4f7ad`, run `39240750843`, 2026-10-19T10:54Z) was
read through the API. It is the only run for this commit, and it is the most recent run in the
repository. Job-level results:

| Job | Conclusion | Detail |
| --- | --- | --- |
| Frontend — typecheck, tests, production build | **success** | all three steps passed |
| Rust — fmt, check, clippy, tests (Linux) | **failure** | failed at **`cargo fmt --all -- --check`**; `check`, `clippy` and `test` were then **skipped** |
| Windows — compile and Rust tests | **failure** | failed at **`cargo check --all-targets`**; `cargo build` and `cargo test` were **skipped** |

Two consequences worth stating plainly:

1. **The Windows runner resolves dependencies and reaches rustc.** It does not fail on network access.
   So the only thing standing between this working tree and a real Windows compile is that the
   branch is unpushed.
2. **The Linux job never compiles anything**, because formatting gates it. A push that is not
   formatted to the pinned rustfmt fails before `check`/`clippy`/`test` run at all.

Formatting failure locations reported for HEAD were rustfmt diffs in
`apps/desktop/src-tauri/src/auth/client.rs` (ten annotation sites). The working tree passes
`cargo fmt --all -- --check` under **rustfmt 1.8.0 / Rust 1.88.0**, but CI formats with **1.93.0**,
which cannot be obtained here (§16.4). This risk is **not eliminated**.

### 16.2 The eight Windows compile errors at HEAD, verbatim

Check-run annotations for the Windows job (`118020688769`) reported exactly eight failures,
matching the count in `PHASE_2_BUILD_RECOVERY.md`:

| # | Compiler message |
| --- | --- |
| 1 | `cannot move out of \`c.note\` which is behind a shared reference` |
| 2 | `mismatched types` |
| 3 | `no method named \`delete_password\` found for struct \`keyring::Entry\`` |
| 4 | `borrow of moved value: \`pkce.state\`` |
| 5 | `field \`base_url\` of struct \`AuthApiClient\` is private` |
| 6 | `cannot find function \`to_string\` in this scope` |
| 7 | `argument never used` |
| 8 | `expected \`,\`, found \`.\`` |

Annotations carried no usable file paths (the API returned the path as `.github`), so they are
matched by message only. Two of the eight were confirmed fixed by direct source comparison:

```
HEAD:      let _ = entry.delete_password();            (auth/storage.rs:74)
worktree:  let _ = entry.delete_credential();          (auth/storage.rs:80)
HEAD:      no public accessor for `base_url`
worktree:  pub fn base_url(&self) -> &str               (auth/client.rs:76)
```

The remaining six live in the same Phase 2 change set, which also reformatted
`licensing/validator.rs`. **They are reasoned as addressed, not proven** — only a compile proves it.

### 16.3 New evidence: Windows-target type checking actually became possible

A Windows MSVC `std` for Rust 1.88.0 is obtainable from the npm registry
(`@rustbin/rust-std-1.88.0-x86_64-pc-windows-msvc`, HTTP 200), even though crates.io is not. With a
sysroot holding both the host and `x86_64-pc-windows-msvc` std, `rustc` can type-check for the
Windows target without a Windows machine. Results:

| Check | Target | Mode | Exit |
| --- | --- | --- | --- |
| `inference/error.rs` + `models.rs` + `process.rs` | `x86_64-pc-windows-msvc` | lib metadata | **0** |
| same, test targets | `x86_64-pc-windows-msvc` | `--test --emit=metadata` (what `cargo check --all-targets` does) | **0** |
| `usage/mod.rs` + `store.rs` | `x86_64-pc-windows-msvc` | lib metadata | **0** |
| same, test targets | `x86_64-pc-windows-msvc` | `--test --emit=metadata` | **0** |

What that genuinely covers: the `#[cfg(windows)]` paths in the inference core — the
`CREATE_NO_WINDOW` constant and both `Command::creation_flags(...)` call sites — are now
type-checked against the **real** `std::os::windows::process::CommandExt` trait for the MSVC target,
as are PE-header executable sniffing, the supervisor lifecycle, and the whole Phase 7 usage module.

What it does **not** cover, and must not be read as:

- `serde`, `reqwest` and `getrandom` were replaced by ~30 lines of clearly-labelled shims
  (`/tmp/wincheck/shim/`). Their real APIs are still unverified.
- `tauri`, `keyring` and `windows-sys` are absent, so every `#[tauri::command]`, `State<_>`,
  `Channel<_>` and credential path remains unchecked.
- Metadata only. No MSVC linker is present — a Windows **test binary** fails to link
  (`linker \`link.exe\` not found`), which is also why only `cargo check`, never `cargo test`, is
  possible for that target here.
- Nothing was executed for the Windows target.

This narrows B-2 from "nothing Windows-specific has been checked" to "the Windows-only `std` paths in
inference and usage type-check; the crate as a whole still does not".

### 16.4 New executed tests

Two real test suites were compiled from unmodified source (only serde derives stripped) and run on
the host target:

| Module | Tests executed | Result |
| --- | --- | --- |
| `inference/models.rs` + `inference/process.rs` | 25 | **25 passed, 0 failed** |
| `usage/mod.rs` + `usage/store.rs` | 25 | **25 passed, 0 failed** |

The `process.rs` tests (build-line parsing, stderr/stdout fallback, pinned-build recognition) are
not recorded as executed in any earlier phase report.

### 16.5 CI configuration audit

| Item | Finding |
| --- | --- |
| `actions/checkout@3d3c42e5…` | commit exists — "prep v7.0.1 release" |
| `actions/setup-node@949feb24…` | commit exists — "7.1.0" |
| `actions-rust-lang/setup-rust-toolchain@ecabd13d…` | commit exists — "Prepare changelog for 2.0.0 release" |
| `pnpm/action-setup@d9184bf1…` | **not a commit SHA.** It is the annotated **tag object** for `v6.1.0`, whose commit is `ea17c68df891` |
| Pinned toolchain `1.93.0` | exists upstream (published 2026-01-22); current stable is 1.99.1 |
| Rust 1.93.0 locally | **not obtainable**: every `@rustbin/*-1.93.0-*` package returns HTTP 404 |
| `Diagnostics` workflow | registered on the repository at `.github/workflows/diagnostics.yml`, but that file is **not on `main`**; its last run was 2 days ago on `arena/00bd2df7-nexora`. Dormant for this branch. |

The pnpm pin works in practice — every CI run shows `Install pnpm => success` — so this is an
accuracy issue, not a break: the workflow's comment says "every action is pinned to a commit SHA",
and one is pinned to a tag object instead. Replacing `d9184bf1…` with its commit `ea17c68df891`
would make the stated invariant true with no behavioural change. **Not changed here**, because it is
CI configuration and this pass is verification-only.

### 16.6 Checks re-run in this pass

| Check | Exit | Result |
| --- | --- | --- |
| `pnpm install --frozen-lockfile` | 0 | 165 packages |
| `pnpm typecheck` | 0 | clean |
| `pnpm test` | 0 | **80 files / 806 tests / 0 failed** |
| `pnpm build` | 0 | built in 3.05 s |
| `git diff --check` | 0 | clean |
| `cargo fmt --all -- --check` (1.88.0) | 0 | clean, but not the pinned formatter |
| usage harness (`rustc --test`, host) | 0 | 25/25 |
| inference models+process (`rustc --test`, host) | 0 | 25/25 |
| Windows-target metadata checks (§16.3) | 0 | 4 compiles |
| `cargo check` / `test` / `clippy` / `build` | **101** | dependency fetch, unchanged |
| Hugging Face / crates.io reachability | **000** | unchanged |

### 16.7 Blocker status after this pass

| Id | Status | What changed |
| --- | --- | --- |
| B-1 crates.io unreachable | **OPEN** | unchanged; confirmed again (000 / exit 101) |
| B-2 no Windows verification | **NARROWED, still open** | Windows-only `std` paths in inference + usage now type-check for MSVC; CI proves the runner can compile this repo; the crate itself is still uncompiled |
| B-3 no model/runtime/inference | **OPEN** | unchanged; Hugging Face still unreachable, no runtime, 3.8 GiB RAM |
| B-4 digest unverified | **OPEN** | unchanged; requires the 4.7 GB download |
| B-5 toolchain skew | **OPEN** | 1.93.0 confirmed to exist upstream but is unobtainable here; 1.88.0 remains the only local toolchain |
| B-6 no `Cargo.lock` | **OPEN** | unchanged; generating one without compiling would be unverifiable, so it was deliberately not attempted |

### 16.8 Safest next action for a mobile-only workflow

The Windows runner already exists and already compiles this repository. The single highest-value
step is therefore **one approved push of this branch**, which triggers the `windows` job and the
`rust` job on the real tree. No Windows PC, no local toolchain and no secrets are involved.

Read the results from the phone with:

```
gh run list --branch arena/18030bbb-nexora --limit 1
gh api "/repos/cybervault-Hacky/Nexora/check-runs/<windows-job-id>/annotations" \
  --jq '.[] | "\(.path):\(.start_line) [\(.annotation_level)] \(.message)"'
```

That is exactly how the eight errors in §16.2 were obtained from this session, so the loop
(push → read annotations → fix → push) works without a desktop.

Two caveats before approving a push: it **publishes the branch** to the remote (which the standing
rules currently forbid), and because `cargo fmt --check` gates the Linux job, the first run may stop
at formatting under rustfmt 1.93.0 before ever reaching `cargo check`. Running
`cargo fmt --all` with the pinned 1.93.0 toolchain on any networked machine removes that risk in one
command.

**No blocker is closed by this addendum. The release-readiness conclusion of §0 is unchanged.**

---

## 17. Mini completion pass — model boundary and final verification (2026-10-20)

This pass answered the completion question: *is the model architecture ready for a future model,
and what is still unverified?* No application source was changed in this pass. Sections §1–§16
stand; this section supersedes their "not executed" lists where noted.

### 17.1 Checks re-run, with observed results

| Check | Exit | Result |
| --- | --- | --- |
| `pnpm install --frozen-lockfile` | 0 | 165 packages |
| `pnpm typecheck` | 0 | clean |
| `pnpm test` | 0 | **80 files / 806 tests / 0 failed** |
| `pnpm build` | 0 | built in 2.19 s; 541.44 kB JS, 98.01 kB CSS |
| `git diff --check` | 0 | clean |
| `cargo fmt --all -- --check` | 0 | clean — **rustfmt 1.8.0 / Rust 1.88.0, not the 1.93.0 pin** |
| Usage harness (`rustc --test`) | 0 | **25 / 25** |
| Inference harness | 0 | **48 / 48** — models 19, process 6, acquisition 7, registry 16 |
| Export harness | 0 | **27 / 27** |
| ZIP validated by Python `zipfile` | 0 | 4 entries, `testzip()` → `None` (all CRCs correct), UTF-8 flag set, unicode + empty files extracted intact |

**100 Rust tests were executed in this pass** (25 + 48 + 27), compiled from the real source files.
This is the first pass in which the acquisition tests ran at all.

### 17.2 What is newly executed, and what the harnesses exclude

New since §16: the **7 acquisition tests** now pass, including
`hashes_a_file_with_the_standard_test_vector`, which asserts SHA-256 of `abc` against the published
NIST value. The purchase/verify/finalise logic is therefore exercised:
URL built from the registry, refusal to publish a partial file, refusal to overwrite an existing
model, `verify_file` reporting a mismatch instead of failing, and a missing file producing
`not_configured`.

Harness composition, stated precisely so the evidence is not overstated:

| Harness | Real source files | Substitutions and exclusions |
| --- | --- | --- |
| Usage | `usage/mod.rs`, `usage/store.rs` | serde derives stripped; `usage/commands.rs` (Tauri) excluded |
| Inference | `inference/error.rs`, `models.rs`, `process.rs`, `registry.rs`, `acquisition.rs` | `serde`, `reqwest`, `serde_json`, `getrandom`, `sha2` replaced by shims; `settings.rs` replaced by a one-constant stub (`MAX_CONTEXT_SIZE`); `download`/`receive` (async network path) removed; one serde-serialisation test removed |
| Export | `export/{mod,plan,single,zip}.rs`, `workspace/scanner.rs` | serde stripped; `export/commands.rs` (Tauri) excluded; scanner's own test module excluded; `serde_json` and `getrandom` shimmed |

The `sha2` shim is a **FIPS 180-4 SHA-256 implemented for the harness**, not the `sha2` crate. It was
checked against three published vectors — `abc`, the empty string, and the 448-bit message — and all
three matched before it was used. Digest comparisons in those tests are therefore meaningful, but
they do not prove the crate itself resolves.

### 17.3 The model integration boundary

> **Superseded in part by §19 (2026-10-20, later).** This section describes the tree when Mini was
> the only profile. The registry now holds two profiles, `get_model_profile` returns the *selected*
> profile, `start_model_download`, `verify_model` and `verify_file` take a profile id, and the
> load/generation guards in `docs/nexora-sonnet/PHASE_5_7_CHAT_ROUTING.md` §8 apply. The
> statements below are kept as the historical record.


This is the answer to "can Sonnet or Opus be added later without rebuilding the application?"
**Yes — the inference path is already model-agnostic.** What is Mini-specific is metadata and
acquisition, not inference.

**Already generic (no change needed for a second model):**

| Layer | Evidence |
| --- | --- |
| Model metadata | `ModelRegistry::resolve(id)` + `validate()` over a `&'static [ModelProfile]` table |
| Model selection | `Settings.model_path` / `runtime_path` — arbitrary paths, no profile check |
| Engine life cycle | `Supervisor::start(runtime, model, ctx, dir)` — takes paths and a context size |
| Chat streaming | `start_generation` contains **zero** references to the registry or a profile |
| Streaming, cancellation, timeouts | Independent of which model the runtime loaded |
| UI chat / history | Unaware of model identity |

**Mini-specific today (three call sites, all metadata/acquisition):**

| Site | Current behaviour | Why it is Mini-specific |
| --- | --- | --- |
| `inference/registry.rs:145` | `static PROFILES: [ModelProfile; 1]` | fixed-length array; a second profile needs the length and the entry |
| `commands::get_model_profile()` | argless, returns Mini | Phase 4 decision: exactly one approved profile |
| `commands::start_model_download()` | resolves `NEXORA_MINI_ID` internally | download is pinned to the approved artefact |
| `acquisition::verify_file()` | compares against the Mini digest | integrity check is against the approved artefact |

**Smallest path to a second model** (not done in this pass, by instruction): add one
`ModelProfile` constant and one array entry; give the three commands above an optional
`profile_id: Option<String>` defaulting to Mini (backward compatible); add a profile selector to
`ModelSetup`. No second inference manager, no duplicated streaming code, no new settings format.

**Licensing of the current artefact:** Apache-2.0 (`Qwen/Qwen2.5-Coder-7B-Instruct-GGUF`), recorded
in the profile with a reference URL and carried through the registry tests. The runtime
(`llama.cpp`) is MIT and is **not** redistributed by this repository — the user supplies
`llama-server`.

### 17.4 Still blocked — unchanged, and not worked around

| Blocker | Observed this pass |
| --- | --- |
| `huggingface.co`, `cdn-lfs.huggingface.co` | HTTP **000** (egress filter) |
| `index.crates.io` | HTTP **000** → `cargo check`/`clippy`/`test` cannot resolve |
| Windows host | none; no MSVC, no WebView2 |
| RAM | 3.8 GiB total — below the 4.68 GB artefact before any KV cache |
| Local model / runtime | none present |

Consequently: **no genuine inference was demonstrated**, the **digest was not independently
verified**, and **Windows compilation and Clippy did not run**. No model file, checksum or output was
fabricated to fill these gaps.

### 17.5 Status

| Requirement | Status |
| --- | --- |
| Reusable model boundary understood and documented | **Yes** (§17.3) |
| Export behaviour verified | **Yes at logic level** (27 tests + independent ZIP validation); not click-through on Windows |
| Daily usage verified | **Yes at logic level** (25 tests: reset, boundary, restart, corruption, concurrency, fail-closed) |
| Available project checks pass | **Yes** (typecheck, 806 tests, build, fmt, diff-check) |
| Real Mini inference demonstrated | **No — blocked** |
| Windows compile / Clippy | **No — blocked** |
| Existing work preserved | **Yes**; nothing committed or pushed |

**Mini is architecture-ready for a future model addition, and its non-runtime behaviour is verified.
It is not release-verified.** Two proofs remain outstanding and both require a networked machine:
one real generation through the approved artefact, and a Windows `cargo check`/`clippy`/`test` run.

---

## 18. Final Mini implementation pass and model handoff (2026-10-20)

This pass made the last three source changes to the acquisition path and then closed the Mini
implementation. §1–§17 stand unchanged; this section records only what changed and what a future
model integration must reuse.

### 18.1 Source changes in `inference/acquisition.rs`

Three defects in the download path were confirmed by reading the code, and each was fixed in place.
Nothing else in the module moved.

| # | Defect | Fix | Effect on the approved flow |
| --- | --- | --- | --- |
| 1 | `client.get(..).send()` had **no time bound**. `STALL_TIMEOUT` only covered the body stream, so a host that accepted the TCP connection and never sent response headers left the command — and the Cancel button — waiting indefinitely. | The request is wrapped in `tokio::time::timeout(STALL_TIMEOUT, ..)` and returns `network` with "did not answer within 60 seconds. Nothing was installed." | None. A healthy host answers headers in milliseconds. |
| 2 | Redirects used the client default (10 hops) and would follow an **https → http** downgrade. | `redirect::Policy::limited(MAX_REDIRECTS = 5)` plus a check that `response.url().scheme() == "https"` before any body byte is written. | None. Hugging Face answers `/resolve/` with one https hop to its CDN. |
| 3 | Cancel was ignored between the last byte and the rename. Pressing Cancel during the final digest still **installed the file**, while the UI reported a cancelled download. | The publish branch is now `if !completed || cancel.load(..)`: a late cancel removes the `.part` file and reports `cancelled: true`. | None. |

Verification of these three is **static plus harness**: `acquisition.rs` still compiles as part of the
harness, `cargo fmt --all -- --check` is clean, and the 7 acquisition + 16 registry tests still pass
(§18.2). All three live in `receive`/`download`, which the harness excludes because they need the real
`reqwest`/`tokio` stack, and there is no network in this environment. **They have not been exercised
over a live connection.** They are marked accordingly below.

### 18.2 Checks run in this pass

| Check | Exit | Observed |
| --- | --- | --- |
| `pnpm install --frozen-lockfile` | 0 | 165 packages |
| `pnpm typecheck` | 0 | clean |
| `pnpm test` | 0 | **80 files / 806 tests** |
| `pnpm build` | 0 | 2.29 s; `index-D330U5cq.js` 541.44 kB |
| `cargo fmt --all -- --check` | 0 | clean (rustfmt 1.8.0 / Rust 1.88.0) |
| Harness: acquisition + registry | 0 | **23 / 23** (7 acquisition, 16 registry) |
| `git diff --check` | 0 | clean |

The harness SHA-256 shim was re-checked against the three NIST vectors (`abc`, the empty string, and
the 448-bit message) before use. One correction to note: my first attempt used 56 `0x61` bytes as the
448-bit message, which is the wrong message for that vector; the published 56-character string was
substituted and all three then matched.

The usage (25) and export (27) harnesses, and the models/process (25) group, were run in the
**previous** pass on this same tree. They were not re-run here because the sandbox was recycled and
the modules they cover were not modified in this pass; their results are not re-claimed as new
evidence.

**Export cancellation needed no change.** `lib/export.ts::cancelExport` already wraps `invoke` in
`try/catch` and resolves to `false`, so `void cancelExportCommand()` in `useExport.ts` cannot produce
an unhandled rejection. The `cancelling` flag is cleared in the `finally` of the export run and reset
at the start of the next one. The earlier note that this call "lacks a `.catch`" described a real
coding habit but not a live defect; no edit was made.

### 18.3 Model integration handoff

> **Superseded in part by §19 (2026-10-20, later).** This section describes the tree when Mini was
> the only profile. The registry now holds two profiles, `get_model_profile` returns the *selected*
> profile, `start_model_download`, `verify_model` and `verify_file` take a profile id, and the
> load/generation guards in `docs/nexora-sonnet/PHASE_5_7_CHAT_ROUTING.md` §8 apply. The
> statements below are kept as the historical record.


**Reusable as-is, with no changes for a second model:**

| Component | File | Why it is generic |
| --- | --- | --- |
| Profile table and validation | `inference/registry.rs` | `ModelRegistry::new(&'static [ModelProfile])`, `resolve(id)`, `validate()` |
| Settings | `inference/settings.rs` | `model_path`, `runtime_path`, `context_size` — no profile identity |
| Engine life cycle | `inference/process.rs` | `Supervisor::start(runtime, model, ctx, dir)`, bounded `stop()`, `Drop`, `poll_failure()` |
| Streaming and cancellation | `inference/client.rs` | First-token and idle timeouts, `Notify` cancel, SSE parsing |
| Generation command | `inference/commands.rs::start_generation` | Contains **no** registry or profile reference |
| Download pipeline | `inference/acquisition.rs` | `plan_for(profile)` already takes a profile |
| Usage allowance | `usage/mod.rs` | Keyed on policy, not on model |
| Export | `export/*` | Independent of the model |

**The exact model-specific configuration points:**

1. `inference/registry.rs:145` — `static PROFILES: [ModelProfile; 1]`: add the constant and extend the
   array. `ModelProfile` carries `id`, `display_name`, `upstream_model`, `upstream_revision`,
   `source { repository, revision, file_name, size_bytes, sha256, url }`, `context_length`,
   `licence`, `compatibility_notes`, and `locally_verified`.
2. `inference/commands.rs::get_model_profile` — currently argless and returns Mini. Needs an optional
   `profile_id` defaulting to `NEXORA_MINI_ID`.
3. `inference/commands.rs::start_model_download` — resolves `NEXORA_MINI_ID` internally; the same
   optional parameter applies.
4. `inference/acquisition.rs::verify_file` — resolves and compares `NEXORA_MINI_ID`; the same
   parameter applies.

Those four sites are the whole of the coupling. With `profile_id: Option<String>` defaulting to Mini,
existing callers and the current UI keep working unchanged. No second supervisor, streaming function,
settings file, or usage counter is needed.

**Blockers that need a suitable machine** (unchanged; no workaround attempted):

- No route to `huggingface.co` / `cdn-lfs.huggingface.co` (HTTP 000) and none to `index.crates.io`,
  so no artefact, no `cargo check`, no Clippy.
- 3.8 GiB RAM against a 4 683 073 536-byte artefact.
- No Windows host, so no MSVC compile, no WebView2 UI pass.

**Unresolved concerns that must not be treated as fixed:**

- The three §18.1 fixes are **not** exercised over a live connection.
- `verify_file` still hashes a multi-GB file with no cancel path (it runs on `spawn_blocking`, so it
  does not stall other commands, but the UI cannot abort it).
- Download resume is not implemented: an interrupted transfer restarts from zero.
- The digest has never been checked against a real 4.68 GB file in this repository.
- Windows has never compiled this tree.

---

## 19. Registry shape change for the second model (2026-10-20)

Nexora Sonnet was added as a second registered profile. Mini's metadata and behaviour are
unchanged, but one structural change affects the description in §4–§5 and in
`PHASE_4_MODEL_REGISTRY.md`:

| Before | After | Why |
| --- | --- | --- |
| `ModelProfile.source: ModelSource` | `ModelProfile.artifact: ArtifactState` (`Approved(ModelSource)` or `Pending { note }`) | A profile whose artefact has no recorded revision/length/digest must be representable without inventing one |
| `upstream_revision: &'static str` | `upstream_revision: Option<&'static str>` | Same reason |
| `PROFILES: [ModelProfile; 1]` | `PROFILES: [ModelProfile; 2]` | Sonnet added |
| `get_model_profile()` (argless, Mini) | returns the **selected** profile (Mini when nothing is chosen) | The setup screen must follow the chat selector |

Mini's values are byte-identical: same id, revision, file name, 4 683 073 536 bytes, and
SHA-256, and it remains `DEFAULT_MODEL_ID`. Existing settings files load unchanged (`modelId`
is optional), and a model file that matches no registered artefact is still accepted while
Mini is selected.

The earlier statements in §4 that "there is exactly one profile" and that `source` is a
non-optional struct are now stale; they described the tree at the time and are left in place
rather than rewritten. Current behaviour is described in `docs/nexora-sonnet/` §3–4.

No Mini test was removed or weakened. The registry tests were extended from 17 to 27, and the
acquisition tests from 7 to 12.

# Phase 9 — Comprehensive verification

Date: 2026-10-20. Branch `arena/18030bbb-nexora`, HEAD `9bd4f7a`.
Everything below was executed in this environment; nothing is carried over from an earlier
report unless the row says so.

## 1. Result summary

| Area | Result |
| --- | --- |
| Frontend typecheck | **Pass** (exit 0) |
| Frontend tests | **Pass — 81 files / 812 tests** |
| Production build | **Pass** — `index-C_w6vt2T.js` 544.23 kB (gzip 152.50 kB), 2.13 s |
| Rust formatting | **Pass** — every file in `src/`, rustfmt 1.8.0 (Rust 1.88.0) |
| Rust harness tests (registry + acquisition) | **Pass — 40 / 40** |
| `cargo check` / `clippy` / `cargo test` | **Blocked** — no crate resolution offline |
| Genuine Sonnet inference | **Not performed** — impossible here |
| Windows / MSVC / WebView2 | **Not performed** — no Windows host |
| Repository preserved | **Yes** — nothing committed, pushed, or discarded |

## 2. What the 40 Rust tests cover

Compiled from the real `registry.rs`, `acquisition.rs`, and `error.rs` against a local
`sha2`/`getrandom` shim (the SHA-256 implementation was re-checked against the three NIST
vectors before use).

| Group | Count | Examples |
| --- | --- | --- |
| Registry metadata | 27 | both profiles resolve; Sonnet is pending; Mini values unchanged; validation rejects malformed revision/digest/context |
| Selection policy | 6 | Mini↔Mini ok; Mini selected while Sonnet loaded → `model_mismatch`; Sonnet selected → `artifact_pending`; unknown id → `unknown_model`; unidentified file allowed only for the default |
| Acquisition | 12 | approved URL built from the registry; pending profile has no plan; pending profile cannot be verified; `installed_path` rejects a truncated file and a renamed file; `profile_for_path` attributes by name |

Excluded from the harness, and therefore **not** type-checked: `commands.rs`, `mod.rs`,
`lib.rs`, and the async `download`/`receive` functions. They require `tauri`, `tokio`,
`reqwest`, and `serde`. What was verified for them:

- `rustfmt --check` parses every file (syntax valid, formatting clean);
- `rustc --emit=metadata` on `lib.rs` produces **no syntax errors** — only unresolved-crate
  and missing-attribute errors, which is the expected signature of compiling without
  dependencies;
- the UI contract test (`rustContract.test.ts`) re-parses the Rust sources and fails if a
  command's arguments, struct fields, or registration drift from what the UI invokes. It
  passes, which cross-checks the command surface against the frontend.

That is not the same as a type-check, and this document does not claim it is.

## 3. Mini regression

| Check | Result |
| --- | --- |
| Mini profile id, revision, size, digest, context | unchanged (asserted in tests) |
| Mini remains the default | `DEFAULT_MODEL_ID == NEXORA_MINI_ID`; `selected(None)` → Mini |
| Legacy settings file without `modelId` | loads and selects Mini (settings tests) |
| Unknown configured model file | still allowed while Mini is selected |
| Existing 806 frontend tests | all still pass |
| Mini documentation | preserved, with one addendum recording the registry shape change |

## 4. Model selection behaviour

| Scenario | Observed behaviour |
| --- | --- |
| Nothing selected | Mini is selected and reported by `get_model_profile` |
| Select Sonnet | `select_model` persists the id; catalogue reports `selected` |
| Send while Sonnet selected | refused by `start_generation` with `artifact_pending`, before any usage unit is spent |
| Select Mini again | restored; sending works when the engine is running |
| Unknown id | rejected, never rewritten to the default |
| Switch while streaming | selector disabled; conversation untouched |
| No desktop core (browser) | selector renders nothing |

## 5. Acquisition, cancellation, integrity

Verified by test: URL construction from the registry, refusal of a pending artefact before any
side effect, `installed_path` requiring both the exact file name and the exact length,
`profile_for_path` attribution, publish refusing a partial file and an existing destination.

**Not verified here:** a real download, real cancellation during transfer, real redirect
behaviour, and a real digest comparison against a 4.68 GB or 9 GB file. There is no route to
the model host and no artefact on disk. The download hardening (bounded `send()`, 5-hop
redirect limit, https-only after redirect, cancel-before-publish) is present and parses, but
has never been exercised over a live connection.

## 6. Genuine local inference

**Not demonstrated.** Blockers, all measured:

| Blocker | Measurement |
| --- | --- |
| Model host | `huggingface.co`, `cdn-lfs.huggingface.co` unreachable |
| Model artefact | none on disk; Sonnet's digest/length not recorded |
| Runtime | no `llama-server` present |
| Crate registry | `index.crates.io` unreachable → `cargo check` fails at `futures-util` |
| Memory | 3.8 GiB total; Sonnet needs ≈ 11 GB at 8,192 tokens |
| Windows | no Windows host |

No model file, checksum, or output was invented to fill these gaps.

## 7. Security and licence

| Requirement | Status |
| --- | --- |
| No new Tauri permissions | No changes to capabilities or allowlists |
| CSP unchanged | No change |
| No network from the UI | Unchanged; downloads remain in Rust |
| No silent model substitution | Enforced by `check_selection` in the core, surfaced by the UI |
| No unverifiable download | `plan_for` refuses a profile without an approved digest |
| No overwrite of an existing model | Unchanged (`already_exists`) |
| Credentials | None involved in this feature |
| Model licence | Apache-2.0 recorded with a reference; weights are never redistributed |
| Runtime licence | llama.cpp (MIT) still supplied by the user, not bundled |

## 8. Completion decision

**NOT FULLY VERIFIED / BLOCKED.**

Implemented and passing: the second profile, selection persistence, the chat selector, the
routing guard, profile-aware acquisition and verification, documentation, and full frontend
verification.

Not satisfied, so completion cannot be claimed:

1. **No real Sonnet inference.** The artefact is unapproved and the environment cannot run it.
2. **No Windows or MSVC build**, and no WebView2 run.
3. **No Clippy, no `cargo check`, no `cargo test`** — dependencies cannot resolve.
4. The three download-hardening behaviours remain unexercised over a live connection.
5. The KV-cache arithmetic that justifies the 8,192-token default comes from published
   configuration, not from a file in this repository.

Smallest next step that changes the outcome: on a networked 16 GB machine, record the artefact
metadata (Phase 1.5), run `cargo check`/`clippy`/`test`, download once, and record a real
generation.

---

## 9. Addendum — artefact resolution run (2026-10-20)

The blocker recorded in §8 item 1 ("no approved artefact") is **resolved**. The model itself
still cannot run here, so the completion decision moves from "blocked on metadata" to
"implemented, artefact verified, inference still unverified".

### 9.1 What changed

| Item | Before | After |
| --- | --- | --- |
| `NEXORA_SONNET.artifact` | `Pending` | `Approved(ModelSource { … })` |
| Revision | none | `d0a692ef765eefbf2fabb130b3cb2e8917e3d225` |
| Size | none | `8 988 110 272` |
| SHA-256 | none | `c1e659736d89ac1065fb495330fb824d94001974a4bfa78e7270e43476a8d940` |
| `upstream_revision` | `None` | `aedcc2d42b622764e023cf882b6652e646b95671` |
| Licence reference | repository root | the pinned commit's `LICENSE` blob |
| `check_selection` | took an id, resolved internally | takes the profile, so the `Pending` branch stays testable |

Only the registry and its tests changed. Acquisition, download hardening, cancellation,
redirect limits, the chat selector and Mini's behaviour were **not** modified.

### 9.2 Checks run in this addendum

| Check | Result |
| --- | --- |
| `pnpm install --frozen-lockfile` | 0 |
| `pnpm typecheck` | **0** |
| `pnpm test` | **0 — 81 files / 812 tests** |
| `rustfmt --check`, whole `src/` tree | **0** |
| Rust harness (registry + acquisition) | **43 / 43** (was 40) |
| `cargo check` / `clippy` / `cargo test` | **Blocked** — `futures-util` unresolvable |

New tests assert the exact artefact values, that the licence reference points at the same
commit as the weights, that the download URL is the pinned one, and that a truncated Sonnet
file is not reported as installed. The `Pending` refusal path is still covered, now with a
locally built profile.

### 9.3 Still unverified

1. **No genuinely executed Sonnet inference.** 3 GB RAM available against ≈ 10 GiB needed.
2. **No download.** Sandbox egress to `huggingface.co` returns HTTP 000; the digest is
   host-published, not locally recomputed.
3. **No llama.cpp load** of this file, and no runtime version check against it.
4. **No Windows/MSVC/WebView2 verification.**
5. **No Clippy.**

### 9.4 Decision

**NOT FULLY VERIFIED.** The artefact is verified and the profile is safely enabled — the
download path refuses any file whose length or digest differs from these values. What is not
proven is that Nexora can *run* it, because no environment available here has the memory or
the network to try.

Smallest next step: on a 16 GB machine with llama.cpp, download through Nexora, confirm the
digest passes, load it, and record one generation.

## 10. Addendum — hardware-aware readiness (2026-10-21)

Branch `arena/18030bbb-nexora`, HEAD `9bd4f7a` (no commit made). This addendum records the
checks for the readiness phase only; §§1–9 stand as written.

### 10.1 What changed

| File | Change |
| --- | --- |
| `inference/readiness.rs` | **New.** Pure `assess()` / `estimate()`, path-keyed `VerificationLedger`, 14 states with stable `as_str()` names, 4 check statuses, byte conversions, 39 tests |
| `inference/hardware.rs` | `DiskSpace`, `disk_space()`, `nearest_existing()`, Linux `statvfs` + Windows `GetDiskFreeSpaceExW`, 3 tests |
| `inference/registry.rs` | `kv_bytes_per_token` (`57 344` Mini, `196 608` Sonnet) + geometry test |
| `inference/mod.rs` | Declares `readiness`; holds the session ledger; `verification_status` is path-aware; `ActiveDownload` carries the profile id |
| `inference/commands.rs` | Readiness folded into every catalogue row; `check_model_readiness`; `get_disk_space`; disk re-check before a download; memory + verification gates before a load; `verify_model` and a completed download write the ledger |
| `src/lib.rs` | Both new commands registered |
| UI | `Readiness`/`ResourceEstimate`/`DiskSpace` types, `fetchModelReadiness`, `fetchDiskSpace`, `features/models/readiness.ts`, `ModelReadinessCard`, selector and setup wiring, storage row, CSS |

No Mini or Sonnet artefact value was changed, and no evidence in `ARTIFACT_EVIDENCE.md` was
edited.

### 10.2 Checks run

| Check | Result |
| --- | --- |
| `pnpm install --frozen-lockfile` | **0** |
| `pnpm typecheck` | **0** |
| `pnpm test` | **0 — 83 files / 833 tests** (was 81 / 812) |
| `pnpm build` | **0** — `index-D93vR3Cp.js` 549.95 kB (gzip 153.95 kB), CSS 102.37 kB |
| `rustfmt --check`, all 42 files in `src/` | **0** |
| Rust harness (readiness + hardware + registry + models) | **0 — 93 / 93** (39 + 5 + 30 + 19) |
| `rustc --emit=metadata src/lib.rs` | **Parse-level only** — every diagnostic is an unresolved crate (`tauri`, `serde`, `tokio`, `reqwest`, `sha2`); no syntax errors |
| `cargo check` / `clippy` / `cargo test` | **Blocked** — `index.crates.io` unreachable; `futures-util` cannot be resolved |
| Windows build / MSVC / WebView2 | **Not run** — no Windows environment |

### 10.3 Real measurements taken

`hardware.rs` was compiled standalone and executed, then cross-checked against the OS:

| Reading | Module | OS, same instant |
| --- | --- | --- |
| Free disk (first sample) | 19 480 596 480 B | `df -B1` 19 480 596 480 B — **exact** |
| Total disk (first sample) | 21 834 924 032 B | `df -B1` 21 834 924 032 B — **exact** |
| Free disk (second sample, after `pnpm install`) | 19 475 763 200 B | not sampled at that instant |
| Total RAM | 3.8 GiB (rounded from ~4.08 GB) | `free -b` 4 131 278 848 B → 3.85 GiB |
| Available RAM | 3.5 GiB | `free -b` 3 784 679 424 B → 3.52 GiB |

`statvfs` and `df -B1` returned byte-identical free and total values in the same second. The
memory fields agree with `free` once the module's 0.1 GiB display rounding is applied. The readiness engine was then run against these live values and produced
`ready_to_download` (with a memory warning) for both profiles, and `insufficient_memory` for
both once a verified file is imagined present. That is a readiness calculation, **not** an
inference run.

### 10.4 Still unverified after this addendum

1. **No genuinely executed Sonnet inference**, and none for Mini in this session.
2. **No download** of either artefact here; the readiness numbers use the published lengths.
3. **No Windows execution** of the `GetDiskFreeSpaceExW` or `GlobalMemoryStatusEx` branches.
4. **No real low-memory / low-disk machine**; those paths are covered by fixtures.
5. **No Clippy and no `cargo test`**, for the dependency-resolution reason above.

### 10.5 Decision

**Readiness implemented and unit-tested; inference still not demonstrated.** The three
milestones remain separate: (1) hardware detection — implemented, exercised on Linux, not on
Windows; (2) readiness logic — verified by 39 Rust tests and a frontend/backend contract test;
(3) genuine inference — **not achieved**.

---

## 11. Phase 7 — documentation, integration cleanup and verification readiness (2026-10-22)

Branch `arena/a656312a-nexora`, base `0a5d0ae`. This section does not claim any new capability
works end to end. It records what was re-checked, what was changed, and the checklist a later
verification run must satisfy. Phase 8 (final comprehensive verification) was not started.

### 11.1 Integration defects fixed (source-level)

| # | Defect demonstrated by reading the code | Fix | Evidence |
| --- | --- | --- | --- |
| 1 | `load_model` did not check for an active reply. `Supervisor::start` stops the running child before it spawns, so a load started during a reply cut that reply off. The unit was already committed, because the engine had accepted the request. | `registry::check_load_allowed`; `load_model` refuses `busy` after it claims the loading marker. | harness test; source review of `commands.rs` |
| 2 | `start_generation` did not check for a load in flight. A message could be answered by whichever process was healthy while the engine was being replaced. | `registry::check_generation_allowed`, evaluated under the reply-slot lock. | harness test; source review |
| 3 | The request `send()` that waits for response headers had no bound and was not raced against Stop. A Stop before the headers arrived was ignored. | Raced with `cancel.notified()` and bounded by `FIRST_TOKEN_TIMEOUT`. Neither path charges a unit. | source review **only** |
| 4 | A process that failed to start was not recorded against its profile, so the engine panel could show an older failure or none. | `record_failure(plan.profile_id, …)` in the spawn-error branch. | source review **only** |

Behaviour that was reviewed and **not** changed, because it is a product decision: a reply the
user stops after the engine has started streaming still consumes one usage unit (the documented
rule in [PHASE_5_7_CHAT_ROUTING.md](./PHASE_5_7_CHAT_ROUTING.md) §7.2). Confirm before changing it.

### 11.2 Documentation corrected

| File | Correction |
| --- | --- |
| `READINESS.md` §8.1 | Readiness test count is 41 in source, not 39. |
| `PHASE_5_RUNTIME.md` §3.4, §5.4 | The 4 characters/token budget is typical of English prose, not a bound for dense code. The earlier word "conservative" was wrong. Behaviour unchanged. |
| `PHASE_5_RUNTIME.md` §3.2, §3.3, §4 | Added the Phase 7 lifecycle rules, the send-phase change, and the re-run results. |
| `PHASE_5_7_CHAT_ROUTING.md` §8 | New: the load/generation exclusion rule, its ordering, and what it does not prove. |
| `nexora-mini/PHASE_8_VERIFICATION.md` §17.3, §18.3 | Marked as superseded in part by §19. They still describe the one-profile tree. The historical text is kept. |
| `README.md` (Sonnet index) | Points to §8 and §11. |

Test counts in older records were taken from different harness subsets, so they do not match
the current source. Current source counts (`#[test]` attributes, 2026-10-22): `readiness.rs` 41,
`registry.rs` 41, `loading.rs` 13, `budget.rs` 9, `models.rs` 19, `process.rs` 6, `acquisition.rs` 14,
`client.rs` 14, `usage/` 25, `export/` 27, `workspace/scanner.rs` 15. Frontend: 84 test files.

### 11.3 Checks run in this phase

Baseline run on the unchanged tree and final run after the edits. Toolchains are as stated.

| Check | Command | Result |
| --- | --- | --- |
| Install | `pnpm install --frozen-lockfile` (pnpm 9.15.4, the `packageManager` pin) | 0, 165 packages |
| Typecheck | `pnpm typecheck` | 0 |
| Frontend tests | `pnpm test` | 0, **849 / 849, 84 files** |
| Frontend build | `pnpm build` | 0, `index-N2i249U6.js` 550.55 kB |
| Rust format | `rustfmt --check --edition 2021` on all 44 files under `src-tauri/src` (rustfmt 1.8.0 / rustc 1.88.0, from npm) | 0. Not the pinned 1.93.0, which cannot be obtained here. |
| Standalone harness | `registry.rs` + `error.rs` from the real source; serde derives and the two serde_json tests stripped; `rustc --test` | 0, **41 / 41** (baseline 39, plus 2 new). This is not `cargo test`. |
| Full Cargo | `cargo metadata` in `apps/desktop/src-tauri` | **101**: `index.crates.io` TLS failure. Blocked. |

### 11.4 Verification checklist for a later run

Status key: **NOT RUN** means it was never executed. **BLOCKED** names the blocker. Nothing here
is marked passed unless its row says so.

| # | Requirement | Command or procedure | Prerequisites | Expected evidence | Status on 2026-10-22 |
| --- | --- | --- | --- | --- | --- |
| 1 | Full Rust app compiles with real dependencies | `cargo generate-lockfile`, then `cargo check --all-targets --locked` and `cargo clippy --all-targets -- -D warnings`, in `apps/desktop/src-tauri` | Networked host with crates.io; Rust 1.93.0 (`rust-toolchain.toml`); no `Cargo.lock` is committed yet | Exit 0 for both | **BLOCKED**: crates.io unreachable (`cargo metadata` exit 101). 1.93.0 unobtainable. |
| 2 | Approved GGUF downloads and its SHA-256 verifies locally | Start the download in Model setup, or run `verify_model` on the file. Independent check: `sha256sum qwen2.5-coder-14b-instruct-q4_k_m.gguf`, `stat -c %s`. | `huggingface.co` reachable; about 9 GB free | Length 8 988 110 272; SHA-256 `c1e65973…476a8d940` | **BLOCKED**: `huggingface.co` returns HTTP 000. Disk is fine: 19.35 GB free. |
| 3 | Pinned llama.cpp `b11714` loads that exact file | `probe_runtime` (expect build 11714); then Load model | `llama-server` b11714; about 11 GiB available memory at 8 192 tokens | `/health` 2xx; engine panel shows `loaded` | **BLOCKED**: no binary, no file. Host: 3.56 GiB available of 3.85 GiB total (`/proc/meminfo`), against an estimate of 10.87 GiB. |
| 4 | A real prompt produces a complete streamed reply | Send a message with a loaded model | Item 3 | Incremental deltas, then a `finish_reason`; no "ended before finishing" error | **NOT RUN** (depends on 3). |
| 5 | Cancel, errors and profile switching with a running engine | Stop mid-reply; switch during a reply (expect `busy`); load during a reply (expect `busy`, §11.1 #1); send during a load (expect `busy`, #2); kill the engine (expect `runtime_failure` for the loaded profile) | Item 3 | Each case shows the stated result. Stop ends in "stopped" and charges per §11.1 note. | **NOT RUN**. The refusal paths are harness-tested for the pure rule only. |
| 6 | Mini works, and no cross-profile fallback | With Mini loaded: send a message. Select Sonnet while Mini is loaded: send is refused with `model_mismatch`, before a unit is spent. Sonnet selected with a Mini file: load is refused. | Mini's own GGUF (4.68 GB) and item 3's runtime | Reply from Mini; refusal text names the loaded model; no unit change | **NOT RUN**. Refusal rules are covered by 41 harness tests and frontend tests; live behaviour is not. |
| 7 | Windows app builds and runs with MSVC and WebView2 | Windows CI `windows` job; `cargo build --target x86_64-pc-windows-msvc`; `tauri build`; launch the installed app | Windows host or runner with MSVC, WebView2 runtime and the Windows SDK | Build succeeds; app opens; `GetDiskFreeSpaceExW` and `GlobalMemoryStatusEx` return real values | **BLOCKED**: no Windows host. Not run here. Earlier metadata-only type checks are in the Mini record (§16.3); they were not repeated in Phase 7. |
| 8 | Security, licensing, documentation and Git/worktree preservation | `pnpm test` (includes the IPC contract test), `git diff --check`, `git status`; inspect `capabilities/` and the CSP; confirm no cloud endpoint in the source | None beyond the repo | Tests pass; no new permission or CSP change; licence reference present | **Partly run**: tests pass (row above); Git and permission checks are in §11.5. |

Host reading used for item 3, taken with `/proc/meminfo` and `df -B1 /`: MemTotal 4 034 452 kB,
MemAvailable 3 729 040 kB, free disk 19 350 503 424 B.

### 11.5 Git and worktree

Branch `arena/a656312a-nexora`, HEAD `0a5d0ae` (unchanged). The worktree started clean, so there
were no pre-existing uncommitted Mini or Sonnet changes to preserve. Nothing was reset, cleaned,
stashed, or reverted. Modified files (uncommitted): the four `inference/*.rs` files listed in §11.1;
`budget.rs` (comments only); and the docs `nexora-mini/PHASE_8_VERIFICATION.md`,
`nexora-sonnet/{README,READINESS,PHASE_5_RUNTIME,PHASE_5_7_CHAT_ROUTING,PHASE_9_VERIFICATION}.md`.
No commit, push, merge, tag, or release was made. `pnpm install` created `node_modules/` and `dist/`,
both git-ignored. `cargo metadata` wrote no `Cargo.lock`. `git diff --check` is clean.

### 11.6 Open items for the owner

1. Decide whether a user-stopped reply that had begun streaming should consume a usage unit (§11.1).
2. Decide whether the load-during-reply refusal should instead cancel the reply first. The current
   rule refuses and makes the user stop it.
3. Decide whether to tighten the prompt-budget heuristic for code-heavy prompts. That would refuse more
   requests; it is not changed here.

## 12. Phase 8 final verification — matrix A–K (run 2026-10-22, current tree)

This section is the **current** run. Sections 1–11 are historical and are not changed to match it.
Evidence classes: **[AUTO]** automated suite on a real toolchain; **[HARNESS]** `rustc --test` on
real source with serde and serde_json stripped, which is not a Cargo build; **[RUNTIME-SYNTH]** the
pinned llama.cpp binary with a random-weight synthetic model, which proves transport only and is
**not** E or F; **[SOURCE]** code reading only; **[BLOCKED]** not run, with the reason.

Outcome: **NOT FULLY VERIFIED / BLOCKED.** No required check actually failed. Several
required checks are blocked by the environment (see §12.3).

### 12.1 Environment at run time

| Item | Reading |
| --- | --- |
| Network | `huggingface.co`, `static.rust-lang.org`, `crates.io`, `static.crates.io` → HTTP 000. `api.github.com`, `codeload.github.com`, `pypi.org` reachable. `objects.githubusercontent.com` unreachable, so GitHub release assets cannot be downloaded. |
| Host | 2 vCPU, MemTotal 4 034 452 kB (3.85 GiB), MemAvailable about 3.5 GiB, 19 GB free disk |
| Toolchain | No `cargo`, `rustc`, or `llama-server` on PATH. A rustc 1.88 + rustfmt from npm (`/tmp/rustcheck`) was used only for the harness. Pinned Rust 1.93.0 is not obtainable here. |
| Git | Branch `arena/a656312a-nexora`; base `0a5d0ae`; see §12.2 K |

### 12.2 Matrix

| ID | Requirement | Command / procedure | Evidence class | Result | Verdict |
| --- | --- | --- | --- | --- | --- |
| **A** | Frontend | `npm run typecheck`, `npm test -- --run`, `npm run build` in `apps/desktop` | [AUTO] | typecheck exit 0; **849 / 849 tests, 84 files**; build exit 0, `built in 1.81s`. Log: `/tmp/harness/frontend-gate.log` (outside repo). | **PASS** |
| **B** | Full Rust compile | `cargo check --all-targets --locked` | — | Not run. `cargo` is absent, and crates.io is unreachable (Phase 7: `cargo metadata` exit 101). | **BLOCKED** |
| **C** | Rust tests and Clippy | `cargo test --all-targets`; `cargo clippy --all-targets -- -D warnings` | [HARNESS] only | Clippy not run. Harness: `registry`+`error` **41/41** (`build.py`); `error`, `registry`, `settings`, `loading`, `budget`, `usage` **88/88** (`harness_extended.py`). `commands.rs`, `client.rs`, `process.rs`, `acquisition.rs`, `readiness.rs`, `models.rs`, `export/`, `workspace/` were not compiled. | **BLOCKED** (the harness does not make C pass) |
| **D** | Artefact download and SHA-256 | Model setup download, or `curl -I` to the pinned resolve URL | — | `https://huggingface.co/Qwen/Qwen2.5-Coder-14B-Instruct-GGUF/resolve/d0a692ef…/qwen2.5-coder-14b-instruct-q4_k_m.gguf` → **HTTP 000**. Nothing downloaded. Published values remain in `ARTIFACT_EVIDENCE.md` (historical, host-published). | **BLOCKED** |
| **E** | Pinned llama.cpp loads the approved artefact | `llama-server --version`; load the Sonnet GGUF | [RUNTIME-SYNTH] for the binary only | Built `b11714` from the codeload source tarball (commit `c0fd786`; build number and commit passed with `-DLLAMA_BUILD_NUMBER`/`-DLLAMA_BUILD_COMMIT` because there is no `.git`). `--version` line: `version: 0.7.0-dev (build 11714, commit c0fd786)`. `reports_pinned_build` splits on non-digits and finds `11714`. The **approved** GGUF was never loaded. | **BLOCKED** |
| **F** | Genuine end-to-end generation and streaming | Send a prompt to the loaded approved model, or Mini | [RUNTIME-SYNTH] | Synthetic 2-layer random model, served with Nexora's flags (`--host 127.0.0.1 --no-ui --jinja --api-key-file --ctx-size 4096 --parallel 1`). `stream_check.py`: HTTP 200, `text/event-stream`, 38 content deltas, `finish_reason:"length"`, `data: [DONE]`. Output is random text, so this is **not** language quality and **not** F. | **BLOCKED** |
| **G** | Cancellation, errors, lifecycle | Stop mid-reply; kill engine; load and switch during reply | [RUNTIME-SYNTH] + [HARNESS] + [SOURCE] | Runtime, pinned binary: (1) **Auth:** no key → 401 `Invalid API Key`; wrong key → 401. (2) **Disconnect:** client closed after 5 deltas of a 4 000-token request; server log `stop: cancel task, id_task = 65`, released at 174 tokens, and the next request was served at once (the 4 000-token request was not run to completion). (3) **Truncation:** engine SIGKILLed after 5 deltas gave 39 deltas with no `finish_reason` and no `[DONE]`; `stream_completed` returns `false`, so Nexora would show an error, not a finished reply. Application-level Stop, busy refusals, and `runtime_failure` **not** exercised live (no Nexora app run). | **BLOCKED** |
| **H** | Mini regression; no silent fallback | Load Mini, send, switch to Sonnet with Mini loaded | [HARNESS] + [AUTO] | Mini GGUF cannot be fetched (same host as D). Refusal rules (`check_load_allowed`, `check_generation_allowed`, `model_mismatch`) are covered only by harness and frontend unit tests, not live. No synthetic model was ever presented as Mini. | **BLOCKED** |
| **I** | Windows, MSVC, WebView2 | `cargo build --target x86_64-pc-windows-msvc`; `tauri build`; launch installed app | — | No Windows host. Not run. | **BLOCKED** |
| **J** | Security, licence, docs | Capability/CSP/Cargo diff vs base; runtime flags; auth enforcement; licence reference | [SOURCE] + [RUNTIME-SYNTH] | Diff vs `0a5d0ae` for `capabilities/`, `tauri.conf.json`, `Cargo.toml`, `ui/index.html`: **empty** (unchanged). CSP unchanged. `process.rs` passes `--api-key-file` and binds `127.0.0.1`; no `0.0.0.0` bind; no `--slot-save-path`. Auth enforced by the pinned binary (401 above). Licence Apache-2.0 was read at the pinned commit in an earlier run; **not re-fetched today** (host unreachable). Docs corrected in this run: §3.3 of `PHASE_5_RUNTIME.md` (`[DONE]`); `3 GB RAM` → `3.85 GiB total, about 3.5 GiB available` in `ARTIFACT_EVIDENCE.md`, `PHASE_1_2_MODEL.md`, `README.md`. | **PASS** (scoped to security surface and docs; the licence is historical evidence) |
| **K** | Git and worktree | `git status`, `git diff --check`, `git stash list`, branch and HEAD | [SOURCE] | Branch `arena/a656312a-nexora`; no stashes; no existing work reset, cleaned, or reverted. Committed in this run, as the user explicitly asked for a PR (see §12.4). Verification scripts added under `docs/nexora-sonnet/verification/`. Nothing merged, tagged, or released. `git diff --check` result is recorded in §12.4. | **PASS** |

### 12.3 Overall outcome and remaining blockers

**NOT FULLY VERIFIED / BLOCKED.** Required checks B, C (Clippy and full tests), D, E, F, G (app-level),
H (live), and I are blocked or not run. Nothing has actually failed. The Mini and Sonnet models were
never run. The synthetic-model results are transport evidence only.

Remaining blockers, each with the evidence needed to clear it:

1. **Network (B, C, D, E, F, H):** `crates.io`, `static.crates.io`, `huggingface.co`, and
   `static.rust-lang.org` are unreachable. Evidence needed: HTTP 200 from these hosts, then
   `cargo generate-lockfile && cargo check --all-targets --locked && cargo clippy --all-targets -- -D warnings`
   on Rust 1.93.0, plus `cargo test`, exit 0. Also the SHA-256 of the downloaded file matching
   `c1e65973…476a8d940`, with length 8 988 110 272.
2. **Memory (E, F for Sonnet):** about 3.5 GiB available against the estimated 10.87 GiB at
   8 192 tokens. Evidence needed: a host with at least about 12 GiB free, then `probe_runtime`
   reporting build 11714, `/health` 2xx, and a streamed reply with `finish_reason`.
3. **Mini live run (H):** Mini GGUF (4.68 GB) download. Same network blocker.
4. **Windows (I):** a Windows MSVC/WebView2 host or CI run. Not available here.
5. **Application-level lifecycle (G):** Stop, busy refusals, and `runtime_failure` need a running Nexora
   desktop build talking to a real loaded model.
6. **Licence re-check (J):** re-fetch the LICENSE at `d0a692ef…` when the host is reachable.

### 12.4 Git and PR — conflict reported

Phase 8 said "do not commit, push, merge, tag, or release". The latest user request asks for a pull
request. A PR requires a commit and a push, so the explicit PR request was followed for those steps only. Nothing was
merged, tagged, or released. The commit is on `arena/a656312a-nexora`, and the pushed branch is
the PR head. The commit hash and PR URL are reported in the final answer.

`git diff --check` was run before the commit; its result is recorded in the final answer.

### 12.5 Reproduction

Scripts are in `docs/nexora-sonnet/verification/` (see its README). The synthetic model and the
llama.cpp build live in `/home/user/.nexora-verify/`, outside the repository, and are not committed.

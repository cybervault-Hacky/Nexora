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

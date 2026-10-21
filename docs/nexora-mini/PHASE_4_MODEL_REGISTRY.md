# Phase 4 — Model Profile & Registry Integration

**Project:** Nexora Mini — local-first AI coding studio
**Phase:** 4 of the Mini programme
**Date:** 2026-10-20
**Branch:** `arena/18030bbb-nexora`
**Base commit:** `9bd4f7ad36ab012b50e48333c27fdc9b8244d5ff` (`main` = `origin/main`)
**Status:** Integration plumbing complete. **Not committed, not pushed.** No weights downloaded; no inference run; no model-quality claim made.

| Document | Lines | Bytes | SHA-256 |
| --- | --- | --- | --- |
| `PHASE_1_AUDIT.md` | 780 | 44,544 | `a40e72970f849bcef5d8d21869d25160bbbb18ff9ac95efc47f4b2796e9e8a42` |
| `PHASE_2_BUILD_RECOVERY.md` | 662 | 38,678 | `c1cbdfb7922aca3d2b79f5b155338b83ab6ec0612eacc4332d3094611947423a` |
| `PHASE_3_MODEL_EVALUATION.md` | 706 | 49,782 | `9621f33afd67d3b36e3a63052823f9b2447b5452682794b2f9435cae2dca1fee` |

All three earlier documents were re-hashed after this phase's edits and are byte-identical to their delivered versions (section 13).

---

## 1. Executive summary

Phase 4 adds **one** typed model profile and **one** metadata registry to the existing inference core, and exposes the profile through **one** read-only Tauri command. Nothing else changed in behaviour.

What exists now:

* `src/inference/registry.rs` — a new 447-line module containing the `ModelProfile` type, the single approved `NEXORA_MINI` constant, a `ModelRegistry` that resolves a stable identifier to a profile, validation of the profile's own metadata, and 17 unit tests.
* `InferenceError::unknown_model` — a typed error code for an identifier the registry does not know.
* `get_model_profile` — an argument-less Tauri command returning the profile as JSON. Registered alongside the ten existing inference commands.
* `fetchModelProfile()` in `ui/src/lib/inference.ts` — a typed wrapper, plus the `ModelProfile` interface in `ui/src/lib/ipc.ts`.
* Three new frontend tests and one updated assertion in the source-contract test.

What deliberately does **not** exist:

* No new persisted setting. `inference-settings.json` is unchanged, and no model identifier is written to disk.
* No download, no URL fetch, no file probe, no process launch, no network call anywhere in the new code.
* No model-selector UI, no download manager, no catalogue, no second profile, no trait hierarchy or plugin system.
* No change to `tauri.conf.json`, capabilities, CSP, `models.rs`, `process.rs`, `settings.rs`, `client.rs`, `hardware.rs`, or `sse.rs`.

The single most important semantic rule, repeated in code and in this document: **a resolvable profile means "this metadata is known and self-consistent", never "the model is installed" and never "the model will run here".**

Verification is partial for the same reasons Phase 2 recorded: crates.io remains unreachable from the sandbox, so the crate still cannot be compiled or tested natively. `cargo fmt --all -- --check` exits 0; `cargo check`, `cargo test`, and `cargo clippy` cannot run. Frontend typecheck, 727 tests, and the production build all pass. Details and exact exit codes are in section 11.

---

## 2. Scope

In scope, and delivered:

1. Audit of the existing model path, validation, settings, supervisor, IPC, and frontend surfaces before any change.
2. A minimal typed profile for the Phase 3 approved candidate, with authoritative sources and explicit uncertainty.
3. A registry that resolves a stable ID, rejects unknown IDs with a typed error, and refuses duplicate or inconsistent entries.
4. A read-only IPC surface for that metadata, wired through the existing typed frontend layer and the source-contract test.
5. Tests for metadata, resolution, unknown IDs, duplicates, malformed metadata, and the frontend call shape.
6. A documented decision about settings/backward compatibility.
7. This report.

Out of scope, and not done:

* Downloading, bundling, converting, or shipping any GGUF file.
* Running the model, benchmarking it, or measuring memory, latency, or output quality.
* A model selector, download manager, catalogue, marketplace, or second model.
* Quota/reset logic, single-file/ZIP export, Download Mode, accounts, payments, licensing enforcement, telemetry, or the Founder Control Centre.
* Any roadmap change. Any Phase 5 work.

---

## 3. Baseline at the start of Phase 4

Read from the repository, not from previous prose:

```
$ git rev-parse --abbrev-ref HEAD
arena/18030bbb-nexora
$ git rev-parse HEAD
9bd4f7ad36ab012b50e48333c27fdc9b8244d5ff
$ git rev-parse main origin/main
9bd4f7ad36ab012b50e48333c27fdc9b8244d5ff
9bd4f7ad36ab012b50e48333c27fdc9b8244d5ff
$ git status --porcelain=v1
 M apps/desktop/src-tauri/src/auth/client.rs          # 15 Phase 2 files, uncommitted
 ...
?? docs/nexora-mini/
$ git diff --stat | tail -1
15 files changed, 945 insertions(+), 266 deletions(-)
```

* 15 modified Rust files from Phase 2, carried forward untouched.
* `docs/nexora-mini/` untracked, holding the three earlier reports.
* Nothing staged. No stash. No tags. No `Cargo.lock` anywhere in the tree or in history.
* `apps/desktop/ui/src/lib/*.ts` unmodified; `node_modules/` and `ui/dist/` ignored.

Phase 4 changed files only after this baseline was recorded.

---

## 4. Audit: what already existed

The design had to fit the machinery that was already there rather than replace it.

### 4.1 Settings

`src/inference/settings.rs`

| Item | Behaviour |
| --- | --- |
| `Settings` | `model_path: Option<String>`, `runtime_path: Option<String>`, `context_size: u32` |
| Serde | `#[serde(rename_all = "camelCase", default)]` — missing fields default, unknown fields are ignored |
| File | `inference-settings.json` in the Tauri `config_dir` |
| `normalised()` | trims whitespace and quotes; empty becomes `None`; context clamped to `MIN_CONTEXT_SIZE ..= MAX_CONTEXT_SIZE` |
| Bounds | `MIN_CONTEXT_SIZE = 2_048`, `MAX_CONTEXT_SIZE = 131_072`, `DEFAULT_CONTEXT_SIZE = 32_768` |
| `SettingsStore::load` | missing or corrupt file yields defaults rather than an error |
| `SettingsStore::save` | writes only what the user supplied |

There is **no** model identifier, no tier, no profile reference, and no registry pointer in this format. The user's model path and runtime path are theirs alone; nothing in Phase 4 writes to either.

### 4.2 Model validation

`src/inference/models.rs`

* `inspect(path, context_size, available_ram) -> ModelInfo` opens the file, requires a `.gguf` extension, at least 16 MiB, the `GGUF` magic, and a supported version (2–3).
* `looks_like_executable(path)` is the only executable-path check and is used by `probe_runtime`.
* The RAM estimate is a heuristic: `size + 0.5 + ctx/32768 × 1.5 + 1.0` GiB, with 2 GiB headroom.

The registry neither calls `inspect` nor duplicates it. A profile is metadata, and validation stays a separate, user-initiated action against a real file.

### 4.3 Supervisor

`src/inference/process.rs` keeps a single `RunningServer` (child, port, API key, key file, stderr tail). It launches llama.cpp's `llama-server` with a structured argument vector — no shell — on `127.0.0.1` with an ephemeral port and an API key file. `stop()` kills and polls for at most two seconds; `Drop` stops the child. There is no pool, no queue, and no second slot.

Phase 4 adds nothing to this file. Registering a profile cannot start, stop, or reconfigure the supervisor.

### 4.4 IPC and frontend

Ten inference commands were registered: `get_app_status`, `get_system_info`, `get_settings`, `save_settings`, `validate_model`, `probe_runtime`, `load_model`, `unload_model`, `start_generation`, `cancel_generation`.

`ui/src/lib/inference.ts` wraps each one 1:1. `ui/src/lib/ipc.ts` holds the shared wire types. `ui/src/lib/rustContract.test.ts` parses the Rust sources and the TypeScript sources and asserts that:

* every registered command is invoked by the UI,
* the argument keys match the Rust signatures,
* exactly five commands are argument-less, and
* four specific commands have pinned argument names.

That last assertion is the reason this phase had to touch the test as well as the wrapper: adding an argument-less command changes a pinned list. Nothing was loosened; the list gained one exact entry.

---

## 5. Profile design

### 5.1 Shape

```rust
pub struct ModelProfile {
    pub id: &'static str,
    pub display_name: &'static str,
    pub publisher: &'static str,
    pub upstream_model: &'static str,
    pub upstream_revision: &'static str,
    pub format: &'static str,
    pub quantization: &'static str,
    pub licence: LicenceRef,
    pub source: ModelSource,
    pub context: ContextGuidance,
    pub compatibility_notes: &'static [&'static str],
    pub locally_verified: bool,
}
```

All fields are static strings, numbers, and small copies. The struct is `Copy`, serialises as camelCase JSON, and holds no paths, no handles, and no user data. `LicenceRef`, `ModelSource`, and `ContextGuidance` are separate types so the serialised JSON is readable and each part can be validated on its own.

A flat record was chosen over a trait or a provider abstraction on purpose. There is one model, one runtime, and one process. A trait hierarchy would add extension points nobody asked for and would make "which model is this?" a dynamic question in a phase that only needs a static answer. A second profile, if approved later, is another constant in the same table.

### 5.2 Stable identity

```
nexora-mini-qwen2.5-coder-7b-instruct-q4-k-m
```

Lowercase, dot-and-hyphen only, and stable across releases: it names the product tier, the upstream family, the parameter count, the instruction tuning, and the quantisation. It is **not** derived from a file path, a machine, a user, or a version number, so it can be persisted later without migration. `is_stable_id` enforces the character set so a future entry cannot smuggle in a path separator or whitespace.

### 5.3 Approved values and where they come from

| Field | Value | Authority |
| --- | --- | --- |
| Display name | `Nexora Mini` | Phase 2/3 product decision |
| Publisher | `Qwen team, Alibaba Group` | Model card |
| Upstream model | `Qwen/Qwen2.5-Coder-7B-Instruct` | Phase 3 §6.1 |
| Upstream revision | `c03e6d358207e414f1eca0bb1891e29f1db0e242` | HF model repository revision recorded in Phase 3 |
| Format | `GGUF` | GGUF repository |
| Quantisation | `Q4_K_M` | Phase 3 matrix |
| Licence | `Apache-2.0` | `LICENSE` in the GGUF repository, 11,343 B, stock text, no NOTICE |
| Licence reference | `…/Qwen2.5-Coder-7B-Instruct-GGUF/blob/main/LICENSE` | Same file |
| Source repository | `Qwen/Qwen2.5-Coder-7B-Instruct-GGUF` | Phase 3 |
| Source revision | `13fb94bfda8c8cf22497dc57b78f391a9acb426a` | Repository commit |
| File name | `qwen2.5-coder-7b-instruct-q4_k_m.gguf` | Repository tree listing |
| Size | `4,683,073,536` bytes | Tree listing (`size`) |
| SHA-256 | `509287f78cb4d4cf6b3843734733b914b2c158e43e22a7f4bf5e963800894d3c` | Tree listing (`lfs.oid`) — read from host metadata, **not** computed locally |
| Recommended context | `32,768` | Publisher README for this file |
| Documented maximum | `32,768` | Same |
| Locally verified | `false` | No download, no run |

Two notes on provenance:

* The digest is the LFS object identifier Hugging Face publishes for that path. It is the SHA-256 of the file by that host's definition, but this project did not hash the bytes itself. When a download flow exists, it must recompute the digest and compare — Phase 5 work, not done here.
* The GGUF metadata advertises `context_length = 131,072`, while the publisher's README documents 32,768 and states that the longer window requires YaRN, which only vLLM supports. The profile records 32,768 and describes the higher figure as provisional in the compatibility notes. It does not silently adopt the metadata value.

### 5.4 Context: recommended vs documented maximum

`ContextGuidance` has two numbers rather than one, because they answer different questions:

* `recommended` — what Nexora should offer a user for this profile (32,768).
* `documented_max` — the ceiling the publisher claims for this exact file (32,768). Anything above it is provisional and unsupported.

Validation requires `0 < recommended <= documented_max <= MAX_CONTEXT_SIZE`, so a future profile cannot recommend more context than it documents, and cannot document more than the settings layer can store.

The profile does **not** change the user's context setting. `ModelSetup.tsx` still offers 8,192 / 16,384 / 32,768 / 65,536 and still saves whatever the user picks. A profile recommending 32,768 is guidance, not an override.

### 5.5 Runtime compatibility notes

Recorded as human-readable strings, because they are advice rather than machine-checkable facts:

1. The publisher documents 32,768 tokens for this file; the 131,072 in its metadata needs YaRN (vLLM only).
2. Use the single-file artefact; split GGUF shards are not supported (the repository also publishes split variants of the same quantisation, which this profile does not name).
3. Nexora passes no GPU-offload or KV-cache flags, so this profile runs on CPU.
4. Written against llama.cpp build 11714; another build is reported, not blocked.

These mirror the runtime facts in `process.rs` — no `-ngl`, no KV-cache flags, `llama-server` only — without changing them.

### 5.6 What the profile does not claim

`locally_verified: false` is a field, not a comment, so it travels to the UI. It stays false until a recorded run exists. There is no field for speed, memory, quality, or "works on Windows", because none of those were measured. Phase 3's numbers are not copied into the profile: a metadata record is the wrong place to launder benchmark figures into runtime guarantees.

---

## 6. Registry design

```rust
pub struct ModelRegistry { profiles: &'static [ModelProfile] }

pub static APPROVED: ModelRegistry = ModelRegistry::new(&PROFILES);
static PROFILES: [ModelProfile; 1] = [NEXORA_MINI];
```

### 6.1 Resolution flow

1. Caller passes an identifier (`APPROVED.resolve(id)`).
2. The identifier is trimmed. Empty or whitespace-only input returns `invalid_request`.
3. Profiles are scanned for an exact match on the stable ID.
4. No match returns `unknown_model`, quoting at most 80 characters of the caller's input.
5. More than one match returns `internal` — the registry is corrupt, and picking either entry would be arbitrary.
6. The matched profile is validated by `validate()`.
7. The validated profile is returned by reference with a `'static` lifetime, so callers cannot hold a borrow tied to a temporary.

The command layer calls `resolve(NEXORA_MINI_ID)` and copies the result. Because the argument is a constant, the UI cannot ask for an arbitrary profile; the error path exists for direct callers and for the future case where an identifier arrives from persisted state or a URL.

### 6.2 Validation gates

| Field(s) | Rule | Failure code |
| --- | --- | --- |
| `id` | non-empty; lowercase letters, digits, `.`, `-` only | `internal` |
| `display_name`, `publisher`, `upstream_model`, `format`, `quantization` | non-blank after trim | `internal` |
| `licence.id`, `licence.reference` | non-blank | `internal` |
| `upstream_revision`, `source.revision` | exactly 40 lowercase hex characters | `internal` |
| `source.repository` | non-blank | `internal` |
| `source.file_name` | ends with `.gguf` | `internal` |
| `source.size_bytes` | non-zero | `internal` |
| `source.sha256` | exactly 64 lowercase hex characters | `internal` |
| `context` | `0 < recommended <= documented_max <= 131_072` | `internal` |
| `compatibility_notes` | every entry non-blank | `internal` |

`internal` is the honest code: a malformed static table is a programming defect, not a user mistake, and it must never be presented as "the model is missing". Blank input is `invalid_request`; an unrecognised but well-formed ID is `unknown_model`.

The checks run at resolution time rather than only in tests, so a bad entry cannot reach the UI merely because it compiles.

### 6.3 Duplicates

Duplicate detection is structural: the scan stops at the second match. The table is `static`, so a duplicate is a source-code defect rather than a runtime race, but the guard means a future table built from any source still cannot silently resolve to "whichever came first". The test constructs a two-element slice containing the same profile twice and asserts `internal`.

### 6.4 What the registry cannot do

* It has no filesystem access: no `std::fs`, no `Path`, no `canonicalize`.
* It has no process access: no `Command`, no `Child`.
* It has no network access: no HTTP client, no URL construction from profile strings.
* It has no reference to `Settings`, so it cannot read or overwrite the user's model path.
* It cannot report installation state: nothing in it observes the disk.

---

## 7. IPC integration

### 7.1 Rust command

```rust
#[tauri::command]
pub fn get_model_profile() -> InferenceResult<ModelProfile> {
    registry::APPROVED.resolve(registry::NEXORA_MINI_ID).copied()
}
```

* Argument-less by design. There is no identifier parameter, no path parameter, and no flag, so the command cannot be used to probe a filesystem or to enumerate profiles.
* Synchronous and read-only: no `State`, no `AppHandle`, no async, no locks.
* Returns the typed error, which serialises as `{ code, message }` like every other inference command.
* Registered in `lib.rs` immediately after `get_settings`, so the inference command block stays contiguous.

`InferenceError::unknown_model` is new. Its `code` is a `&'static str` literal, matching the existing constructor pattern and the contract test that inspects every `InferenceError::…(` call site.

### 7.2 Frontend

`ui/src/lib/ipc.ts` gains a `ModelProfile` interface mirroring the Rust serialisation exactly: `id`, `displayName`, `publisher`, `upstreamModel`, `upstreamRevision`, `format`, `quantization`, `licence { id, reference }`, `source { repository, revision, fileName, sizeBytes, sha256 }`, `context { recommended, documentedMax }`, `compatibilityNotes`, `locallyVerified`.

`ui/src/lib/inference.ts` gains:

```ts
export function fetchModelProfile(): Promise<ModelProfile> {
  return call<ModelProfile>("get_model_profile");
}
```

The existing `call` helper already provides the two behaviours this needs: outside the desktop shell it rejects with `desktop_only` rather than attempting IPC; inside, it normalises transport failures through `toInferenceError`.

No component calls `fetchModelProfile` yet. That is deliberate — a model selector is out of scope — so there is no visual or behavioural change in the app. The wrapper is exported and type-checked so the next phase can consume it without re-plumbing.

### 7.3 Contract test

`rustContract.test.ts` pins the argument-less command set. The list now reads:

```
get_app_status, get_model_profile, get_settings, get_system_info, load_model, unload_model
```

Nothing else in the contract was relaxed: the parser still requires the bare `tauri::command` attribute, still requires every registered command to be invoked by the UI with identical argument keys, and still pins the four stateful command signatures. The test also verifies that `get_model_profile` is invoked with no argument object, which is why the wrapper passes `undefined` through `call` rather than a second argument.

---

## 8. Settings and backward compatibility

### 8.1 Decision

**No new persisted setting, and no schema change.**

Reasons, in order of weight:

1. One profile is approved. A registry with a single entry does not need a stored pointer: the command resolves the constant directly, and there is no ambiguous "which model" question to answer.
2. A stored identifier would create state that can go stale. A file written by a future build, then read by an older build, would carry a field the older build ignores — or worse, a build that trusts it would report a profile for a file that is not there.
3. The existing settings file describes the user's machine — where their model and runtime live, and how much context they want. A catalogue identifier is product metadata, not machine configuration. Mixing the two invites exactly the silent replacement this phase forbids.
4. Backward compatibility is then trivially preserved: the file format is untouched, so every existing file loads exactly as before, including files with unknown fields (ignored by `#[serde(default)]`) and files with missing fields (defaulted).

### 8.2 Consequences

* `inference-settings.json` is read and written by the same code as before; `settings.rs` has no Phase 4 edits.
* A user's configured model path, runtime path, and context are never read, rewritten, or "corrected" by the registry.
* An unknown or missing identifier is safe because no identifier is persisted. When the command resolves the constant, the only reachable failures are a corrupt static table (`internal`), which cannot happen at runtime without a code change.
* If a later phase persists a profile ID, that phase must add a migration that treats an unknown ID as "unset" and leaves the user's paths alone. That work is explicitly not done here.

### 8.3 Why the alternative was rejected

Adding `profile_id: Option<String>` with `#[serde(default)]` would have been mechanically backward-compatible — serde would ignore it in old files and default it in new ones. It was still rejected: it would encode a product decision in a user settings file, it would make the profile resolvable-but-unused in every existing install, and it would tempt a future change to *derive* the model path from the profile. The registry is better as a constant that the core owns.

---

## 9. Security boundaries

| Boundary | Position after Phase 4 |
| --- | --- |
| Local inference only | Unchanged. No cloud endpoint, no API key, no account, no telemetry in the new code or anywhere it touches. |
| No auto-download | Unchanged. The profile records a URL as text; nothing fetches it. A repository name in a struct is data, not a download instruction. |
| No arbitrary executable | Unchanged. `probe_runtime` still validates via `looks_like_executable`; the registry never touches executables. |
| No arbitrary path | Strengthened trivially: the new command accepts no input, so there is no path to validate. |
| Filesystem scope | Unchanged. No new capability, no new scope, no new directory access. |
| CSP / capabilities | Unchanged. `tauri.conf.json` and `capabilities/main.json` are untouched. |
| Process permissions | Unchanged. The supervisor is the only process launcher and was not modified. |
| Secrets | Unchanged. The profile contains no key, token, or user identifier. |
| Profile ≠ installed | Enforced by design: no disk access, no `AppStatus` change, `locally_verified: false`. |
| Load/unload/cancel/cleanup | Unchanged. The single-slot supervisor, the two-second kill/poll stop, and the `Drop` cleanup are exactly as Phase 2 left them. |

The one new information disclosure is intentional and public: the profile describes an open-weight artefact under Apache-2.0 that anyone can download. It reveals nothing about the user's machine.

---

## 10. Tests

### 10.1 Rust unit tests (17, in `registry.rs`)

| Test | What it pins |
| --- | --- |
| `mini_profile_matches_the_phase_3_recommendation` | Every approved value: ID, display name, publisher, upstream model and revision, format, quantisation, licence, repository, revision, file name, size, digest, context |
| `the_profile_does_not_claim_local_verification` | `locally_verified == false` |
| `the_approved_registry_contains_one_profile` | Exactly one entry is registered |
| `resolves_the_stable_id` | The constant resolves to itself |
| `resolution_trims_surrounding_whitespace` | Leading/trailing whitespace is tolerated |
| `rejects_an_unknown_id` | `unknown_model`, message quotes the input |
| `rejects_a_blank_id` | `invalid_request` |
| `rejects_duplicate_registrations` | Two identical entries → `internal` |
| `rejects_a_profile_without_a_display_name` | Blank descriptive field → `internal` |
| `rejects_a_profile_with_a_non_gguf_artefact` | `.bin` filename rejected |
| `rejects_a_profile_with_a_malformed_revision` | Short revision rejected |
| `rejects_a_profile_with_a_malformed_upstream_revision` | Empty upstream revision rejected |
| `rejects_a_profile_with_a_malformed_digest` | Empty, short, uppercase, and non-hex digests rejected |
| `rejects_a_profile_with_an_inconsistent_context` | Recommended > documented rejected |
| `rejects_context_beyond_the_settings_maximum` | Documented > 131,072 rejected |
| `rejects_an_empty_compatibility_note` | Blank note rejected |
| `serialises_with_stable_camel_case_keys` | JSON keys and the boolean are exactly what `ipc.ts` declares |

### 10.2 How those tests were executed

`cargo test` cannot resolve dependencies in this sandbox (section 11). To avoid claiming an unverified test suite, the module was compiled and executed standalone:

* A temporary harness (`/tmp/regcheck`, outside the repository) provides stub `error` and `settings` modules and includes a **copy** of `registry.rs` with the serde derive and attributes stripped — nothing in the repository was altered to make this work.
* `rustc --edition 2021 --test main.rs` → compiled cleanly.
* `./regtest` → **16 passed; 0 failed**. The serialisation test is the one excluded, because it needs `serde_json`.

This is evidence that the resolution and validation logic type-checks and behaves as described. It is **not** a `cargo test` run of the crate, and it is reported as such.

### 10.3 Frontend tests

| Test | File | What it pins |
| --- | --- | --- |
| `cannot read model metadata from a browser preview` | `inference.test.ts` | Rejects with `desktop_only`; `invoke` is never called |
| `reads the approved profile without sending an identifier` | `inference.test.ts` | Exact command name, `undefined` arguments, payload returned unchanged |
| `normalises a rejected profile lookup` | `inference.test.ts` | A typed core rejection survives the wrapper |
| Argument-less set includes `get_model_profile` | `rustContract.test.ts` | Rust registration and TypeScript invocation agree |

Totals after this phase: **75 test files, 727 tests, all passing** (was 724).

### 10.4 What is not covered

* No test executes the command through Tauri's IPC layer, because that requires a running desktop shell.
* No test opens a real GGUF file against the profile's expected size or digest. That belongs to a download/validation phase.
* No Windows-specific assertion. Nothing in this phase is platform-conditional, but that is an argument, not a test result.

---

## 11. Commands run and exact results

All commands were run from the repository on 2026-10-20.

| Command | Exit | Result |
| --- | --- | --- |
| `cargo fmt --all -- --check` | **0** | Clean. Executed with rustfmt 1.8.0-stable (1.88.0 toolchain); the repository pins 1.93.0, so this is version-skewed evidence |
| `cargo metadata --no-deps --offline` | **0** | Manifest parses |
| `cargo check --all-targets` | **101** | `error: failed to get futures-util as a dependency … failed to download from https://index.crates.io/config.json … [35] SSL connect error` |
| `cargo test` | **101** | Same dependency-resolution failure; no test binary built |
| `cargo clippy --all-targets -- -D warnings` | **101** | `error: no such command: clippy` — the component is not installed and cannot be fetched |
| `rustc --edition 2021 --crate-type lib --emit=metadata <file>` | n/a | Per-file parse check of `registry.rs`, `commands.rs`, `error.rs`, `mod.rs`: 0 syntax errors; only `E0432`/`E0433` from unresolved external crates and from `super::` in a single-file context |
| `rustc --edition 2021 --test main.rs` (harness) | **0** | 16/16 registry tests pass (see 10.2) |
| `pnpm typecheck` | **0** | `tsc --noEmit` clean |
| `pnpm test` | **0** | 75 files / 727 tests passed |
| `pnpm build` | **0** | `index-*.js` 522.59 kB (gzip 146.38 kB), `index-*.css` 93.42 kB (gzip 14.62 kB); Vite's pre-existing >500 kB chunk warning |
| `git diff --check` | **0** | No whitespace errors |

### Blockers, unchanged from Phase 2

| ID | Blocker | Effect |
| --- | --- | --- |
| B-1 | `index.crates.io`, `static.crates.io`, `crates.io`, and three public mirrors all fail at TLS (exit 101) | No dependency resolution; no real `Cargo.lock`; no native compilation |
| B-2 | Repository pins Rust 1.93.0; the sandbox has 1.95.0 and rustfmt 1.88.0, with no 1.93 package available anywhere reachable | Formatting evidence comes from 1.88; it is not the pinned toolchain |
| B-3 | No Windows host or runner available without pushing | Windows compilation and the Tauri desktop build remain unverified |
| B-4 | `clippy` component absent and unfetchable | Lint gate never executed |
| B-5 | No `Cargo.lock` was ever committed and CI publishes no artifacts | Dependency versions cannot be reproduced from the repository |

**Native compilation and the full Rust test suite remain unverified.** Formatting and parsing are not compilation, and this document does not treat them as such.

---

## 12. Files created and modified

Created:

| Path | Lines | Contents |
| --- | --- | --- |
| `apps/desktop/src-tauri/src/inference/registry.rs` | 447 | Profile types, approved constant, registry, validation, 17 tests |
| `docs/nexora-mini/PHASE_4_MODEL_REGISTRY.md` | this file | Phase 4 report |

Modified (Phase 4 only):

| Path | Change |
| --- | --- |
| `src/inference/mod.rs` | `pub mod registry;` |
| `src/inference/error.rs` | `unknown_model` constructor |
| `src/inference/commands.rs` | `get_model_profile` command and import |
| `src/lib.rs` | Command registered in `generate_handler!` |
| `ui/src/lib/ipc.ts` | `ModelProfile` interface |
| `ui/src/lib/inference.ts` | `fetchModelProfile()` wrapper and type import |
| `ui/src/lib/inference.test.ts` | Three tests and the fixture |
| `ui/src/lib/rustContract.test.ts` | `get_model_profile` added to the pinned argument-less set |

Net effect on the working tree: 23 tracked files changed (`+1,048 / −266`), plus one new Rust file and the untracked docs directory. The Phase 2 edits are untouched: no file was reverted, reformatted, or edited to accommodate this phase.

---

## 13. Confirmations

* **Branch and base unchanged.** Work is on `arena/18030bbb-nexora` at `9bd4f7ad…`. No commit, push, merge, tag, branch, or visibility change. No reset, clean, stash, revert, or rebase.
* **Earlier reports preserved.** `PHASE_1_AUDIT.md`, `PHASE_2_BUILD_RECOVERY.md`, and `PHASE_3_MODEL_EVALUATION.md` re-hash to the values in the header table after all Phase 4 edits.
* **Unrelated work preserved.** The 15 Phase 2 Rust files and all frontend files outside the four listed above are unchanged by this phase.
* **No weights.** Nothing was downloaded, converted, bundled, or written to disk. The largest file created is this report.
* **No inference.** No model was loaded, no process was started, no benchmark was run, and no output, speed, memory, or quality figure is claimed.
* **No capability change.** `tauri.conf.json`, `capabilities/main.json`, `Cargo.toml`, and the dependency set are untouched.
* **No Phase 5.** Download flow, digest verification, UI surfacing, and real-model validation are listed for a later phase and were not started.

---

## 14. Handover notes for the next phase

If Phase 5 is approved, the natural order is:

1. **Download and verify.** Fetch the single-file `q4_k_m` artefact, recompute SHA-256, compare with `profile.source.sha256`, and check the byte count. Refuse on mismatch. This is the first point at which the profile's identity fields become enforceable.
2. **Validate against the profile.** Compare the GGUF header's context length with `context.documented_max` and warn when they disagree, instead of adopting the larger number silently.
3. **Surface it.** Show `displayName`, `publisher`, `licence`, and the context guidance in the existing setup flow — behind an explicit user action, never as a substitute for `validate_model`.
4. **Earn `locally_verified`.** Set it only after a recorded run on the target hardware, with the build and settings recorded. Until then it stays `false`.
5. **Persist an ID only if needed.** If a second profile is ever approved, add the migration described in section 8.2 — unknown ID is unset, user paths untouched.

Open questions this phase deliberately left open: whether a second tier will exist, what the final minimum hardware claim is, and whether context above 32,768 will ever be offered. None of them are answered by inventing fields now.

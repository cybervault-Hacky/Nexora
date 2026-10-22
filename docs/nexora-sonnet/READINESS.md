# Hardware-aware model readiness

## 1. Scope and status

Nexora decides whether a model can be downloaded, verified, loaded, or used from **one
authoritative backend result per profile**. The Rust core measures the machine, compares the
published artefact with the configured context window, and returns a single state. The UI
renders that state; it does not compute one of its own.

| Milestone | Status |
| --- | --- |
| Hardware detection implemented and tested | **Implemented and exercised on Linux** (real `/proc/meminfo` and `statvfs`); Windows code written, never run |
| Readiness logic verified by automated tests | **Yes** — pure Rust engine, 39 unit tests, plus a frontend contract test that reads the Rust source |
| Genuine Sonnet inference on real hardware | **No** — this host has 3.5 GiB available against ≈ 10.9 GiB needed |

Nothing in this document claims that Sonnet was downloaded, loaded, or run. The measurements
quoted in §7 were taken on this Linux sandbox and are labelled as such.

## 2. What is measured, and what is not

### 2.1 Memory

| Platform | Source | Total | Available |
| --- | --- | --- | --- |
| Windows | `GlobalMemoryStatusEx` (`windows-sys`, `Win32_System_SystemInformation`) | `ullTotalPhys` | `ullAvailPhys` |
| Linux | `/proc/meminfo` | `MemTotal` | `MemAvailable` |
| Anything else | none | `None` | `None` |

Rules that the implementation holds to:

- **Total is never presented as available.** They are separate fields end to end
  (`totalRamGib`, `availableRamGib`), and every fit decision uses available.
- **A partial reading is unknown.** If either half cannot be read, the pair is treated as
  `MemoryStatus::Unknown`; a real total is never paired with an invented available figure.
- **Unknown is a first-class answer.** `None` means "not measured", and the UI prints
  "Detection unavailable" rather than a number.
- Values are rounded to 0.1 GiB for display only; comparisons use the unrounded value.

### 2.2 Free disk space

The measurement is taken at **the folder the model is actually written to**, not at the
process's working directory:

1. `start_model_download` creates the destination folder, then calls `disk_space(&directory)`.
2. `readiness_for` measures the default model folder (`<app data>/models`) or the folder of
   the configured `modelPath`.
3. `get_disk_space` measures the same folder for the hardware panel.

| Platform | Mechanism | Notes |
| --- | --- | --- |
| Linux, 64-bit | `statvfs` via a locally declared `#[repr(C)]` struct | No new crate; `f_bavail` (free to the user) is reported as free, `f_blocks` as total. Verified against `df -B1` (§7). |
| Windows | `GetDiskFreeSpaceExW` (`Win32_Storage_FileSystem`) | `lpFreeBytesAvailableToCaller` as free, `lpTotalNumberOfBytes` as total. Never executed here. |
| Other | none | `free_bytes: None` with a reason string. |

A path that does not exist yet — the normal case before a first download — is resolved by
walking up to its nearest existing parent (bounded at 64 hops). Nothing is created to
measure it. If no parent exists, the result is unknown.

Failure modes are explicit rather than silent: a failed `statvfs`/`GetDiskFreeSpaceExW` call,
a CString that cannot be built, a zero block size, or a platform without an implementation
all produce `free_bytes: None` plus `unavailable_reason`.

### 2.3 What is deliberately not detected

- **GPU / VRAM.** Nexora passes no offload flags, so VRAM does not change the decision.
- **Swap.** Not counted as usable memory; the number is the OS's "available" figure.
- **Peak memory of a running engine.** Not sampled; the estimate is arithmetic.
- **Other processes.** Impossible to do portably, and it would be stale by the time a load
  starts.

## 3. Estimates: method and constants

Every number below is derived from the **published artefact size** and the **configured
context size**. None is a measurement of this machine's free memory at load time.

```
weights_gib        = artefact_size_bytes / 2^30
kv_cache_gib       = kv_bytes_per_token × context_size / 2^30
runtime_overhead   = 1.0 GiB                      (RUNTIME_OVERHEAD_GIB)
estimate_gib       = weights_gib + kv_cache_gib + runtime_overhead
required_free      = artefact_size_bytes + 512 MiB (INSTALL_HEADROOM_BYTES)
memory_headroom    = 2.0 GiB                       (MEMORY_HEADROOM_GIB)
usable_available   = max(0, available_gib - memory_headroom)
```

`kv_bytes_per_token` comes from each model's own `config.json` (`num_hidden_layers ×
num_key_value_heads × head_dim × 2 (K and V) × 2 bytes`), not from a rule of thumb:

| Profile | Layers | KV heads | Head dim | Bytes/token | KV @ 8 192 | KV @ 32 768 |
| --- | --- | --- | --- | --- | --- | --- |
| Nexora Mini | 28 | 4 | 128 | 57 344 | 0.4375 GiB | 1.75 GiB |
| Nexora Sonnet | 48 | 8 | 128 | 196 608 | 1.50 GiB | 6.00 GiB |

Worked results at the configured context of 8 192:

| | Mini | Sonnet |
| --- | --- | --- |
| File | 4 683 073 536 B = 4.3615 GiB | 8 988 110 272 B = 8.3708 GiB |
| Free space required | 5 219 944 448 B = 4.86 GiB | 9 524 981 184 B = 8.87 GiB |
| Memory estimate | 5.80 GiB | 10.87 GiB |

At 32 768 tokens the same profiles estimate 7.11 GiB and 15.37 GiB respectively. This is why
there is **no single "minimum RAM" number**: the KV cache is a function of the context the
user configured, and it doubles/quadruples with it.

**Where the partial and temporary bytes are accounted for.** A download writes
`.<file name>.<random>.part` in the destination and renames it only after the length and digest
match. The temporary file therefore occupies the same number of bytes as the final artefact —
`install_bytes` — and the peak requirement during a download is that same figure plus the
512 MiB reserve, which is what `peak_download_bytes` and `required_free_bytes` report. When a
`.part` file actually exists, its current length is read from disk and reported as
`download_incomplete`; it is never added to the estimate as if it were installed.

Two further rules matter:

- **The context is read, never adjusted.** `settings.context_size` is used as-is. If a model
  would not fit at the configured context, the answer is "not enough memory", not a silently
  reduced context.
- **Only the cache is geometry-derived.** When a profile has no published KV geometry
  (`kv_bytes_per_token: None`), the cache falls back to the same blunt allowance the GGUF
  inspector uses (0.5 GiB + 1.5 GiB per 32k tokens) so the two checks cannot disagree. This
  allowance is labelled conservative, not exact.

Every figure is presented as an estimate in the UI, including in state messages that carry
the number ("An estimate, not a hard minimum.").

## 4. The readiness state machine

One state per profile, decided in this order (first match wins):

| # | State | When | Download | Verify | Load |
| --- | --- | --- | --- | --- | --- |
| 1 | `unavailable` | no approved artefact | no | no | no |
| 2 | `loaded` | engine running **this** profile and healthy | no | no | no |
| 3 | `runtime_failure` | engine tied to this profile reported an error | no | no | no |
| 4 | `loading` | engine starting this profile, not healthy yet | no | no | no |
| 5 | `downloading` | a download for **this** profile is in flight | no | no | no |
| 6 | `verification_failed` | same-named file of the wrong size, or a digest mismatch | no | see note | no |
| 7 | `download_incomplete` | `.part` file present, nothing installed | yes | no | no |
| 8 | `insufficient_memory` | installed + verified, but estimate > usable available | no | no | no |
| 9 | `downloaded_verified` | installed + verified, another engine running | no | no | no |
| 10 | `ready_to_load` | installed + verified, engine idle, memory fits | no | no | **yes** |
| 11 | `unverified` | exact name and length present, digest not checked this session | no | **yes** | no |
| 12 | `insufficient_storage` | free space < required | no | no | no |
| 13 | `detection_unavailable` | both disk and memory unknown | **yes** (warned) | no | no |
| 14 | `ready_to_download` | nothing installed, storage fits or is known | **yes** | no | no |

Notes on the less obvious rows:

- **`verification_failed` covers two different artefacts.** A wrong-length file at the
  published name and a digest mismatch both mean "this is not the approved file". The UI
  explains which. Neither is downloaded over: `publish` already refuses to overwrite.
- **A stale digest failure is not sticky.** The failure is reported only while the offending
  file still exists. Deleting it returns the row to `ready_to_download`; otherwise the only
  way out would be to restart the app.
- **Memory blocks loading, not downloading.** Before the file exists, a memory shortfall is a
  `warning` check (`ready_to_download`), because RAM can change and the download is still
  useful. Once the file is installed and verified it becomes `insufficient_memory`.
- **Unknown measurements do not block a download.** If both disk and memory are unknown the
  state says so plainly, and the download is still allowed with a warning. This is the only
  place where a blocked action would be worse than a warned one: the user may know their
  machine better than the platform API does.
- **Storage is only required before a download.** Once the file is installed, low free space
  does not make the model unloadable, so later states do not re-check it.
- **A failure is attributed only with evidence.** `runtime_failure` requires that the engine
  had recorded loading *this* profile; a failure is never pinned on whichever model happens to
  be selected. The readiness card points at the engine status for the runtime's own words
  rather than copying a stderr tail (which can contain paths) into a second place.

Only genuinely blocked actions are refused. Selecting a profile is never blocked; saving
paths is never blocked; the composer disables sending only when the loaded model is not the
selected one (the existing `selection_error`).

## 5. Where the decision is enforced

| Layer | Component | Role |
| --- | --- | --- |
| Detection | `inference/hardware.rs` | `system_info()`, `disk_space()`, `nearest_existing()` |
| Arithmetic | `inference/readiness.rs` | `estimate()`, `assess()`, state names, formatting |
| Command | `inference/commands.rs` | builds `ReadinessInput` from real files + real hardware; `get_model_catalog`, `check_model_readiness`, `get_disk_space` |
| Enforcement | `start_model_download` | re-measures free space after creating the folder, refuses with `insufficient_storage` before writing a byte |
| Enforcement | `load_model` | refuses an unverified published artefact (`model_unverified`) and a load the machine cannot hold (`insufficient_memory`) before spawning |
| Trust | `InferenceState::verified` | session ledger written only by `verify_model` and by a completed download, **keyed by the exact file path** that was hashed |
| UI | `ModelReadinessCard`, `ModelSelector`, `ModelSetup` | render the backend's state, headline, detail, estimate and checks |

Two invariants are worth stating separately:

1. **A file's presence is not "downloaded".** `installed_path` requires the published file
   name **and** exact byte length. A `.part` file is reported as `download_incomplete` and is
   never loadable. A wrong-length file is reported as `verification_failed`.
2. **"Verified" is never assumed.** The ledger is empty at process start, so after a restart
   an installed model reads `unverified` until its digest is confirmed again. `load_model`
   refuses to run an unverified published artefact; the setup flow hashes it once and retries,
   so this costs one delay per session, not an extra decision. Because the record is
   path-keyed, verifying one copy does not enable a different file with the same name in
   another folder.

## 6. UI behaviour

- **Chat composer** — the existing compact dropdown. Each option reads
  `Nexora Mini — <state label>`, and the line beneath is the backend's own sentence. A model
  can read "Ready to load" only when the core said `ready_to_load`.
- **Model manager** — one `ModelReadinessCard`: state badge, headline, explanation, model file
  size, free space required, memory estimate with the context it was computed at, each check
  with its status word (`OK` / `Warning` / `Blocked` / `Unknown`) and message, and a standing
  disclaimer that the numbers are estimates. The card shows "Reading…" or a read error rather
  than the last known state when the core cannot answer.
- **Hardware panel** — a "Model storage" row with free and total space at the model folder,
  or "Detection unavailable".
- **Download button** — disabled only when `canDownload` is false, with the backend's
  explanation as the tooltip.
- **Load** — unchanged in shape. If the core refuses with `model_unverified`, the setup flow
  hashes the file, shows the result, and retries once; a mismatch is reported and nothing is
  loaded.
- No new page, no new route, no separate Mini/Sonnet screen. The single chat interface and
  conversation state are untouched by a model change.

## 7. Measurements taken on this host

`inference/hardware.rs` was compiled and run directly to confirm the platform code, not just
the arithmetic:

```
measured memory: total=Some(3.8) GiB available=Some(3.5) GiB platform=linux
measured disk (repo): free=Some(19480596480) B total=Some(21834924032) B unavailable=None
```

A later run, after `pnpm install` had written `node_modules`, reported
`free=Some(19475763200) B` — the measurement is live, not cached.

Cross-checked against the OS:

```
$ df -B1 /home/user/Nexora
/dev/root  21834924032  2069114880  19480596480  10% /

$ free -b
Mem:  4131278848  346599424  2535354368  ...  3784679424
```

The `statvfs` values match `df -B1` exactly (free and total); the memory figures agree with
`free` to the 0.1 GiB rounding the module applies. The full readiness engine was then run
against those measurements at context 8 192:

```
Nexora Mini    -> ready_to_download   (5.80 GiB needed; 3.5 GiB available → warning)
Nexora Sonnet  -> ready_to_download   (10.87 GiB needed; 3.5 GiB available → warning)
both, if installed+verified -> insufficient_memory
```

This is a **readiness** result computed from real hardware. It is not an inference run.

## 8. Tests

### 8.1 Rust (41 readiness + 5 hardware tests)

Counts are the `#[test]` functions in `readiness.rs` and `hardware.rs` as found in the source on
2026-10-22. Earlier phase records quoted 39 for readiness; that figure is superseded.

- Conversions: bytes→GiB, GiB→bytes, saturation, negative/NaN input, display rounding,
  MiB formatting below 1 GiB.
- Estimates: Sonnet and Mini file sizes, KV cache at several contexts, unknown geometry
  falls back to a positive cache, required free space = file + headroom, peak ≥ install.
- States: clean machine, low disk, low memory before/after install, unknown disk, unknown
  memory, both unknown, missing artefact, exact-but-unverified, digest mismatch,
  wrong-size file, partial file, active download, loading, loaded, runtime failure, another
  model running.
- Actions: `can_download` / `can_verify` / `can_load` / `can_generate` follow the table in §4.
- Stable wire names: all 14 state names and 4 check statuses, uniqueness, snake_case.
- Verification ledger: a digest is tied to one path, another folder's copy does not match, a
  failure clears an earlier match and a match clears an earlier failure, and a fresh session
  starts unverified.
- Transitions: verifying the same file is the only change between `unverified` and
  `ready_to_load` (the estimate does not move), and a cancelled download returns to
  `ready_to_download` with nothing installed.
- Hardware: `/proc/meminfo` parsing, real `statvfs` values for a temp directory, a missing
  path never panics, nearest-existing-parent walk-up.

### 8.2 Frontend

- `ModelSelector`: option labels come from the backend state; a blocked Sonnet never reads
  "Ready"; the status line is the backend's detail; `data-state` carries the state.
- `ModelReadinessCard`: every state renders a label and badge; check statuses appear as words
  as well as colour; model size and required free space are shown separately; the disclaimer
  is always present; a read failure replaces the card rather than showing stale data.
- `ModelSetup`: readiness is rendered verbatim, a blocked download is disabled with the
  reason, the verify-then-retry load path is exercised for both a matching and a mismatching
  file.
- `readiness.test.ts` reads `readiness.rs` and asserts the frontend's state list equals the
  Rust `as_str()` output — a rename on either side fails the build's tests.
- `rustContract.test.ts` pins `get_disk_space` to a single `targetDir` argument and
  `check_model_readiness` to no arguments.

### 8.3 What the tests do not prove

- **No Windows execution.** The `GetDiskFreeSpaceExW` branch compiles only in a Windows
  build, which has never been produced here; no MSVC or WebView2 check exists.
- **No macOS execution.** It reports "unknown", which is a designed answer, not a tested one.
- **No real low-memory or low-disk conditions.** Those cases are simulated with fixtures.
  Fixtures prove the decision logic, not the hardware behaviour of a real 16 GB laptop.
- **No `cargo test`.** The crate's full dependency graph cannot be resolved in this sandbox;
  the modules above run in a harness with serde stubbed, and `commands.rs` is parse-checked
  only.
- **No genuine inference.** See §1.

## 9. Limitations to keep in mind

1. **Estimates, not guarantees.** The runtime overhead is a flat 1 GiB and the headroom is a
   flat 2 GiB. A real `llama-server` load can exceed these because of page cache, allocator
   behaviour, or the runtime's own scratch buffers.
2. **Available memory moves.** The value read before a load is not the value during a load;
   another application can take the memory in between. The check reduces the chance of a
   failed load; it cannot prevent it, and a failure is still reported by the supervisor.
3. **Disk measurement is per filesystem.** Free space at the model folder is what matters; a
   different volume is measured at its own path through `get_disk_space`.
4. **Session-scoped verification.** A digest checked in one session is not remembered after a
   restart, by design. The cost is one hash of a multi-GB file; the benefit is never claiming
   a file is the approved artefact because it was once checked.
5. **No GPU.** If a future build enables offload, these estimates become wrong in the
   optimistic direction for RAM and useless for VRAM.
6. **Readiness is not a promise.** `ready_to_load` means "every check the core can perform
   passed": exact length, verified digest, configured-context fit in available memory, engine
   idle. The load can still fail, and that failure is reported as `runtime_failure`.

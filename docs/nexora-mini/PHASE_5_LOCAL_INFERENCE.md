# Phase 5 — Local AI Inference Integration

Status: **implementation written and reviewed on the branch; native compilation,
genuine inference and Windows verification not demonstrated.**

| Area | State at the end of Phase 5 |
| --- | --- |
| Approved model profile | One profile, resolved by id, read-only IPC (`get_model_profile`) |
| Acquisition | Explicit, cancellable download with temp file, size gate and SHA-256 check |
| Integrity of the published digest | **Pending** — transcribed from host metadata, never recomputed here |
| Runtime/model compatibility | Modelled and checked in code; **no runtime was launched** |
| Chat integration | Existing supervisor and streaming client reused; no second process manager |
| Frontend | Setup panel with profile, download, verify and honest failure states |
| Cargo compile / `cargo test` | **Not run** — dependency resolution blocked; standalone harnesses ran 35 test functions, 26 of them new (see §11) |
| Genuine local inference | **Not demonstrated** — no GGUF, no `llama-server` on this machine |
| Windows build | **Not run** — no Windows host, no Windows target std installed |
| Phase 6 | Not started |

The single most important sentence in this report: the project now has a
complete, reviewed *path* from an approved profile to a verified file to a
running local model, but nothing in this environment ever put a real byte
through that path. Everything below distinguishes what was executed from what
was only written and read.

---

## 1. Starting state

Verified with `git status`, `git diff --stat`, `git rev-parse` before any Phase 5
edit:

- Branch: `arena/18030bbb-nexora`
- HEAD: `9bd4f7ad36ab012b50e48333c27fdc9b8244d5ff`
- 23 modified tracked files (+1,048 / −266) carrying the Phase 2 build-recovery
  fixes and their formatting
- Untracked: `apps/desktop/src-tauri/src/inference/registry.rs` (Phase 4) and
  `docs/nexora-mini/` (Phases 1–4)
- No `Cargo.lock`, no stash, no tags, no staged changes
- Nothing reset, reverted, cleaned or rebased at any point

The Phase 1–4 reports were not touched. Their digests at the end of Phase 5:

| File | Lines | Bytes | SHA-256 prefix |
| --- | --- | --- | --- |
| `PHASE_1_AUDIT.md` | 780 | 44,544 | `a40e7297…` |
| `PHASE_2_BUILD_RECOVERY.md` | 662 | 38,678 | `c1cbdfb7…` |
| `PHASE_3_MODEL_EVALUATION.md` | 706 | 49,782 | `9621f33a…` |
| `PHASE_4_MODEL_REGISTRY.md` | 512 | 32,585 | `68a12950…` |

## 2. What already existed and had to be preserved

Phase 5 is an integration phase, not a rewrite. The following were read in full
and reused unchanged in behaviour:

- `inference/process.rs` — the one and only process supervisor. A single
  `RunningServer` slot, argv array (no shell), loopback binding, ephemeral port,
  32-byte hex session key written to a `0600` file, `--parallel 1`, `--no-ui`,
  `--jinja`, `--timeout 600`, `CREATE_NO_WINDOW` on Windows, 180 s load timeout,
  64 KiB stderr tail, `PINNED_BUILD = "11714"` used as a *report-only* label.
- `inference/client.rs` — OpenAI-compatible `/v1/chat/completions` streaming
  over `no_proxy()` loopback, bearer auth, 600 s first-token timeout, 120 s idle
  timeout, 400,000-character response cap, typed status mapping.
- `inference/sse.rs` — line-buffered SSE parsing with a bounded buffer.
- `inference/models.rs` — GGUF header inspection (magic, version 2–3, metadata
  table), minimum size, RAM estimate, executable-path validation.
- `inference/settings.rs` — persisted `modelPath`, `runtimePath`,
  `contextSize` (default 32,768).
- `features/chat/*` — session-scoped streaming, cancellation, persistence.
- `ModelSetup.tsx` / `ModelManagerPage.tsx` / `EngineStatus.tsx` — the existing
  design system and state vocabulary.

No CSP, capability, permission or executable-path rule was relaxed. `git diff`
shows **no change** to `tauri.conf.json`, `capabilities/`, `package.json` or the
dependency versions used by the supervisor.

## 3. Architecture of the Phase 5 path

```
 registry::NEXORA_MINI (compile-time)
        │
        ├── get_model_profile  ──► UI reads display name, quantization,
        │                          licence, size, recommended context
        │
        └── acquisition::plan_for
                 │  url = https://huggingface.co/{repo}/resolve/{rev}/{file}
                 ▼
        start_model_download(target_dir?)          ← explicit user action only
                 │
                 ├─ refuse if <file>.gguf already exists   (already_exists)
                 ├─ write  .<file>.<rand>.part             (same directory)
                 ├─ incremental SHA-256 + 250 ms progress events
                 ├─ Content-Length must equal 4,683,073,536 before a byte is kept
                 ├─ received == size, digest == profile.sha256
                 └─ rename part → final (size re-check, never overwrite)
                          │
                          ▼
        ModelSetup prefills modelPath (no auto-save, no auto-load)
                          │
                 save_settings → models::inspect + looks_like_executable
                          │
                    load_model  (explicit)
                          │
                 process::Supervisor::start  ──► llama-server on 127.0.0.1:<ephemeral>
                          │                       with a fresh 32-byte key file
                 health poll (300 ms, 180 s)  ──► early-exit detection with stderr
                          │
                    start_generation (single slot)
                          │
                 client::stream_chat ──► Channel deltas ──► useConversation
```

Three properties are structural rather than conventional:

1. **One process manager.** Acquisition and generation both go through
   `InferenceState`; there is no second supervisor, no second HTTP client stack
   and no side-car daemon.
2. **One approved profile.** `get_model_profile` takes no arguments;
   `registry::APPROVED` holds exactly one entry and `resolve` rejects any other
   id with `unknown_model`.
3. **Registration is not installation.** Nothing in the registry is treated as
   present on disk. `get_model_profile` does not touch the filesystem, and the
   UI shows "Download Nexora Mini" until a file actually passes validation.

## 4. The approved profile

Source of truth: `apps/desktop/src-tauri/src/inference/registry.rs`.

| Field | Value |
| --- | --- |
| Profile id | `nexora-mini-qwen2.5-coder-7b-instruct-q4-k-m` |
| Display name | Nexora Mini |
| Upstream model | `Qwen/Qwen2.5-Coder-7B-Instruct` |
| Upstream revision | `c03e6d358207e414f1eca0bb1891e29f1db0e242` |
| GGUF repository | `Qwen/Qwen2.5-Coder-7B-Instruct-GGUF` |
| GGUF revision | `13fb94bfda8c8cf22497dc57b78f391a9acb426a` |
| Artifact | `qwen2.5-coder-7b-instruct-q4_k_m.gguf` |
| Size | 4,683,073,536 bytes (4.361 GiB) |
| SHA-256 | `509287f78cb4d4cf6b3843734733b914b2c158e43e22a7f4bf5e963800894d3c` |
| Licence | Apache-2.0 (stock text, 11,343 bytes, no NOTICE in the repository) |
| Documented max context | 32,768 |
| Recommended context | 32,768 |
| Locally verified | `false` |

Why this artifact, in one paragraph: the 7B Q4_K_M is the largest quantization of
the smallest approved family that fits a 16 GB Windows laptop with a 32K window
and room for the operating system; the file is single-shard (Nexora does not
support split GGUF), the licence is a plain Apache-2.0 with no field-of-use
restrictions, and the architecture (`qwen2`) is one the chosen runtime has
supported since long before the advisory build label. Phase 3 contains the
full comparison against 14B and DeepSeek-Coder-V2-Lite; nothing in Phase 5
changes that decision.

## 5. Acquisition

### 5.1 What the code does

`acquisition::plan_for` derives the download URL from the *compiled-in* profile:

```
https://huggingface.co/Qwen/Qwen2.5-Coder-7B-Instruct-GGUF/resolve/13fb94bf…/qwen2.5-coder-7b-instruct-q4_k_m.gguf
```

There is no user-supplied URL, no host parameter and no "paste a link" field.
The host, revision, file name and length all come from the registry, so the
download cannot silently drift to a different file.

`start_model_download(app, state, target_dir, on_event)`:

- creates the destination directory (`create_dir_all`);
- defaults to `app_data_dir()/models`; an explicit directory must be absolute;
- refuses with `already_exists` if the final file name is present — **the UI
  never overwrites an existing model**;
- takes a single download slot (`state.download`), so two downloads cannot race;
- returns `busy` if a download is already running;
- clears the slot on success, failure and cancellation alike.

`receive` writes to `.<name>.<16 hex chars>.part` in the **same directory**, so the
final rename is atomic on one filesystem. Before writing a byte it compares the
HTTP `Content-Length` with the approved size; a captive portal, an HTML error
page or a truncated object is rejected with `integrity_failed` and nothing is
kept. While streaming it:

- hashes each chunk incrementally (no second pass over 4.7 GiB);
- rejects a body longer than the approved size;
- emits `progress` at most every 250 ms and always on the final chunk;
- flushes and `sync_all`s the file before the rename.

After the stream ends it requires `received == size`, sends a `verifying` event,
compares the digest, and only then calls `publish`, which re-checks the length,
re-checks that the destination is still free, and renames. Any error, cancel or
digest mismatch removes the `.part` file. A cancelled download returns
`{ path: null, cancelled: true }` and leaves **nothing** behind — an interrupted
download can never look complete.

`verify_model(path)` hashes a file the user already has and returns an
`IntegrityReport` (`sizeMatches`, `digestMatches`, expected and actual values).
It never imports the file, never changes the configured path and never fails
just because the digest differs — the UI needs to *explain* a mismatch, not
crash on it.

### 5.2 What the UI does

`ModelSetup` gained an "Approved model profile" panel:

- profile heading, `Q4_K_M · Apache-2.0` chip, and
  `Qwen/Qwen2.5-Coder-7B-Instruct · 4.36 GiB · recommended context 32,768 tokens`;
- **Download Nexora Mini** → **Downloading…** with
  `Downloading 1.72 of 4.36 GiB (39%).`, then
  `Verifying the SHA-256 before publishing the file…`;
- **Cancel download** while active; the result reads
  `Download cancelled. Nothing was installed.`;
- **Verify my file** → **Hashing…** → one of three honest outcomes:
  - match: `This file matches the approved Nexora Mini SHA-256.`
  - same size, different digest: `The size matches, but the SHA-256 differs. This
    is not the approved Nexora Mini file.`
  - different size: `This file is X GiB; the approved Nexora Mini file is Y GiB.
    It may still be a valid GGUF, and Nexora will treat it as a custom model.`
- on success the download **prefills** the path field and says
  `Downloaded and verified against the approved SHA-256. Review the path, then
  save and load.` It does not save, does not load, and does not start anything.

Every download control is gated by `isDesktopShell()`; in the browser preview the
buttons are disabled and no IPC is attempted. Nothing runs on mount. The lead
copy states the policy: *"Downloads are explicit and verified; nothing is
fetched at startup."*

### 5.3 What was deliberately not built

- **No runtime download.** `llama-server` remains a user-supplied executable,
  validated by `looks_like_executable` and `probe_runtime`, exactly as before.
  Adding a runtime download would have required a second integrity pipeline and
  a distribution decision that is not approved.
- **No bundling.** No weights, no binaries, no installer payload changes.
- **No background or startup download**, no "download and load" shortcut.
- **No auto-selection of the downloaded path.** The user still presses Save.
- **No range/resume support.** An interrupted download restarts; the partial
  file is deleted, so there is no false resume state.
- **No free-space preflight.** A full disk surfaces as a typed `storage` error
  and the `.part` file is removed. A preflight check is listed as open work.
- **No proxy configuration.** The downloader uses `reqwest` defaults; proxies
  were not exercised in any environment.

### 5.4 Why no download was actually performed here

Downloading 4.68 GB would have been pointless and misleading in this sandbox:

1. There is no `llama-server` binary. A released Windows build could not run on
   this Linux sandbox, and building one here is not possible (no CMake/compiler
   stack and no crates.io).
2. The Rust downloader has never been compiled, so running it would have meant
   writing a separate ad-hoc script — proving that *curl* can fetch a file, not
   that Nexora can.
3. There is no second source against which to confirm the recorded digest, so a
   successful fetch would not have closed the integrity question either.

The honest status is therefore: **the acquisition code is reviewed and
parse-checked, and has never executed.** `locally_verified` stays `false`.

## 6. Integrity status

| Question | Answer |
| --- | --- |
| Is there a pinned file name and byte length? | Yes |
| Is there a full 64-hex SHA-256 in the registry? | Yes |
| Was that digest computed on a file in this environment? | **No** |
| Where did it come from? | The hosting repository's published LFS metadata, transcribed into `registry.rs` |
| Does the app verify downloads against it? | Yes, before the rename |
| Would a transcription error be caught? | **No** — a wrong constant would reject the correct file and accept a corrupted one that happens to match the typo |

This distinction matters. The download path protects against truncation,
transport corruption and a swapped file *relative to the recorded constant*. It
does not independently prove that the constant is the publisher's digest.

To close it, someone with a trusted network and 5 GB of disk must either:

1. download the file once, run `sha256sum` (or `Get-FileHash -Algorithm SHA256`)
   on it, and compare with `509287f7…`; or
2. read the `lfs.oid` field from the Hugging Face API *and* an independent mirror
   or the publisher's own checksum file, and record both sources.

Until then the registry, the UI copy and this document use the word
"approved", never "verified by Nexora", and `locally_verified` remains `false`.
No characters were guessed or padded; the digest is either exactly the value
above or it is wrong, and the flag says which state we are in.

Licence note for later distribution: the weights are Apache-2.0 and the
repository ships the stock licence text with no `NOTICE`. Apache-2.0 permits
redistribution with the licence and notices; the runtime (llama.cpp) is MIT and
would require its licence text if a binary were ever bundled. Neither is
bundled today, so no new licence obligation is created by Phase 5 — but any
future "download the runtime for me" feature must revisit this.

## 7. Runtime and model compatibility

### 7.1 What the combination is

| Property | Value | Where it comes from |
| --- | --- | --- |
| GGUF architecture | `qwen2` | GGUF metadata; llama.cpp architecture table |
| Layers | 28 | upstream `config.json` |
| Hidden size | 3,584 | upstream `config.json` |
| Attention | 28 Q heads / 4 KV heads, head dim 128 | upstream `config.json` |
| Vocabulary | 152,064 | upstream `config.json` |
| Positions at native scale | 32,768 (`rope_theta` 1e6, no `rope_scaling`) | upstream `config.json` |
| Context advertised in GGUF metadata | 131,072 | GGUF metadata |
| Context the publisher documents for this file | 32,768 (YaRN beyond that is vLLM-only) | GGUF repository README |
| Quantization | Q4_K_M | file name and registry |
| Runtime | any `llama-server` that reports a recognizable build; `11714` is advisory | `process.rs` |

Nexora therefore offers **32,768**, not 131,072. The registry records both
numbers (`recommended` and `documented_max`) and a compatibility note explains
the difference. `load_model` additionally reads the file's own metadata and
refuses a request larger than the model reports:

> "This model file reports a maximum context of {maximum} tokens, but Nexora is
> set to {}. Lower the context window before loading."

For this profile that check passes at 32,768 and would fail at 65,536, which is
the intended outcome: the 131,072 in the metadata is not usable without YaRN,
and Nexora passes no such flag.

### 7.2 Memory

KV cache is `2 × layers × kv_heads × head_dim × 2 bytes` per token:
`2 × 28 × 4 × 128 × 2 = 57,344` bytes/token = 56 KiB/token (f16, llama.cpp
default; Nexora passes no `--cache-type-k/v`).

| Context | KV cache | Weights | Typical runtime + buffers | Approx. total |
| --- | --- | --- | --- | --- |
| 8,192 | 0.44 GiB | 4.36 GiB | ~1.0 GiB | ~5.8 GiB |
| 16,384 | 0.88 GiB | 4.36 GiB | ~1.0 GiB | ~6.2 GiB |
| 32,768 | 1.75 GiB | 4.36 GiB | ~1.0 GiB | ~7.1 GiB |

`models::inspect` uses a deliberately crude estimator:
`weights + (0.5 + ctx/32768 × 1.5) + 1.0` GiB, and requires
`estimated ≤ available − 2.0`. At 32K that is **7.36 GiB required and 9.36 GiB
available**, which is conservative for this file (the real KV figure is
1.75 GiB, not 2.0 GiB). On a 16 GB machine with a browser open, available RAM is
often 9–12 GiB, so 32K should pass and 8K definitely will.

That is a statement about *this* combination on a *typical* 16 GB machine, not a
promise that every 16 GB machine runs every context. A 16 GB laptop with 8 GiB
already committed, or a machine whose integrated GPU steals memory, will be told
to lower the context — and the UI already surfaces the precise numbers. Nexora
never claims 32K when the model file, the runtime and the available memory do
not all agree.

### 7.3 What was not verified

- The runtime was never executed, so no claim is made that a specific build
  loads this file, that prompt prefill behaves as expected, or that generation
  reaches 32K without degradation.
- `PINNED_BUILD = "11714"` is used only in a compatibility message. The label is
  real: `api.github.com/repos/ggml-org/llama.cpp/releases/tags/b11714` returns
  HTTP 200 for tag `b11714`, targeting commit
  `c0fd78626405c6d9b934efef0ec7291fcc20e3d6` (published 2026-10-17), with 36
  assets including `llama-b11714-bin-win-cpu-x64.zip` (19,662,606 bytes). But it
  is marked **prerelease**, and no binary from it exists on this machine, so
  nothing about its runtime behaviour is verified here.
- No GPU flag is passed and no GPU is assumed. CPU-only is the supported path.

## 8. Failure model

### 8.1 Typed errors

| Code | Typical trigger | User-visible consequence |
| --- | --- | --- |
| `unknown_model` | profile id not in the registry | profile panel shows the fallback copy |
| `not_configured` | no model path, no runtime path, empty verify path | "Choose …" message |
| `invalid_model` | wrong extension, too small, bad magic/version, unreadable metadata, context above the file's maximum | validation panel; load refused before spawn |
| `already_exists` | destination file already present | "Nexora never overwrites a model file" |
| `integrity_failed` | wrong length, over-long body, digest mismatch | "Nothing was installed" |
| `spawn_failed` | executable rejected or process did not start | load error |
| `load_failed` | early exit during load, or 180 s timeout | reason + stderr tail |
| `not_running` | generation or unload without a loaded model | "Load a local model before sending a message." |
| `not_ready` | health check fails before generation | "The local engine is not responding. Reload the model." |
| `busy` | second generation or second download | refused, not queued |
| `auth_failed` | runtime rejects the session key | no silent retry |
| `invalid_request` | malformed request, non-absolute download folder | rejected before work starts |
| `engine_rejected` | other 4xx/5xx from the runtime | status + first 300 chars |
| `timeout` | first token > 600 s or idle > 120 s | generation fails, partial text kept |
| `protocol` | unparseable stream / SSE | generation fails |
| `network` | download transport error | typed error, `.part` removed |
| `storage` / `io` | filesystem failure | typed error |
| `internal` | poisoned mutex, join failure | "Restart Nexora" |
| `desktop_only` (frontend) | download/verify called in a browser | buttons disabled, no IPC |

`registration ≠ installed ≠ runnable` is preserved end to end: the profile is
always displayed, the "installed" claim comes only from `models::inspect` on a
real file, and "runnable" comes only from a successful `load_model`.

### 8.2 Loading

`load_model` is explicit and idempotent in the safe direction: it always stops
any existing process first, so a changed model or context actually takes effect.
It reads and validates the GGUF *before* spawning anything, compares the file's
metadata context with the requested context, then starts the supervisor and
polls `/health` every 300 ms for 180 s. On each poll it also asks the supervisor
whether the child exited; if it did, the error carries the captured stderr tail
and the process is stopped. On timeout the same diagnostics are returned and the
process is stopped. A successful load returns a full `AppStatus` snapshot.

### 8.3 Generation

`start_generation` validates the request, requires a running supervisor,
performs a health check, claims the single active-generation slot (`busy` if
taken), and streams through `client::stream_chat`. Deltas go to the UI over a
`Channel`, tagged with the request id. The client header/status mapping turns
401/403 into `auth_failed`, 503 into `not_ready`, 400/404/422 into
`invalid_request`, and everything else into `engine_rejected`.

The UI sends `max_tokens: 1024` and `temperature: 0.2` for every turn; the
server clamps the ceiling to 8,192 and rejects a request that exceeds the
conversation limits (64 turns, 100,000 chars per turn, 400,000 chars total).
There is no `response_format` or grammar constraint, and no workspace file
context is injected — the model sees the system prompt plus the conversation.

On the UI side (`useConversation`) the lifecycle is:

- a user message and an empty `streaming` assistant message are committed
  together, so the reply has a place to land before the first token;
- every delta is checked against the live request id *and* the live conversation
  id before being appended — a late delta cannot resurrect a reply in a chat the
  user has left;
- completion with text marks the message `complete`;
- completion with **no text** removes the empty assistant bubble and shows
  "The model finished without producing any text." — it never invents a reply;
- an error after partial text marks the message `error` with the failure text and
  keeps what arrived; an error before any text removes the empty bubble;
- cancellation marks the reply `stopped` and never fabricates content; the
  persistence layer maps a still-`streaming` message to `stopped` on save/load,
  and `toChatTurns` excludes `error` and empty turns from the next request.

`stop()` calls `cancel_generation(request_id)`, which flips the atomic flag and
notifies the client. The stream future returns a `cancelled` outcome and the
response is dropped. The code comments state that the pinned runtime stops
generating for that slot when the connection closes; that behaviour was **not**
observed here, because no runtime was run. No `/slots/{id}` call is made — the
same comments record that slot actions are rejected without
`--slot-save-path`.

If the browser window dies mid-stream, `mountedRef` prevents state writes and
skips the `onSettled` IPC round trip. If the runtime dies mid-stream, the write
fails and the user gets a typed error with whatever text had arrived. If the
runtime dies between generations, the health gate returns `not_ready` before a
request is sent.

### 8.4 Unloading

`unload_model` cancels any active generation, stops the child, and clears
`last_error`. It is only reachable from the engine indicator when the state is
`ready` or `generating`, and it is explicit. Because the desktop app is
single-user and single-slot, "never silently unload another user's active work"
reduces to: no timer, no memory-pressure watchdog and no automatic unload exists
in this codebase. Loading a new configuration does stop the previous process —
that is the user's explicit action and is documented in the UI copy.

## 9. Security posture (unchanged)

- Tauri CSP, capabilities and permissions: **no diff**.
- No shell: the supervisor passes an argv array to `Command`, never a string.
- Loopback only, ephemeral port, `--no-ui`, `--parallel 1`.
- Fresh 32-byte hex key per launch, written with `0600`, passed via
  `--api-key-file`, sent as a bearer token, never logged.
- Executable path validation unchanged (`looks_like_executable`, then
  `probe_runtime --version` before load).
- The download URL is a compile-time constant built from the registry; there is
  no URL, host or header supplied by the renderer.
- The downloader logs status codes, byte counts and digests — never the session
  key, never a credential. There are no credentials to leak: the request is an
  anonymous HTTPS GET to a public repository.
- `reqwest` follows redirects with its default policy; the content-length gate
  and the digest are the real guarantees, not the initial host name. A future
  hardening step could pin the expected CDN host, but that would trade
  robustness for a rule the publisher does not guarantee.
- **New in Phase 5:** `reqwest` gained the `rustls-tls` feature. Before this
  phase the crate had no TLS backend at all, because it only ever spoke to
  loopback. `rustls` was chosen over `native-tls` so the Windows build does not
  acquire an OpenSSL dependency. Certificate verification is at its default
  (on); there is no `danger_accept_invalid_certs`, no custom root store and no
  certificate pinning. The cost is a larger dependency graph (`rustls`, `ring`,
  root certificates) with no `Cargo.lock` to pin it — see B-5 — and a Windows
  MSVC build of `ring` that this environment cannot exercise.

## 10. Files

### Added on this branch (all still uncommitted)

| File | Lines | Purpose |
| --- | --- | --- |
| `src-tauri/src/inference/registry.rs` | 447 | One approved profile, validation of its own constants, 17 tests (created in Phase 4) |
| `src-tauri/src/inference/acquisition.rs` | 482 | URL plan, streaming download, temp file, publish, verify, 7 tests (Phase 5) |
| `docs/nexora-mini/PHASE_5_LOCAL_INFERENCE.md` | this file | Phase 5 report |

### Modified

| File | Change |
| --- | --- |
| `src-tauri/Cargo.toml` | `reqwest` gains `stream` and `rustls-tls`; `tokio` gains `fs` and `io-util` (already-enabled features otherwise unchanged) |
| `src-tauri/src/inference/models.rs` | GGUF metadata table parser, context length, architecture, display name; 10 new tests |
| `src-tauri/src/inference/commands.rs` | `start_model_download`, `cancel_model_download`, `verify_model`; context pre-check in `load_model` |
| `src-tauri/src/inference/error.rs` | `integrity_failed`, `already_exists`, `unknown_model` |
| `src-tauri/src/inference/mod.rs` | registers `acquisition`; download slot on `InferenceState` |
| `src-tauri/src/lib.rs` | three new IPC handlers |
| `ui/src/lib/ipc.ts` | `ModelDownloadEvent`, `ModelDownloadOutcome`, `IntegrityReport`, `ModelProfile`; `ModelInfo` gains `architecture`, `name`, `contextLength` |
| `ui/src/lib/inference.ts` | `fetchModelProfile`, `startModelDownload`, `cancelModelDownload`, `verifyModel` with browser fallbacks |
| `ui/src/components/ModelSetup.tsx` | approved-profile panel, download/verify state machine, honest notes and errors |
| `ui/src/styles/app.css` | `.setup__mini*` panel styles |
| `ui/src/pages/ModelManagerPage.tsx` | copy mentions the download |
| test files | `ModelSetup.test.tsx` (+5, 17 total), `inference.test.ts` (+6, 29 total), `rustContract.test.ts` (new pins), `ModelManagerPage.test.tsx`, three `App.*` fixtures |

No production string literal outside the new setup panel changed; the remaining
diff in `App.*.test.tsx` is the added `name`/`architecture`/`contextLength`
fields on `ModelInfo` fixtures.

## 11. What was actually executed

Environment: Linux sandbox, `rustc`/`cargo` 1.88.0 (repo pins 1.93.0), rustfmt
1.8.0-stable, no clippy, no crates.io access, no llama.cpp, no GGUF, no Windows.

| Check | Command | Result |
| --- | --- | --- |
| TypeScript types | `pnpm typecheck` | **exit 0** |
| Full frontend suite | `pnpm test` | **75 files / 739 tests passed** (97.95 s) |
| Setup panel | `pnpm vitest run src/components/ModelSetup.test.tsx` | **17/17** |
| Inference IPC wrappers | `pnpm vitest run src/lib/inference.test.ts` | **29/29** |
| Model manager page | `pnpm vitest run src/pages/ModelManagerPage.test.tsx` | **4/4** |
| Rust/TS contract pins | `pnpm vitest run src/lib/rustContract.test.ts` | pass (inside the 739) |
| Production build | `pnpm build` | **exit 0**; pre-existing >500 kB chunk warning |
| Rust formatting | `cargo fmt --all -- --check` | **exit 0** (rustfmt 1.88.0, version-skewed against the 1.93.0 pin) |
| Rust dependencies | `cargo check --all-targets` | **exit 101** — `failed to get 'futures-util'`; `index.crates.io/config.json` → `[35] SSL connect error` |
| Rust tests | `cargo test` | **exit 101** — same dependency fetch failure |
| Clippy | `cargo clippy --all-targets -- -D warnings` | **exit 101** — `no such command: clippy` |
| Parse check | `rustc --edition 2021 --sysroot … --emit=metadata` on `models.rs`, `commands.rs`, `registry.rs`, `acquisition.rs` | only `E0432`/`E0433` (missing crates / `super`) and no syntax diagnostics |
| `models.rs` tests | standalone `rustc --test` harness in `/tmp/modcheck` | **19/19 passed** |
| `registry.rs` tests | standalone harness in `/tmp/regcheck` | **16/16 passed** (the serde serialisation test was excluded: no `serde` available) |
| `acquisition.rs` tests | — | **not executed** (needs `reqwest`, `sha2`, `getrandom`, `tokio`) |
| Runtime release provenance | `GET api.github.com/repos/ggml-org/llama.cpp/releases/tags/b11714` | **HTTP 200**; tag exists, `prerelease: true`, target `c0fd786…`, 36 assets |
| Whitespace sanity | `git diff --check` | **exit 0** |

The registry harness is a copy of `registry.rs` with serde derives and the one
serde-dependent test stripped, compiled against a two-field stand-in for
`InferenceError`. It executes the real profile-validation logic; it does not
prove that `serde` derives compile, that Tauri commands are wired, or that the
crate builds. Those remain in the "not run" column.

New Rust tests written in Phase 5: **34** (17 registry, 7 acquisition, 10
models). New or extended frontend tests: **11+** plus contract pins.

Of those 34, **26 ran** in the standalone harnesses (16 registry + 10 models,
alongside the 9 pre-existing `models` tests). **Eight did not run**: the seven
acquisition tests, which need external crates, and the one registry
serialisation test, which needs `serde`.

## 12. Genuine inference

**No real model was loaded and no real token was generated in Phase 5.**

Evidence for that statement:

- `find /home/user -name '*.gguf'` returns nothing, and neither `llama-server`
  nor `llama-cli` is on the machine. (A 441 KB synthetic random-weight GGUF was
  used for parser tests in Phase 1; it is not in the workspace, is below the
  16 MiB floor and was never loaded.)
- There is no `llama-server` binary on this machine, no build toolchain and no
  network path to a release archive.
- The 4.68 GB approved artifact was not downloaded (see 5.4).
- The Rust crate has never been compiled here, so even the downloader that would
  fetch the file has not run.

What would count as proof, and has not happened:

1. `cargo build` succeeds with the pinned toolchain and a real `Cargo.lock`.
2. The download completes, `sha256` matches on the real file, and
   `locally_verified` flips to `true` on an actually computed digest.
3. `load_model` starts `llama-server`, `/health` returns ready within 180 s, and
   the GGUF metadata context is accepted at the configured window.
4. A chat message produces streamed deltas that visibly originate from the model
   (not fixture text), the assistant message completes, and the text is
   plausible for the prompt.
5. The same run is repeated after a cancel, after killing the runtime, and with
   an idle timeout, to show the error paths with a real process.

Until all five are recorded, the correct claim is "integration implemented and
unit-tested where the environment permitted; inference unproven".

## 13. Blockers

| Id | Blocker | Exact evidence | Consequence |
| --- | --- | --- | --- |
| B-1 | crates.io unreachable | `cargo check` → `failed to download from https://index.crates.io/config.json` → `[35] SSL connect error`; three mirrors also return `000` | No Rust compile, no Rust tests, no clippy, no `Cargo.lock` |
| B-2 | Toolchain skew | sandbox 1.88.0 vs `rust-toolchain.toml` 1.93.0; rustfmt 1.8.0-stable | Formatting is version-dependent; CI's 1.93 rustfmt may differ |
| B-3 | No Windows host | no Windows machine or runner; only `x86_64-unknown-linux-gnu` std installed; 20 Windows targets are listed but have no std | No MSVC/`tauri build`/installer verification |
| B-4 | No clippy | `cargo clippy` → `no such command`; cannot be installed offline | Lint gate unknown |
| B-5 | No `Cargo.lock` | zero commits ever touched either path; CI publishes no artifacts | Dependency resolution is floating; builds are not reproducible |
| B-6 | No runtime or weights | no `llama-server`, no GGUF, no way to fetch a release | Genuine inference and end-to-end download unproven |
| B-7 | Digest unconfirmed | `locally_verified: false`; single source | See §6 |

Everything else in this phase was written *around* these blockers rather than
pretending they were solved.

## 14. Open questions

1. **Integrity provenance.** Who computes the authoritative digest, and which
   second source is acceptable? HF's `lfs.oid` and the publisher's own metadata
   are the same source; an independent mirror or a manual `Get-FileHash` on a
   trusted machine is needed.
2. **Runtime distribution.** Today the user supplies `llama-server`. Shipping or
   auto-downloading it raises code-signing, MIT-licence-notice and update-policy
   questions that are not decided.
3. **Build label.** `11714` is a real tag, but it is a prerelease that was three
   days old at the time of writing. Pinning product copy to a moving prerelease
   is fragile; the support statement should name a stable release range and the
   app should report the build parsed from the user's binary.
4. **Context on 16 GB.** 32K is comfortable for this file on a clean 16 GB
   machine; the product still needs a documented recommendation for machines
   with less available RAM, and a preflight free-disk check for the 4.7 GB
   download.
5. **Patch-format reliability.** Generation sends a plain system prompt and no
   `response_format`/grammar. If Nexora later asks the model for strict patch
   syntax, unconstrained decoding will be the weakest link; a GBNF/JSON-schema
   constraint is the obvious mitigation and is not implemented.
6. **Resumable downloads.** Interrupted transfers restart from zero. That is safe
   but unfriendly on slow connections; a range request would need its own
   integrity story.
7. **Proxy/TLS environments.** Corporate TLS interception would break the
   download with a typed `network` error; there is no proxy or custom-CA path.
8. **Model quality on repository-scale tasks.** Phase 3 measured public
   benchmarks. Nothing here measures whether 7B Q4 produces acceptable patches
   on this project's own files; that remains a Phase 8 evaluation question.

## 15. Out of scope, confirmed

Phase 6 (single-file export, ZIP export, Download Mode) was **not started**.
Phase 7 (daily quota/reset), Sonnet/Opus, accounts, cloud AI APIs, telemetry,
payments and the Founder Control Centre were not touched. No commit, push,
merge, release or repository setting was changed. No weights were added to Git.
No existing model file or runtime setting was overwritten or auto-selected.

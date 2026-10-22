# Phase 5 — Local inference integration and runtime compatibility

**Status: implementation complete for everything this machine can execute.
Inference is not demonstrated.**

This phase did not add a second inference engine, a new page, or a new model. It
made the existing supervisor's load decision explicit, closed four ways the app
could have claimed more than it knew, and checked the pinned runtime against its
own source. No model file exists here, no `llama-server` binary exists here, and
there is no `cargo` binary, so nothing below is a claim that a model loaded.

---

## 1. The runtime Nexora drives

| Fact | Value | Source |
| --- | --- | --- |
| Release tag | `b11714` | GitHub release API, HTTP 200 |
| Tag → commit | `c0fd78626405c6d9b934efef0ec7291fcc20e3d6` | `git/ref/tags/b11714` |
| Published | `2026-10-17T16:17:08Z` | release API |
| Assets | 36 | release API |
| **Pre-release** | **`prerelease: true`** | release API |

`process::PINNED_BUILD` is `"11714"`. `Supervisor::start` runs
`llama-server --version`, takes the first line, and `reports_pinned_build`
checks the build number in it, so a different runtime is reported rather than
silently used. Nexora does not bundle or download the binary: the user supplies
`llama-server`, exactly as before.

Because upstream marks `b11714` as a pre-release, this document calls it the
*pinned build*, never a stable release.

### Exact launch

```
llama-server
  --model <absolute path to .gguf>
  --host 127.0.0.1
  --port <free loopback port chosen by Nexora>
  --ctx-size <configured context>
  --parallel 1
  --no-ui
  --jinja
  --api-key-file <app config dir>/runtime-session.key
  --timeout 600
```

Arguments are passed as a structured `argv`; no shell is involved. stdin and
stdout are `null`, stderr is piped and drained by a thread that keeps the last
64 KiB for diagnostics. On Windows the child is created with
`CREATE_NO_WINDOW`. The session key file is written immediately before the
spawn and removed if the spawn fails; it is also removed at the next start, so a
crashed run cannot leave a usable key behind.

Every flag above was found in `common/arg.cpp` at commit `c0fd786…`:

| Flag | Line in `arg.cpp` |
| --- | --- |
| `--ctx-size` | 1642 |
| `--parallel` | 2548 |
| `--model` | 3049 |
| `--host` | 3335 |
| `--port` | 3351 |
| `--no-ui` / `--no-webui` | 3491 |
| `--api-key-file` | 3524 |
| `--timeout` | 3573 |
| `--jinja` / `--no-jinja` | 3692 |

`--jinja` matters for the artefact: it makes the runtime use the chat template
stored in the GGUF (`tokenizer.chat_template`) instead of a built-in one.
`--parallel 1` matches the app's single active generation.

### Readiness handshake

Nexora polls `GET /health` every 300 ms for up to 180 s (`LOAD_TIMEOUT`), and
only treats a **2xx** answer as "loaded". That is the correct signal, and it was
verified in the runtime's own source at the pinned commit:

* `tools/server/server.cpp` starts the HTTP server before the model loads, so
  `/health` can answer during loading.
* `tools/server/server-http.cpp` runs `middleware_server_state`, which returns
  **503** with `{"error":{"message":"Loading model","code":503}}` while the
  server is not ready. That middleware runs *before* API-key validation.
* `tools/server/server-context.cpp` answers `{"status":"ok"}` from `get_health`
  only when the model is ready.

So a 2xx `/health` cannot happen before the model is loaded. Nexora does not
relax this to "any response".

During the wait, a child that exits is detected immediately through
`poll_failure`, which surfaces the runtime's own stderr tail instead of waiting
out the timeout. On timeout, `diagnostics` is included in the message.

### What Nexora sends

`POST http://127.0.0.1:<port>/v1/chat/completions`, `Authorization: Bearer <session key>`, body:

```json
{
  "model": "local",
  "stream": true,
  "messages": [{ "role": "system", "content": "…" }, { "role": "user", "content": "…" }],
  "max_tokens": 1024,
  "temperature": 0.2
}
```

Checked at the pinned commit:

* `server-common.cpp` `oaicompat_chat_params_parse` requires `messages` (each
  non-assistant message must carry `content`), reads `stream`, and honours
  `max_tokens`; it never validates the `model` field, so `"local"` is accepted.
* `--jinja` is what makes the request use the GGUF's own template; without it
  tool-related fields are rejected.
* `/v1/chat/completions` is registered alongside `/chat/completions` in
  `server.cpp`, and both sit behind the same key middleware, which accepts
  `Authorization: Bearer …` or `X-Api-Key`.

---

## 2. The pinned artefact

| Field | Value |
| --- | --- |
| Repository | `Qwen/Qwen2.5-Coder-14B-Instruct-GGUF` |
| Revision | `d0a692ef765eefbf2fabb130b3cb2e8917e3d225` |
| File | `qwen2.5-coder-14b-instruct-q4_k_m.gguf` |
| Size | 8 988 110 272 bytes (8.37 GiB) |
| SHA-256 | `c1e659736d89ac1065fb495330fb824d94001974a4bfa78e7270e43476a8d940` |
| Licence | Apache-2.0 |
| Context | 32 768 tokens (publisher) |
| Architecture | `qwen2` |

`src/llama-arch.cpp` at `c0fd786…` defines `LLM_ARCH_QWEN2 = "qwen2"` and
`LLM_KV_TOKENIZER_CHAT_TEMPLATE = "tokenizer.chat_template"`, so the pinned file
is an architecture the runtime knows and carries its own template. **This is
source-level compatibility, not proof of a successful load.** No load was
attempted: the file is not present, and the machine has ~3.5 GiB available
against an estimated 10.9 GiB for this profile at 8 192 tokens.

KV geometry used by the estimate (from the 14B `config.json`: 48 layers, 8 KV
heads, head dim 128): 196 608 bytes/token → 1.50 GiB at 8 192, 6.00 GiB at
32 768. Full arithmetic and measured host readings are in
[READINESS.md](./READINESS.md); the artefact provenance is in
[ARTIFACT_EVIDENCE.md](./ARTIFACT_EVIDENCE.md).

---

## 3. What this phase changed

### 3.1 One pre-spawn load decision (`inference/loading.rs`, new)

`load_model` used to inspect the file, then work out afterwards which profile the
path belonged to. That allowed a selection and a file to disagree until the first
message. `loading::plan` now returns the decision, or a typed refusal, before a
process exists:

* the selected profile's artefact must be approved;
* a published artefact may only be loaded for the profile it belongs to — a
  Sonnet selection pointing at `qwen2.5-coder-7b-instruct-q4_k_m.gguf` is
  refused with `model_mismatch`, never loaded and refused later;
* a published artefact must have been digest-verified **at that path in this
  session** (`model_unverified`), which is what the path-keyed ledger answers;
* the default profile (Nexora Mini) may still load a GGUF no profile publishes,
  which is the documented "connect a file you already have" workflow; any other
  profile may not (`model_mismatch`);
* the context window must be inside the app's 2 048–131 072 range and must not
  exceed the maximum the GGUF itself declares (`invalid_model`). It is never
  reduced to make a load succeed.

The returned `profile_id` is the attribution recorded on success, so a
name-level mismatch cannot creep back in through a second code path.

### 3.2 Load lifecycle

* The single load slot is claimed before the spawn; a second click during a load
  is refused with `busy` instead of racing the first.
* Readiness shows `loading` only for the profile actually being loaded, and
  never for the other row.
* A process that dies during the wait, or times out, clears the loaded mark,
  records a failure attributed to the profile, and releases the slot.
* `unload_model` refuses while a load is in flight, so a deliberate unload
  cannot be mistaken for a load failure.
* **Phase 7:** `load_model` refuses with `busy` while a reply is streaming,
  instead of starting a new engine that would stop the running one mid-answer.
  `start_generation` refuses with `busy` while a load is in flight. The loading
  marker is set before the reply check, and the reply check runs under the same
  lock that claims the reply slot, so the two cannot interleave. Nothing is
  cancelled on the user's behalf.
* **Phase 7:** a failure to *start* the process (for example a runtime path that
  is not executable) is now recorded against the profile being loaded. Before
  this, the setup form showed the error but the engine panel could show an older
  failure, or none.
* `status()` and every readiness read call `reconcile_engine()`, which reaps a
  child that died since the last snapshot, drops the stale "loaded" mark, and
  records the supervisor's reason against the profile that was running. A dead
  engine can therefore no longer be reported as "ready to load" with the old
  model still shown as loaded.

### 3.3 Streaming (`inference/client.rs`)

A closed stream is no longer a successful short answer. The runtime can exit, or
the socket can be cut, between tokens; the old code returned `Ok` with no
`finish_reason`, and the UI showed a truncated reply as complete. Completion now
requires the `[DONE]` sentinel **or** a non-empty `finish_reason`
(`stream_completed`). Otherwise the request fails with a plain-language network
error and the partial text stays marked as failed. Cancellation still returns
`finish_reason: "stopped"` and is unaffected.

Both signals were checked against the pinned commit by source reading:

* `server-task.cpp` `to_json_oaicompat_chat_stream` appends a final chunk whose
  `finish_reason` is `"stop"`, `"length"`, or `"tool_calls"`.
* `server-context.cpp`: the source reading suggested the OpenAI-compatible
  endpoint sends no `[DONE]`.

**Correction (2026-10-22, Phase 9 run).** That source reading was wrong for this
build. Running the pinned `llama-server` b11714 (built from the codeload source,
synthetic test model only) on `/v1/chat/completions` with `stream: true` produced
`finish_reason` followed by `data: [DONE]`. `stream_completed` accepts either
signal, so the code and its behaviour are unchanged. Only this paragraph was wrong.

A healthy reply completes on `finish_reason` or `[DONE]`. A stream that stops
before either is genuinely truncated. The Phase 9 run observed this with the engine
killed mid-reply: 39 deltas, no `finish_reason`, no `[DONE]`.

**Phase 7 — the wait for response headers.** The request `send()` used to be
unbounded and was not raced against Stop, so a Stop pressed before the engine
answered was ignored until then. It is now raced against the cancel signal and
bounded by the same 600 s first-token limit. A Stop in that window returns the
"stopped" outcome with nothing charged; a silent engine fails with `timeout`,
also without a charge, because the engine never accepted the request. This is
source-level only: it has not been exercised against a running `llama-server`.

The comment above the error-envelope parser claimed the fixtures came from a
locally built server. They did not — no server has ever run here. It now says
they are transcribed from the runtime's contract in `server-http.cpp`.

### 3.4 Prompt budget (`inference/budget.rs`, new)

The runtime truncates a prompt longer than the context window, which silently
drops the oldest turns. `start_generation` now refuses such a request *before*
the engine is contacted and before usage is reserved. The estimate is
heuristic (ASCII at 4 characters/token, every non-ASCII character at 2 tokens, plus
the system prompt and per-turn framing) and is used only to refuse a prompt that clearly
cannot fit; it is never reported as a token count. The 4 characters per token rate is
typical of English prose. It is **not** a bound for dense source code, which can take
more tokens per character, so a code-heavy prompt can pass this guard and still be
truncated by the runtime. (Phase 7 corrected an earlier description that called this
rate conservative; the behaviour is unchanged.)
Refusal moves no daily unit, and the single active slot still guarantees one
charge per accepted generation.

### 3.5 Registry

`ModelRegistry::by_file_name` and `by_id` are the only lookups; `resolve`
remains the strict, erroring one used for user input.
`acquisition::profile_for_path` delegates to `by_file_name` instead of repeating
the search.

---

## 4. Tests actually run

All commands below were executed in this workspace; each exited 0.

| Check | Command | Result |
| --- | --- | --- |
| Install | `pnpm install --frozen-lockfile` | up to date |
| Typecheck | `pnpm typecheck` | no errors |
| Frontend tests | `pnpm test` | **833 passed / 833**, 83 files |
| Frontend build | `pnpm build` | built; `index-D93vR3Cp.js` 549.95 kB, `index-BTNBSDFM.css` 102.37 kB |
| Rust formatting | `rustfmt --check` on all 44 files under `src-tauri/src` | clean |
| Rust unit tests (standalone) | `rustc --test` harness, rustc 1.88.0 | **119 passed / 119** (151 after Phase 6 added the `usage` tests — see [PHASE_5_7_CHAT_ROUTING.md](./PHASE_5_7_CHAT_ROUTING.md)) |

The standalone harness compiles the dependency-free modules —
`budget` 9, `loading` 13, `readiness` 41, `registry` 32, `models` 19,
`hardware` 5 — using a stub `serde` that supplies trait shapes only, with
`Serialize` derives stripped and the two `serde_json` registry tests removed.
**It is not a substitute for `cargo test`**, and it says nothing about
`commands.rs` or `mod.rs`, which need Tauri and cannot be compiled here.

New tests in this phase:

* `loading`: own verified file; Mini with its own file; Mini with a custom file;
  Sonnet refusing a custom file; either selection loading the other's file;
  unverified artefact; context below the minimum, above the app maximum, and
  above the file's declared maximum; empty path; profile without an approved
  artefact; plan attribution.
* `budget`: ASCII and non-ASCII estimates, framing, exact fit, over-limit
  refusal and its message, the minimum supported context.
* `readiness`: an in-flight load reads `loading` for that profile and not for the
  other; a failure attributed to another profile does not taint this row.
* `client`: `[DONE]`, `finish_reason`, and a stream that simply stops.

**Phase 7 re-run (2026-10-22).** Frontend typecheck, tests (849 / 849, 84 files) and the
build passed on the unchanged frontend. The standalone registry/error harness now runs
41 / 41 (the two new guard tests added). `rustfmt --check` passes on all 44 files under
`src-tauri/src`. The `commands.rs` and `client.rs` edits made in Phase 7 are **not compiled**
by any check in this environment; they are source-reviewed only. See
[PHASE_9_VERIFICATION.md](./PHASE_9_VERIFICATION.md) §11.

### Not run, and why

* **`cargo check` / `cargo clippy` / `cargo test`** — there is no `cargo` binary
  in this environment and no reachable crate registry. Full Rust compilation is
  **BLOCKED**, not passed.
* **Any inference** — no model file, no `llama-server`, 3.5 GiB available against
  ~10.9 GiB estimated. `INFERENCE NOT DEMONSTRATED`.
* **Windows/MSVC/WebView2** — this is Linux. No Windows claim is made.

---

## 5. Remaining limitations

1. **No load and no generation was attempted.** Everything about the handshake
   and the chat request is verified against the runtime's source and covered by
   unit tests, not by a running engine.
2. **`b11714` is a pre-release upstream.** Its flags, routes and middleware were
   read at the pinned commit, but upstream may change before a stable tag.
3. **Chat-template behaviour is unproven here.** The file carries
   `tokenizer.chat_template` and `--jinja` enables it; whether the template
   produces the intended turns can only be seen in a real run.
4. **The prompt budget is a heuristic.** It refuses requests that clearly cannot fit,
   but it is not a tokenizer count and does not bound dense code. A request it allows
   can still be truncated by the runtime, for a long prompt or a long reply.
5. **A custom GGUF loaded under Nexora Mini has no profile attribution.** The
   selector cannot claim "Nexora Mini is loaded" for a file that is not Mini's
   published artefact, so that row shows its own state and the engine panel
   shows what is running. This is deliberate.
6. **The memory gate only covers published artefacts.** KV-cache size needs the
   model's layer and head geometry, which Nexora only knows for the two
   registered files. A custom GGUF is checked for existence, format, header and
   declared context, but not for whether it fits in memory; the runtime's own
   error is the authority there.
7. **`commands.rs` and `mod.rs` are not compiled here.** They parse cleanly
   (rustfmt) and were reviewed by hand, but a full `cargo build` is the only
   proof, and it is blocked.

### If you have a machine that can run it

1. Install the `b11714` build of `llama-server` for your platform and point
   Nexora at it; `probe_runtime` should report build 11714.
2. Download Nexora Mini or Sonnet in the model manager (verified before
   install), or select the GGUF yourself and press **Verify file**.
3. Load. `/health` must return 2xx before the row reads `loaded`.
4. Send a message; the reply should stream incrementally, and **Stop** should
   end it with a "stopped" note rather than an error.
5. Watch the engine panel while the model loads: its stderr tail is the
   authoritative failure message if the load does not complete.

# Inference Integration Design

**Status:** design only. Nothing here is implemented.
**Scope:** how the Rust core owns a local `llama-server` process, talks to it, contains it, and degrades safely.
**Invariants:** the webview never talks to the runtime; model output is untrusted data; the server is replaceable; failure is a normal state, not an exception.

---

## 1. Principles

| # | Principle | Consequence |
|---|---|---|
| P-1 | **Rust owns the lifecycle.** The UI can request a state; it cannot spawn, kill, or address the runtime. | No port or key ever reaches JavaScript. |
| P-2 | **The runtime is a child, not a library.** | A native crash or hang is recoverable without killing the app. |
| P-3 | **Every byte from the model is untrusted.** | Tool calls are parsed, schema-validated, and authorised before execution; content is never executed or interpolated into a shell. |
| P-4 | **Fail closed and explain.** | Missing model, insufficient VRAM, or a wedged server produces a named state with a next action, never a spinner. |
| P-5 | **No silent degradation.** | Context truncation, partial offload, and spill to system memory are surfaced, not hidden. |
| P-6 | **One pinned build per release.** | Version is a property of the app, not of the machine. |

## 2. Components and data flow

```
┌───────────────────────────── Tauri app ─────────────────────────────┐
│  Webview (React)                                                     │
│     │  invoke / channel (IPC only, no HTTP)                          │
│     ▼                                                                │
│  nexora-inference (Rust)                                             │
│     ├─ Supervisor      lifecycle FSM, restart budget, job object     │
│     ├─ Client          reqwest + SSE, timeouts, cancellation         │
│     ├─ Health          readiness + generation probe + stall watchdog │
│     ├─ Budget          VRAM/RAM detection, context selection         │
│     └─ Parser          tool-call extraction + schema validation      │
│     │  loopback HTTP + per-session bearer key                        │
│     ▼                                                                │
│  llama-server.exe  (child process, job object, 127.0.0.1:<ephemeral>)│
└──────────────────────────────────────────────────────────────────────┘
```

The webview's only channel is Tauri IPC. There is no second HTTP listener, no WebSocket to the model, and no configuration that exposes the runtime port to the front end.

## 3. Lifecycle state machine

```
Stopped ──start──▶ Starting ──listening──▶ Loading ──/health 200──▶ Ready
   ▲                  │                      │                       │
   │                  │ spawn fail           │ load timeout          │ request
   │                  ▼                      ▼                       ▼
   │               Failed ◀──────────────────┘                    Busy
   │                  │                                               │
   │                  │ retry (budget)                                │ idle
   │                  └──────────────▶ Starting                       ▼
   │                                                               Ready
   └──────────── stop / unload ◀── Stopping ◀── cancel / error ───────┘
```

| State | Meaning | UI |
|---|---|---|
| `Stopped` | No process. | "Model not loaded" + Load button |
| `Starting` | Process spawned, port not yet answering. | "Starting engine…" with elapsed time |
| `Loading` | `/health` returns 503. | "Loading model… 42%" (indeterminate; no fake percentage) |
| `Ready` | Model loaded, no request in flight. | Composer enabled |
| `Busy` | One request in flight. | Streaming; Stop enabled |
| `Stopping` | Cancel or unload in progress. | "Stopping…" and then "Restarting…" if escalation occurs |
| `Failed` | Spawn/load failed, or crash detected. | Named cause + Retry / Choose smaller model |
| `Unavailable` | No model installed, disk full, or VRAM insufficient. | Onboarding or model picker |

Transitions are owned by the supervisor and emitted to the UI as events. The UI does not infer state from the absence of tokens.

## 4. Model discovery and configuration

### 4.1 Manifest

The app ships a signed `model-manifest.json`; downloaded models get a sidecar `*.nexora.json` record.

```json
{
  "id": "qwen3.8-27b-ud-q4km",
  "display_name": "Qwen3.8 27B (Q4)",
  "repo": "unsloth/Qwen3.8-27B-GGUF",
  "revision": "4ca720788d1e01f1bff70c033e0d0028fd02e502",
  "file": "Qwen3.8-27B-UD-Q4_K_M.gguf",
  "sha256": "…",
  "size_bytes": 16464440224,
  "quant": "UD-Q4_K_M",
  "params_b": 27.78,
  "license": "Apache-2.0",
  "license_file": "licenses/Apache-2.0.txt",
  "notices": "notices/Qwen3.8-27B.txt",
  "context_native": 262144,
  "context_recommended": 32768,
  "kv_bytes_per_token_f16": 65536,
  "min_vram_gb": 20,
  "min_ram_gb": 24,
  "multimodal": false,
  "tool_format": "hermes",
  "tier": "h3"
}
```

### 4.2 Discovery order

1. Models recorded in the local store that verify against their sidecar hash.
2. Models in the user's chosen folder (read-only import), verified on first use.
3. Nothing else. The app never scans the disk for `*.gguf`, because an arbitrary file is an untrusted input and a multi-gigabyte scan is a poor user experience.

### 4.3 Selection

```
eligible = models where
    file exists and hash matches
    and required_vram <= detected_budget
    and required_ram <= detected_ram - 4 GB
    and license is on the allowlist
pick = highest tier_score(eligible) with user override
```

If nothing is eligible, the app enters `Unavailable` and offers the largest model that *would* fit after a quant change, with the exact reason.

## 5. Load and unload

### 5.1 Spawn

```text
llama-server.exe
  --model        <store>\models\<id>\<file>.gguf
  --host         127.0.0.1
  --port         <ephemeral, chosen by Rust>
  --api-key-file <session-key-path>
  --no-webui
  --parallel     1
  --ctx-size     <computed: 8192 | 16384 | 32768>
  --jinja
  --reasoning-format deepseek          # thinking models only
  --reasoning-budget 2048              # thinking models only
  --cache-ram    2048
  --ctx-checkpoints 4
  --cors-origins http://localhost.invalid   # deny browser origins; native client sends none
  --no-context-shift                   # never silently drop the start of the conversation
  --threads      <physical cores - 2>  # CPU path only
  --timeout      600
```

Deliberately **absent**: `--tools`, `--agent`, `--mcp-servers-*`, `--media-path`, `--log-prompts-dir`, `--metrics`, `--props`, `--slot-save-path`, `--ssl-*`, `--host 0.0.0.0`, `-hf` (which would download at runtime and auto-attach an `mmproj`).

`--api-key-file` is used instead of `--api-key` because command lines are readable by other processes (WMI); a key in `argv` is a key disclosed.

### 5.2 Process containment

- Create the child in a **job object** with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`, so an app crash cannot leave an orphaned server holding VRAM.
- Set `CREATE_NO_WINDOW`; redirect stdout/stderr to a 64 KiB ring buffer per stream (never to a world-readable file).
- Do **not** run the runtime under the restricted token or LPAC used for agent tools: it needs GPU and model-file access, and a broken CUDA context is worse than the threat it mitigates. Containment is by job object, loopback binding, key auth, and disabled features.
- Record the child PID and verify the listening socket belongs to it before trusting `/health` (a local process could squat the port).

### 5.3 Readiness

`/health` 200 means "model loaded". It does **not** mean "can generate" (issue #27388). Readiness therefore requires:

1. `/health` returns 200;
2. `GET /props` with the key succeeds;
3. a **generation probe** (`max_tokens: 1`, fixed prompt) returns a token within the probe timeout.

The probe runs once at load and then every 60 s while idle, and before each user turn if the last probe is older than 5 minutes. Cost: one token, negligible.

### 5.4 Unload

1. Refuse new requests; enter `Stopping`.
2. If idle, `POST /slots/0?action=erase` (best effort, 500 ms timeout).
3. Send a graceful shutdown; wait 3 s.
4. Terminate the job object; wait 2 s; verify exit.
5. Confirm VRAM returned to baseline within 5 s; if not, report a leak and keep the tier blocked until the app restarts.

## 6. Streaming

| Concern | Design |
|---|---|
| Transport | `POST /v1/chat/completions` with `stream: true`; parse SSE `data:` frames; stop at `[DONE]` |
| Framing | Buffer until `\n\n`; ignore comments and keep-alives; tolerate CRLF |
| UTF-8 | Decode incrementally with a carry buffer; never split a multi-byte sequence across events |
| Backpressure | Coalesce token deltas and flush to the UI at most every 33 ms (30 Hz); never one IPC message per token |
| Usage | Capture `usage`/`timings` when present; if absent, count tokens server-side from the final chunk |
| Reasoning | Route `reasoning_content` to a separate UI stream; never mix it into the answer or the transcript sent back as context |
| Errors mid-stream | A malformed frame is logged and skipped; three consecutive malformed frames abort the turn with `protocol_error` |
| Size cap | Abort if the assistant message exceeds 4 MB or 100k tokens |
| Persistence | Write the transcript only after the turn reaches a terminal state (`complete`, `cancelled`, `failed`), with the partial text marked as partial |

## 7. Cancellation

Cancellation is an escalation ladder because no documented endpoint guarantees a stop ([`runtime-evaluation.md`](runtime-evaluation.md) §3.6).

| Step | Action | Deadline | Effect |
|---|---|---|---|
| 0 | Mark turn `cancelling`; UI acknowledges immediately | ≤300 ms | User sees "Stopping" |
| 1 | Drop the SSE connection | immediate | Server *should* cancel; may not |
| 2 | `POST /slots/0?action=erase` | 500 ms | Drops cached context |
| 3 | Query `/slots` | 1 s | If `is_processing` is false, done |
| 4 | Terminate the job object; restart | 2 s from step 0 | Guaranteed stop; model reloads |
| 5 | If restart fails twice, enter `Failed` | — | Offer retry or smaller model |

Rules:

- **Never send `X-Conversation-Id`.** It disables disconnect-cancellation and creates unbounded sessions upstream (#29498).
- **One request in flight.** A second request queues in Rust, never in the server. This avoids the Vulkan abort hang (#27604) and makes slot accounting trivial.
- **Never reuse a server after a forced kill** without a fresh load; the KV cache is gone and the process may be in an undefined state.
- **The transcript records "interrupted by user"**, not "completed". A cancelled turn is not evidence of success (doc 05 honesty vocabulary).

## 8. Timeouts and crash recovery

| Phase | Timeout | On expiry |
|---|---|---|
| Spawn → listening | 10 s | Kill job, `Failed(spawn)` |
| Listening → `/health` 200 | 120 s GPU / 180 s CPU | Kill job, `Failed(load_timeout)` |
| Readiness probe | 15 s | Retry once, then `Failed(probe)` |
| Request → first token | 30 s GPU / 180 s CPU | Cancel, `Failed(prefill_timeout)` |
| Inter-token stall | 60 s GPU / 120 s CPU | Cancel, restart if repeated |
| Total turn | 15 min (agent step), 30 min hard | Cancel, mark interrupted |
| Idle unload | 10 min (configurable) | Unload to release VRAM |
| Graceful stop | 3 s | Terminate job |

**Restart budget:** 3 restarts in 10 minutes, exponential backoff 1/2/4 s with jitter. On the fourth, open the circuit: `Failed(crash_loop)`, keep the model unloaded, and show the last stderr tail (redacted) plus a "Send diagnostics" action. Never restart in a loop that the user cannot see.

**Crash classification:**

| Signal | Classification | Response |
|---|---|---|
| Exit code 0 after a stop request | Clean | None |
| Exit code ≠ 0 during load | Load failure | Fall back one quant; if none, `Unavailable` |
| Exit ≠ 0 during generation | Runtime crash | Restart, mark turn failed, offer retry from checkpoint |
| Process alive, no tokens, probe fails | Wedge | Kill and restart |
| Process alive, decode rate <20% of baseline | Spill or throttle | Warn; reduce context/offload |
| Display driver reset in event log | GPU fault | Restart with lower offload; disable that backend for the session |

**Suspend/resume:** on `WM_POWERBROADCAST` resume, wait 2 s, run the generation probe. If it fails, restart. Do not assume the GPU context survived sleep.

## 9. Health checks

| Check | Endpoint | Auth | Cadence | Purpose |
|---|---|---|---|---|
| Liveness | `GET /health` | public | 10 s while loading; 60 s idle | Process is up and model reports loaded |
| Identity | `GET /props` | key | at load, then hourly | We are talking to our child, with the expected model |
| Generation | 1-token completion | key | at load; before a turn if stale >5 min | Proves decode works, not just HTTP |
| Stall | token timestamps | — | during generation | Catch wedge while `/health` still says 200 |
| Memory | NVML + process counters | — | 10 s | Detect spill and leaks |
| Listener | socket → PID match | — | at load | Detect port squatting |

A failed liveness check never immediately kills the process: two consecutive failures, or one failed generation probe, triggers the escalation. A single missed poll under heavy prefill is normal.

## 10. Context and memory budget

```
budget_bytes          = vram_total - 1.5 GB (desktop reserve)
weights_bytes         = model file size + 5% (metadata, alignment)
scratch_bytes         = max(1.0 GB, 0.06 × weights_bytes)
kv_available          = budget_bytes - weights_bytes - scratch_bytes
ctx_max_by_memory     = floor(kv_available / kv_bytes_per_token_f16)
ctx                  = min(ctx_max_by_memory, tier_default, ctx_native)
```

Rules:

- If `ctx < 8192`, the model is not eligible for that machine. Offer a smaller quant.
- Context is chosen from `{8192, 16384, 32768}`; arbitrary values create untestable configurations.
- **KV quantisation is not a default at 1.0.** Q8_0 KV roughly halves cache size but changes quality; it becomes an option only after the sensitivity run in [`benchmark-protocol.md`](benchmark-protocol.md) §9.
- Compaction at 80% of the configured context (Phase 1 doc 03), with a visible marker in the transcript. No silent context shift (`--no-context-shift`).
- If NVML reports >95% VRAM or the decode p50 falls >50% below the session baseline, pause new turns and offer to reduce context; do not let the driver decide.
- On 16 GB systems, refuse models whose weights + KV + 4 GB RAM reserve exceeds physical RAM; Windows paging turns a slow model into a frozen desktop.

## 11. Tool-call parsing and validation

The parser is a security boundary, not a convenience.

```
response ──▶ extract tool_calls (structured field only)
              │
              ├─ none ──▶ treat as text answer
              └─ calls ──▶ for each:
                            1. name ∈ allowlist?           no → reject turn
                            2. JSON parses?                no → one repair attempt, then fail
                            3. validates against schema?   no → reject with error to model
                            4. limits: ≤16 calls, ≤2 MB args, depth ≤8
                            5. path/command args → PathGuard + command policy
                            6. requires approval? → pause, ask user
                            7. execute, capture result, feed back
```

| Rule | Reason |
|---|---|
| Only the structured `tool_calls` field is executable | Text that *looks* like a call is model output, not intent; executing it is a prompt-injection path |
| `additionalProperties: false` on every tool schema | Prevents argument smuggling |
| Unknown tool name rejects the **turn**, not just the call | A model inventing tools is not in a recoverable state |
| One repair attempt maximum | Retry loops are how agents burn budgets |
| Tool arguments are never interpolated into a shell | Commands are built from validated fields, not string concatenation |
| `write`, `delete`, `git` follow doc 05 approval rules | Deletion always requires approval; `secrets.read`, `outside.*`, `git.write` are denied |
| Reasoning content is stripped before tool extraction | Thinking text can contain fake calls |
| Duplicate identical calls are rejected | Common failure mode in small models |

Parse failures produce `Unverified`/`Failed` states — never a success report. If the model returns prose instead of a call, the agent workflow decides whether to re-prompt; the runtime layer does not guess.

## 12. Local HTTP server hardening

### 12.1 Network surface

| Control | Implementation |
|---|---|
| Bind address | Literal `127.0.0.1`, never `localhost` (which can resolve to `::1`), never `0.0.0.0` |
| Port | Ephemeral, chosen by binding `127.0.0.1:0` in Rust, passed to the child, retried up to 5× on collision |
| Verification | After spawn, confirm the listening socket's owning PID equals the child PID |
| Firewall | **No inbound rule is created.** The installer must not add one. Optionally assert no existing rule matches the port. |
| IPv6 | Not bound; no dual-stack listener |
| LAN exposure | Impossible by construction; a test asserts that a connection to the machine's LAN IP on that port fails |
| Proxy env | The Rust client ignores `HTTP_PROXY`/`HTTPS_PROXY` for loopback (explicit no-proxy) |
| CORS | Server default is `*` with credentials enabled and the `Origin` echoed; a non-matching `--cors-origins` denies browser origins without affecting the native client **[U]** — verify at the pin |

### 12.2 Authentication

- A 256-bit random key is generated **per app session**, written to `%LOCALAPPDATA%\Nexora\runtime\session.key` with an ACL granting only the current user, and deleted on exit.
- The key is passed via `--api-key-file`, never `argv`.
- The Rust client sends `Authorization: Bearer <key>` on every request. `/health` is exempt because the server ignores auth there; it is only used as a liveness signal and is never trusted for identity.
- Tauri IPC authorises the caller: only the app's own webview can invoke inference commands. No remote content, no `dangerousRemoteDomainIpcAccess`.

### 12.3 Request hygiene

| Control | Why |
|---|---|
| Strip `Origin`, `Referer`, `Cookie`, `X-Conversation-Id`, and any unknown `X-*` headers before forwarding | Removes the documented cancellation bypass (#29498) and browser-origin ambiguity |
| Send `Content-Type: application/json` only | No form posts, no multipart |
| Cap request body at 8 MB | Prevents memory abuse |
| Cap response body at 4 MB per turn | Prevents unbounded transcript growth |
| Disable redirects in the client | No SSRF pivot |
| Reject non-JSON responses | A HTML error page is not a model answer |
| 60 s connect/read timeout on health, per-request deadlines elsewhere | No hung sockets |

### 12.4 Disabled features

`--no-webui` is mandatory. Built-in tools, MCP, media path, prompt logging, metrics, slot persistence, and `POST /props` are never enabled. The web UI is an HTML/JS application served from the same origin as the API; on a loopback server with permissive CORS it is an unnecessary second front end and a documented XSS/CSRF surface area.

### 12.5 What this does not protect against

- A **local administrator** or malware running as the same user can read the key file and call the server. Loopback auth is defence against other users and browser-based attackers, not against the machine's owner.
- A **malicious model file** can exploit the parser/loader. That is a memory-safety risk in a C++ process; the job object limits blast radius but cannot prevent code execution. Acquisition policy and hashes are the control ([`model-acquisition-policy.md`](model-acquisition-policy.md)).
- A **prompt injection** in repository content can ask the model to call tools. The tool gateway, not the server, is the enforcement point.

## 13. Offline and unavailable states

| State | Trigger | UI |
|---|---|---|
| `NoModel` | Fresh install, nothing downloaded | Onboarding with size and licence per model |
| `Downloading` | Explicit user action | Progress, pause/resume, disk requirement, hash verification at the end |
| `DownloadInterrupted` | Network loss, app exit | Resume from byte offset; verify the final hash |
| `DiskFull` | Pre-flight check fails | Required vs available, with the exact shortfall |
| `VramInsufficient` | Budget check fails | "This model needs ~20 GB; you have 12 GB" + smaller option |
| `DriverUnsupported` | Backend init fails | CPU fallback offered explicitly, with the speed warning |
| `RuntimeMissing` | Archive corrupt or blocked by AV | Repair action; never silently re-download |
| `CrashLoop` | Restart budget exhausted | Diagnostics, safe mode (CPU, 7B) |
| `Offline` | No network | Everything still works; only downloads are disabled |

The app is fully functional with no network. There is no telemetry call, no licence check, and no remote feature flag in the inference path.

## 14. Threat model delta

| Threat | Vector | Control |
|---|---|---|
| T-31 | Malicious local page scans loopback ports | Random port, per-session key, IPC-only UI |
| T-32 | Port squatting impersonates the server | PID verification + authenticated `/props` |
| T-33 | Header-driven cancellation bypass / memory DoS (#29498) | Header stripping, single in-flight, response cap |
| T-34 | Wedged server looks healthy (#27388) | Generation probe + stall watchdog |
| T-35 | Tool-call injection through file content | Structured-field-only execution, schema validation, approval gates |
| T-36 | Model file tampering | Pinned SHA-256, mirror, signature on manifest |
| T-37 | Key disclosure through process arguments | `--api-key-file`, ACL, per-session key |
| T-38 | Orphaned runtime after app crash | Job object with kill-on-close |
| T-39 | Native crash from a malformed GGUF | Process isolation; acquisition checks; no privileged token |
| T-40 | Silent context truncation | `--no-context-shift`, compaction marker, context shown in UI |

These extend, and do not replace, T-01…T-30 in Phase 1 doc 07.

## 15. Observability

- Structured events: `runtime.spawn`, `runtime.ready`, `runtime.first_token`, `runtime.stall`, `runtime.cancel{step}`, `runtime.restart{reason}`, `runtime.exit{code}`, `budget.ctx_selected`, `tool.invalid{reason}`.
- Metrics retained locally: load time, TTFT, decode rate, peak VRAM/RAM, cancel latency, restart count.
- **No prompt or completion logging by default.** Diagnostics bundles are opt-in, redacted by the Phase 1 four-stage redaction, and never include model files.
- A crash report contains the runtime build tag, model hash, flags, exit code, and the last 64 KiB of stderr after redaction.

## 16. Contract tests Phase 3 must pass

| # | Test | Pass condition |
|---|---|---|
| CT-01 | Spawn → ready → stop, 50 cycles | No leak, no orphan, p95 load within estimate |
| CT-02 | Kill the child externally during generation | Supervisor detects within 5 s and recovers |
| CT-03 | Kill the app during generation | No orphaned server; VRAM released |
| CT-04 | Occupy the chosen port before spawn | Retry succeeds on a new port |
| CT-05 | Wrong API key | 401 from the server; client reports auth failure |
| CT-06 | `/health` 200 but generation stalled | Probe fails, restart triggers |
| CT-07 | Cancel at 10 random points | ≥95% stop ≤2 s; UI ack ≤300 ms |
| CT-08 | Malformed/truncated tool arguments | Rejected, one repair, no execution |
| CT-09 | SSE split mid-UTF-8 and mid-frame | No mojibake, no duplicate tokens |
| CT-10 | 4 MB+ response | Aborted cleanly, partial text retained |
| CT-11 | Model deleted between sessions | `NoModel`, no crash |
| CT-12 | VRAM artificially constrained | Model refused with a clear reason, no spill |
| CT-13 | Sleep/resume | Probe detects a dead context and restarts |
| CT-14 | LAN connection attempt to the runtime port | Refused |
| CT-15 | Request with `X-Conversation-Id` | Header stripped; cancellation still works |

If CT-06, CT-07, or CT-14 fails, the runtime choice is re-opened ([`runtime-evaluation.md`](runtime-evaluation.md) §9).

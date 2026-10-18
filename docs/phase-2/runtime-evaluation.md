# Windows Inference Runtime Evaluation

**Scope:** runtime selection for Nexora 1.0 on Windows x64.
**Method:** primary-source documentation review, release-artifact inspection, and issue-tracker review on 2026-10-17.
**No runtime was installed or executed.** Capability claims are documentary; performance claims are absent.

---

## 1. Requirements the runtime must meet

Derived from Phase 1 (docs 03, 04, 07, 09). These are gates, not preferences.

| ID | Requirement | Source of requirement |
|---|---|---|
| R-01 | Runs natively on Windows 11 x64 (and Windows 10 22H2 while it remains in support). | Doc 01 platform scope |
| R-02 | Fully offline after installation; no account, no telemetry dependency, no cloud call. | Doc 01 FR-01, doc 07 |
| R-03 | CPU inference on AVX2 hardware, with GPU acceleration optional. | Doc 04 tier model |
| R-04 | NVIDIA CUDA and a vendor-neutral GPU path (Vulkan or equivalent). | Doc 11 vendor matrix |
| R-05 | Loads GGUF quantisations from Q2 to Q8 without re-export. | Doc 04 manifest design |
| R-06 | OpenAI-compatible chat completions with token streaming over SSE. | Doc 03 IPC/inference supervisor |
| R-07 | JSON-schema or grammar-constrained output, plus tool calling with a documented parser. | Doc 05 tool gateway |
| R-08 | Cancellation that actually stops generation and frees the slot. | Doc 03 §inference |
| R-09 | Redistributable under a licence that permits bundling in a commercial desktop app. | Doc 09 installer |
| R-10 | Embeddable in a Tauri 2 lifecycle: start, health-check, stop, crash-detect, restart. | Doc 03 |
| R-11 | Local HTTP surface that can be locked to loopback with authentication. | Doc 07 inference hardening |
| R-12 | One pinned build per Nexora release, with reproducible hashes. | Doc 09 |

## 2. Candidates and eligibility

| Runtime | Licence | Windows x64 | Verdict |
|---|---|---|---|
| **llama.cpp** (`llama-server`) | MIT **[V]** | Native binaries: CPU, CUDA 12.4, CUDA 13.4, Vulkan, ROCm 10.0, SYCL, OpenVINO, OpenCL Adreno **[V]** | **Primary** |
| **Ollama** | MIT **[V]** | Native installer and zip; bundles its own runner **[V]** | **Fallback / external provider** |
| **vLLM** | Apache-2.0 **[U]** | Not a documented installation platform; docs list NVIDIA CUDA, AMD ROCm, Intel XPU, Apple Metal and CPU on Linux-family targets **[V]** | Rejected for 1.0 |
| **TensorRT-LLM** | Apache-2.0 **[U]** | NVIDIA-only, Linux-first, complex build chain; Phase 1 already deferred it | Rejected for 1.0 |
| **ONNX Runtime GenAI** | MIT **[V]** | Native Windows; CPU, CUDA, DirectML, TensorRT-RTX, OpenVINO, QNN, WebGPU **[V]** | Deferred |
| **mistral.rs** | MIT **[V]** | Rust-native, runs on Windows; smaller model/quant matrix | Deferred / watchlist |
| **candle** | MIT/Apache-2.0 **[U]** | Rust-native framework, not a server product | Not a runtime candidate |
| **LM Studio** | Proprietary **[U]** | Windows app | Excluded: cannot be bundled or redistributed |

LM Studio is excluded on distribution grounds carried from Phase 1. Its licence terms were not re-reviewed in this phase; if it is ever mentioned in product material, `Q2-06` requires confirmation first.

## 3. llama.cpp — primary candidate

### 3.1 Distribution reality (this is the packaging constraint)

| Fact | Evidence |
|---|---|
| Repository licence is **MIT**. | `ggml-org/llama.cpp` metadata **[V, S-01]** |
| The semantic release `v0.7.0` was published **2026-10-15T17:13:19Z** and attaches **one asset: `nightly-tag.txt`**. | Release API **[V, S-02]** |
| Windows binaries are published on **rolling build tags** `b####`, several per day. Build `b11714` (2026-10-17T16:17:08Z) attaches 34 assets. | Release API **[V, S-02]** |
| Windows assets in `b11714` include `llama-b11714-bin-win-cpu-x64.zip`, `...-win-cuda-12.4-x64.zip`, `...-win-cuda-13.4-x64.zip`, `...-win-vulkan-x64.zip`, `...-win-rocm-10.0-x64.zip`, `...-win-sycl-x64.zip`, `...-win-openvino-2026.4.1-x64.zip`, plus `cudart-llama-bin-win-cuda-12.4-x64.zip` and a CUDA 13.4 runtime package. | Release API **[V, S-02]** |
| `v0.7.0` changed the default server port from 8080 to **9931** and refactored the chat/session layer. | Release notes **[V, S-02]** |

Consequences:

1. **Pin a build tag, not a version.** `runtime-manifest.json` must record `b11714` (or whichever build passes the contract tests), the asset filename, its SHA-256, and the build date. A "latest" fetch is not acceptable.
2. **Mirror the artifact.** Build tags are retained, but a desktop installer cannot depend on a live GitHub download at install time. The acquisition policy requires an internal mirror + hash allowlist ([`model-acquisition-policy.md`](model-acquisition-policy.md) §6).
3. **Nightly builds are not stability-tested releases.** A build can regress between tags. The contract test suite (`Q2-07`) must run against the exact pinned build before every Nexora release.
4. **Source builds remain the audited path.** Windows source builds use CMake + Visual Studio 2022 (MSVC or clang) with the CUDA toolkit or Vulkan SDK as needed **[V, S-04]**. This is the fallback if a mirrored binary cannot be trusted, at the cost of a much heavier build pipeline.

### 3.2 Backend support on Windows

| Backend | Windows status | Notes |
|---|---|---|
| CPU | Strong **[V]** | x64 and arm64 builds; AVX2 baseline is realistic for the 1.0 hardware floor; AVX-512 and AMX are runtime-detected **[V, S-04]** |
| NVIDIA CUDA | Strong **[V]** | Separate CUDA 12.4 and 13.4 x64 builds. The 12.4 build has the lower driver floor; 13.4 needs newer drivers. Exact minimum driver versions were **not verified** here (`Q2-08`). |
| AMD ROCm/HIP | Limited on Windows **[V]** | A Windows ROCm 10.0 build exists. `HSA_OVERRIDE_GFX_VERSION` is explicitly **not supported on Windows**, so unsupported GPUs cannot be forced onto a compatible target **[V, S-04]**. |
| Vulkan (AMD/Intel/NVIDIA) | Broad **[V]** | Vendor-neutral path. Carries the most driver variability and the most upstream hang reports (§3.5). |
| Intel SYCL / OpenVINO | Present **[V]** | Builds exist; not a 1.0 supported path. |
| Vulkan on ARM64 | Present **[V]** | Out of scope (no ARM64 Windows target at 1.0). |

**VRAM oversubscription behaves differently on Windows.** Unified-memory fallback via `GGML_CUDA_ENABLE_UNIFIED_MEMORY` is a Linux facility; on Windows the equivalent is the NVIDIA control panel's "System Memory Fallback" **[V, S-04]**. This matters operationally: a model that appears to fit can silently spill into system memory and degrade by an order of magnitude instead of failing with an out-of-memory error. Nexora must budget VRAM explicitly and must not rely on the driver to fail loudly ([`integration-design.md`](integration-design.md) §8).

### 3.3 Server capabilities that matter to Nexora

| Capability | Detail | Grade |
|---|---|---|
| OpenAI-compatible endpoints | `/v1/chat/completions`, `/v1/completions`, `/v1/models`, `/v1/responses`, plus native `/completion`, `/props`, `/slots`, `/metrics` | **[V, S-03]** |
| Streaming | SSE token stream; a `timings`/`usage` object reports prompt and predicted token counts | **[V, S-03]** |
| Constrained output | `--grammar`/`--grammar-file` (GBNF) and `--json-schema`/`--json-schema-file` | **[V, S-03]** |
| Tool calling | OpenAI-style function calling with the Jinja template engine; `--jinja` is now **enabled by default** in the current server | **[V, S-03]** |
| Template control | `--chat-template`/`--chat-template-file`, `--chat-template-kwargs`; built-in templates include `gpt-oss`, `mistral-v7`, `deepseek3`, `qwen`-family chatml variants | **[V, S-03]** |
| Reasoning control | `--reasoning-format`, `--reasoning-budget`, `--reasoning-effort`, `--reasoning-preserve` | **[V, S-03]** |
| Context management | `--ctx-size` (default: from model), `--ctx-checkpoints` (default 32), `--cache-ram` (default 8192 MiB), `--kv-unified`, `--cache-idle-slots`, `--context-shift` (default off) | **[V, S-03]** |
| Parallelism | `--parallel` (default auto). Nexora must force `1`. | **[V, S-03]** |
| Slot control | `POST /slots/{id}?action=save|restore|erase` | **[V, S-03]** |
| Idle unload | `--sleep-idle-seconds` (default disabled) | **[V, S-03]** |
| Router mode | `--models-dir`, `--models-preset`, `--models-max`, autoload — a multi-model manager | **[V, S-03]** |
| Auth | `--api-key` and `--api-key-file` (one key per line, `#` comments) | **[V, S-03]** |
| TLS | `--ssl-key-file`, `--ssl-cert-file` (loopback does not need it) | **[V, S-03]** |
| Timeouts | `--timeout` default 3600 s; `--sse-ping-interval` default 30 s | **[V, S-03]** |
| Web UI | Enabled by default; disable with `--no-webui` | **[V, S-03]** |
| Built-in agent tools | `--tools` exposes `read_file`, `write_file`, `edit_file`, `exec_shell_command`, `grep_search`, `file_glob_search`, `get_info`; `--agent` additionally enables the CORS proxy. **Both default off.** | **[V, S-03]** |
| MCP servers | `--mcp-servers-config` / `--mcp-servers-json`, stdio transport, child processes run with server privileges | **[V, S-03]** |
| Local media | `--media-path` serves local files via `file://` | **[V, S-03]** |
| Prompt logging | `--log-prompts-dir` writes prompts to disk | **[V, S-03]** |

The last four rows are the dangerous ones. Nexora's launcher must never pass `--tools`, `--agent`, `--mcp-servers-*`, `--media-path`, or `--log-prompts-dir`.

### 3.4 Authentication and loopback behaviour

| Fact | Grade |
|---|---|
| `--host` defaults to `127.0.0.1`; multiple comma-separated addresses are accepted, and overlapping addresses are documented as undefined behaviour. | **[V, S-03]** |
| `--cors-origins` defaults to `*`. Setting `localhost` reflects the Origin header only when it is localhost. With `--tools`/`--agent`, CORS is narrowed to localhost automatically. | **[V, S-03]** |
| `GET /health` (and `/v1/health`) is **public: no API-key check**. Returns 503 `Loading model` or 200 `{"status":"ok"}`. | **[V, S-03]** |
| The API-key check applies to the inference endpoints; the docs describe the key as the authentication mechanism for the server. | **[V, S-03]** |
| The server binds no firewall rule and has no concept of a LAN-only or public bind guard. Exposing `--host 0.0.0.0` is a one-flag mistake. | **[V, S-03]** |

`/health` being unauthenticated is acceptable on loopback but means it cannot be used as proof of identity, and it is reachable by any local process. It must not be the only health signal (§3.5).

### 3.5 Known defects that shape the design

These are **open upstream issues at the time of review**, not fixed defects. They are the strongest argument for a supervisor that distrusts the server.

| Issue | Effect | Design response |
|---|---|---|
| **#29498** — a client-set `X-Conversation-Id` disables disconnect-cancellation and accumulates uncapped resumable-stream sessions, a memory-DoS **[V, S-05]** | A caller can make cancellation stop working and grow memory without bound | Never send the header; strip unknown headers at the Rust boundary; cap request/response sizes; enforce single-slot **and** a per-request wall clock |
| **#27388** — server wedge mid-decode; `/health` returns OK but `/slots` hangs; process needs SIGKILL **[V, S-06]** | Health checks can lie | Health = `/health` **plus** a bounded 1-token generation probe at a known cadence; watchdog kills the process group on probe timeout |
| **#27604** — Vulkan: server-wide hang after a client aborts a streaming request while another is in flight **[V, S-07]** | Cancellation on Vulkan can hang the whole server | One in-flight request per process; serialise; never rely on abort semantics on Vulkan without a restart budget |
| **#19173** — `/v1/responses` cannot cancel a stream to stop generation **[V, S-08]** | Cancellation is endpoint-dependent | Use `/v1/chat/completions` only; do not use the Responses API for interactive turns |
| **#27733** — a trailing `<think>` tail can make the PEG parser discard an otherwise complete generation (Qwen3.8) **[V, S-09]** | A valid answer can be lost to a parser fault | Treat parsed content as an optimisation; always retain raw text; on parse failure, fall back to raw output and mark the turn unparsed |
| **Malformed or incomplete tool-call arguments** under some templates/grammars; very large tool schemas can hit repetition limits **[S, S-35]** | Tool calls that look valid but are truncated | Validate every call against the tool's JSON schema; reject on failure; keep schemas small and flat |

### 3.6 Cancellation semantics, as documented

There is **no dedicated cancel endpoint** for chat completions. The documented "control" endpoint (`POST /v1/chat/completions/control`) supports only `reasoning_end`, which forces the end of a reasoning block; it does not stop generation **[V, S-03]**. Practical cancellation options are:

1. **Close the SSE connection.** Disconnect-cancellation is intended to stop the task, but issue #29498 shows a way for it to be disabled, and issue #27604 shows a Vulkan hang on abort. **[V]**
2. **Erase the slot** (`POST /slots/{id}?action=erase`) to drop cached context. This is not a guaranteed stop. **[V, S-03]**
3. **Terminate the process group** and restart. This is the only cancellation with a hard guarantee. Cost: model reload (seconds), lost KV cache, and a visible "restarting" state.

Phase 1's 2-second escalation becomes: close stream → wait 300 ms for the process to report idle → erase slot → if still busy at 2 s, kill the process group and restart with backoff. The UI must show "stopping" and then "restarting", never a silent hang. See [`integration-design.md`](integration-design.md) §6.

### 3.7 Tauri 2 / Rust integration

Two viable shapes:

| Shape | How | Trade-off |
|---|---|---|
| **Sidecar + HTTP** (recommended) | Tauri spawns `llama-server.exe` as a managed child process; Rust talks to it with `reqwest` over loopback and parses SSE | Process isolation: a crash or hang is recoverable without taking the app down; the same client code works for Ollama and a future remote provider; language boundary is a documented HTTP contract |
| **In-process FFI** | Rust bindings to `libllama`/`libggml` | Lower overhead and no port, but a native crash kills the app, GPU errors become app errors, and every upstream ABI change forces a rebuild. Rejected for 1.0. |

The HTTP contract is a feature, not a compromise: it is what makes the `InferenceProvider` trait from Phase 1 real, and it is what allows the fallback runtime without rewriting the agent.

### 3.8 llama.cpp scorecard

| Criterion | Assessment | Grade |
|---|---|---|
| Windows x64 support | Native CPU/CUDA/Vulkan builds per rolling tag | **[V]** |
| CPU inference | First-class; AVX2 baseline | **[V]** |
| NVIDIA CUDA | Dedicated builds; driver floor differs by CUDA version | **[V]** |
| AMD / Vulkan | Vulkan broad; ROCm Windows narrow; no `HSA_OVERRIDE` | **[V]** |
| Model / quant compatibility | GGUF is its native format; Q2–Q8 and K/IQ variants | **[V]** |
| VRAM / RAM behaviour | Explicit offload control; Windows fallback can silently spill | **[V]** |
| Context window | Large contexts supported but KV-bound, not model-bound | **[V/E]** |
| Token speed | Not measured | **—** |
| Structured output + tools | JSON schema, GBNF, Jinja tools, reasoning extraction | **[V]** |
| Streaming / cancellation | Streaming solid; cancellation unreliable at the edges | **[V]** |
| Stability / recovery | Open wedge and hang issues; needs supervisor | **[V]** |
| Packaging / redistribution | MIT; CUDA runtime has its own EULA | **[V]** |
| Tauri/Rust integration | Clean via sidecar HTTP | **[V]** |
| Offline install/update | Fully offline once mirrored; no self-updater | **[V]** |
| Local server security | Loopback default, key auth, but permissive CORS and public `/health` | **[V]** |

**Recommendation: primary runtime.** No other candidate satisfies R-01…R-12 simultaneously.

## 4. Ollama — fallback and external provider

| Fact | Grade |
|---|---|
| MIT-licensed repository with 183k+ stars and daily activity. | **[V, S-10]** |
| Release `v0.40.6` published 2026-10-16T22:00:46Z with `ollama-windows-amd64.zip`, `ollama-windows-amd64-rocm.zip`, `ollama-windows-arm64.zip`, `OllamaSetup.exe`, `install.ps1`, and `sha256sum.txt`. | **[V, S-10]** |
| Ships a Windows installer that runs a local server and pulls models from a registry; exposes an HTTP API and an OpenAI-compatible surface. | **[V, S-10]** |
| Not designed to be an embedded sidecar: it is a background service with its own model store, its own update path, and its own port. | **[U]** — inferred from its install model; exact service behaviour and update policy were **not** verified. |

**Why it is not primary.** Nexora needs to own the runtime lifecycle (R-10), pin one build per release (R-12), and guarantee offline behaviour after a single download (R-02). A second service with its own store and updater duplicates model storage (multi-GB per model), adds a second update channel that Nexora does not control, and makes "which runtime produced this output?" ambiguous in bug reports. Its Windows ROCm build is also a separate artifact, widening the support matrix.

**Why it is still valuable.** If the pinned llama.cpp build regresses on a specific GPU/driver combination, Ollama gives users a working path without waiting for a Nexora release. It is also the lowest-effort way to run the benchmark protocol on a second runtime to check that conclusions are not llama.cpp-specific. It should be supported as an **externally detected provider**, never silently installed.

**Conditions if used** (`Q2-09`): bind to loopback only, verify the advertised port, record version per run, keep the model store separate and document the disk cost, and never assume its sampling defaults match llama.cpp's.

## 5. vLLM — rejected for desktop

vLLM's installation documentation lists hardware platforms as NVIDIA CUDA, AMD ROCm, Intel XPU, Apple Silicon (via vLLM-Metal), and CPU on x86/AArch64/Apple/IBM Z **[V, S-11]**. Windows is **not** a documented installation platform. The practical Windows route is WSL2, which:

- breaks the "native Windows desktop app" premise and complicates installer, GPU driver access, and support;
- adds a Linux VM's memory overhead on machines where VRAM is the scarce resource;
- cannot be bundled as a small sidecar;
- introduces a second OS update surface.

vLLM is an excellent server for throughput and batching, which is precisely the opposite of Nexora's single-user, single-slot, latency-sensitive workload. **Rejected for 1.0**, retained as a reference implementation for continuous batching ideas.

## 6. ONNX Runtime GenAI — deferred, with a real niche

| Fact | Grade |
|---|---|
| MIT licence; active repository; explicitly supports Windows with CPU, CUDA, DirectML, TensorRT-RTX, OpenVINO, QNN and WebGPU execution providers. | **[V, S-12]** |
| Positions itself as the engine behind Foundry Local, Windows ML, and the VS Code AI Toolkit. | **[V, S-12]** |

Its niche is Windows-native distribution with DirectML, which would cover AMD and Intel GPUs without Vulkan's driver variability. The blocker is **model format and coverage**: it consumes ONNX models, not GGUF. Nexora's shortlist is published as GGUF, and 2026 architectures (Qwen3.5 hybrid attention, `nemotron_h_moe`, `gemma4`) would each need conversion, validation, and a separate quantisation pipeline. That is a second model supply chain, not a drop-in.

**Decision:** defer. Re-evaluate if (a) DirectML stability becomes a support problem for Vulkan, or (b) official ONNX builds of the chosen models appear with tool-calling templates intact.

## 7. mistral.rs and candle — watchlist

| Fact | Grade |
|---|---|
| `mistral.rs` is MIT-licensed, actively developed, Rust-native, ~7.8k stars. | **[V, S-13]** |
| `candle` is a Rust ML framework, not a server; it provides primitives, not a product surface. | **[U]** |

`mistral.rs` is the most plausible **in-process** replacement for the sidecar in a later phase: no HTTP, no port, no external process. Against it: narrower quantisation/model coverage than llama.cpp, a much smaller user base to shake out GPU-specific bugs, and no equivalent of llama-server's documented OpenAI surface, slot control, and grammar tooling. In-process also reintroduces the failure mode the sidecar exists to contain: a native crash becomes an app crash.

**Decision:** watchlist. Reassess at Phase 6 (hardening) if the sidecar's process cost or packaging size becomes a problem.

## 8. Ten-dimension comparison

Legend: **A** adequate/strong, **B** workable with caveats, **C** weak or unverified, **—** not applicable or not evaluated.

| Dimension | llama.cpp | Ollama | vLLM | ONNX GenAI | mistral.rs | candle |
|---|---|---|---|---|---|---|
| Windows x64 support | **A** native | **A** native | **C** via WSL2 | **A** native | **A** Rust | **B** library |
| CPU inference | **A** | **A** | **B** | **A** | **B** | **B** |
| NVIDIA CUDA | **A** | **A** | **A** | **A** | **B** | **B** |
| AMD / Vulkan | **B** Vulkan; ROCm narrow | **B** Vulkan/ROCm builds | **B** ROCm on Linux | **B** DirectML | **C** | **C** |
| Model / quant compatibility | **A** GGUF Q2–Q8 | **A** GGUF via registry | **A** safetensors/FP8 | **C** ONNX only | **B** | **C** |
| VRAM / RAM behaviour | **A** explicit offload | **B** automatic | **B** PagedAttention | **B** | **B** | **C** |
| Context window | **A** model+KV bound | **A** | **A** | **B** | **B** | **C** |
| Token speed | not measured | not measured | not measured | not measured | not measured | not measured |
| Structured output + tools | **A** schema+grammar+Jinja | **B** format param | **A** guided decoding | **B** | **B** | **C** |
| Streaming / cancellation | **B** no cancel endpoint | **B** | **A** abort | **B** | **B** | **C** |
| Stability / recovery | **B** open wedge issues | **B** opaque | **A** server-grade | **B** | **C** | **C** |
| Packaging / redistribution | **A** MIT | **A** MIT | **A** Apache | **A** MIT | **A** MIT | **A** |
| Tauri/Rust integration | **A** sidecar HTTP | **A** HTTP | **C** VM | **B** bindings | **B** in-process | **C** |
| Offline install/update | **A** mirrored | **B** registry pull | **C** | **B** | **B** | **C** |
| Local server security | **B** key + loopback, weak CORS | **C** unverified auth | **B** | **—** no server | **—** | **—** |

No row scores token speed, because nothing was measured. Any table that fills that cell from a vendor blog is a table to distrust.

## 9. Recommendation

**Primary:** `llama.cpp`, pinned build `b11714`, shipped as three runtime packs:

| Pack | Contents | Size class | When installed |
|---|---|---|---|
| CPU | `llama-*-bin-win-cpu-x64.zip` | tens of MB **[E]** | Always (baseline that cannot fail on GPU absence) |
| NVIDIA | `llama-*-bin-win-cuda-12.4-x64.zip` + matching `cudart` package | hundreds of MB **[E]** — Phase 1 budgeted 300–600 MB | On demand, when an NVIDIA GPU with a sufficient driver is detected |
| Vulkan | `llama-*-bin-win-vulkan-x64.zip` | tens of MB **[E]** — Phase 1 budgeted 30–60 MB | On demand, for AMD/Intel GPUs |

The CUDA 13.4 pack is not shipped at 1.0. It can be added later for GPUs whose drivers require it. Shipping one CUDA major version keeps the download and the support matrix small (`P2-D-02`).

**Fallback:** Ollama `v0.40.6`, detected as an external provider, never bundled by default.

**Reversal gate.** If, during the Phase 3 contract tests, any of the following is true on two or more REF machines, re-open the runtime decision:

1. The pinned build cannot complete a 4,000-token prompt without a wedge (#27388-class) in 20 consecutive runs.
2. Cancellation requires a process kill in more than 1 in 20 attempts after the 2 s escalation.
3. Tool-call validity on the standard 40-task set is below 95% for reasons attributable to the runtime parser rather than the model.
4. The CUDA pack fails to initialise on a driver version that NVIDIA lists as supported for CUDA 12.4.
5. Installer size or DLL conflicts prevent a clean per-user install on a machine without Visual Studio runtimes.

**Limitations of this evaluation.** It is documentary. It did not execute a single token. It cannot detect a regression in build `b11714`, a Windows-specific memory leak, a driver-specific hang, or a template mismatch between a model's chat template and llama.cpp's parser. Those are exactly what the Phase 3 contract tests and the benchmark protocol exist to find.

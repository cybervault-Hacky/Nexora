# 04 — Local AI Runtime and Model Strategy

**Status:** Runtime proposed; model selection deliberately deferred to Phase 2 benchmarks  
**Related:** [03-application-architecture.md](03-application-architecture.md), [05-coding-agent-workflow.md](05-coding-agent-workflow.md), [11-hardware-benchmarking.md](11-hardware-benchmarking.md)

---

## 1. Requirements

The local inference layer must:

1. Run entirely on the user's Windows PC after model download, with no paid external API required.
2. Support GGUF quantized models on CPU and on NVIDIA, AMD, and Intel GPUs where practical, with automatic fallback.
3. Stream tokens with low latency and support cancellation.
4. Support tool calling and schema-constrained structured output reliably enough for an agent.
5. Fit models into 16–24 GB VRAM tiers without silent quality collapse, and degrade explicitly when it cannot.
6. Expose a stable internal interface so the runtime and model can change without rewriting the agent or UI.
7. Keep the runtime process sandboxed, loopback-only, authenticated, and replaceable.
8. Be licence-compatible with commercial desktop software.
9. Be pinnable and verifiable, so an upstream update cannot silently change behaviour.
10. Recover from crashes, OOM, corrupt files, and driver resets without killing the application.

Non-requirements at 1.0: training, fine-tuning, distributed inference, multi-GPU clusters, tensor parallelism, model serving to other machines, and mobile.

---

## 2. Runtime landscape

| Runtime | Licence | Windows | Backends | GGUF | Native tools | Structured output | Model mgmt | Fit for Nexora |
|---------|---------|---------|----------|------|--------------|-------------------|------------|----------------|
| **llama.cpp / llama-server** | MIT | Native binaries | CPU, CUDA, Vulkan, HIP, SYCL | Yes (primary) | Yes via chat template + parsers | GBNF, JSON schema | None (build our own) | **Primary candidate** |
| **Ollama** | MIT | Native | Wraps llama.cpp; CUDA/ROCm/CPU | Yes | Yes | Via llama.cpp | Yes (registry, pull) | Good UX, weak embedding control |
| **LM Studio** | Proprietary | Native | llama.cpp/Metal/etc. | Yes | Yes | Yes | Yes | Not redistributable; reject |
| **vLLM** | Apache 2.0 | Linux-first; WSL only | CUDA/ROCm | Limited | Yes | Guided decoding | Partial | Server-class; reject for desktop |
| **TensorRT-LLM** | Apache 2.0 | Windows builds complex | NVIDIA only | No | Yes | Constrained decoding | Partial | High performance, high integration cost; defer |
| **ONNX Runtime GenAI** | MIT | Native | CPU, DirectML, CUDA | No (ONNX) | Partial | Partial | Partial | Watchlist |
| **mistral.rs** | MIT | Rust, CUDA/CPU | CUDA, Metal, CPU | Yes | Partial | Partial | Partial | Watchlist |
| **candle** | MIT/Apache | Rust, CUDA/CPU | CUDA, Metal | Partial | No server | Partial | No | Embedding/experiments only |
| **MLC LLM** | Apache 2.0 | Via TVM | Vulkan/CUDA | Compiled | Partial | Partial | Partial | Reject (complexity) |
| **ExLlamaV2** | MIT | NVIDIA-focused | CUDA | EXL2 | No | Partial | No | Reject (format/coverage) |

### 2.1 Recommendation: llama.cpp `llama-server` as a pinned sidecar

**Advantages**

- MIT licence; mature C/C++ codebase; active releases; Windows CPU/CUDA/Vulkan binaries.
- GGUF is the broadest quantization ecosystem, including MoE models and K-quants.
- OpenAI-compatible `/v1/chat/completions`, SSE streaming, `--jinja` chat templates, tool-call parsers, GBNF grammars, JSON-schema response format, speculative decoding, and prompt caching.
- CPU/GPU hybrid offload (`-ngl`) is essential for 8–16 GB VRAM machines.
- A separate process gives crash isolation and lets us kill the GPU context without killing the UI.
- We control flags, context size, KV cache type, batching, and logging — necessary for hardware-aware tuning.

**Disadvantages and mitigations**

| Disadvantage | Mitigation |
|--------------|------------|
| Rapid upstream changes | Pin a release per app version; run contract tests; upgrade deliberately with benchmarks |
| No built-in model manager | Build registry, downloader, verification, and storage in `nexora-inference` |
| Tool-call parsing can be model-specific and occasionally malformed | Validate every call; prefer grammar-constrained envelopes when native parsing is unreliable |
| Server defaults are not hardened | Loopback, random port, bearer token file, no web UI, no experimental agent tools, job object |
| Windows GPU variance | Ship separate CUDA/Vulkan/CPU runtime packs; detect and fall back |
| Long-context KV memory is large | Context presets, KV quantization, explicit memory math, capacity warnings |
| Cancellation semantics need verification | Phase 2 spike tests close-connection, slot erase, and forced restart |

**Alternatives considered**

- **Ollama as the engine:** excellent developer UX and model pulling, but it runs an opinionated daemon with its own ports and lifecycle, gives less control over flags and quantization, updates independently, and complicates clean uninstall/security. It remains a candidate as an **optional external provider** for users who already run it, not the bundled engine.
- **LM Studio:** proprietary; cannot be bundled or redistributed. Reject.
- **vLLM:** excellent throughput, but Linux/WSL and GPU-only; wrong shape for a Windows desktop app. Could be a future remote provider.
- **TensorRT-LLM:** potentially faster on NVIDIA, but engine builds are GPU-specific, integration is heavy, and AMD/Intel/CPU coverage is absent. Defer; the `InferenceProvider` interface allows it later.
- **ONNX Runtime GenAI:** promising cross-platform path and Microsoft alignment, but model coverage, quantization, and tool-call parity lag the GGUF ecosystem. Watchlist.

### 2.2 Provider abstraction

```rust
trait InferenceProvider {
    async fn capabilities(&self) -> ModelCapabilities;
    async fn load(&mut self, config: LoadConfig) -> Result<LoadReport>;
    async fn stream_chat(&self, req: ChatRequest, sink: DeltaSink)
        -> Result<Completion>;
    async fn cancel(&self, request_id: RequestId) -> Result<()>;
    async fn embed(&self, req: EmbedRequest) -> Result<Vec<Vec<f32>>>;
    async fn health(&self) -> HealthStatus;
    async fn unload(&mut self) -> Result<()>;
}
```

Implementations: `LlamaCppProvider` (bundled), `OllamaProvider` (optional external, post-1.0), `OpenAiCompatibleProvider` (optional BYO endpoint, off by default, deferred). The agent depends only on the trait and `ModelCapabilities`; it must not know which runtime is active.

---

## 3. Runtime packaging and Windows backends

### 3.1 Runtime packs

| Pack | Contents | Approx. size | When installed |
|------|----------|--------------|----------------|
| CPU | `llama-server.exe`, ggml CPU DLLs, VC++ runtime | 30–60 MB | Always, bundled |
| CUDA 12.x | CUDA build + redistributable runtime DLLs | 300–600 MB | On demand when NVIDIA + compatible driver |
| Vulkan | Vulkan build | 30–60 MB | On demand for AMD/Intel or NVIDIA fallback |
| HIP/ROCm | AMD build | 400–800 MB | Later phase, only if validated |

- GPU packs are **downloaded on demand**, hash-verified, and stored per runtime version, so the installer stays small and updates do not re-download unchanged packs.
- DLL loading uses absolute paths and `SetDefaultDllDirectories(LOAD_LIBRARY_SEARCH_DEFAULT_DIRS | LOAD_LIBRARY_SEARCH_APPLICATION_DIR)` to reduce DLL-hijack risk. The app never adds the current directory or user-writable folders to the DLL search path.
- CUDA runtime redistribution terms and VC++ runtime redistribution are reviewed before shipping. If CUDA redistribution is not acceptable, the fallback is a Vulkan-first build or requiring the NVIDIA driver's compatible runtime.
- Runtime packs are signed as part of the release, and each file is listed in the signed manifest with a SHA-256.

### 3.2 Backend selection

```
Detect GPU vendor/VRAM/driver
├─ NVIDIA + driver >= minimum + CUDA pack available → CUDA
├─ Vulkan device with enough VRAM → Vulkan
├─ Integrated GPU with shared memory → CPU (or Vulkan if measured faster)
└─ Otherwise → CPU
```

Selection is a **starting point**, not a promise. Phase 2 benchmarks compare CUDA vs Vulkan vs CPU on the same model and prompt; the app exposes a manual override with a warning. On hybrid laptops, the app prefers the discrete GPU only when the power profile and driver allow it, and explains battery impact.

### 3.3 Runtime version pinning

- One pinned llama.cpp release per Nexora release, recorded in `runtime-manifest.json` with build flags and hashes.
- Contract tests run against the pinned build: health, chat, streaming, stop, tool call, JSON schema, long context, unicode, cancellation.
- Upgrades require: release notes review, contract tests, benchmark regression, security review, and a staged rollout.
- The app can run a **previous runtime pack** if the new one fails health checks (rollback).

---

## 4. Model evaluation criteria and shortlist

### 4.1 Criteria

| Criterion | Weight | What is measured |
|-----------|--------|------------------|
| Coding and agentic quality | 30 | Nexora Coding Eval pass rate, SWE-bench-style tasks, multi-file edits, tool validity |
| Tool-call and structured-output reliability | 15 | Valid call rate, schema compliance, recovery from malformed output |
| Speed on reference hardware | 15 | Prefill tok/s, decode tok/s, time to first token, at 0/8K/32K context |
| Memory fit and stability | 10 | VRAM/RAM at target context, OOM rate, load time |
| Licence and redistribution | 10 | Commercial use, notice obligations, weight redistribution, acceptable-use terms |
| Runtime compatibility | 8 | llama.cpp support, GGUF availability, chat template, tokenizer quirks |
| Context quality | 7 | Retrieval and repo comprehension at 32K/128K, degradation beyond trained range |
| Ecosystem and maintenance | 5 | Release cadence, community, quant quality, known issues |

A model must pass **hard gates** before scoring: commercial licence verified; runs on the pinned runtime; valid tool calls above threshold; no known critical security issue; reproducible downloads; acceptable behaviour on honesty/adversarial evals.

### 4.2 Shortlist (facts as of 2026-10-17; not a final selection)

| Model | Architecture | Context | Licence | Q4 footprint (approx.) | Provisional role | Notes / caveats |
|-------|--------------|---------|---------|------------------------|------------------|-----------------|
| **Qwen3-Coder-30B-A3B-Instruct** | 30.5B MoE, 3.3B active, 48 layers, 128 experts/8 active, GQA 32Q/4KV | 262,144 native; up to 1M with YaRN | Apache 2.0 | ~18–20 GB Q4_K_M | Default candidate for 24 GB GPUs | Agentic-coding training and function-call format; 24 GB card leaves limited KV headroom at 32K |
| **Qwen3.8-27B** | 27.8B dense multimodal, hybrid attention (Gated DeltaNet + full attention), 64 layers | 262,144 native; 1M YaRN | Apache 2.0 | ~16.5–17 GB Q4 | High-quality candidate for 24 GB | Released Aug 2026; strong vendor coding claims; runtime/GGUF and tool-call support must be independently verified |
| **Devstral Small 2 24B** | 24B dense, agentic coding, vision input | 256K | Apache 2.0 | ~15 GB Q4 | Agentic alternative for 24 GB | Designed for tool use and multi-file edits; Mistral hosted API deprecated, open weights remain |
| **gpt-oss-20b** | 21B MoE, 3.6B active, MXFP4 | 128K | Apache 2.0 | ~12–14 GB | Default candidate for 16 GB GPUs | Strong reasoning/tool use; less code-specialized than Qwen/Devstral; Harmony format support must be verified in llama.cpp |
| **Qwen3-Coder-Next** | 80B MoE, ~3B active (per official Qwen README) | 256K | Verify model card (Qwen family is generally Apache 2.0) | ~45–50 GB Q4 | High-end 48 GB+ tier only | GGUF available; too large for mainstream 24 GB machines without heavy offload |
| **Qwen2.5-Coder-7B-Instruct** | 7B dense | 32K native (verify) | Apache 2.0 | ~4.5–5 GB | 8 GB fallback / fast mode | Older but proven; agentic reliability lower; good for chat and small edits |
| **Codestral 22B** | 22B dense | 256K | Mistral Non-Production Licence | ~13 GB | **Excluded** | Licence forbids commercial/production use; not acceptable for Nexora |
| **Gemma 4 31B** | 31B dense | 256K (verify) | Gemma Terms of Use | ~19 GB | Watchlist | Quality is competitive, but terms include use restrictions and redistribution obligations; legal review required |
| **Llama 4 Scout** | 109B MoE, 17B active | 10M claimed | Llama 4 Community Licence | ~60 GB+ | Not planned | Licence and memory profile are a poor fit for desktop 1.0 |

**Important:** vendor benchmark claims are marketing until independently measured. Several 2026 models also have limited or inconsistent GGUF/tool-call support at the time of writing. Phase 2 must benchmark actual quantized files on the pinned runtime, not the BF16 model cards.

### 4.3 Why no model is selected now

- Vendor benchmarks do not predict performance on 16–24 GB consumer GPUs with quantized KV caches.
- Tool-call reliability varies materially between base models, chat templates, and runtime parsers.
- The 2026 landscape is changing monthly; a model chosen in October may be superseded or deprecated by launch.
- Licence terms for some candidates require legal review.
- The final choice depends on the owner's minimum hardware tier and download-size budget (Q-02, Q-03).

**Provisional architecture defaults** (not a product promise): 16 GB VRAM → gpt-oss-20b or Qwen3-Coder-30B-A3B at IQ3/Q3 with 8–16K context; 24 GB VRAM → Qwen3-Coder-30B-A3B Q4 or Qwen3.8-27B Q4 with 16–32K; 48 GB+ → Qwen3-Coder-Next Q4 or a 30B model with longer context; 8 GB → Qwen2.5-Coder-7B Q4 with a clear "light mode" warning.

---

## 5. Hardware tiers and model fit

| Tier | Reference hardware | Practical model class | Context target | Expected experience |
|------|--------------------|-----------------------|----------------|---------------------|
| **Minimum** | 16 GB RAM, 8 GB VRAM or CPU-only, 4-core CPU | 7B Q4 | 8–16K | Chat, small edits, simple static sites; agent loops slow and less reliable |
| **Recommended** | 32 GB RAM, 12–16 GB VRAM (RTX 3060 12GB / 4060 Ti 16GB) | 20–30B MoE at Q3/Q4 or 20B MXFP4 | 8–16K | Useful agentic coding; some context pressure; 30B may be tight |
| **Performance** | 64 GB RAM, 24 GB VRAM (RTX 3090/4090) | 27–30B Q4 | 16–32K | Primary target for the full experience |
| **Enthusiast** | 128 GB RAM, 32–48 GB+ VRAM (RTX 5090 or workstation) | 80B MoE Q4 or 30B with long context | 32–128K | Best local quality; still not frontier-hosted quality |

**Reality checks**

- VRAM capacity is not performance: memory bandwidth, compute, driver maturity, and thermals matter more than the marketing tier.
- Laptop GPUs throttle; an "RTX 4090 laptop" is not a desktop 4090.
- Integrated GPUs share system RAM; a 16 GB laptop may have less usable model memory than a 16 GB discrete GPU.
- A more expensive PC does not guarantee a model runs well; the model must fit with KV cache, compute buffers, display, and OS overhead.
- CPU-only inference is supported for chat and small edits but is not a good experience for long agentic loops; the UI must say so.

---

## 6. Memory and context math

### 6.1 Budget formula

```
VRAM_needed ≈ weights
            + KV_cache
            + compute_buffers (0.5–2.0 GB, model/backend dependent)
            + display/OS reserve (1.0–2.5 GB)
            + safety_headroom (0.5–1.0 GB)
```

KV cache per token (FP16):

```
2 (K and V) × n_layers × n_kv_heads × head_dim × 2 bytes
```

Quantized KV roughly scales by bytes per element: Q8_0 ≈ 1.06 bytes, Q4_0 ≈ 0.56 bytes, plus alignment overhead.

**Worked example — Qwen3-Coder-30B-A3B-Instruct** (48 layers, 4 KV heads, head_dim 128):

- FP16 KV per token = 2 × 48 × 4 × 128 × 2 = **98,304 bytes ≈ 96 KiB/token**
- 32K context ≈ **3.0 GiB**; Q8_0 ≈ **1.6 GiB**; Q4_0 ≈ **0.9 GiB**
- Q4_K_M weights ≈ 19 GB → with Q8 KV at 32K, a 24 GB card is at the edge before compute buffers and display reserve.

This is why "it fits in 24 GB" is not enough. The app must choose context and KV precision from a measured budget, not a static table.

**Worked example — gpt-oss-20b:** MXFP4 weights ~13 GB; 16 GB cards should default to 8–16K context, not the full 128K. The app will show the estimated KV cost before loading.

### 6.2 Automatic configuration

1. Probe VRAM/RAM and current usage.
2. Estimate weights from file size.
3. Compute the largest context that fits a target headroom (default 1.5 GB VRAM free).
4. Choose KV precision: Q8_0 first, Q4_0 only if needed; warn on Q4.
5. Choose GPU layers: all layers if it fits; otherwise offload as many as possible; last resort CPU.
6. Validate by loading and running a short warm-up; if OOM, step down (context → KV precision → GPU layers → CPU) and report the final configuration.
7. Persist the successful profile per model + GPU + driver; re-evaluate after driver or app updates.

### 6.3 Context management

| Budget item (32K example) | Tokens | Notes |
|---------------------------|--------|-------|
| System and policy | 1,200–1,800 | Stable prefix for cache reuse |
| Tool schemas | 1,200–2,000 | Compact descriptions only |
| Task state / plan | 1,500–2,500 | External journal is authoritative |
| Repository map | 2,000–4,000 | Symbols, paths, imports |
| Retrieved code | 6,000–10,000 | AST-aware chunks with provenance |
| Conversation | 6,000–8,000 | Recent turns + summary |
| Output reserve | 3,000–6,000 | Tool calls and patches |
| Safety reserve | 1,000–2,000 | Avoid overflow mid-turn |

For 16K contexts, halve retrieval and conversation, keep the task state and tool schemas, and prefer summaries over raw history. For 8K, run a "compact agent" mode with fewer tools and stricter step limits.

**Compaction:** when usage exceeds 80%, summarize older turns into a structured memory object (goal, decisions, files touched, commands run, failures, open questions) and keep the last 4–6 turns verbatim. Compaction is visible in the UI and never silently drops the task journal. The journal lives in SQLite, not in the prompt.

**Prompt caching:** order prompts from most stable to least stable (system → tools → repo map → retrieved context → conversation → current turn) to maximize prefix reuse. llama.cpp slot reuse and `cache_prompt` behaviour are verified in the Phase 2 spike.

**Long context is not free:** quality can degrade beyond the trained/validated range, attention cost grows, and KV memory can exceed the GPU. Nexora will label context sizes as "tested" vs "maximum" and default to tested values.

---

## 7. Streaming

- **Transport:** HTTP SSE to `POST /v1/chat/completions` with `stream: true` and usage reporting where supported.
- **Parsing:** a Rust SSE parser handles multi-line events, comments, partial UTF-8, and `[DONE]`; malformed events are logged and skipped, not fatal.
- **Delta flow:** accumulate text in the provider, emit a `Delta` every 16–33 ms or every 16–64 tokens, whichever comes first. The UI appends to a buffer and flushes on `requestAnimationFrame`.
- **Finish reasons:** `stop`, `length`, `tool_calls`, `content_filter`, `error`; the UI maps each to a clear state. A length stop is shown as "truncated", never "complete".
- **Usage:** prompt/completion tokens from the server when available; otherwise count with the model tokenizer in Rust. Usage is stored per message and per run.
- **Cancellation:** the UI sends `cancel(request_id)`; the provider closes the HTTP stream immediately and marks the message "stopping". If the server does not stop within 2 s, Nexora sends a slot-erase request if supported; otherwise it restarts the inference process and marks the run interrupted. The Phase 2 spike must confirm the exact semantics of the pinned llama.cpp release.
- **No mid-stream reconnect:** reconnecting would duplicate or lose tokens. A dropped stream ends the turn with a recoverable error and a "Retry" action that starts a new turn from the persisted conversation.
- **Backpressure:** bounded channel (e.g., 256 deltas); if the UI is slow, deltas are coalesced rather than dropped mid-grapheme. The final text is always reconciled from the accumulated buffer.

---

## 8. Tool calling and structured output

### 8.1 Strategy

Use a **two-tier** approach:

1. **Native tool calling** via the model's chat template (`--jinja`) and the runtime's tool-call parser, when the model+runtime pair passes the tool-call contract tests.
2. **Grammar-constrained fallback** using a JSON schema or GBNF envelope when native parsing is unreliable or the model lacks native support.

Both paths produce the same internal `ToolCall` structure, validated in Rust before any policy decision.

### 8.2 Contract

```json
{
  "name": "fs.write",
  "arguments": {
    "path": "src/components/Hero.tsx",
    "content": "...",
    "expected_hash": "sha256:..."
  },
  "call_id": "call_01..."
}
```

Rules:

- Tool names are allowlisted; unknown names are rejected with a structured error returned to the model.
- Arguments are validated against a JSON Schema with strict types, size limits, and path rules.
- `expected_hash` is required for edits to existing files to prevent stale writes.
- Mutating calls are serialized; read-only calls may batch (max 4) if the model supports it.
- The model never receives raw shell, arbitrary executable paths, environment variables, or credentials.
- A tool call is shown in the UI as a card as soon as it validates; execution status follows.

### 8.3 Reliability techniques

- Keep schemas compact; large grammars can hit parser limits and degrade output.
- Validate and repair only mechanical JSON issues (whitespace, trailing commas) with a strict parser; do not let the model "heal" arbitrary malformed JSON by inventing fields.
- On invalid output: one retry with the validation error appended, then stop and report. Do not loop indefinitely.
- Temperature 0–0.2 for tool calls and patches; higher only for ideation.
- Repetition detection: stop on repeated n-grams, repeated identical tool calls, or identical error signatures.
- Grammar constraints can reduce fluency; use them for the envelope, not for free-form code content unless necessary. Code content should be plain text inside a validated field.

### 8.4 Structured plans and reports

Plans and final reports use JSON schemas, but the UI never depends on perfect JSON for safety. If schema validation fails after one retry, the agent falls back to a plain-text report and marks the structured fields unavailable. Execution decisions always come from validated tool calls, never from a free-text claim.

---

## 9. Model catalogue, download, verification, and updates

### 9.1 Manifest

```json
{
  "manifest_version": 1,
  "model_id": "qwen3-coder-30b-a3b-instruct-q4_k_m",
  "display_name": "Qwen3-Coder 30B",
  "family": "Qwen3-Coder",
  "parameters": "30.5B total / 3.3B active",
  "architecture": "qwen3moe",
  "quantization": "Q4_K_M",
  "context_native": 262144,
  "context_tested": 32768,
  "licence": "Apache-2.0",
  "licence_url": "https://huggingface.co/Qwen/Qwen3-Coder-30B-A3B-Instruct",
  "capabilities": ["chat", "tools", "json_schema", "code"],
  "requirements": {
    "min_ram_gb": 24,
    "min_vram_gb": 12,
    "recommended_vram_gb": 24,
    "disk_gb": 20
  },
  "files": [
    {
      "url": "https://huggingface.co/.../resolve/<commit>/model-Q4_K_M.gguf",
      "sha256": "...",
      "size": 0
    }
  ],
  "runtime_min_version": "bXXXX",
  "chat_template": "qwen3-coder",
  "tool_format": "native_qwen",
  "release_date": "2025-07-31",
  "status": "approved"
}
```

- Manifests are fetched over HTTPS from a Nexora-controlled endpoint or bundled at release; each manifest set is signed (Ed25519/minisign). The app verifies the signature before trusting hashes.
- URLs pin a commit/revision, not `main`.
- `context_tested` is a Nexora measurement, not the vendor maximum.
- `status` can be `approved`, `experimental`, `deprecated`, or `blocked`.

### 9.2 Download pipeline

```
licence shown → disk check → size/ETA → download to .part (resumable)
→ SHA-256 verify while streaming → atomic rename
→ GGUF header validation → runtime load test (optional)
→ mark installed → record in SQLite
```

- Resume uses HTTP Range; a changed ETag or content length invalidates the partial file.
- Disk check reserves `size × 1.1` plus temp space; refuses to start if insufficient.
- Downloads are user-initiated, pausable, cancellable, and never run silently in the background.
- Metered connections are detected via Windows network APIs; the app warns before a large download.
- Corrupt or mismatched files are quarantined, not deleted silently; the user can retry or remove.
- Model files are treated as untrusted input to the runtime; the app does not execute them and validates magic/header/size before load.

### 9.3 Storage layout

```
%LOCALAPPDATA%\Nexora\models\
  qwen3-coder-30b-a3b-instruct\
    Q4_K_M\
      <commit>\
        model.gguf
        manifest.json
        LICENSE.txt
```

Content-addressed dedupe is optional; clarity and safe deletion matter more than saving a few GB. The model manager shows exact disk usage and allows relocation to another drive.

### 9.4 Updates

- Model updates are explicit and separate from app updates.
- A new version downloads alongside the old one, verifies, and only then becomes selectable; the user can roll back.
- Deprecated models remain usable but are marked "no longer recommended" with a reason.
- The catalogue is cached for offline display; no update check occurs when offline mode is enabled.

---

## 10. Embeddings and project retrieval

Local retrieval improves quality and reduces context pressure. Design:

- **Primary:** lexical search (ripgrep-style) + symbol index from tree-sitter + import graph. Works offline, deterministic, no extra model.
- **Optional semantic:** a small embedding model (e.g., a 100–300M parameter text embedding model with a permissive licence) run through llama.cpp's embeddings endpoint or a separate small process.
- **Storage:** vectors in SQLite (`sqlite-vec` extension) or a compact local index; chunk metadata includes path, symbol, start/end line, and hash.
- **Chunking:** AST-aware 40–80 line chunks with 10–20% overlap; fall back to fixed windows for unsupported languages.
- **Indexing:** incremental on file change, debounced; respect `.gitignore` and user exclusions; skip binaries and files > 1 MB; cap total indexed tokens per project.
- **Privacy:** the index is local and contains code; it is deleted with the project data and excluded from diagnostics by default.
- **Cost:** embedding a 50k-file project must not block the UI; background queue with progress and pause.
- **Evaluation:** measure retrieval hit rate on the Nexora eval tasks before making semantic retrieval a default. Lexical + symbol context may be sufficient and cheaper.

Embedding model choice is **deferred** to Phase 2/4. The architecture exposes a `ContextProvider` trait so lexical, symbol, and vector retrieval can be combined.

---

## 11. Benchmarking before final model selection

The full methodology is in [11-hardware-benchmarking.md](11-hardware-benchmarking.md). Summary:

### 11.1 Hardware benchmarks

- `llama-bench`-style measurements at 0, 8K, and 32K context: prompt processing (pp512), token generation (tg128), time to first token, peak VRAM (NVML/DXGI), system RAM, load time.
- Real agent traces, not just synthetic tokens: multi-turn tool loops, patch generation, build/test cycles.
- Three reference machines minimum: 12 GB VRAM, 16 GB VRAM, 24 GB VRAM; plus a CPU-only baseline.
- Thermal/power state recorded; laptop results separated from desktop results.

### 11.2 Quality benchmarks

- **Nexora Coding Eval (NCE):** 40–60 tasks across static site, React app, full-stack, 3D/animation, bug fix, refactor, and adversarial honesty.
- Metrics: task success, build success, test success, patch-apply success, tool-call validity, repair cycles, tokens, wall time, false-success rate, destructive-action rate.
- External benchmarks (SWE-bench Verified, LiveCodeBench, Terminal-Bench, Aider polyglot) are recorded for context but are not decisive; contamination and harness differences make them unreliable for product selection.
- Blind human review of 20 outputs per model for visual quality, code quality, and instruction following.

### 11.3 Selection rule

A model is eligible only if it passes hard gates (licence, runtime, tool validity ≥ 98%, false-success rate 0 in adversarial tests). Eligible models are ranked by the weighted scorecard. The best model per hardware tier may differ. Final selection happens at the end of Phase 2, with owner approval, and is re-evaluated at each major release.

---

## 12. Failure modes and recovery

| Failure | User-visible symptom | Automatic response | Escalation |
|---------|----------------------|--------------------|------------|
| Model file corrupt | Load fails immediately | Reject, offer re-download | Diagnostics entry |
| Insufficient VRAM at load | Load error/OOM | Step down context/KV/layers; retry | CPU fallback with warning |
| OOM during long context | Stream error | Compact context, reduce to tested size, retry once | Ask user to start a new task |
| Context overflow | Request rejected | Compact + truncate retrieval, retry once | Explain and offer a smaller task |
| Malformed tool call | Tool card error | One validation retry | Stop and report; no execution |
| Repeated identical tool call | Loop | Detect and stop | Report the loop with evidence |
| Runtime hang | No tokens | Timeout, kill job, restart once | Safe mode / CPU |
| Driver reset | Crash | Restart, reduce GPU usage | CPU fallback |
| Tokenizer mismatch | Garbled output/usage | Reject model profile | Mark model incompatible |
| Unsupported chat template | Tool/role errors | Fall back to generic template | Disable native tools for that model |
| Disk full during download | Download fails | Pause, preserve partial, explain | Free space and resume |
| Network loss | Download/install fails | Pause and retry | Offline mode remains usable |

Every automatic recovery is logged with the decision and the resulting configuration, and surfaced in the task report when it affects results.

---

## 13. Licensing and compliance

| Component | Licence | Obligation / risk | Action |
|-----------|---------|-------------------|--------|
| llama.cpp | MIT | Include copyright/licence notice | Bundle notice |
| Qwen3-Coder / Qwen3.8 | Apache 2.0 | Notices, patent grant, no trademark misuse | Include licence; do not imply endorsement |
| Devstral Small 2 | Apache 2.0 | Notices | Include licence |
| gpt-oss-20b | Apache 2.0 | Notices | Include licence |
| Gemma | Gemma Terms of Use | Use restrictions, redistribution terms | Legal review before inclusion |
| Llama 4 | Llama Community Licence | 700M MAU clause, acceptable-use policy | Avoid at 1.0 |
| Codestral | Mistral Non-Production Licence | Prohibits production/commercial use | Exclude |
| CUDA runtime | NVIDIA EULA | Redistribution list restrictions | Legal review; bundle only permitted files |
| Node.js | MIT | Notice | Bundle notice |
| Playwright | Apache 2.0 | Notice | Notice |
| Edge/WebView2 | Microsoft terms | Distribution/automation terms | Use installed runtime; no redistribution |
| Fonts (Inter, JetBrains Mono) | SIL OFL | Licence inclusion, reserved names | Bundle OFL texts |

- The app ships `THIRD_PARTY_NOTICES.md` and a Settings → About → Licences view.
- Models are downloaded by the user, not bundled, which simplifies redistribution but does not remove licence obligations for use.
- A licence gate blocks download until the user acknowledges the model's terms.
- Legal review is required before enabling any model whose terms are not clearly permissive.

---

## 14. Decisions and trade-offs

| ID | Decision | Recommendation | Alternatives | Status | Revisit if |
|----|----------|----------------|--------------|--------|------------|
| D-02 | Inference runtime | llama.cpp `llama-server` sidecar | Ollama, vLLM, TensorRT-LLM, ONNX GenAI | Proposed | Phase 2 contract/performance spike fails |
| D-16 | Runtime distribution | Bundle CPU pack; download GPU packs on demand | Bundle all packs; require user install | Proposed | Offline GPU install becomes a hard requirement |
| D-17 | Model distribution | Download from pinned HF revisions with signed manifest + SHA-256 | Bundle weights; trust HF at runtime | Proposed | HF availability/licensing changes |
| D-18 | Model selection | Benchmark shortlist; choose per tier | Pick one model now | Deferred | Owner sets hardware floor and download budget |
| D-19 | Tool calling | Native first, grammar fallback | Grammar-only; native-only | Proposed | Tool validity < 98% on shortlist |
| D-20 | Quantization | Q4_K_M / Q5_K_M dense, Q4_K_M MoE; Q8_0 KV default | Q2/Q3 for small GPUs; FP16 | Proposed | Quality/memory benchmarks show better trade-off |
| D-21 | Embeddings | Lexical + symbols first; semantic optional | Semantic-first | Deferred | Retrieval hit rate is insufficient |
| D-22 | External providers | Optional Ollama/BYO, off by default, post-1.0 | Never; core dependency | Deferred | Owner approves product position |

**Trade-off summary:** llama.cpp sacrifices some peak throughput and polish compared with vendor-specific engines, but gives the broadest hardware coverage, the best GGUF ecosystem, a permissive licence, and full process control. A 30B model at Q4 sacrifices some quality versus BF16 but is the only way to fit mainstream 24 GB GPUs; the UI must communicate the difference rather than hide it.

---

## 15. Open questions

- Q-02: What is the minimum supported GPU/RAM tier? This determines whether 7B fallback is first-class or best-effort.
- Q-03: What is the maximum acceptable default download size (10 GB, 20 GB, 40 GB)?
- Q-13: Is a CPU-only mode officially supported at 1.0, and with what expectations?
- Q-14: Is a semantic index acceptable by default, or opt-in due to disk/CPU cost?
- Q-15: Should Nexora support an external Ollama endpoint at 1.0 as an escape hatch?
- Q-16: Which model families require legal review before inclusion?
- Q-17: Is there budget for a 48 GB+ reference GPU for high-end benchmarking?

---

## 16. Sources

Official model cards and repositories (accessed 2026-10-17):

- Qwen3-Coder repository and model family — https://github.com/QwenLM/Qwen3-Coder
- Qwen3-Coder-30B-A3B-Instruct — https://huggingface.co/Qwen/Qwen3-Coder-30B-A3B-Instruct
- Qwen3.8-27B — https://huggingface.co/Qwen/Qwen3.8-27B
- Devstral Small 2 24B — https://huggingface.co/mistralai/Devstral-Small-2-24B-Instruct-2512
- gpt-oss-20b — https://huggingface.co/openai/gpt-oss-20b
- Qwen2.5-Coder-7B-Instruct — https://huggingface.co/Qwen/Qwen2.5-Coder-7B-Instruct
- llama.cpp — https://github.com/ggml-org/llama.cpp
- llama.cpp server — https://github.com/ggml-org/llama.cpp/blob/master/tools/server/README.md
- llama.cpp grammars — https://github.com/ggml-org/llama.cpp/blob/master/grammars/README.md
- Ollama — https://github.com/ollama/ollama
- vLLM — https://github.com/vllm-project/vllm
- TensorRT-LLM — https://github.com/NVIDIA/TensorRT-LLM
- ONNX Runtime GenAI — https://github.com/microsoft/onnxruntime-genai
- mistral.rs — https://github.com/EricLBuehler/mistral.rs
- Hugging Face Hub API — https://huggingface.co/docs/hub/api
- SQLite vector extension (sqlite-vec) — https://github.com/asg017/sqlite-vec

Vendor benchmark claims are explicitly not treated as evidence of local performance. All figures are planning estimates and must be measured in Phase 2.

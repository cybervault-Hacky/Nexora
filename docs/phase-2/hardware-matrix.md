# Hardware Matrix

**Every number in this document is an estimate.** No Windows machine, GPU, or model was available during Phase 2, and no benchmark was executed. There are **zero measured results** in this phase.

The purpose of this document is to (a) give Phase 3 defensible planning ranges, (b) state exactly how they were derived so they can be falsified, and (c) define the measurements that will replace them. A number here must never be quoted as a benchmark, in product material or elsewhere.

---

## 1. Tiers under evaluation

| Tier | Reference class | VRAM | System RAM | Role |
|---|---|---|---|---|
| **H0 — CPU-only** | 8-core desktop, no discrete GPU, AVX2 | — | 16 GB | Floor; light mode only |
| **H1 — 12 GB** | RTX 3060 12 GB, or RX 6700 XT | 12 GB | 16 GB | Entry GPU tier |
| **H2 — 16 GB** | RTX 4060 Ti 16 GB, or RX 7800 XT 16 GB | 16 GB | 16–32 GB | Mainstream tier |
| **H3 — 24 GB** | RTX 3090 / 4090, or RX 7900 XTX | 24 GB | 32 GB | Recommended tier |

These map onto REF-A…REF-E in Phase 1 doc 11. They are **not** a support commitment: a specific GPU model can fail for driver reasons even when its VRAM class is supported.

## 2. Assumptions

| Assumption | Value used | Basis |
|---|---|---|
| OS + desktop VRAM reserve | 1.5 GB on Windows 11 at 1080p, **included in every peak-VRAM figure below** | Typical DWM/compositor usage; **[U]**, must be measured |
| OS + app RAM reserve | 4.0 GB | Windows 11 idle + Tauri app; **[U]** |
| NVMe sequential read | 2.0–3.0 GB/s | Common PCIe 3.0/4.0 NVMe range; **[U]** |
| GPU memory bandwidth (RTX 3060) | 360 GB/s | Vendor specification **[V]**, publicly documented |
| GPU memory bandwidth (RTX 4060 Ti 16 GB) | 288 GB/s | Vendor specification **[V]** |
| GPU memory bandwidth (RTX 4090) | 1,008 GB/s | Vendor specification **[V]** |
| DDR4-3200 dual-channel | 51.2 GB/s | Arithmetic **[E]** |
| DDR5-5600 dual-channel | 89.6 GB/s | Arithmetic **[E]** |
| Decode efficiency, dense GPU | 0.55 of peak bandwidth | Planning factor; **[U]** |
| Decode efficiency, MoE GPU | 0.50 | Routing + attention overhead; **[U]** |
| Decode efficiency, CPU | 0.50 dense / 0.35 MoE | **[U]** |
| Effective prefill throughput, RTX 4090 | 35–60 TFLOP/s | Roofline with quantised GEMM efficiency; **[E]** |
| Effective prefill throughput, RTX 4060 Ti | 12–20 TFLOP/s | **[E]** |
| Effective prefill throughput, RTX 3060 | 8–14 TFLOP/s | **[E]** |
| Effective prefill throughput, 8-core AVX2 CPU | 0.30–0.60 TFLOP/s | **[E]** |
| Peak VRAM | weights + KV + 1.0–1.5 GB compute/scratch | **[E]** |
| Peak RAM, full GPU offload | 4 GB OS/app + 1–2 GB staging | **[E]** |
| Peak RAM, CPU inference | 4 GB OS/app + weights + KV | **[E]** |

The efficiency factors are the weakest part of the model. They are deliberately ranges, and the resulting token rates are given as ranges, not points.

## 3. Method

**Decode rate** (memory-bound):

```
tokens_per_second ≈ (bandwidth × efficiency) / bytes_read_per_token
```

For dense models, `bytes_read_per_token` is the whole quantised file. For MoE models it is approximately the active-parameter share, plus attention and router weights; using the total file size for an MoE would understate throughput by 5–10×.

**Time to first token** (compute-bound prefill):

```
prefill_tokens_per_second ≈ effective_TFLOPs / (2 × active_parameters) × 10^12
TTFT ≈ prompt_tokens / prefill_tokens_per_second + queue + network
```

**Load time**:

```
load_seconds ≈ weight_bytes / disk_read + 2–4 s overhead
```

**Worked example (H3, Qwen3.8-27B UD-Q4_K_M, 4K prompt, 32K context):**

- Decode: `1008 × 0.55 / 16.46 GB` ≈ **33.7 tok/s**, planning range **25–40**.
- Prefill: `45e12 / (2 × 27.78e9)` ≈ **810 tok/s**; 4,096 tokens ≈ **5.1 s**, planning range **4–8 s**.
- KV at 32K FP16: **2.0 GiB**; fixed recurrent state ≈ 0.14 GiB.
- Peak VRAM: `15.33 GiB weights + 2.14 GiB KV + 1.2 GiB scratch + 1.5 GiB desktop` ≈ **20.2 GiB** of 22.3 GiB usable → fits, ~2.1 GiB headroom.
- Load: `16.46 / 2.5 + 3` ≈ **9.6 s**, planning range **8–14 s**.

## 4. Estimated matrix

Ranges are `p25–p75` planning estimates, not confidence intervals. **Grade: [E] for every cell.**

### 4.1 H3 — 24 GB VRAM, 32 GB RAM

| Model / quant | Load | Peak VRAM | Peak RAM | TTFT @4K | Prefill | Decode | 32K context |
|---|---|---|---|---|---|---|---|
| **Qwen3.8-27B / UD-Q4_K_M** | 8–14 s | 20.5–22.0 GB | 6–8 GB | 4–8 s | 600–1,000 tok/s | **25–40 tok/s** | Fits with ~2 GB headroom |
| Qwen3-Coder-30B-A3B / Q4_K_M | 9–15 s | 22.0–23.3 GB | 6–8 GB | 3–6 s | 700–1,200 tok/s | **55–100 tok/s** | 16K marginal; use Q4_K_S (17.46 GB) if 16K is required |
| Devstral Small 2 / Q4_K_M | 7–12 s | 19.5–21.0 GB | 6–8 GB | 5–9 s | 500–900 tok/s | **28–45 tok/s** | Fits at 16–24K only |
| gpt-oss-20b / MXFP4 | 6–10 s | 15.0–16.0 GB | 6–8 GB | 2–4 s | 1,500–2,500 tok/s | **80–140 tok/s** | Fits with large headroom |
| Gemma 4 31B / Q4_K_M | 10–16 s | 21.5–22.6 GB | 6–8 GB | 6–10 s | 500–850 tok/s | **22–35 tok/s** | 16K only; 32K OOM risk |
| Nemotron Bolt / IQ2 | 11–18 s | **>24 GB** | 6–8 GB | 4–7 s | 900–1,500 tok/s | **60–110 tok/s** | **Does not fit a 24 GB desktop**; NVIDIA's recipes target 28–32 GB |

### 4.2 H2 — 16 GB VRAM, 16–32 GB RAM

| Model / quant | Load | Peak VRAM | Peak RAM | TTFT @4K | Prefill | Decode | 32K context |
|---|---|---|---|---|---|---|---|
| **gpt-oss-20b / MXFP4** | 6–10 s | 14.8–15.6 GB | 6–8 GB | 2.5–5 s | 900–1,600 tok/s | **45–80 tok/s** | Fits; ~0.5–1 GB headroom |
| Mellum2.1 / Q4_K_M | 5–8 s | 10.4–11.2 GB | 6–8 GB | 2–4 s | 1,200–2,000 tok/s | **55–95 tok/s** | Fits comfortably |
| Devstral Small 2 / Q3_K_M | 6–10 s | 14.8–15.5 GB | 6–8 GB | 5–9 s | 550–900 tok/s | **9–16 tok/s** | 8K only |
| Qwen3-Coder-30B-A3B / Q2_K | 7–12 s | 14.0–15.0 GB | 6–8 GB | 4–7 s | 600–1,000 tok/s | **35–60 tok/s** | 8K only. Q3_K_M (14.71 GB) does **not** fit once overhead is counted |
| Qwen3.8-27B / UD-Q3_K_XL | 7–12 s | 15.8–16.4 GB | 6–8 GB | 5–9 s | 500–900 tok/s | **17–27 tok/s** | **Q3 spills at 8K**; only Q2 fits, at 8K and lower quality |

### 4.3 H1 — 12 GB VRAM, 16 GB RAM

| Model / quant | Load | Peak VRAM | Peak RAM | TTFT @4K | Prefill | Decode | 32K context |
|---|---|---|---|---|---|---|---|
| **Mellum2.1 / Q4_K_M** | 4–7 s | 10.2–11.0 GB | 6–8 GB | 2–4 s | 700–1,200 tok/s | **55–100 tok/s** | Fits with ~1 GB headroom |
| Mellum2.1 / MXFP4_MOE | 2–5 s | 9.3–10.0 GB | 6–8 GB | 1.5–3 s | 800–1,400 tok/s | **60–110 tok/s** | Fits with headroom |
| Qwen3.8-27B / UD-Q2_K_XL | 5–9 s | **>12 GB** | 6–8 GB | — | — | — | **Does not fit at any quant** (9.83 GB weights already leaves <1 GB) |
| Qwen2.5-Coder-7B / Q4_K_M | 3–5 s | 7.4–8.2 GB | 6–8 GB | 1.5–3 s | 800–1,400 tok/s | **28–45 tok/s** | Fits at 16K |

### 4.4 H0 — CPU-only, 8 cores, 16 GB RAM

| Model / quant | Load | Peak RAM | TTFT @4K | Prefill | Decode | Practical context |
|---|---|---|---|---|---|---|
| **Qwen2.5-Coder-7B / Q4_K_M** | 3–6 s | 9–10 GB | 60–150 s | 25–60 tok/s | **3.5–7 tok/s** | 8–16K |
| Mellum2.1 / Q4_K_M | 4–8 s | 12.5–13.5 GB | 35–80 s | 50–110 tok/s | **6–12 tok/s** | 8–16K; RAM-tight |
| Qwen3.8-27B / UD-Q4_K_M | 8–15 s | 22 GB+ | >5 min | 10–20 tok/s | **0.8–1.8 tok/s** | Not supported |
| gpt-oss-20b / MXFP4 | 6–11 s | 17–18 GB | 3–7 min | 12–25 tok/s | **1.5–3 tok/s** | Does not fit 16 GB RAM |

CPU-only is a **compatibility floor**, not a supported experience for agentic work. At 4 tok/s a 500-token answer takes over two minutes, and a multi-turn agent loop becomes unusable. The product must say this plainly rather than shipping a slow path with the same claims.

## 5. Long-context behaviour

| Tier / model | Context that fits | Behaviour at the limit | Risk |
|---|---|---|---|
| H3 / Qwen3.8-27B | 32K comfortable; 64K marginal | Prefill grows linearly; TTFT at 64K ≈ 70–120 s | VRAM spill → 10–20× slower decode, not an error |
| H3 / Qwen3-Coder-30B | 16K comfortable; 32K marginal | 3 GiB KV at 32K plus 18.6 GB weights | OOM on 24 GB with compositor active |
| H3 / gpt-oss-20b | 64K comfortable; 128K marginal | Sliding-window layers keep KV cheap; attention still costs compute | Prefill latency dominates |
| H2 / gpt-oss-20b | 32K comfortable | ~0.75 GiB KV; ~14.5 GB total | Little headroom for the compositor |
| H2 / Mellum | 32K comfortable; 128K feasible | 14 KiB/token | RAM, not VRAM, becomes the constraint if offloading |
| H1 / Mellum | 32K comfortable | ~10 GB total | Context beyond 32K competes with the desktop |
| H0 / Qwen2.5-7B | 8–16K | 0.44–0.88 GiB KV | RAM ceiling; prefill latency at 16K > 4 min |

**Practical rule for the product:** default context per tier is 32K (16K on 8 GB VRAM, 8K on CPU), user-adjustable up to the measured safe limit, with a warning that raising it can push memory into a degraded state. The model's native context must not be shown as an available setting.

## 6. Stability and failure modes

| Failure | Where it shows up | Estimated frequency | Detection | Response |
|---|---|---|---|---|
| VRAM exhaustion / spill | Decode collapse to <2 tok/s, no error | Common near the limit **[E]** | Watchdog on inter-token latency; NVML usage | Reduce context, lower offload, reload |
| Hard OOM at load | Process exits during model load | Occasional **[E]** | Exit code + stderr | Fall back one quant tier, inform user |
| CUDA driver reset (TDR) | Process loses device, may exit | Rare **[U]** | Exit code, event log | Restart with lower offload; mark session interrupted |
| Vulkan hang after cancel | Server stops responding | Unknown; upstream issue #27604 open **[V]** | Generation probe timeout | Kill process group, restart, disable Vulkan for that driver |
| Server wedge mid-decode | `/health` 200, no tokens | Unknown; upstream issue #27388 open **[V]** | Probe timeout + token stall | Kill and restart |
| Thermal throttling | Decode rate falls over 10–30 min | Common on laptops **[E]** | Clock/temperature sampling | Show sustained rate, not burst rate |
| RAM pressure on 16 GB systems | Paging, UI stutter | Common when a 32 GB-class process runs **[E]** | Commit charge, page faults | Block the model tier; explain minimum RAM |

No stability figure in this table is measured. "Common" and "rare" are qualitative planning judgements, marked accordingly.

## 7. UI responsiveness

The runtime must not be allowed to make the app feel broken. Phase 1 budgets: cold start ≤2.5 s (app shell), cancel acknowledged ≤300 ms, 30 Hz streaming updates, idle memory ≤250 MB.

| Scenario | Expectation | Basis |
|---|---|---|
| App start while model loads | Shell interactive in ≤2.5 s; model area shows progress | Load happens in a child process, never on the UI thread |
| Streaming a 25 tok/s answer | 25 UI updates/s cap; coalesce to 30 Hz | Token events can arrive faster than frames |
| Streaming a 100 tok/s answer | Coalesce to 30 Hz; never one React render per token | Phase 1 doc 03 |
| Cancel | Acknowledge ≤300 ms, then "stopping" state | Process kill may take up to 2 s |
| Full GPU offload on 12 GB | Compositor may stutter if headroom <1 GB | Reserve 1.5 GB; cap offload layers |
| CPU-only inference, 8 threads | UI thread starved if all cores saturated | Leave 1–2 cores; set process priority below normal |
| Scrolling a 10k-message chat during streaming | ≥50 fps | Phase 1 doc 03 gate |

The last row is a Phase 3 gate that does not depend on the model: if virtualisation is correct, runtime speed is irrelevant to scroll performance. It must be tested with a synthetic stream, not by waiting for a slow model.

## 8. Measurement plan (Phase 3)

### 8.1 What to measure

| Metric | Method | Acceptance |
|---|---|---|
| Load time | Monotonic clock: process spawn → `/health` 200 → first successful 1-token completion | Report p50/p95 per tier |
| TTFT | Client: request sent → first SSE `content` token; cross-check server `timings.prompt_ms` | Report p50/p95 at 1K/4K/16K prompts |
| Prefill rate | `prompt_tokens / prompt_ms` from server timings | tok/s |
| Decode rate | `predicted_tokens / predicted_ms`; also client inter-token p50/p95 | tok/s, sustained 10 min |
| Peak VRAM | NVML (`nvidia-smi dmon`) sampled at 1 Hz; vendor tool for AMD/Intel | GB, with model + KV + context recorded |
| Peak RAM | `Get-Counter` working set + commit; process tree, not just parent | GB |
| Context ceiling | Increase `--ctx-size` until failure; binary search | Max stable context per tier |
| Long-context retrieval | Needle-in-haystack + repository-question set at 8/16/32/64K | Accuracy + latency |
| Stability | 100-turn soak; 20 cancels; 10 load/unload cycles; 5 forced kills | Failures per 100 |
| Thermal | 30-min sustained generation, sample clocks/temps | Sustained vs burst ratio |
| UI frame time | PresentMon during streaming and scrolling | p95 frame ≤20 ms, no long tasks >200 ms |
| Cancel latency | Client timestamp → UI acknowledged → process idle | ≤300 ms ack; ≤2 s stop |

### 8.2 Instrumentation constraints

- Record the **exact** runtime build tag, model file hash, quant, context size, KV type, offload layer count, driver version, OS build, and power profile with every run.
- One variable at a time: changing quant and context together produces an uninterpretable result.
- Discard the first run after install (shader/kernel cache warm-up) and report warm runs separately.
- Laptop benchmarks must state power mode; "plugged in, best performance" is the only comparable setting.
- Never pool results from different driver versions.

### 8.3 What would change these estimates

A single measured result can move a tier by a factor of two. The largest uncertainties, in order:

1. **Windows VRAM overhead** — if the compositor takes 2.5 GB rather than 1.5 GB, the 24 GB tier loses its 32K context.
2. **MoE kernel efficiency** — the 55–100 tok/s range for a 3B-active model is wide because expert locality and routing kernels vary more than dense GEMM.
3. **CPU memory bandwidth** — single-channel or slow DDR4 halves every CPU figure.
4. **Driver maturity** — Vulkan on a new AMD driver can be 40% slower or hang under cancellation.
5. **Quantisation quality** — throughput is predictable from file size; quality at Q2/Q3 is not.

## 9. Recommended system requirements (provisional)

| Level | Specification | Experience |
|---|---|---|
| **Minimum** | Windows 11 23H2 x64, 4-core CPU with AVX2, 16 GB RAM, 20 GB free, CPU-only (8 GB VRAM optional) | Light mode: 7B Q4, 8K context, 3–7 tok/s, single-turn assistance. 8 GB RAM is below the ~9 GB working set and will page, so Phase 1's 16 GB floor stands. |
| **Recommended** | 8-core CPU, 16 GB RAM, 12 GB VRAM, 40 GB free | Mellum 12B Q4, 32K, 55–100 tok/s |
| **Best** | 8+ core CPU, 32 GB RAM, 24 GB VRAM, 60 GB free | Qwen3.8-27B Q4, 32K, 25–40 tok/s |

Windows 10 22H2 is out of general support since 2025-10-14; consumer ESU runs to 2027-10-12 and commercial ESU to 2028-10-10 **[V, S-31]**. Nexora 1.0 should require Windows 11 23H2+ and treat Windows 10 as unsupported, rather than shipping an installer that may fail on missing WebView2 or driver components. Windows 11 itself requires TPM 2.0, UEFI Secure Boot, 4 GB RAM, 64 GB storage, and DirectX 12/WDDM 2.0 **[V, S-32]** — which conveniently sets a floor close to Nexora's own.

## 10. Honesty statement

Every performance figure in this document is a **planning estimate**. None was produced by running Nexora, llama.cpp, or any model. When Phase 3 produces measurements, this document must be rewritten from results, and the estimates retained only in an appendix labelled "superseded estimates". If a measurement contradicts an estimate, the measurement wins and the discrepancy is recorded — not smoothed over.

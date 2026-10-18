# 11 — Hardware Assumptions and Benchmarking Strategy

**Status:** Proposed; benchmark execution deferred to Phase 2  
**Related:** [04-ai-runtime-and-model-strategy.md](04-ai-runtime-and-model-strategy.md), [08-testing-and-quality.md](08-testing-and-quality.md)

---

## 1. Purpose

This document defines the hardware Nexora will support, how it detects capabilities, how it will benchmark models and runtimes before final selection, and how it will avoid the common mistake of assuming that an expensive PC automatically runs a model well.

**No model or runtime has been benchmarked yet.** All numbers in this document are planning envelopes or cited vendor claims. Phase 2 must produce measured evidence.

---

## 2. Hardware detection

At first run and on demand, Nexora collects a local hardware profile:

| Signal | Source | Use |
|--------|--------|-----|
| OS edition/build/architecture | Windows API | Compatibility gate |
| CPU brand, physical/logical cores, base clock | `GetLogicalProcessorInformationEx`, registry | Thread count, CPU tier |
| CPU features (AVX2, AVX-512, FMA) | CPUID | Runtime pack selection |
| RAM total/available | `GlobalMemoryStatusEx` | Model fit, context budget |
| GPU adapters (name, vendor, device id) | DXGI | Backend selection |
| Dedicated/shared VRAM | DXGI + NVML (NVIDIA) | Model fit |
| Driver version | DXGI/NVML | Compatibility, crash correlation |
| CUDA driver API version | `cuDriverGetVersion` | CUDA pack eligibility |
| Vulkan support/version | Vulkan loader | Vulkan eligibility |
| Disk free/total, type (SSD/HDD) | `GetDiskFreeSpaceEx`, storage properties | Download/load checks |
| Power source/battery | `GetSystemPowerStatus` | Laptop performance warnings |
| Display resolution/DPI | Windows API | UI scaling |
| WebView2 version | Registry/API | Compatibility |
| Locale/IME | Windows API | UI behaviour |

**Rules:**

- Detection never uploads data; it is stored locally and included in diagnostics only with user review.
- VRAM reporting has quirks (DXGI can under-report); use NVML for NVIDIA and a conservative fallback otherwise.
- The profile produces a **fit estimate**, not a performance promise. The UI says "fits with reduced context" or "likely too large", never "will run at X tokens/s" before measurement.
- The profile is re-evaluated after driver updates, model changes, and app updates.

---

## 3. Supported hardware tiers

| Tier | CPU | RAM | GPU | Free disk | Intended experience |
|------|-----|-----|-----|-----------|---------------------|
| **Minimum** | 4-core x64 with AVX2 | 16 GB | 8 GB VRAM or CPU-only | 20 GB | 7B-class chat and small edits; simple static sites; agent slow/limited |
| **Recommended** | 6–8 core | 32 GB | 12–16 GB VRAM | 50 GB | 20–30B quantized; useful agentic coding at 8–16K context |
| **Performance** | 8–12 core | 64 GB | 24 GB VRAM | 100 GB | 27–30B Q4 at 16–32K; the primary full experience |
| **Enthusiast** | 12+ core | 128 GB | 32–48 GB+ VRAM | 200 GB | 80B MoE or 30B with long context; best local quality |

**Notes:**

- 16 GB system RAM with an 8 GB GPU is the realistic floor; 16 GB with no GPU is only suitable for 7B CPU inference.
- Laptops with the same GPU name as desktops may be 30–50% slower due to power/thermal limits; results are reported separately.
- Integrated GPUs share system RAM; the model budget is constrained by both.
- Multi-GPU is not supported at 1.0.
- Windows ARM64 is not supported at 1.0; the app detects it and explains.

### 3.1 GPU vendor matrix

| Vendor | Backend | Status | Caveats |
|--------|---------|--------|---------|
| NVIDIA Turing/Ampere/Ada/Blackwell | CUDA 12.x | Primary | Driver minimum; CUDA runtime redistribution review |
| NVIDIA Pascal and older | Vulkan/CPU | Best-effort | Slower; some quant kernels unavailable |
| AMD RDNA2/3/4 | Vulkan | Supported | Driver quality varies; ROCm/HIP later |
| AMD older | Vulkan/CPU | Best-effort | Performance may be poor |
| Intel Arc / Iris Xe | Vulkan | Supported/best-effort | Shared memory on iGPU; measure |
| CPU only | AVX2 | Supported | Usable for chat/small edits; not for long agent loops |

The app selects the best available backend, benchmarks a short warm-up, and allows a manual override. A backend that crashes or produces garbage is marked unusable for that device/driver and reported.

---

## 4. Benchmark dimensions

### 4.1 Hardware performance

| Metric | Method | Why it matters |
|--------|--------|----------------|
| Model load time | Spawn → health → warm-up | Perceived responsiveness |
| Time to first token (TTFT) | Request → first delta | Conversational feel |
| Prefill throughput (pp512) | Prompt processing tok/s | Repo context ingestion |
| Decode throughput (tg128) | Generation tok/s | Editing speed |
| Throughput at 8K/32K context | Same prompts at depth | Long-task viability |
| Peak VRAM | NVML/DXGI sampling at 1 Hz | OOM risk |
| Peak system RAM | Process counters | Stability |
| Thermal/power behaviour | 15-minute sustained run | Laptop throttling |
| Crash/OOM rate | Repeated runs | Reliability |
| Cancellation latency | Stop → process idle | User control |

### 4.2 Quality

| Metric | Method | Why it matters |
|--------|--------|----------------|
| Task success | Nexora Coding Eval (NCE) required checks | Real usefulness |
| Build success | Build exit code | Basic correctness |
| Test success | Test exit code + count | Behavioural correctness |
| Patch apply rate | Diffs applied without conflict | Agent reliability |
| Tool-call validity | Valid calls / total | Agent reliability |
| Repair efficiency | Cycles to recover | Time/cost |
| Long-context retrieval | Needle/symbol tasks at 8K/32K | Repo comprehension |
| Instruction following | Brief adherence rubric | Design quality |
| Honesty | Adversarial/impossible tasks | Trust |
| Visual quality | Blind human rubric | Professional output |
| Quantization degradation | Q4 vs Q5/Q8 on a subset | Acceptable trade-off |

---

## 5. Benchmark harness

### 5.1 Components

1. **Hardware probe** — writes a JSON profile of the machine.
2. **Runtime runner** — starts the pinned llama-server with a matrix of configs (backend, context, KV type, GPU layers).
3. **Synthetic benchmark** — llama.cpp `llama-bench` or equivalent for pp/tg at context depths.
4. **Task runner** — executes NCE tasks end to end with the real agent, tools, and verification.
5. **Sampler** — collects VRAM/RAM/CPU/GPU utilisation and temperatures at 1 Hz.
6. **Reporter** — emits JSON + Markdown + charts; stores config hashes and logs.

### 5.2 Run protocol

- Clean reboot or idle machine; close background apps; plug in power; high-performance plan.
- Record ambient temperature where possible; laptop chassis and fan mode.
- Warm-up: 2 runs discarded; measure 5 runs; report median and IQR.
- Context depths: 0, 8K, 32K (64K/128K only for models that claim support and fit).
- Fixed prompts, temperature 0, fixed seed where supported; record the runtime's sampling defaults.
- Run each model at Q4_K_M baseline; additionally Q5/Q8 or IQ3 for sensitivity.
- Run CUDA, Vulkan, and CPU where available; do not compare across different prompt sets.
- Repeat after driver/runtime changes; keep raw logs.

### 5.3 Isolation

- No network during inference benchmarks (except localhost).
- No model downloads during measurement.
- Disable Windows updates and indexing during the run where practical.
- Record Windows build, driver, runtime commit, model hash, and quant.
- Do not pool laptop and desktop results.

### 5.4 Result schema

```json
{
  "run_id": "2026-11-02T10:00:00Z-ref-c",
  "hardware": {"cpu": "...", "ram_gb": 64, "gpu": "RTX 4090", "vram_gb": 24,
                "driver": "...", "os": "Windows 11 24H2"},
  "runtime": {"name": "llama.cpp", "commit": "...", "backend": "cuda12",
               "flags": ["-ngl", "99", "--ctx-size", "32768"]},
  "model": {"id": "...", "quant": "Q4_K_M", "sha256": "..."},
  "metrics": {"load_s": 8.4, "ttft_ms": 420, "pp512_tps": 1800,
               "tg128_tps": 42, "peak_vram_gb": 22.1, "peak_ram_gb": 9.8},
  "quality": {"nce_success": 0.72, "tool_validity": 0.99, "false_success": 0},
  "notes": "..."
}
```

---

## 6. Nexora Coding Eval (NCE)

The full task design is in [08-testing-and-quality.md](08-testing-and-quality.md). For model selection:

- Run the full suite (40–60 tasks) on each eligible model and reference machine.
- Use three repetitions; report median and range.
- Separate scores by tier and category; do not hide weak categories in an average.
- Include adversarial honesty and destructive-action tests; a single failure disqualifies a model.
- Include at least 20% holdout tasks not used for prompt tuning.
- Blind human review of 20 outputs per model for visual and code quality.

**Hard gates before scoring:**

| Gate | Threshold |
|------|-----------|
| Licence permits commercial desktop use | Pass |
| Runs on pinned runtime without patches | Pass |
| Tool-call validity | ≥ 98% |
| False-success claims | 0 |
| Unauthorized destructive actions | 0 |
| Crash/OOM rate on target tier | < 2% of runs |
| Download size within owner budget | Pass |

Models that fail a gate are excluded regardless of quality score.

---

## 7. Selection scorecard

| Criterion | Weight | 1 (poor) | 3 (acceptable) | 5 (excellent) |
|-----------|--------|----------|----------------|---------------|
| Coding/agentic quality | 30 | < 40% NCE | 55–65% | > 75% |
| Tool-call reliability | 15 | < 90% | 96–98% | > 99.5% |
| Speed on target tier | 15 | < 10 tok/s | 20–30 tok/s | > 45 tok/s |
| Memory fit/stability | 10 | OOM common | fits with reduced ctx | fits comfortably |
| Licence | 10 | restrictive | review needed | Apache/MIT |
| Runtime compatibility | 8 | patched build | works with caveats | first-class GGUF/tools |
| Context quality | 7 | degrades < 8K | good at 16K | strong at 32K+ |
| Ecosystem/maintenance | 5 | stale | active | active + quants + tools |

**Rule:** quality must be ≥ 3.5/5 on the target tier, tool validity ≥ 98%, and all hard gates pass. The highest weighted score wins per tier; a single global winner is not required. The final choice is made at the end of Phase 2 with owner approval.

**Tie-breakers:** download size, load time, licence clarity, quant quality, community support.

---

## 8. Reference machines

Phase 2 requires access to:

| ID | Spec | Purpose | Status |
|----|------|---------|--------|
| REF-A | Win11, 6-core, 16 GB, RTX 3060 12 GB, SATA SSD | Minimum recommended | Needed |
| REF-B | Win11, 8-core, 32 GB, RTX 4070 Ti Super 16 GB, NVMe | Mainstream | Needed |
| REF-C | Win11, 12-core, 64 GB, RTX 4090 24 GB, NVMe | Performance | Needed |
| REF-D | Win11, 8-core, 32 GB, no dGPU | CPU-only baseline | Needed |
| REF-E | Win11, 12-core, 64 GB, RTX 5090 32 GB (if available) | High-end | Optional |

If physical machines are unavailable, rent Windows GPU workstations with comparable consumer GPUs; do not substitute Linux results for Windows driver behaviour. Document any substitutions.

**Owner input required:** access to these machines or a budget for cloud GPU time (Q-17).

---

## 9. Benchmark outputs and reporting

- `benchmarks/<date>/<hardware-id>/<model>/<runtime>/run.json`
- A summary table per tier: model, quant, context, backend, TTFT, tok/s, peak VRAM, NCE success, tool validity, notes.
- Charts: speed vs context, VRAM vs context, quality vs quant, quality vs speed.
- A written recommendation with dissenting evidence and uncertainty.
- Raw logs and configs retained; no cherry-picking.
- Re-run at each major release and after model/runtime changes.

---

## 10. Limitations

- Vendor benchmarks are not comparable across harnesses, prompts, and quantization.
- Quantized models can behave differently from the BF16 checkpoint used in published scores.
- Consumer GPUs vary by board power, cooling, and laptop limits.
- Small sample sizes produce noisy results; use medians and report ranges.
- Eval tasks can be memorized or overfitted; rotate holdouts and add real cases.
- Human aesthetic judgments are subjective; use a rubric and multiple reviewers.
- A model that performs well on benchmarks may still be poor at a specific user's stack; the catalogue should offer alternatives.
- Windows driver updates can change performance and stability; re-test after major updates.

---

## 11. Decisions

| ID | Decision | Recommendation | Status |
|----|----------|----------------|--------|
| D-03 | Final model | Defer until Phase 2 benchmarks | Deferred |
| D-51 | Benchmark harness | llama-bench + NCE task runner + sampler | Proposed |
| D-52 | Baseline quant | Q4_K_M; sensitivity Q5/Q8/IQ3 | Proposed |
| D-53 | Reference hardware | 3 GPU tiers + CPU baseline | Needed |
| D-54 | Selection | Per-tier winner + hard gates | Proposed |
| D-55 | Reporting | Raw JSON + summary + human review | Proposed |

Open: Q-02 minimum tier; Q-03 download budget; Q-17 hardware access; Q-33 whether to support 8 GB VRAM officially; Q-34 whether AMD is a first-class target at launch.

---

## 12. Sources

- llama.cpp `llama-bench` — https://github.com/ggml-org/llama.cpp/tree/master/tools/llama-bench
- NVIDIA NVML — https://developer.nvidia.com/nvidia-management-library-nvml
- DXGI — https://learn.microsoft.com/windows/win32/direct3ddxgi/dxgi-overview
- Windows memory API — https://learn.microsoft.com/windows/win32/api/sysinfoapi/nf-sysinfoapi-globalmemorystatusex
- Windows power status — https://learn.microsoft.com/windows/win32/api/winbase/nf-winbase-getsystempowerstatus
- SWE-bench — https://www.swebench.com/
- Terminal-Bench — https://www.tbench.ai/
- LiveCodeBench — https://livecodebench.github.io/
- Aider polyglot — https://aider.chat/docs/leaderboards/
- MLPerf client (reference for client benchmarking discipline) — https://mlcommons.org/benchmarks/client/

Access date: 2026-10-17.

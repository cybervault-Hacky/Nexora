# Phase 2 — Local AI Model & Inference Runtime Evaluation

**Status:** complete (research and documentation only)
**Branch:** `arena/00bd2df7-nexora`
**Access date for all sources:** 2026-10-17 (see [`sources.md`](sources.md))
**Approved inputs:** [`../phase-1/`](../phase-1/README.md) at commit `e28cb99`

---

## 1. What this phase is

Phase 1 chose a provisional stack: Tauri 2 desktop, Rust core, a bundled `llama-server` sidecar, and a shortlist of open-weight coding models. Phase 2 tests those provisional choices against primary evidence and produces the decisions Phase 3 needs before any code is written.

This phase answers four questions:

1. Which Windows inference runtime should Nexora ship, and what is the fallback?
2. Which model, at which quantization, for each supported hardware tier?
3. What must be measured before the choice is final, and how?
4. What must be built so the runtime is safe, recoverable, and replaceable?

It does **not** build any of it. There is no application code, no dependency installation, no model download, no runtime binary, and no benchmark result in this phase.

## 2. Method and evidence grades

Every non-trivial claim in these documents carries a grade:

| Grade | Meaning | How it was obtained |
|---|---|---|
| **[V]** | Verified | Read from a primary source: official repository, release API, server documentation, model card, `config.json`, or file listing. |
| **[S]** | Secondary | Reputable third-party reporting or community measurement, not the vendor's own card. Treated as a lead, not a fact. |
| **[E]** | Estimate | Derived here from a stated formula with published inputs (memory bandwidth, file size, KV geometry). **Never a measurement.** |
| **[U]** | Unverified | Assumption, inference, or vendor marketing claim that has not been independently checked. |

A vendor benchmark score is **[U]** even when it appears on an official model card: it is a claim, produced with the vendor's harness, prompt, and sampling settings. Nexora's own numbers require the protocol in [`benchmark-protocol.md`](benchmark-protocol.md).

## 3. What was not done (and why it matters)

| Not done | Reason | Consequence |
|---|---|---|
| Any performance measurement | The evaluation environment is Linux, 2 vCPU, 3.8 GiB RAM, no GPU. It cannot represent any Windows target. | **Zero measured figures exist.** Every number in [`hardware-matrix.md`](hardware-matrix.md) is an estimate with a stated method. |
| Model or runtime downloads | Phase 2 mandate forbids them. | File sizes and hashes come from Hugging Face file listings, not from local verification. |
| Tool-call validity testing | Requires the runtime and models running. | The ≥98% tool-validity gate from Phase 1 remains unmeasured. |
| Legal review of `OpenMDW-1.1` | Out of scope for engineering docs. | Nemotron 3.5 Bolt is watchlisted, not recommended. |
| Physical Windows GPU matrix (REF-A…D) | Hardware unavailable. | REF machines are specified but not provisioned; `Q2-01` is the blocking question. |

Phase 1 documents 11 and 12 assumed Phase 2 would include hardware spikes. The authorised mandate is research-only, so that assumption is **not** satisfied. It is carried forward as a Phase 3 gate, not silently dropped (`C2-03`).

## 4. Headline recommendations

These are recommendations for owner approval, not final selections. Decisions are argued in [`runtime-evaluation.md`](runtime-evaluation.md), [`model-evaluation.md`](model-evaluation.md), and [`decisions-and-risks.md`](decisions-and-risks.md).

### 4.1 Runtime

| Role | Choice | Why |
|---|---|---|
| **Primary** | `llama.cpp` — pinned `llama-server` build `b11714` (2026-10-17), MIT | Only candidate that ships Windows x64 CPU, CUDA, and Vulkan binaries, runs entirely offline, exposes an OpenAI-compatible streaming API with schema-constrained output and Jinja tool calling, and is permissively licensed for bundling. |
| **Fallback** | Ollama `v0.40.6`, MIT, external provider only | Independent Windows runner with its own GPU selection. Used if the pinned build fails on a supported machine class. Not bundled into the installer by default. |
| **Deferred** | ONNX Runtime GenAI (MIT), `mistral.rs` (MIT) | Windows-native and Rust-native respectively, but model-format coverage does not match the GGUF shortlist at 1.0. |
| **Rejected** | vLLM, TensorRT-LLM, LM Studio | No documented native Windows platform (vLLM); NVIDIA-datacenter and build complexity (TensorRT-LLM); proprietary redistribution terms (LM Studio). |

The critical packaging finding is that **llama.cpp does not attach binaries to its semantic release tag**. `v0.7.0` (2026-10-15) carries only `nightly-tag.txt`; the Windows binaries live on rolling `b####` tags. Pinning must therefore pin a *build tag*, not a *version*, and the installer needs an internal mirror. This corrects an implicit Phase 1 assumption (`C2-02`).

### 4.2 Models by tier

All are Apache-2.0 unless stated. Sizes are the actual GGUF file sizes from the Hub listing, not "approximately".

| Tier | Primary | Quant / size | Context for 1.0 | Why |
|---|---|---|---|---|
| **24 GB VRAM** | Qwen3.8-27B | `UD-Q4_K_M` / 16.46 GB | 32K | Best vendor-reported agentic coding in the 27B class; hybrid attention keeps KV at ~64 KiB/token. |
| **16 GB VRAM** | gpt-oss-20b | `MXFP4` / 12.11 GB | 32K | 3.6B active parameters, 24 KiB/token KV, native tool format, trained at MXFP4 rather than post-quantised. |
| **12 GB VRAM** | Mellum2.1-12B-A2.5B-Thinking | `Q4_K_M` / 8.07 GB | 32K | 2.5B active, 14 KiB/token KV, 131K native, Apache-2.0; fits with room for the OS and the UI. |
| **8 GB VRAM / CPU-only** | Qwen2.5-Coder-7B-Instruct | `Q4_K_M` / 4.68 GB | 16K VRAM / 8K CPU | Proven, small, fast enough on CPU; explicitly a "light mode" with lower agentic reliability. |
| **48 GB+ (test only)** | Qwen3-Coder-Next | `Q4` / ~45–50 GB | 64K | Not a 1.0 default. Retained for high-end validation. |

Watchlist, not defaults: Devstral Small 2 24B (strong agentic claims, expensive KV), Gemma 4 31B (Apache-2.0, now licence-clean, but 18.3 GB leaves little headroom on 24 GB), GLM-4.7-Flash (MIT, 200K, parser untested), Nemotron 3.5 Bolt 74B-A9B (OpenMDW-1.1, 2-bit, legal review needed).

### 4.3 The context-window trap

Marketing context lengths are not usable context lengths. With FP16 KV:

| Model | KV per token | 32K context | 128K context |
|---|---|---|---|
| Qwen3.8-27B | ~64 KiB + ~0.15 GiB fixed | ~2.0 GiB | ~8.0 GiB |
| Qwen3-Coder-30B-A3B | 96 KiB | 3.0 GiB | 12.0 GiB |
| Devstral Small 2 24B | 160 KiB | 5.0 GiB | 20.0 GiB |
| gpt-oss-20b | 24 KiB (sliding) | 0.75 GiB | 3.0 GiB |
| Mellum2.1 12B | 14 KiB (sliding) | 0.44 GiB | 1.75 GiB |

On a 24 GB card, a 16.46 GB model plus 8 GiB of KV does not fit with compute buffers. **Nexora should advertise 32K as the default working context on 24 GB and treat anything beyond as an expert setting**, regardless of what the model card says. The full table and the caching options that change it are in [`model-evaluation.md`](model-evaluation.md) §5.

## 5. Deliverables

| # | Document | Contents |
|---|---|---|
| 1 | [`runtime-evaluation.md`](runtime-evaluation.md) | Ten-dimension comparison of llama.cpp, Ollama, vLLM, ONNX GenAI, mistral.rs/candle; server hardening; recommendation. |
| 2 | [`model-evaluation.md`](model-evaluation.md) | Verified specs, revisions, licences, quant sizes, KV geometry, evidence quality, tier recommendations. |
| 3 | [`hardware-matrix.md`](hardware-matrix.md) | 12/16/24 GB and CPU-only estimates, long-context behaviour, stability, UI responsiveness, measurement plan. |
| 4 | [`benchmark-protocol.md`](benchmark-protocol.md) | Ten-dimension protocol, task set, fixed settings, scoring, result schema, acceptance gates. |
| 5 | [`integration-design.md`](integration-design.md) | Rust-owned lifecycle, streaming, cancellation, recovery, health, tool validation, loopback hardening. |
| 6 | [`model-acquisition-policy.md`](model-acquisition-policy.md) | Sources, revision pinning, integrity, resume, storage, notices, malicious-file handling. |
| 7 | [`decisions-and-risks.md`](decisions-and-risks.md) | Phase 2 decisions, risks, Phase 1 corrections, open questions, Phase 3 handoff. |
| 8 | [`sources.md`](sources.md) | Every source with URL, access date, type, and what it supports. |
| 9 | This file | Scope, method, summary, acceptance. |

## 6. Corrections and clarifications to Phase 1

These are the only Phase 1 changes made in this phase. Every one is recorded in [`decisions-and-risks.md`](decisions-and-risks.md) §4.

| ID | Phase 1 text | Correction | Action |
|---|---|---|---|
| **C2-01** | Doc 04: "Gemma 4 31B — Gemma Terms of Use … legal review required" | Gemma 4 is **Apache-2.0** on the official model card and is not gated. The Gemma Terms apply to Gemma 3 and earlier. | Doc 04 table cells corrected; licence risk table updated. |
| **C2-02** | Doc 04: "Windows CPU/CUDA/Vulkan binaries" implied to ship with releases | True only for rolling `b####` builds. The `v0.7.0` release has **no** binary assets. | Phase 1 left intact; pinning policy in [`model-acquisition-policy.md`](model-acquisition-policy.md). |
| **C2-03** | Docs 11/12 planned Phase 2 hardware spikes and measurements | Phase 2 was authorised as research-only; no hardware was available. | Carried as a Phase 3 gate; no numbers invented. |
| **C2-04** | Doc 04: Qwen3-Coder-Next "80B MoE, ~3B active" | Confirmed by the technical report (80B-A3B). Some secondary sources say "235B / 22B"; that is contradicted by the report and is not used. | No edit; recorded here. |
| **C2-05** | Doc 04: Qwen3-Coder-Next licence "verify model card" | Verified **Apache-2.0** on the Hub repository. | No edit; resolved in Phase 2. |
| **C2-06** | Doc 04: Llama 4 Scout listed as "not planned" | Left as is. | No change. |
| **C2-07** | Doc 04: 24 GB tier → 30B-A3B Q4 or Qwen3.8 Q4 at 16–32K | KV arithmetic shows 30B-A3B Q4 does not fit 32K on 24 GB; Qwen3.8-27B does. | Mapping tightened in Phase 2; Phase 1 text unchanged. |
| **C2-08** | Doc 03: `--jinja` must be passed | `--jinja` is now the server default; Nexora still passes it explicitly. | Clarification only. |

## 7. Acceptance checklist

| Criterion | State |
|---|---|
| Phase 1 reviewed; contradictions and gaps recorded | Done — §6 and `decisions-and-risks.md` §4 |
| Windows runtimes compared against verified evidence | Done — `runtime-evaluation.md` §3–§8 |
| Model specs and licences verified against primary sources | Done — `model-evaluation.md` §2, with revision SHAs |
| Hardware requirements distinguished from estimates | Done — every figure in `hardware-matrix.md` carries a grade; no measurement is claimed |
| Benchmark protocol reproducible | Done — `benchmark-protocol.md` §3–§7 |
| Primary runtime and preferred model recommended with trade-offs | Done — §4 above and `decisions-and-risks.md` §2 |
| Uncertainty and missing measurements recorded | Done — §3, `hardware-matrix.md` §7, `decisions-and-risks.md` §5 |
| No unverified claim presented as fact | Done — grading is applied throughout |
| Committed to the authorised private branch | Done — commit recorded in the Phase 2 report |

## 8. Open questions blocking Phase 3

| ID | Question | Blocks |
|---|---|---|
| Q2-01 | Can REF-A…REF-D class hardware be provisioned for measurement? | Any measured claim; final model choice. |
| Q2-02 | Is there budget/approval to mirror pinned runtime builds and model files? | Offline install, supply-chain integrity. |
| Q2-03 | Legal sign-off on `OpenMDW-1.1` and on shipping CUDA runtime components | Nemotron; CUDA pack distribution. |
| Q2-04 | Confirm IPC-only (webview never reaches the runtime port; no reverse proxy) as permanent architecture. | Server hardening; contract test CT-14. |
| Q2-05 | Must 1.0 support AMD and Intel GPUs, or NVIDIA only with a CPU fallback? | Runtime pack matrix; support load. |

Full list: `decisions-and-risks.md` §5.

## 9. Navigation

- Phase 1 index: [`../phase-1/README.md`](../phase-1/README.md)
- Phase 1 runtime strategy: [`../phase-1/04-ai-runtime-and-model-strategy.md`](../phase-1/04-ai-runtime-and-model-strategy.md)
- Phase 1 hardware protocol: [`../phase-1/11-hardware-benchmarking.md`](../phase-1/11-hardware-benchmarking.md)
- Phase 1 roadmap and open questions: [`../phase-1/12-roadmap-risks-and-decisions.md`](../phase-1/12-roadmap-risks-and-decisions.md)

**Changelog**

| Date | Change |
|---|---|
| 2026-10-17 | Phase 2 created: nine documents, runtime and model evaluation, hardware estimates, benchmark protocol, integration design, acquisition policy, decisions and risks, sources. |

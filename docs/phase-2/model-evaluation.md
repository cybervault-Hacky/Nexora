# Model Evaluation

**Scope:** re-evaluation of the Phase 1 shortlist against primary sources, plus models released since.
**Method:** model card, `config.json`, licence metadata, and GGUF file listings read on 2026-10-17.
**No model was downloaded or run.** Benchmark scores quoted from vendors are claims, graded **[S]** or **[U]**, never treated as measured results.

---

## 1. Shortlist and decision rules

Phase 1 shortlisted seven models and excluded two. Since then, four relevant releases landed: Qwen3.8-27B (Aug 2026), Gemma 4 31B (2026, Apache-2.0), JetBrains Mellum2.1 (Oct 2026), and NVIDIA Nemotron 3.5 Bolt (Oct 2026). GLM-4.7-Flash (Jan 2026, MIT) was also missing from the Phase 1 table.

A model is eligible for 1.0 only if **all** of the following hold:

| Gate | Rule |
|---|---|
| G-1 Licence | Commercial use and redistribution permitted; notices can be satisfied by the installer. |
| G-2 Format | A GGUF quantisation exists from the vendor or a reputable quantiser. |
| G-3 Runtime | Architecture is supported by the pinned llama.cpp build. |
| G-4 Memory | Fits a supported tier with at least 8K context and ≥1 GiB compute headroom. |
| G-5 Tools | Chat template supports tool calling with a parser llama.cpp understands. |
| G-6 Evidence | At least one published coding/agentic evaluation, with the harness named. |

G-6 is deliberately weak: it admits vendor claims as *evidence of existence*, not as proof of quality. The Phase 3 benchmark is what decides.

## 2. Verified model facts

| Model | Repository | Revision (SHA) | Parameters | Architecture | Native ctx | Licence |
|---|---|---|---|---|---|---|
| **Qwen3.8-27B** | `Qwen/Qwen3.8-27B` | `1d4bf0f2` | 27.78B **[V]** | Dense multimodal; 64 layers, 48 Gated-DeltaNet linear-attention + 16 full-attention; 24 Q / 4 KV heads; head_dim 256 | 262,144 (1M YaRN) | Apache-2.0 **[V]** |
| **Qwen3-Coder-30B-A3B** | `Qwen/Qwen3-Coder-30B-A3B-Instruct` | `b2cff646` | 30.5B total / 3.3B active **[V]** | MoE, 128 experts / 8 active, 48 layers, 32 Q / 4 KV | 262,144 (1M YaRN) | Apache-2.0 **[V]** |
| **Qwen3-Coder-Next** | `Qwen/Qwen3-Coder-Next` | `a7fbcb5c` | 80B total / ~3B active **[S]** | `qwen3_next` hybrid attention MoE, 512 experts / 10 active | 262,144 | Apache-2.0 **[V]** |
| **Devstral Small 2 24B** | `mistralai/Devstral-Small-2-24B-Instruct-2512` | `55c5b41e` | 24B **[V]** | `mistral3` dense + vision tower; 40 layers, 32 Q / 8 KV, head_dim 128 | 256K claimed; config allows 393,216 | Apache-2.0 **[V]** |
| **gpt-oss-20b** | `openai/gpt-oss-20b` | `6cee5e81` | 21B total / 3.6B active **[V]** | MoE, 32 experts / 4 active, 24 layers alternating sliding (128) and full attention | 131,072 | Apache-2.0 **[V]** |
| **Mellum2.1-12B-A2.5B** | `JetBrains/Mellum2.1-12B-A2.5B-Thinking` | not pinned **[U]** | 12B total / 2.5B active **[V]** | MoE, 64 experts / 8 active, 28 layers, 32 Q / 4 KV, sliding window 1024 (3 of 4 layers) | 131,072 | Apache-2.0 **[V]** |
| **Gemma 4 31B** | `google/gemma-4-31B-it` | `842da379` | 31B (official) **[V]** | Dense multimodal; 60 layers, 50 sliding (1024) + 10 global; 32 Q / 16 KV local, 4 global KV, global head_dim 512 | 262,144 | Apache-2.0 **[V]** |
| **GLM-4.7-Flash** | `zai-org/GLM-4.7-Flash` | `7dd20894` | 31.2B total / ~3B active **[S]** | `glm4_moe_lite`, MLA, 4 experts active | 200K claimed | MIT **[V]** |
| **Nemotron 3.5 Bolt 74B-A9B** | `nvidia/NVIDIA-Nemotron-3.5-Bolt-74B-A9B-24GB` | `e0fecb69` **[V]** (GGUF repo) | 74B total / 9B active **[V]** | `nemotron_h_moe` Mamba-2/Transformer hybrid LatentMoE + vision | 262,144 | OpenMDW-1.1 **[V]** |
| **Qwen2.5-Coder-7B** | `Qwen/Qwen2.5-Coder-7B-Instruct` | `c03e6d35` | 7.62B **[V]** | Dense `qwen2`, 28 layers | 32,768 | Apache-2.0 **[V]** |

Row sources, in order: **S-14** Qwen3.8-27B, **S-15** Qwen3-Coder-30B, **S-16** Qwen3-Coder-Next, **S-17** Devstral, **S-18** gpt-oss-20b, **S-19** Mellum2.1, **S-20** Gemma 4, **S-21** GLM-4.7-Flash, **S-22** Nemotron Bolt, **S-23** Qwen2.5-Coder-7B. Qwen family context and tooling claims: **S-33**; release dates: **S-34**.

Revision SHAs are the Hub repository commit at review time. They must be re-pinned at build time and stored in the model manifest, not copied from this table ([`model-acquisition-policy.md`](model-acquisition-policy.md) §4).

## 3. Licence analysis

| Licence | Commercial use | Redistribution | Derivatives / quantisation | Obligations | Risk for Nexora |
|---|---|---|---|---|---|
| **Apache-2.0** (Qwen, Devstral, gpt-oss, Mellum, Gemma 4, Qwen2.5) | Yes | Yes, including bundled | Yes, must state changes | Ship `LICENSE`, retain `NOTICE` if present, include attribution, do not imply endorsement, no trademark grant | Low. The only work is packaging notices and recording quantisation as a modification. |
| **MIT** (GLM-4.7-Flash) | Yes | Yes | Yes | Include copyright and permission notice | Low. |
| **OpenMDW-1.1** (Nemotron Bolt) | Yes, per the card | Yes, per the card | Yes, per the card | Retain licence and notices; patent-termination clause | **Medium**: not OSI-approved; full text not reviewed here; legal sign-off required (`Q2-03`). |
| **Qwen3.8-Max custom** (2.4T-A95B) | Conditional | Conditional | Conditional | Bespoke terms | **High**: excluded, not needed at desktop scale. |
| **Mistral Non-Production** (Codestral) | No | No | No | — | **Prohibited**: excluded. |
| **Llama 4 Community** | Yes with conditions | Yes with conditions | Yes with conditions | 700M MAU clause, acceptable-use policy | **High**: excluded on memory profile too. |

Three practical consequences:

1. **Quantisation is a modification.** Apache-2.0 §4(b) requires modified files to carry prominent notices. The installer must include a `THIRD-PARTY-NOTICES` file that names the base model, the quantiser, the quantisation, and the licence — not just a copy of the licence text.
2. **Model output is not covered by the model licence in the way users assume.** We should not make claims about output ownership. Nexora's own terms must address user content separately (Phase 5/legal, `Q2-10`).
3. **The Gemma correction matters commercially.** Phase 1 marked Gemma 4 as "legal review required" under the old Gemma Terms. It is Apache-2.0. It remains watchlisted for memory reasons, not legal ones (`C2-01`).

## 4. Quantisation inventory (actual file sizes)

Sizes are decimal GB from the Hub file listing; GiB in brackets where useful for VRAM planning. `UD-` is Unsloth Dynamic quantisation; `MXFP4` is the native 4-bit floating-point format gpt-oss was trained with, not a post-hoc quantisation.

| Model | Quant | Bytes | Decimal GB | Fits |
|---|---|---:|---:|---|
| Qwen3.8-27B | UD-Q2_K_XL | 9,828,981,664 | 9.83 | 16 GB at 8K; 12 GB does not fit |
| | UD-Q3_K_XL | 13,146,393,504 | 13.15 | 16 GB usually spills; Q2 only |
| | **UD-Q4_K_M** | **16,464,440,224** | **16.46** | **24 GB, target** |
| | UD-Q4_K_XL | 17,559,178,144 | 17.56 | 24 GB, tight |
| | UD-Q5_K_M | 19,771,509,664 | 19.77 | 24 GB at 8K only |
| | Q8_0 | 29,047,086,048 | 29.05 | 32 GB+ |
| Qwen3-Coder-30B-A3B | UD-Q2_K_XL | 11,788,590,240 | 11.79 | 16 GB at 8K; 12 GB does not fit |
| | Q3_K_M | 14,711,850,144 | 14.71 | 16 GB at 8K, marginal |
| | Q4_K_M | 18,556,689,568 | 18.56 | 24 GB, tight |
| | Q8_0 | 32,483,935,392 | 32.48 | 48 GB |
| Devstral Small 2 | Q3_K_M | 11,474,619,552 | 11.47 | 16 GB at 8K |
| | IQ4_XS | 12,780,424,352 | 12.78 | 16 GB at 8K, tight |
| | Q4_K_M | 14,334,446,752 | 14.33 | 24 GB |
| | Q8_0 | 25,055,317,152 | 25.06 | 32 GB+ |
| gpt-oss-20b | **MXFP4** | **12,109,566,624** | **12.11** | **16 GB, target** |
| Mellum2.1 12B | MXFP4_MOE | 7,029,537,056 | 7.03 | 12 GB, roomy |
| | **Q4_K_M** | **8,071,295,264** | **8.07** | **12 GB, target** |
| | Q6_K | 10,879,834,400 | 10.88 | 16 GB, 32K |
| | Q8_0 | 12,925,417,760 | 12.93 | 16 GB |
| Gemma 4 31B | Q3_K_M | 14,736,608,192 | 14.74 | 24 GB |
| | Q4_K_M | 18,323,733,440 | 18.32 | 24 GB, very tight |
| | Q8_0 | 32,635,677,632 | 32.64 | 48 GB |
| Nemotron Bolt | IQ2 (native) | 22,018,326,528 | 22.02 | Does not fit 24 GB |
| | + mmproj | 1,733,692,736 | 1.73 | vision only; skip for code |
| Qwen2.5-Coder-7B | **Q4_K_M** | **4,683,073,536** | **4.68** | **8 GB, target** |

Sources: `S-14`, `S-15`, `S-17`, `S-18`, `S-19`, `S-20`, `S-21`, `S-22`, `S-23`, plus the GGUF trees `S-24`, `S-25`, `S-26`, `S-27`, `S-28`, `S-29`, `S-30`. Sizes are **[V]**; the "fits" column is **[E]**, computed with the KV and overhead model in §5 and [`hardware-matrix.md`](hardware-matrix.md) §3.

## 5. KV-cache geometry and usable context

KV bytes per token = `2 (K and V) × layers × KV heads × head_dim × bytes_per_element`. For FP16 that is 2 bytes. Sliding-window layers cap at the window size instead of growing with context. Hybrid linear-attention layers carry a fixed recurrent state rather than a per-token cache.

| Model | Layers | KV heads × head_dim | KV per token | 8K | 32K | 128K | Fixed state |
|---|---|---:|---:|---:|---:|---:|---:|
| Qwen3.8-27B | 64 (16 full) | 4 × 256 | 64 KiB | 0.5 GiB | 2.0 GiB | 8.0 GiB | ~0.14 GiB |
| Qwen3-Coder-30B-A3B | 48 | 4 × 128 | 96 KiB | 0.75 GiB | 3.0 GiB | 12.0 GiB | — |
| Devstral Small 2 | 40 | 8 × 128 | 160 KiB | 1.25 GiB | 5.0 GiB | 20.0 GiB | — |
| gpt-oss-20b | 24 (12 full) | 8 × 64 | 24 KiB | 0.19 GiB | 0.75 GiB | 3.0 GiB | ~3 MiB |
| Mellum2.1 12B | 28 (7 full) | 4 × 128 | 14 KiB | 0.11 GiB | 0.44 GiB | 1.75 GiB | ~42 MiB |
| Gemma 4 31B | 60 (10 global) | 4 × 512 global | 80 KiB | 0.63 GiB | 2.5 GiB | 10.0 GiB | ~0.8 GiB |
| Qwen2.5-Coder-7B | 28 | 4 × 128 | 56 KiB | 0.44 GiB | 1.75 GiB | n/a (32K max) | — |

Derived from each model's `config.json` **[V]**, arithmetic shown here so it can be checked. llama.cpp may allocate slightly differently (alignment, unified KV pool, checkpoints), and KV quantisation to Q8_0 roughly halves these figures **[U]** — the exact saving depends on the cache type implementation.

### What this means for "context claims"

| Claim | Usable on 12 GB | Usable on 16 GB | Usable on 24 GB |
|---|---|---|---|
| Qwen3.8-27B 262K | Not runnable | 8K at Q2 only | **32K** comfortable, 64K marginal |
| Qwen3-Coder-30B 262K | Not runnable | 8K at Q2 | 16K at Q4, 32K marginal |
| Devstral 256K | Not runnable | 8K at Q3 | 16–24K at Q4 |
| gpt-oss-20b 128K | Not runnable | **32K** comfortable, 64–128K marginal | 128K feasible |
| Mellum2.1 128K | **32K** comfortable | 128K feasible | 128K feasible |
| Gemma 4 31B 256K | Not runnable | Not runnable | 16K at Q4; 32K marginal |

These are **[E]**, with one decimal place of precision that the underlying allocation does not deserve. The point is the order of magnitude, not the exact gibibyte. **Nexora must never display the model's native context as the context the user will get.** The UI should show the configured context and warn when the conversation approaches it (Phase 1 doc 03 compaction at 80%).

## 6. Model-by-model assessment

### 6.1 Qwen3.8-27B — primary for 24 GB

**Strengths.** Apache-2.0 with a clean chain. Dense 27.78B, which makes quality predictable and quantisation behaviour stable. Hybrid Gated-DeltaNet attention means only 16 of 64 layers grow a KV cache, so 32K costs 2 GiB rather than 6–8 GiB. Vendor-reported agentic scores are the strongest in the sub-30B class; a third-party summary lists SWE-bench Pro 61.7, Terminal-Bench 2.1 73.0, NL2Repo 42.3 **[S, S-36]**. Template supports tools and three reasoning-effort levels **[V]**. A GGUF conversion exists with architecture `qwen35` and Qwen-family chatml templates are in the runtime's built-in list **[V]**; whether the hybrid-attention path is fully supported by the pinned build must be proven by a load test **[U]**.

**Weaknesses.** 16.46 GB at Q4 leaves ~5 GiB on a 24 GB card once KV and compute buffers are counted — enough for 32K, not for the advertised 262K. It is multimodal: the vision tower and projector add downloads and attack surface. llama.cpp auto-loads an `mmproj` when resolving a Hugging Face repo, so the acquisition policy must deliberately omit it (`P2-D-06`). One open parser bug specifically names Qwen3.8 and can discard a completed generation on a trailing `<think>` tag **[V, S-09]**, which is why raw-text fallback is mandatory.

**Verdict.** Primary for 24 GB. Highest expected quality per gigabyte among the eligible dense models.

### 6.2 gpt-oss-20b — primary for 16 GB

**Strengths.** Native MXFP4 checkpoint (12.11 GB) rather than a post-hoc 4-bit quantisation: the model was trained for this precision, which is a materially different risk profile from Q3/Q4 of a dense model. Only 3.6B active parameters and a 24 KiB/token KV cache, so context is cheap. Harmony-style template with a first-class `gpt-oss` chat template in llama.cpp **[V]**. Apache-2.0.

**Weaknesses.** Less code-specialised than the Qwen-Coder line; no reliable coding score was found in this review **[U]**. 21B total means the expert weights still occupy 12 GB even though few are active. Tool-channel parsing is a distinct format and needs its own validity tests.

**Verdict.** Primary for 16 GB. The native low-precision training is the deciding factor over a 3-bit Devstral.

### 6.3 Mellum2.1-12B-A2.5B — primary for 12 GB and CPU

**Strengths.** 8.07 GB at Q4_K_M, 2.5B active, 14 KiB/token KV, 131K native, sliding-window attention on 21 of 28 layers. Apache-2.0. Vendor-reported 47.0 SWE-bench Verified, 28.0 SWE-bench Pro, 17.4 Terminal-Bench 2.1 with the harness named (Pi v0.73.1, 114K context) **[S, S-37]** — modest absolute scores, but honest and recent. JetBrains ships an official GGUF repository **[V]**.

**Weaknesses.** Newest model in the set, smallest independent evidence base, and a `mellum` architecture whose llama.cpp support must be confirmed on the pinned build (it appears in Transformers docs; llama.cpp support was not verified here) **[U]**. Thinking-only variant adds latency to every turn unless reasoning is bounded.

**Verdict.** Primary for 12 GB. Best fit-for-memory in the shortlist, with the caveat that a 47% SWE-bench model is a junior assistant, not a replacement for a frontier hosted model.

### 6.4 Qwen3-Coder-30B-A3B — alternative for 24 GB, marginal for 16 GB

**Strengths.** Code-specialised MoE, 3.3B active, Apache-2.0, 256K native, tool-call format designed for agentic coding. Decode is fast relative to a dense 30B because only 3.3B parameters are read per token.

**Weaknesses.** 18.56 GB at Q4_K_M is larger than Qwen3.8-27B despite fewer active parameters, and its 96 KiB/token KV is 50% larger. On 24 GB it leaves ~3 GiB, which permits 8–16K, not 32K. On 16 GB even Q3 spills; only Q2 at 8K fits, with a visible quality loss. It is a 2025-generation model competing with a 2026 one.

**Verdict.** Alternative for 24 GB when tool-call format compatibility proves better than Qwen3.8 in the Phase 3 tests. Not the default.

### 6.5 Devstral Small 2 24B — watchlist

**Strengths.** Apache-2.0, 14.33 GB Q4_K_M, explicitly built for agentic file editing and tool use; a widely reported score of 68.0 SWE-bench Verified **[S, S-38]**. Fits 24 GB with 16–24K context.

**Weaknesses.** 160 KiB/token KV makes long context expensive: 20 GiB at 128K. The published chat template embeds a **Mistral Vibe CLI system prompt** and instructs the model to append a "Generated by Mistral Vibe" co-author trailer to commits **[V]**. Shipping that template unchanged would (a) make Nexora's assistant claim to be another product and (b) inject attribution into the user's Git history. Nexora must override the system prompt and strip tool-side commit trailers.

**Verdict.** Watchlist. Reconsider only if it beats Qwen3.8 on multi-file edits in Phase 3, and only with a rewritten prompt/template.

### 6.6 Gemma 4 31B — watchlist (licence now clean)

**Strengths.** Apache-2.0 (corrected, `C2-01`). 262K context, strong general reasoning, Google-maintained template with tool support.

**Weaknesses.** 18.32 GB at Q4_K_M on a 24 GB card leaves ~2 GiB after KV at 16K — not enough for a comfortable desktop workload. A 31B dense model means slow decode relative to MoE options. No verified coding score was found here **[U]**.

**Verdict.** Watchlist for 32 GB+ cards. Not a 24 GB default.

### 6.7 GLM-4.7-Flash — watchlist

**Strengths.** MIT, 30B-A3B, MLA (compact KV), 200K context claimed, 59.2 SWE-bench Verified claimed **[S, S-39]**. Similar memory profile to Qwen3-Coder-30B with a more permissive licence and a newer generation.

**Weaknesses.** Runtime support is via the `deepseek2` architecture path in community GGUFs; agentic behaviour claims are vendor-sourced; the 200K context needs verification against the actual config. Not evaluated for tool-call validity.

**Verdict.** Watchlist. A credible 24 GB alternative if 200K context is a product requirement.

### 6.8 Nemotron 3.5 Bolt 74B-A9B — watchlist, legal gate

**Strengths.** 74B total / 9B active in a 22.02 GB 2-bit checkpoint explicitly designed to run on a 24 GB card alongside other applications **[V]**. Highest nominal capability in the set. llama.cpp recipes are published by NVIDIA for 28/32 GB configurations **[V]**.

**Weaknesses.** 2.0625 bits per weight is an aggressive regime; quality depends on the quantisation-aware training NVIDIA describes, and independent verification is thin **[S]**. OpenMDW-1.1 needs legal review. 22.02 GB on a 24 GB card leaves almost nothing for KV — the published recipes target 28 GB and above, so "24 GB" is nominal, not comfortable.

**Verdict.** Watchlist. Do not ship until legal sign-off and a 24 GB measurement exist.

### 6.9 Qwen3-Coder-Next — high-end reference only

80B total / ~3B active, 512 experts, Apache-2.0. The technical report reports 70.6–71.3 SWE-bench Verified across three scaffolds **[S, S-16]**. At Q4 it is ~45–50 GB; it is not a 1.0 desktop model. Retained as the quality ceiling for the Phase 3 high-end reference machine, if one exists.

### 6.10 Qwen2.5-Coder-7B — light mode

4.68 GB at Q4_K_M from the official Qwen GGUF repo **[V]**, Apache-2.0, 32K native. Older and weaker at agentic work, but it runs on 8 GB and on CPU, and it is the model most likely to make the app usable on a machine that would otherwise be unsupported. It must be labelled as light mode, with the honesty vocabulary from doc 05.

## 7. Tier selection

| Tier | Default | Context | Alternatives | Rationale |
|---|---|---|---|---|
| **24 GB** | Qwen3.8-27B UD-Q4_K_M | 32K | Qwen3-Coder-30B-A3B Q4 (16K), GLM-4.7-Flash (test) | Best quality that fits with usable context |
| **16 GB** | gpt-oss-20b MXFP4 | 32K | Devstral Q3 (8K), Mellum Q6 (32K) | Native low precision + cheap KV |
| **12 GB** | Mellum2.1 Q4_K_M | 32K | Mellum Q6 (needs 16 GB) | Fit without offload |
| **8 GB** | Qwen2.5-Coder-7B Q4_K_M | 16K | Mellum Q4 with partial offload (slow) | Light mode |
| **CPU-only** | Qwen2.5-Coder-7B Q4_K_M | 8–16K | Mellum Q4 if ≥16 GB RAM | Latency floor matters more than quality |
| **48 GB+** | Qwen3-Coder-Next Q4 (test) | 64K | Qwen3.8-27B Q8 | Reference ceiling, not a 1.0 SKU |

"Fits" assumes the model is fully offloaded and that the Windows desktop compositor needs ~1–2 GB of VRAM. Partial offload is supported but must be presented as a degraded mode, because on Windows a silent spill to system memory produces token rates an order of magnitude lower without an error ([`runtime-evaluation.md`](runtime-evaluation.md) §3.2).

## 8. Excluded models

| Model | Reason |
|---|---|
| Codestral 22B | Mistral Non-Production Licence forbids production use. |
| Llama 4 Scout | Llama Community Licence conditions plus ≥60 GB footprint. |
| Qwen3.8-2.4T-A95B | Custom Qwen3.8-Max licence; 2.4T parameters is not a desktop model. |
| Qwen3.8-Flash-Next | Qwen Community Licence 1.0; 125B; no compatibility evidence. |
| DeepSeek V4.x, Kimi K3, GLM-5.x, MiMo | 100B–1.6T class; cannot run on supported tiers. Worth revisiting only if a future tier supports them. |
| Gemma 3 and earlier | Superseded by Gemma 4 under a cleaner licence. |

## 9. Security and limitations of model choice

| Concern | Detail | Mitigation |
|---|---|---|
| Prompt injection via repository content | Every model is susceptible; a stronger model is not a defence. | Phase 1 doc 05/07 controls: untrusted-data framing, tool allowlist, approval gates. Model choice does not change this. |
| Vision inputs | Qwen3.8, Devstral, Gemma 4, Nemotron are multimodal. Images are an injection channel and add download size. | Ship text-only GGUFs; do not download `mmproj`; disable multimodal flags (`P2-D-06`). |
| Tool-call fabrication | Smaller models invent tool names or arguments. | Schema validation, unknown-tool rejection, bounded repair (doc 05). |
| 2-bit quantisation | Nemotron Bolt's 2.06 bpw may fail subtly on long code edits. | Not a default; requires measurement before use. |
| Thinking-token cost | Qwen3.8 and Mellum emit reasoning tokens that consume context and time. | `--reasoning-budget` caps; UI must show thinking separately and never bill it as answer content. |
| Training-data provenance | Not auditable for any model here. | Do not claim provenance; include licences as published; legal review for enterprise claims. |
| Commit-trailer injection | Devstral's template instructs a "Generated by Mistral Vibe" trailer. | Override template; strip trailers in the Git tool. |
| Model deprecation | Vendors can withdraw repos. | Mirror pinned files; never resolve "latest" at runtime. |

## 10. What remains unmeasured

1. Whether any of these models, at these quants, produce valid tool calls at the ≥98% gate.
2. Whether Qwen3.8-27B's `qwen3_5` hybrid attention is correctly and efficiently implemented in build `b11714`.
3. Whether Mellum2.1's `mellum` architecture runs at all on the pinned build.
4. Decode and prefill rates on real Windows hardware.
5. Quality on Nexora's own repository tasks (the only quality claim that matters).
6. Whether 32K is genuinely usable on 24 GB after Windows compositor and driver overhead.

Each is a Phase 3 gate, not a footnote. Until they are answered, "preferred model" means "best-supported bet from documentary evidence".

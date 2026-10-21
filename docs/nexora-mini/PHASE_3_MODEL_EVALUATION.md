# NEXORA MINI - PHASE 3
## Local AI Model Selection and Evaluation

| | |
|---|---|
| **Phase** | 3 - model selection research |
| **Evaluation date** | 2026-10-20 |
| **Evidence cutoff** | sources accessed 2026-10-20 (UTC) |
| **Repository** | `cybervault-Hacky/Nexora`, branch `arena/18030bbb-nexora` |
| **HEAD at start and end of phase** | `9bd4f7ad36ab012b50e48333c27fdc9b8244d5ff` |
| **Status** | Research complete. **No model integrated, downloaded, configured or executed.** |
| **Phase 2 status** | **Still unverified.** See section 10. |
| **Phase 4 status** | **Not started.** |

---

## 0. Status of this phase

This document is an evaluation, not an integration. In this phase Nexora was **not** given:

- a model registry or selector;
- model IDs or profiles wired into application behaviour;
- any downloaded or bundled model weights;
- any change to runtime configuration or to the user's configured model path;
- any started model process;
- any UI, export, quota, account, telemetry, payment or licensing change.

Everything below is desk research against the code that already exists in this repository, plus primary-source model documentation. **No inference, memory, latency or output-quality measurement was performed on this machine.** Where a number is not a published fact, it is labelled as an estimate or as unverified.

### Evidence labels used throughout

| Label | Meaning |
|---|---|
| **[FACT]** | Stated in a primary source (publisher model card, publisher repository, upstream project source) and reproduced here exactly; source ID in brackets. |
| **[DERIVED]** | Arithmetic on published numbers (byte sizes, layer counts, context lengths). The arithmetic is shown so it can be checked. |
| **[EST]** | Engineering estimate from architecture and typical hardware behaviour. **Not measured.** Must be replaced by a measurement before it is used in a product decision. |
| **[UNVERIFIED]** | Could not be established from a primary source, or depends on this machine's Windows behaviour. Listed again in section 8. |

---

## 1. Executive summary

**Primary recommendation:** `Qwen/Qwen2.5-Coder-7B-Instruct-GGUF`, file `qwen2.5-coder-7b-instruct-q4_k_m.gguf`, pinned to repository revision `13fb94bfda8c8cf22497dc57b78f391a9acb426a`.

**Fallback (larger machines only):** `Qwen/Qwen2.5-Coder-14B-Instruct-GGUF`, file `qwen2.5-coder-14b-instruct-q4_k_m.gguf`, revision `d0a692ef765eefbf2fabb130b3cb2e8917e3d225`, run at **8,192-16,384** context on a machine with **32 GB RAM or more**.

**Not recommended for Nexora Mini:** `DeepSeek-Coder-V2-Lite-Instruct`. Reasons: a custom licence with use-based restrictions and PRC governing law, no official GGUF from the publisher, the largest download of the three (9.65 GiB at Q4_K_M), a non-ChatML template that llama.cpp classifies as "Generic", and the weakest instruction-following/edit-format scores of the three.

Why the 7B wins, in the user's stated priority order:

1. **Licensing clarity.** Apache-2.0, stock licence text shipped in the official GGUF repository, no field-of-use restrictions, no acceptance clause, no governing-law surprise.
2. **Reliable local inference.** The publisher - not a third party - ships the GGUF and its own llama.cpp instructions, and llama.cpp lists `qwen2` as a first-class architecture [S11].
3. **16 GB Windows practicality.** 4.36 GiB of weights plus 1.75 GiB of KV cache at the default 32,768 context leaves room for Windows. It is the only one of the three that passes Nexora's own memory gate on a typical 16 GB machine at the default context (section 4.3).
4. **Coding quality.** Publisher-reported HumanEval+ 84.1, MBPP+ 71.7, Aider 55.6 - ahead of DeepSeek-Coder-V2-Lite-Instruct on all three at roughly half the download.
5. **CPU usability.** Smallest working set of the three and a dense architecture with predictable CPU behaviour.

The 14B is a genuine quality upgrade (HumanEval+ 87.2, Aider 58.6) but needs roughly 15.4 GiB of RAM at 32K context before Windows is considered, which is not a 16 GB machine.

**What this phase cannot conclude:** that any candidate reliably emits Nexora's strict single-file JSON patch contract. No candidate has been measured against it, and no public benchmark measures that exact format. That is the first thing the Windows test plan in section 9 must establish.

---

## 2. Audit before acting

### 2.1 Repository state

Verified with `git` immediately before writing this document, and again after writing it.

| Check | Result |
|---|---|
| Branch | `arena/18030bbb-nexora` |
| HEAD | `9bd4f7ad36ab012b50e48333c27fdc9b8244d5ff` |
| `main`, `origin/main` | both `9bd4f7ad36ab012b50e48333c27fdc9b8244d5ff` (equal to HEAD) |
| Staged changes | none |
| Unstaged changes | 15 modified Rust files, `+945 / -266` |
| Untracked | `docs/nexora-mini/` (Phase 1 and Phase 2 reports) |
| Stashes / tags | none |
| Ignored output | `node_modules/`, `apps/desktop/node_modules/`, `apps/desktop/ui/dist/` |

Nothing was reset, reverted, cleaned, stashed or committed. The 15 modified files are the Phase 2 fix set and are untouched by this phase.

### 2.2 What the inference runtime actually does

`apps/desktop/src-tauri/src/inference/process.rs` spawns `llama-server` with exactly this argv [R1]:

```text
--model <path> --host 127.0.0.1 --port <ephemeral> --ctx-size <settings.contextSize>
--parallel 1 --no-ui --jinja --api-key-file <config-dir>/runtime-session.key --timeout 600
```

Consequences for model selection:

- **CPU only.** No `-ngl` / `--n-gpu-layers` and no KV-cache type flags are passed, so llama.cpp keeps its default GPU-layer count of zero. A model is evaluated here as a CPU model.
- **One slot.** `--parallel 1` means a single KV cache of `contextSize` tokens, not `contextSize x parallel`.
- **`--jinja`.** The chat template embedded in the GGUF is authoritative. This is why template shape matters per candidate.
- **`--ctx-size` comes straight from settings.** `DEFAULT_CONTEXT_SIZE = 32_768`, `MIN = 2_048`, `MAX = 131_072` [R2]. Nothing clamps a candidate to its documented usable context.
- **No grammar, no `response_format`, no `stop`.** The request body in `inference/client.rs` is only `model`, `stream`, `messages`, `max_tokens`, `temperature` [R3]. **There is no constrained decoding today.** Format compliance must come from the model.
- **Timeouts.** 180 s to load, 1.2 s health polling, 600 s to first token, 120 s idle, 400,000-character response cap [R1].
- **Build pin.** `PINNED_BUILD = "11714"`, reported to the user but not enforced [R1]. The runtime is user-supplied; Nexora never downloads it.

### 2.3 What the application demands of a model

`workspace/patch_contract.rs` is stricter than a chat UI and is the real target [R4]:

- one JSON object, camelCase keys, `deny_unknown_fields`;
- required `version` (must equal 1), `targetPath` (must equal the requested file), `proposedContent` (the **entire replacement file**, not a diff), optional `explanation`;
- `proposedContent` must be non-empty, must differ from the original, must contain no NUL, and must stay under 200,000 characters / `MAX_PREVIEW_BYTES`.

The parser is tolerant about framing - it will pull JSON out of a fenced block or locate the first JSON object [R4] - but it is not tolerant about shape: an extra top-level key, a missing key, or an unescaped newline inside a string fails the whole generation.

This is the single most product-relevant quality axis, and it is exactly the axis with the least public evidence.

### 2.4 The memory heuristic, and what it does not model

`inference/models.rs` estimates [R5]:

```text
estimated_min_ram_gib = size_gib + (0.5 + context_size / 32768 * 1.5) + 1.0
fits = estimated_min_ram_gib <= max(available - 2.0, 0.0)
```

The comment above it is explicit: *"a deliberately blunt planning figure ... It is an estimate, not a measurement, and it excludes architecture-specific KV growth beyond the allowance."* That is accurate. The allowance is 48 KiB per token, which happens to be close to Qwen2.5-7B's real KV cost (56 KiB/token) and is far below Qwen2.5-14B's (192 KiB/token). Section 4.4 quantifies the gap. This is not a Phase 2 defect; it is a known limitation that Phase 4 must account for.

### 2.5 Derived requirements

From sections 2.2-2.4, a candidate must satisfy:

| ID | Requirement | Source |
|---|---|---|
| DR-1 | Single-file GGUF, version 2 or 3, at least 16 MiB, loadable by a b11714-class runtime | [R5], [R1] |
| DR-2 | Usable on CPU only, at 32,768 context, on a 16 GB Windows machine | [R1], [R2] |
| DR-3 | Architecture and chat template supported by llama.cpp's Jinja path | [R1] |
| DR-4 | Clear licence for redistribution **and** for a future downloader | product constraint |
| DR-5 | Emits Nexora's exact JSON contract reliably | [R4] |
| DR-6 | Pinnable to an exact revision and byte size | reproducibility constraint |
| DR-7 | Does not need flags Nexora does not pass (GPU offload, rope scaling, KV quantisation) | [R1] |

---

## 3. Candidates

Three candidates were evaluated. All are instruction-tuned code models with open weights; all were current when the 16 GB target was fixed, and newer families were considered and excluded (section 3.4).

### 3.1 Qwen2.5-Coder-7B-Instruct (official GGUF)

#### 3.1.1 Identity and revision

| Field | Value | Class |
|---|---|---|
| Publisher | Qwen team, Alibaba Group | [FACT] [S1] |
| Base model | `Qwen/Qwen2.5-Coder-7B` | [FACT] [S1] |
| Parameters | 7.61B total, 6.53B non-embedding | [FACT] [S1] |
| Layers | 28 | [FACT] [S1] |
| Attention | GQA, 28 Q heads / 4 KV heads, hidden 3584, head dim 128 | [FACT] [S1 config]; head dim derived (3584 / 28) |
| Vocab | 152,064 | [FACT] [S1 config] |
| Weights revision (HF) | `c03e6d358207e414f1eca0bb1891e29f1db0e242` | [FACT] [S1] |
| GGUF repository revision | `13fb94bfda8c8cf22497dc57b78f391a9acb426a` (last modified 2024-11-12) | [FACT] [S2] |
| GGUF architecture metadata | `qwen2`, EOS `<\|im_end\|>` | [FACT] [S2] |

#### 3.1.2 Licence

Apache License 2.0 [FACT] [S2]. The official GGUF repository contains a `LICENSE` file of 11,343 bytes whose text is the stock Apache-2.0 grant and redistribution terms [S2]. There is no acceptable-use appendix, no MAU threshold, no registration requirement, and no additional field-of-use restriction. Redistribution obligations are the ordinary Apache-2.0 ones: ship a copy of the licence, retain notices, mark modified files, and reproduce any `NOTICE` file if one exists (none was observed in the repositories). Section 7 covers this in table form.

#### 3.1.3 GGUF availability and quantisations

Published by the model's own publisher, not a third party [S2][S3].

| File | Bytes | GiB | Notes |
|---|---:|---:|---|
| `...-fp16.gguf` | 15,237,853,184 | 14.19 | also split into 4 parts |
| `...-q8_0.gguf` | 8,098,525,184 | 7.54 | split variant also published |
| `...-q6_k.gguf` | 6,254,198,784 | 5.82 | |
| `...-q5_k_m.gguf` | 5,444,831,232 | 5.07 | |
| **`...-q4_k_m.gguf`** | **4,683,073,536** | **4.36** | **recommended candidate** |
| `...-q4_0.gguf` | 4,431,390,720 | 4.13 | |
| `...-q3_k_m.gguf` | 3,808,391,104 | 3.55 | |
| `...-q2_k.gguf` | 3,015,940,032 | 2.81 | |

Sizes are exact byte counts from the Hugging Face tree API [S3]; GiB are derived by dividing by 2^30 [DERIVED]. All are single files except where a split variant is noted; the single-file `q4_k_m` exists and is the one to pin.

*Not verified in this phase:* the GGUF version field inside the file. Nexora accepts versions 2 and 3 [R5], and confirming the header requires the file itself, which was deliberately not downloaded. M-0 covers it.

**Compatibility with the pinned runtime.** `qwen2` is registered in llama.cpp's architecture table [S11], and the pinned build `b11714` is the runtime that must load this file [R1]. Registration is necessary but not sufficient - template rendering, tokenizer handling and RoPE behaviour are build-specific - so the compatibility claim is closed only by actually loading the file in M-0 and streaming from it in M-2.

The publisher's own quickstart for this repository uses `llama-cli` and refers to the Qwen llama.cpp documentation [S2], so no conversion step or third-party re-quantisation is required.

#### 3.1.4 Context

This candidate has a **documentation conflict that matters**:

- The model card for the 7B weights advertises a full 131,072-token context, with YaRN recommended for longer inputs - but the shipped `config.json` declares `max_position_embeddings: 32768` and contains no `rope_scaling` block [S1].
- The publisher's own **GGUF** card says: *"Context Length: Full 32,768 tokens. Note: Currently, only vLLM supports YARN for length extrapolating. If you want to process sequences up to 131,072 tokens, please refer to non-GGUF models."* [S2]
- The Hugging Face API's GGUF metadata block reports `context_length: 131072` for the same repository [S2].

The safe engineering position is **32,768**, the only figure the publisher claims for the GGUF artefact. Nexora passes no rope-scaling or YaRN flag [R1], so it cannot enable the extrapolation path even if the GGUF metadata permits it. Settings above 32,768 should be treated as **[UNVERIFIED]** and must not be offered as a supported configuration for this model without a needle-in-the-code test.

#### 3.1.5 Coding quality (publisher-reported)

From the Qwen2.5-Coder technical report, Table 16 and Table 19 [S6]. These are the publisher's own numbers on the unquantised weights; they were not independently reproduced here and do not describe the q4_k_m artefact.

| Benchmark | Qwen2.5-Coder-7B-Instruct | Qwen2.5-Coder-14B-Instruct | DeepSeek-Coder-V2-Lite-Instruct |
|---|---:|---:|---:|
| HumanEval | 88.4 | 89.6 | 81.1 |
| HumanEval+ | 84.1 | 87.2 | 75.6 |
| MBPP | 83.5 | 86.2 | 82.8 |
| MBPP+ | 71.7 | 72.8 | 70.4 |
| BigCodeBench (full) | 41.0 | 48.4 | 36.8 |
| BigCodeBench (hard) | 18.2 | 22.2 | 16.2 |
| LiveCodeBench (2407-2409) | 18.2 | 23.4 | 16.3 |
| **Aider (edit format), pass@1** | **55.6** | **58.6** | **44.4** |
| Aider pass@2 | 68.4 | 69.2 | 52.6 |
| MultiPL-E average | 76.5 | 79.6 | 73.2 |
| CRUXEval Output-CoT | 65.9 | 79.5 | 52.9 |
| IFEval | 58.6 | not captured | 38.6 |

The Aider row is the closest public proxy for "edit an existing file and emit a machine-consumable patch", which is what Nexora asks for. The 7B beats the DeepSeek model by 11.2 points there, on a 4.36 GiB download versus 9.65 GiB. The 14B is 3.0 points ahead of the 7B - a real but modest gain for more than double the memory.

Cross-check: DeepSeek's own card reports HumanEval 81.1 and MBPP+ 68.8 for the Lite Instruct model [S7], against Qwen's measurement of 81.1 and 70.4 [S6]. HumanEval agrees exactly; MBPP+ differs by 1.6 points, which is within normal harness variance. LiveCodeBench is not comparable because the evaluation windows differ. This is a mild point in favour of both publishers' honesty, and a reminder that benchmark deltas under ~2 points are noise.

#### 3.1.6 Structured output and patch-format adherence

**[UNVERIFIED]** for Nexora's contract. There is no published benchmark of this exact schema. What can be said:

- The chat template is ChatML (`<|im_start|>system ... <|im_end|>`), so Nexora's system prompt is delivered in a real system slot [S2], unlike the DeepSeek candidate.
- llama.cpp's function-calling documentation lists "Qwen 2.5 Coder" among the templates it recognises natively [S10] - relevant only if tool calling is ever used, which it is not today.
- Aider's 55.6 is evidence that the model can follow a strict edit format often enough to complete tasks, not that it will emit clean JSON under Nexora's prompt.
- Quantisation at q4_k_m may reduce format compliance relative to the unquantised model. **[UNVERIFIED]**

#### 3.1.7 CPU practicality

**[EST]** With no GPU layers, decode is memory-bandwidth bound. Reading 4.36 GiB of weights per token through a laptop's dual-channel memory suggests a ceiling in the high single digits and a realistic 4-9 tokens/second on a modern 16 GB laptop; prompt processing in the 40-120 tokens/second range. A cold load from NVMe should reach `/health` in single-digit seconds; Windows Defender's first scan of a new 4.7 GB file can dominate that. Nexora's 180 s load and 600 s first-token allowances are comfortable for this model and would be tested in section 9, not assumed.

#### 3.1.8 Known limitations

- Hard practical context of 32,768 with this artefact [S2]; the 131,072 figure belongs to the non-GGUF weights and a vLLM YaRN path.
- Publisher-reported scores only; no independent reproduction of the q4_k_m file.
- 7B-class reasoning is weaker than the 14B on multi-file or multi-step edits; Nexora's single-file contract limits the exposure.
- Qwen's original Qwen2.5-Coder GitHub repository has since been repurposed to Qwen3-Coder content [S14]; the Hugging Face model cards and the pinned revision remain the authoritative artefacts. This is a source-stability argument for pinning by revision rather than by repository HEAD.

### 3.2 Qwen2.5-Coder-14B-Instruct (official GGUF)

| Field | Value | Class |
|---|---|---|
| Parameters | 14.77B | [FACT] [S4] |
| Layers | 48 | [FACT] [S4] |
| Attention | 40 Q heads / 8 KV heads, hidden 5120, head dim 128 | [FACT] [S4] |
| `max_position_embeddings` | 32,768 | [FACT] [S4] |
| Licence | Apache-2.0 | [FACT] [S5] |
| GGUF repository revision | `d0a692ef765eefbf2fabb130b3cb2e8917e3d225` (last modified 2024-11-12) | [FACT] [S5] |
| `q4_k_m` file | `qwen2.5-coder-14b-instruct-q4_k_m.gguf`, 8,988,110,272 bytes (8.37 GiB) | [FACT] [S5] |
| Other quants | q2_k 5.37, q3_k_m 6.84, q4_0 7.93, q5_k_m 9.79, q6_k 11.29, q8_0 14.62, fp16 27.52 GiB | [DERIVED] [S5] |

**Context.** The 14B `config.json` declares `max_position_embeddings: 32768` [S4], the same value the 7B config declares. There is no YaRN block to appeal to. 32,768 is therefore the architectural ceiling for this model, and Nexora's settings (up to 131,072) can be set past it with no warning.

**Memory consequence.** 48 layers x 8 KV heads x 128 dims x 2 (K and V) x 2 bytes = 196,608 bytes per token = 192 KiB/token [DERIVED]. At 32,768 tokens that is 6.00 GiB of KV cache before 8.37 GiB of weights. Section 4 covers the totals.

**Quality.** HumanEval+ 87.2, MBPP+ 72.8, Aider 58.6; consistently a few points ahead of the 7B and well ahead of DeepSeek-Lite [S6]. BIRD 56.9 / Spider 84.8 on text-to-SQL [S6], which is a better relative result than its code-generation gap.

**Verdict.** The best-quality candidate and the worst fit for the product's stated 16 GB floor. It is a defensible fallback on a 32 GB machine at 8K-16K context, not a default.

### 3.3 DeepSeek-Coder-V2-Lite-Instruct (community GGUF)

| Field | Value | Class |
|---|---|---|
| Publisher | DeepSeek AI | [FACT] [S7] |
| Total / active parameters | 15.7B / 2.4B (MoE, 64 routed experts + 2 shared, 6 per token) | [FACT] [S7 config] |
| Layers | 27 | [FACT] [S7 config] |
| Attention | MLA: 16 heads, `kv_lora_rank` 512, rope dim 64 | [FACT] [S7 config] |
| Declared context | 128K (config `max_position_embeddings` 163,840; YaRN factor 40, original 4,096) | [FACT] [S7 config] |
| Licence | `other` / `deepseek-license` (DeepSeek License Agreement v1.0) | [FACT] [S7][S8] |
| Official GGUF | **none** - no GGUF repository under `deepseek-ai` | [FACT] [S13] |
| Community GGUF | `bartowski/DeepSeek-Coder-V2-Lite-Instruct-GGUF`, revision `8f248fa2072348f77a8bc37754e470de1f61866e` | [FACT] [S9] |
| `Q4_K_M` file | 10,364,416,768 bytes (9.65 GiB) | [FACT] [S9] |
| Other quants | Q2_K 5.99, IQ4_XS 7.98, Q3_K_M 7.57, Q4_K_S 8.88, Q5_K_M 11.04, Q6_K 13.10, Q8_0 15.56 GiB | [DERIVED] [S9] |

**Licence.** Not Apache-2.0 and not OSI-approved. The DeepSeek License Agreement grants commercial use - the model card states that the series "supports commercial use" [S7] - but it is conditional:

- redistribution must include the licence **and** must embed the Attachment A use restrictions as enforceable terms in the downstream agreement, with notice to subsequent users [S8];
- modified files must carry prominent notices; copyright/attribution notices must be retained;
- restricted uses include military use, disinformation, PII misuse, defamation, fully automated decisions affecting legal rights, and discrimination [S8];
- governing law is the PRC, with exclusive jurisdiction in Hangzhou [S8];
- the licensor reserves the right to "restrict (remotely or otherwise)" usage that violates the licence [S8].

There is also a documentation defect worth recording: the model card's front matter points `license_link` at a `LICENSE` file that **does not exist** in the Hugging Face repository - the raw path returns "Entry not found" and the repository tree contains no licence file [S7]. The actual licence lives in the GitHub repository as `LICENSE-MODEL` [S8]. A downstream user following the card's link hits a dead end.

**Runtime fit.** The architecture is supported by llama.cpp (`LLM_ARCH_DEEPSEEK2` is present in `llama-arch.cpp`, and the community GGUF declares `architecture: deepseek2`) [S11][S9]. But this is a converted, imatrix-quantised third-party artefact, not a publisher build. Any defect in the conversion is between the user and the quantiser.

**Template.** The chat template is not ChatML:

```text
<｜begin▁of▁sentence｜>{system}

User: {user}

Assistant: {assistant}<｜end▁of▁sentence｜>User: {next user}

Assistant:
```

The system message is concatenated as naked text with no role delimiter, and there is no turn marker for it [S7]. llama.cpp's function-calling documentation classifies the DeepSeek-Coder-V2-Lite-Instruct template as **Generic** (no native handler) [S10]. Nexora relies entirely on `--jinja` and the model's own instruction-following here, with a weaker structural signal than Qwen's `<|im_start|>system`.

**Quality.** HumanEval+ 75.6, MBPP+ 70.4, Aider 44.4, IFEval 38.6 [S6]. It is behind the 7B on every shared benchmark while being more than twice the download.

**Why it was included at all.** Because it is a 16B-class MoE with 2.4B active parameters: on paper its CPU decode could be faster than the 14B dense model despite the larger file, and its 128K claim is attractive for repository context. Both advantages are unproven here, and both are outweighed by the licence and the size.

### 3.4 Considered and excluded

| Candidate | Why excluded |
|---|---|
| Qwen3-Coder-30B-A3B | 4-bit weights alone exceed a 16 GB machine's practical memory before KV cache; outside the product's stated floor [S14] |
| Qwen3-Coder-Next (80B-A3B) | An order of magnitude outside the hardware target [S14] |
| Qwen2.5-Coder-3B | Cheaper but a clear quality step down; would not give a useful patch contract |
| Qwen2.5-Coder-32B | 4-bit weights ~18 GiB; cannot fit the target machine |
| DeepSeek-Coder-V2-Lite at Q3_K_M (7.57 GiB) | Saves 2 GiB but still needs ~8.8 GiB total at 8K; 3-bit MoE quality is unmeasured and the licence objection remains |
| Qwen2.5-Coder-14B at Q3_K_M (6.84 GiB) | Fits more machines at 8K, but 3-bit degradation on a 14B is not demonstrated to beat 4-bit 7B; would need the same harness |

No newer model was excluded for being "old". Recency was not a criterion; licence, fit and measured quality were.

---

## 4. Quantisation, size, memory and context

### 4.1 Weights are not the memory requirement

llama.cpp memory-maps the model by default, so the file does not have to be copied into private memory. But every weight is touched during a forward pass, so on a machine that is actually using the model, almost all of it becomes resident or page-cache-backed. The planning number must be **weights + KV cache + compute/runtime overhead**, not the download size. This is exactly the inference the brief forbids ("never infer a 16 GB fit from file size alone"), and it is why the KV arithmetic below is done explicitly.

### 4.2 KV cache arithmetic

Formula: `layers x kv_heads x head_dim x 2 (K and V) x 2 bytes (f16) x tokens`. Nexora does not pass a KV-cache type flag, so f16 is the default [R1].

| Model | Per token | 8,192 ctx | 16,384 ctx | 32,768 ctx | 131,072 ctx |
|---|---:|---:|---:|---:|---:|
| Qwen2.5-Coder-7B (28 x 4 x 128) | 57,344 B (56 KiB) | 0.44 GiB | 0.88 GiB | **1.75 GiB** | 7.00 GiB |
| Qwen2.5-Coder-14B (48 x 8 x 128) | 196,608 B (192 KiB) | 1.50 GiB | 3.00 GiB | **6.00 GiB** | 24.00 GiB |
| DeepSeek-Coder-V2-Lite (27 x (512+64) x 2 B) | 31,104 B (30.4 KiB) | 0.24 GiB | 0.47 GiB | **0.95 GiB** | 3.80 GiB |

[DERIVED] from the layer/head counts in [S1][S4][S7]. DeepSeek's MLA caches a 512-wide latent plus a 64-wide rope key per layer; that is still small, which is the model's one clear memory advantage.

### 4.3 Realistic fit on a 16 GB Windows machine

Assumptions, stated so they can be challenged: Windows 11 x64, 16 GB installed, 9-12.5 GiB actually available after the OS and background applications; **+1.0 GiB** for compute buffers, the runtime, and allocator overhead; no GPU offload; f16 KV.

| Model | Weights | KV @32K | Realistic total @32K | Nexora estimate @32K | Verdict on 16 GB @32K |
|---|---:|---:|---:|---:|---|
| Qwen2.5-Coder-7B q4_k_m | 4.36 | 1.75 | **~7.1 GiB** | 7.36 | Fits, with normal headroom |
| Qwen2.5-Coder-14B q4_k_m | 8.37 | 6.00 | **~15.4 GiB** | 11.37 | Does not fit |
| DeepSeek-V2-Lite Q4_K_M | 9.65 | 0.95 | **~11.6 GiB** | 12.65 | Marginal to no; OS pressure |

At 8,192 context the picture changes:

| Model | Realistic total @8K | Nexora estimate @8K | Verdict on 16 GB @8K |
|---|---:|---:|---|
| Qwen2.5-Coder-7B | ~5.8 GiB | 6.23 | Comfortable |
| Qwen2.5-Coder-14B | ~10.9 GiB | 10.25 | Marginal - the estimate is now slightly optimistic |
| DeepSeek-V2-Lite | ~10.9 GiB | 11.53 | Marginal |

[EST] for the overhead constant and the available-RAM range; [DERIVED] for everything else. The overhead figure is the one that section 9 must measure.

**Conclusion.** On the stated 16 GB floor, at the default 32,768 context, only the 7B is a sound default. Everything else is a power-user configuration.

### 4.4 Where Nexora's heuristic and reality diverge

The heuristic charges 1.5 GiB of KV at 32K (48 KiB/token) [R5]:

| Model | Actual KV @32K | Heuristic allowance | Error | Effect |
|---|---:|---:|---:|---|
| Qwen2.5-Coder-7B | 1.75 GiB | 1.50 GiB | -0.25 GiB | Slightly conservative overall; harmless |
| Qwen2.5-Coder-14B | 6.00 GiB | 1.50 GiB | **-4.50 GiB** | Under-warns: 11.37 estimated vs ~15.4 real |
| DeepSeek-V2-Lite | 0.95 GiB | 1.50 GiB | +0.55 GiB | Over-warns slightly |

On a 16 GB machine the 14B fails the gate anyway - it needs 13.37 GiB available, which leaves only ~2.6 GiB for Windows - so the user is warned. The gap becomes dangerous on larger machines: a 24 GB machine has roughly 20 GiB available, so the gate passes at 11.37 even though the real requirement is ~15.4 GiB plus Windows. That is the scenario Phase 4 must not ship. If the 14B is ever offered, the memory model needs an architecture-aware KV term (or a hard per-model context cap), not a bigger blanket allowance.

### 4.5 Context guidance per candidate

| Model | Supported by publisher | Nexora should expose | Reason |
|---|---|---|---|
| Qwen2.5-Coder-7B | 32,768 for GGUF (131,072 only via vLLM/YaRN) | 2,048-32,768 | [S2]; Nexora passes no rope scaling |
| Qwen2.5-Coder-14B | 32,768 (`max_position_embeddings`) | 2,048-16,384 on 32 GB; not offered on 16 GB | [S4]; KV cost at 32K is 6 GiB |
| DeepSeek-V2-Lite | 128K claimed, YaRN factor 40 from a 4,096 base | 2,048-32,768 **if ever adopted** | [S7]; long-context behaviour in b11714 is [UNVERIFIED] |

Note that Nexora's settings permit 131,072 for any model [R2]. A per-model context ceiling is a Phase 4 integration decision, deliberately not implemented here.

---

## 5. Comparison matrix

Weights follow the priority order given for this phase. Scores are 1-5, where 5 is best. **The scores are judgements, not measurements**; the evidence column is what makes them reviewable.

| # | Criterion (weight) | 7B | 14B | DS-Lite | Evidence basis |
|---|---|---:|---:|---:|---|
| 1 | Licensing clarity (20) | 5 | 5 | 2 | Stock Apache-2.0 with a shipped licence file vs a custom use-restricted licence with PRC jurisdiction and a broken licence link (section 7) |
| 2 | Reliable local inference (18) | 5 | 5 | 3 | Publisher-built GGUF and publisher instructions vs community imatrix quant; all three architectures supported by llama.cpp [S11] |
| 3 | 16 GB Windows practicality (18) | 5 | 1 | 2 | Section 4.3 |
| 4 | Coding quality (14) | 3 | 4 | 2 | Table 16 / 17 / 19 [S6] |
| 5 | Structured-output / patch adherence (10) | 3 | 3 | 2 | Aider and IFEval proxies; all unmeasured against Nexora's schema |
| 6 | Context capacity (6) | 3 | 2 | 3 | 32K documented cap for both Qwen GGUFs; DS claims 128K but its runtime path is unverified |
| 7 | CPU usability / startup (6) | 5 | 2 | 4 | Section 3, [EST] only |
| 8 | Size / distribution (4) | 5 | 3 | 2 | 4.36 / 8.37 / 9.65 GiB |
| 9 | Stable reproducible revisions (4) | 5 | 5 | 3 | Publisher repos with commit SHAs vs a third-party quant with no official GGUF upstream |
| | **Weighted score (out of 5)** | **4.40** | **3.50** | **2.22** | |

Sensitivity checks:

- Give the 14B a 32 GB machine (criterion 3 scored 5 instead of 1): 4.22. It still trails the 7B on the weighted model because of CPU speed, size and context ceiling.
- Remove licensing entirely (weight 0, renormalise): 7B 4.25, 14B 3.13, DS 2.28. The ranking does not change.
- Raise structured-output weight to 25 (taking 15 points from licensing): 7B 4.10, 14B 3.20, DS 2.22. Ranking unchanged. The decision is not an artefact of the weights.

The honest reading: the 7B is not the best model. It is the best model **that fits the product's stated floor**, and the gap to the 14B on the metric that matters most (Aider) is 3 points, not 30.

---

## 6. Recommendation

### 6.1 Primary

**Qwen2.5-Coder-7B-Instruct, official GGUF, `q4_k_m`, revision `13fb94bfda8c8cf22497dc57b78f391a9acb426a`, context 32,768.**

- 4,683,073,536 bytes; Sha-256 to be recorded at download (section 9.2).
- Apache-2.0, publisher-built, single file, `qwen2` architecture, ChatML template.
- ~7.1 GiB realistic footprint at 32K on a 16 GB machine.
- Publisher-reported HumanEval+ 84.1 / Aider 55.6 - the strongest evidence available for a model in this envelope.

### 6.2 Fallback

**Qwen2.5-Coder-14B-Instruct, `q4_k_m`, revision `d0a692ef765eefbf2fabb130b3cb2e8917e3d225`, context 8,192-16,384, 32 GB RAM or more.**

Offer only if the Windows harness shows the 7B failing the patch-contract gate, or if the product's minimum hardware rises. Do not offer it at 32K on a 16 GB machine.

### 6.3 Rejected

**DeepSeek-Coder-V2-Lite-Instruct**, for the combination of licence friction, absence of a publisher GGUF, 9.65 GiB download, weak template structure, and the lowest Aider/IFEval scores. It could be reconsidered if: counsel accepts the DeepSeek licence for redistribution; a publisher-signed GGUF appears; and a Windows test shows it beating the 7B on Nexora's own contract.

### 6.4 What would change the recommendation

| Trigger | Consequence |
|---|---|
| 7B first-pass valid JSON < 80% on the section 9 harness | Evaluate the 14B on a 32 GB reference machine before any integration |
| 7B decode < 3 tokens/s, or 16K TTFT > 300 s | Re-open the candidate list; consider 3B or a smaller quant |
| A publisher GGUF appears for a newer small Qwen/DeepSeek model with a permissive licence | Re-run this evaluation; nothing here is irreversible |
| Legal review rejects Apache-2.0 obligations (unlikely) | No candidate survives; revisit the product constraint |
| Product minimum hardware moves to 32 GB | The 14B becomes the primary on quality grounds |

### 6.5 What must be true before approval

This phase recommends a candidate; it does not approve one. Approval requires the Windows evidence in section 9, plus resolution (or explicit acceptance) of the Phase 2 blockers in section 10. **No model should be wired into the product on the strength of this document alone.**

---

## 7. Licence and redistribution review

This is an engineering summary of the published terms, not legal advice. Obtain counsel before Nexora ships weights, hosts a mirror, or automates downloads.

| Question | Qwen2.5-Coder-7B / 14B | DeepSeek-Coder-V2-Lite |
|---|---|---|
| Licence | Apache License 2.0 [S2][S5] | DeepSeek License Agreement v1.0, 23 Oct 2023 [S8] |
| OSI-approved | Yes | No |
| Commercial use | Permitted, unrestricted | Permitted, subject to use restrictions [S7][S8] |
| Field-of-use limits | None | Attachment A: military, minors, disinformation, PII, defamation, automated legal decisions, discrimination [S8] |
| Redistribution | Include licence; retain notices; mark modified files; include `NOTICE` if present (none observed) | Include licence; **embed Attachment A as enforceable downstream terms**; notify recipients; mark modified files; retain notices [S8] |
| Attribution | Retain copyright/attribution notices | Retain notices; no trademark licence [S8] |
| Patent grant | Yes (Apache-2.0 section 3) | Yes, with litigation termination [S8] |
| Trademark | No licence to Qwen marks (Apache-2.0 section 6) | No licence to DeepSeek marks [S8] |
| Governing law | Not specified by Apache-2.0 | PRC law; Hangzhou courts [S8] |
| Remote restriction | None | Licensor reserves the right to restrict usage remotely [S8] |
| Broken/missing licence link | No | Yes - card points to a `LICENSE` file that does not exist [S7] |

**Bundling vs downloading.** Today Nexora neither bundles nor downloads weights; the user supplies a GGUF path [R1]. If that changes:

- *Bundling* makes Nexora a distributor. Apache-2.0 needs the licence text and notices in the installer; the DeepSeek licence additionally needs the use restrictions in the EULA and notice to every recipient.
- *Downloading on the user's behalf* does not necessarily remove Nexora from the distribution chain if Nexora hosts or mirrors the file. A downloader that points at Hugging Face and verifies a hash is the lower-risk design, but that is a Phase 4+ decision.
- *Quantising in-house* would create a derivative; Apache-2.0 permits it with change notices, the DeepSeek licence permits it with the same restrictions carried forward.

**Runtime licence.** llama.cpp is MIT ("Copyright (c) 2023-2026 The ggml authors") [S12]. Nexora requires a user-supplied runtime and does not redistribute it, so no notice obligation is triggered today. If a runtime is ever bundled, the MIT notice must ship with it.

**Open questions for counsel / product:**

1. Does the product's EULA need to reproduce Apache-2.0 in full, or is a bundled `LICENSES/` directory sufficient?
2. If Nexora later offers a download button, is it a distributor?
3. Is the DeepSeek licence acceptable at all for a commercial desktop product, given the PRC jurisdiction clause?
4. Does the Qwen trademark need to appear on a model-selection screen, and in what form?

---

## 8. Unverified assumptions

Recorded so that no later phase silently promotes them to facts.

| ID | Assumption | Why it is unverified | How section 9 resolves it |
|---|---|---|---|
| U-1 | A 16 GB Windows machine has 9-12.5 GiB available | Depends on OS build, startup apps, browser state | M-1 measures it |
| U-2 | 7B decode 4-9 tok/s, 14B 2-4, DS 5-12 | Bandwidth estimate, no measurement | M-5 |
| U-3 | Load-to-healthy in single-digit seconds (7B) | Depends on SSD, Defender, first run | M-2 |
| U-4 | Patch-contract adherence for all three | No public benchmark of this schema | M-6 |
| U-5 | 32K context works correctly with b11714 for the Qwen GGUFs | Publisher says 32K; runtime behaviour at the edge not tested | M-4 plus a needle test |
| U-6 | Context above 32K is degraded without rope flags | Metadata says 131,072; publisher says vLLM-only | Out of scope; do not expose |
| U-7 | DeepSeek GGUF template and YaRN behave correctly in b11714 | Third-party conversion; no publisher test | Not planned - candidate rejected |
| U-8 | mmap behaviour under Windows Defender does not cause stalls | Platform-specific | M-8 |
| U-9 | The +1.0 GiB runtime overhead is adequate | Rule of thumb | M-3 compares measured peak to predicted |
| U-10 | q4_k_m format compliance is close to the unquantised model | No per-quant benchmark | M-6 at q4_k_m; optionally compare q8_0 |
| U-11 | The 7B file's SHA-256 matches the pinned revision | File digest was not captured in this phase; the repository commit pins the LFS pointer | M-0 records the hash |
| U-12 | Paths with spaces / non-ASCII / long paths work on Windows | Not exercised | M-9 |
| U-13 | The model validates in Nexora's GGUF inspector | cropped or split files could fail the 16 MiB / magic / version checks | M-0 |

---

## 9. Reproducible Windows validation plan

**Not executed in this phase.** This is the plan a later phase should run, on a real Windows machine, before any integration.

### 9.1 Preconditions

- Windows 11 x64, 16 GB RAM (primary target), NVMe with at least 20 GB free.
- A second machine with 32 GB RAM if the 14B fallback is to be judged.
- llama.cpp build `b11714` (the pinned build) as a Windows x64 CPU binary, or built from the corresponding commit.
- Nexora built from this branch, with the Phase 2 fixes, once the Phase 2 blockers are cleared.
- A test workspace containing: a small TypeScript file (<200 lines), a medium Rust file (~500 lines), a large TypeScript file (~1,500 lines), a file with a space in its name, a file with a non-ASCII name, and a path near the Windows 260-character limit.

### 9.2 M-0: artefact integrity

```powershell
# Download the single file from the pinned revision, then:
(Get-Item .\qwen2.5-coder-7b-instruct-q4_k_m.gguf).Length    # expect 4683073536
Get-FileHash .\qwen2.5-coder-7b-instruct-q4_k_m.gguf -Algorithm SHA256
```

- Record the SHA-256 in the test log. Confirm it matches the value shown on the file page for revision `13fb94bf...`.
- Open the model in Nexora's setup screen: it must pass the `.gguf`, >=16 MiB, magic and version 2-3 checks without a warning at 32,768 context [R5].
- **Reject** split files, partial downloads and any file whose size differs from the table in 3.1.3.

### 9.3 M-1: baseline

Record CPU model, physical RAM, Windows build, free disk, and available RAM with the test applications open. Use Task Manager and `systeminfo`. Capture a 60-second idle sample before loading.

### 9.4 M-2 to M-5, M-8, M-9: runtime behaviour

Run the runtime manually first, outside Nexora, to isolate runtime problems from application problems:

```powershell
.\llama-server.exe --model .\qwen2.5-coder-7b-instruct-q4_k_m.gguf `
  --host 127.0.0.1 --port 18080 --ctx-size 32768 --parallel 1 --no-ui --jinja --timeout 600
```

Then, through Nexora's own supervisor (so the exact argv in 2.2 is exercised):

| ID | Measurement | Method | Acceptance gate |
|---|---|---|---|
| M-2 | Spawn to `/health` | Timestamp spawn and first 200 from `/health`; sample every 250 ms | <= 60 s |
| M-3 | Peak memory | Sample `(Get-Process llama-server).WorkingSet64` and `PrivateMemorySize64` at 1 Hz for the whole run | Peak <= 85% of physical RAM; predicted 7.1 GiB +/- 1.0 GiB |
| M-4 | Time to first token | Prompts of ~512, ~4,096, ~16,384 and ~32,768 tokens; record wall clock to first SSE delta | 16K TTFT <= 300 s; 32K <= 600 s or documented as unsupported |
| M-5 | Decode rate | 256 generated tokens after warm-up; tokens/second excluding the first | >= 3 tok/s at 32K |
| M-8 | System pressure | Free RAM, disk queue, UI responsiveness during M-4 | No sustained paging; desktop remains usable |
| M-9 | Path handling | Files with spaces, non-ASCII names, near-260-character paths | All load and patch correctly |

### 9.5 M-6: patch-contract harness

Thirty prompts, fixed and version-controlled **outside** the product, each asking for a single-file change against one of the fixtures:

- 10 small files (one-function edits),
- 10 medium files (multi-function edits),
- 10 large files (localized edits in a big file).

For each run record, without editing the product:

1. raw model output (before Nexora's extraction);
2. whether `serde_json` parsed it as `PatchGenerationResponseStrict`;
3. whether `validate()` accepted it (version, exact `targetPath`, non-empty content, size limits, changed content) [R4];
4. the number of diffs and unchanged-line ratio versus the original;
5. whether the file compiles/type-checks after applying the patch.

Report: first-pass valid rate, post-extraction valid rate, failure taxonomy (fenced block, trailing prose, unknown key, truncated content, wrong path, unchanged content), and median diff size. **A single impressive sample is not evidence; the rate is.**

### 9.6 M-7: context knee

Repeat M-4/M-5 at 8,192 and 16,384. Record the point where TTFT or memory becomes unacceptable on 16 GB. This produces the per-model context ceiling that Phase 4 needs, instead of the raw settings maximum.

### 9.7 Evidence to keep

- hardware table and Windows build;
- exact command lines and runtime build string (`llama-server --version` writes to stderr);
- SHA-256 and byte size of the model file;
- CSV of every measurement sample (timestamp, RSS, private bytes, tokens, TTFT);
- 30 raw model outputs and their parsed results;
- a screenshot or log of the Nexora warnings shown at each context setting.

### 9.8 Abort and rollback

Stop the run if: the runtime exits non-zero, `/health` never becomes ready within 180 s, the model file fails validation, Windows reports memory pressure, or a single generation exceeds the 600 s first-token budget. Roll back by stopping the supervisor and restoring the previous `inference-settings.json`; the test must use a separate configuration directory or a backed-up settings file so the user's configured model path is never overwritten.

### 9.9 Explicit non-goals of the test

- No download in CI and no weights in the repository.
- No synthetic GGUF and no substituted model as "evidence".
- No inference, memory or latency claim recorded from a run that did not complete.
- No prompt tuning that hides a format failure; prompt changes are logged separately from the model result.

---

## 10. Outstanding Phase 2 blockers (unchanged)

Phase 3 did nothing to resolve these, and nothing here should be read as doing so. Quoted from `PHASE_2_BUILD_RECOVERY.md` section 8.

| ID | Blocker | Status |
|---|---|---|
| B-1 | No Rust type-check or build: crates.io index and static host TLS-blocked; mirrors also blocked | Open |
| B-2 | 171 of 176 Rust tests unrun | Open |
| B-3 | Windows compile/link/tests unrun | Open |
| B-4 | Clippy unverified (component unavailable) | Open |
| B-5 | No `Cargo.lock` has ever existed in the repository | Open |
| B-6 | rustfmt version skew: formatted with 1.88.0, CI pins 1.93.0 | Open |

Phase 2 verdict remains: **not fully verified / blocked.** Formatting and frontend checks pass (724/724 frontend tests, typecheck, build); the standalone SSE test passes (5/5); full Cargo compilation, the complete Rust suite, clippy and the Windows build are unverified.

Consequence for this phase: **a model cannot be end-to-end validated inside Nexora until B-1 and B-3 are closed**, because "does the app load and stream from this model" is a Cargo-and-Windows question. The section 9 harness is written so that the runtime-only measurements can start first, and the in-app measurements can follow.

---

## 11. What this phase did not do

- No model registry, selector, ID, profile, default or seed configuration.
- No download, no bundled weights, no mirror, no quantisation performed here.
- No change to `inference-settings.json`, to any model path, or to runtime flags.
- No model process started.
- No UI, quota, telemetry, export, ZIP, Download Mode, account or payment work.
- No Sonnet/Opus concept, no roadmap change.
- No commit, push, merge, publish or release.
- No edit to `PHASE_1_AUDIT.md` or to sections 0-8 of `PHASE_2_BUILD_RECOVERY.md`.

**Integration has not started. Phase 4 has not started.**

---

## 12. Sources

All accessed 2026-10-20 unless noted.

| ID | Source | What it supports |
|---|---|---|
| S1 | https://huggingface.co/Qwen/Qwen2.5-Coder-7B-Instruct (card + `config.json` + API metadata, revision `c03e6d35...`) | 7B parameters, layers, heads, context, licence, ChatML template |
| S2 | https://huggingface.co/Qwen/Qwen2.5-Coder-7B-Instruct-GGUF (card, `LICENSE`, API metadata, revision `13fb94bf...`) | GGUF availability, 32,768 GGUF context statement, licence file, template, file list |
| S3 | https://huggingface.co/api/models/Qwen/Qwen2.5-Coder-7B-Instruct-GGUF/tree/main | Exact byte size of every quantisation |
| S4 | https://huggingface.co/Qwen/Qwen2.5-Coder-14B-Instruct (card + `config.json`, revision `aedcc2d4...`) | 14B parameters, 48 layers, 8 KV heads, `max_position_embeddings` 32,768 |
| S5 | https://huggingface.co/Qwen/Qwen2.5-Coder-14B-Instruct-GGUF (card, API metadata, revision `d0a692ef...`) | 14B GGUF sizes and licence |
| S6 | https://arxiv.org/abs/2409.12186 - Qwen2.5-Coder Technical Report v3 (12 Nov 2024) | Tables 16, 17, 19, 20; sections 6.6, 7.1, 7.3 |
| S7 | https://huggingface.co/deepseek-ai/DeepSeek-Coder-V2-Lite-Instruct (card, `config.json`, `tokenizer_config.json`, revision `e434a23f...`) | Parameters, MoE shape, MLA dims, template, licence tag |
| S8 | https://github.com/deepseek-ai/DeepSeek-Coder-V2 (`LICENSE-MODEL`, `README.md`) | DeepSeek License Agreement v1.0 text, use restrictions, commercial-use statement |
| S9 | https://huggingface.co/bartowski/DeepSeek-Coder-V2-Lite-Instruct-GGUF (API metadata, revision `8f248fa2...`) | Community quant sizes, `deepseek2` architecture, template in GGUF |
| S10 | https://github.com/ggml-org/llama.cpp/blob/master/docs/function-calling.md | Template classification ("Generic" for DeepSeek-Coder-V2-Lite-Instruct; native Qwen 2.5 Coder) |
| S11 | https://github.com/ggml-org/llama.cpp/blob/master/src/llama-arch.cpp | `qwen2` and `deepseek2` architecture registrations |
| S12 | https://github.com/ggml-org/llama.cpp/blob/master/LICENSE | MIT licence text |
| S13 | https://huggingface.co/api/models?author=deepseek-ai&search=GGUF | Empty result: no official DeepSeek GGUF |
| S14 | https://github.com/QwenLM/Qwen2.5-Coder (`README.md` now serving Qwen3-Coder content) | Source-stability note; Qwen3-Coder 30B-A3B / 80B-A3B sizing |

Repository references (this checkout, HEAD `9bd4f7ad...`):

| ID | File | Used for |
|---|---|---|
| R1 | `apps/desktop/src-tauri/src/inference/process.rs` | Exact llama-server argv, `PINNED_BUILD`, timeouts |
| R2 | `apps/desktop/src-tauri/src/inference/settings.rs` | Context default/min/max |
| R3 | `apps/desktop/src-tauri/src/inference/client.rs` | Request body; no grammar/`response_format`; system prompt |
| R4 | `apps/desktop/src-tauri/src/workspace/patch_contract.rs` | Strict JSON contract, `deny_unknown_fields`, extraction tolerance |
| R5 | `apps/desktop/src-tauri/src/inference/models.rs` | GGUF validation, memory heuristic and its stated limits |

---

## Appendix A - fact / estimate / unverified register

| Claim | Class | Basis |
|---|---|---|
| 7B q4_k_m is 4,683,073,536 bytes | FACT | [S3] |
| 14B q4_k_m is 8,988,110,272 bytes | FACT | [S5] |
| DS-Lite Q4_K_M is 10,364,416,768 bytes | FACT | [S9] |
| KV costs 56 / 192 / 30.4 KiB per token | DERIVED | layer and head counts, f16 |
| 7B needs ~7.1 GiB at 32K | DERIVED + EST | weights + KV + 1.0 GiB overhead |
| 14B needs ~15.4 GiB at 32K | DERIVED + EST | same |
| 7B decode 4-9 tok/s on a 16 GB laptop | EST | bandwidth reasoning, no test |
| Patch-contract pass rate for any candidate | UNVERIFIED | no benchmark, no test |
| >32K context works with the Qwen GGUFs | UNVERIFIED | publisher says vLLM-only for YaRN |
| A 16 GB machine has 9-12.5 GiB available | UNVERIFIED | configuration-dependent |
| The DeepSeek licence permits commercial use | FACT (publisher statement) | [S7][S8]; not legal advice |
| llama.cpp supports all three candidates' architectures (`qwen2`, `deepseek2`) | FACT | [S11] |

## Appendix B - the exact output Nexora expects

```json
{"version":1,"targetPath":"src/lib/example.ts","proposedContent":"<entire file contents, JSON-escaped>","explanation":"optional, <=1000 chars"}
```

Rules enforced by `PatchGenerationResponseStrict::validate` [R4]:

- `deny_unknown_fields`: any extra key fails;
- `version` must be exactly `1`;
- `targetPath` must equal the requested relative path exactly, must not be absolute, must not traverse, must not be a sensitive file;
- `proposedContent` is full replacement content, non-empty, different from the original, no NUL, <= 200,000 characters and <= `MAX_PREVIEW_BYTES`;
- `explanation` is optional and trimmed.

Why this matters for model choice: a model that is excellent at writing unified diffs can still fail Nexora completely. Aider's "whole edit format" score (Table 19) is therefore a better proxy than HumanEval, and it is the reason the matrix weights structured output at all.

## Appendix C - candidate artefact inventory

| Candidate | Repository | Revision | File | Bytes |
|---|---|---|---|---|
| Primary | `Qwen/Qwen2.5-Coder-7B-Instruct-GGUF` | `13fb94bfda8c8cf22497dc57b78f391a9acb426a` | `qwen2.5-coder-7b-instruct-q4_k_m.gguf` | 4,683,073,536 |
| Fallback | `Qwen/Qwen2.5-Coder-14B-Instruct-GGUF` | `d0a692ef765eefbf2fabb130b3cb2e8917e3d225` | `qwen2.5-coder-14b-instruct-q4_k_m.gguf` | 8,988,110,272 |
| Rejected | `bartowski/DeepSeek-Coder-V2-Lite-Instruct-GGUF` | `8f248fa2072348f77a8bc37754e470de1f61866e` | `DeepSeek-Coder-V2-Lite-Instruct-Q4_K_M.gguf` | 10,364,416,768 |

None of these files was downloaded. The byte counts come from the Hugging Face tree API, not from a local file.

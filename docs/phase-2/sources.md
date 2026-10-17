# Sources

**Access date for every entry: 2026-10-17.**
Retrieval used direct repository APIs and model-card metadata rather than marketing pages wherever possible. Where a source is secondary or vendor-claimed, the row says so.

---

## 1. Method

| Technique | Endpoint / form | Why |
|---|---|---|
| Release and asset inspection | `api.github.com/repos/<org>/<repo>/releases` and `/releases/tags/<tag>` | Proves which binaries actually exist, rather than which are documented |
| Server contract | Raw `tools/server/README.md` from the default branch | The flag and endpoint reference is the contract |
| Issue evidence | `api.github.com/search/issues?q=repo:…+is:issue+…` | Finds open defects that documentation does not mention |
| Model metadata | `huggingface.co/api/models/<org>/<repo>` | Licence tag, revision SHA, config, architecture |
| File sizes and hashes | `huggingface.co/api/models/<org>/<repo>/tree/main?recursive=true` | Exact byte sizes and `lfs.oid` hashes for GGUF files |
| Build geometry | `huggingface.co/<org>/<repo>/raw/main/config.json` | Layer counts, head counts, head dimensions for KV arithmetic |
| Platform support | Official installation documentation | Establishes which operating systems are supported |

Hugging Face API responses embed the full chat template, which makes the payloads large. Only metadata fields were used; templates were noted where they affect behaviour (for example, Devstral's embedded persona) but are not reproduced here.

## 2. Primary sources

| ID | Source | URL | Type | Supports |
|---|---|---|---|---|
| **S-01** | llama.cpp repository and licence | https://github.com/ggml-org/llama.cpp | Primary | MIT licence; project scope; default branch |
| **S-02** | llama.cpp releases: `v0.7.0`, `b11714`, release notes | https://github.com/ggml-org/llama.cpp/releases · https://github.com/ggml-org/llama.cpp/releases/tag/v0.7.0 · https://github.com/ggml-org/llama.cpp/releases/tag/b11714 | Primary | No binaries on `v0.7.0`; Windows assets on `b11714`; default port 9931; chat-layer refactor |
| **S-03** | llama-server reference | https://github.com/ggml-org/llama.cpp/blob/master/tools/server/README.md | Primary | Flags, endpoints, `/health` public, SSE, slots, JSON schema, GBNF, Jinja tools, CORS, auth, timeouts |
| **S-04** | llama.cpp build documentation | https://github.com/ggml-org/llama.cpp/blob/master/docs/build.md | Primary | Windows MSVC/CMake builds; CUDA, Vulkan, HIP, SYCL; unified memory is Linux-only; `HSA_OVERRIDE` unsupported on Windows |
| **S-05** | Issue #29498 | https://github.com/ggml-org/llama.cpp/issues/29498 | Primary | `X-Conversation-Id` disables disconnect-cancellation and accumulates sessions (memory DoS) |
| **S-06** | Issue #27388 | https://github.com/ggml-org/llama.cpp/issues/27388 | Primary | Server wedge; `/health` OK while `/slots` hangs; needs SIGKILL |
| **S-07** | Issue #27604 | https://github.com/ggml-org/llama.cpp/issues/27604 | Primary | Vulkan server-wide hang after client aborts a streaming request |
| **S-08** | Issue #19173 | https://github.com/ggml-org/llama.cpp/issues/19173 | Primary | Responses API cannot cancel a stream |
| **S-09** | Issue #27733 | https://github.com/ggml-org/llama.cpp/issues/27733 | Primary | PEG parse failure on a trailing `<think>` tail discards a completed Qwen3.8 generation |
| **S-10** | Ollama repository and `v0.40.6` release | https://github.com/ollama/ollama · https://github.com/ollama/ollama/releases/tag/v0.40.6 | Primary | MIT; Windows zip/installer/checksums; release date |
| **S-11** | vLLM installation documentation | https://docs.vllm.ai/en/latest/getting_started/installation/index.html | Primary | Supported platforms; Windows not listed |
| **S-12** | ONNX Runtime GenAI | https://github.com/microsoft/onnxruntime-genai | Primary | MIT; Windows; CPU/CUDA/DirectML/TensorRT-RTX/OpenVINO/QNN/WebGPU |
| **S-13** | mistral.rs | https://github.com/EricLBuehler/mistral.rs | Primary | MIT; Rust-native; activity and scale |
| **S-14** | Qwen3.8-27B model card and config | https://huggingface.co/Qwen/Qwen3.8-27B · https://huggingface.co/Qwen/Qwen3.8-27B/blob/main/config.json | Primary | Apache-2.0; 27.78B; 64 layers; 48 linear + 16 full attention; 4 KV heads, head_dim 256; 262,144 context; revision `1d4bf0f2` |
| **S-15** | Qwen3-Coder-30B-A3B-Instruct | https://huggingface.co/Qwen/Qwen3-Coder-30B-A3B-Instruct | Primary | Apache-2.0; 128 experts/8 active; 48 layers; 262,144 context; revision `b2cff646` |
| **S-16** | Qwen3-Coder-Next card and technical report | https://huggingface.co/Qwen/Qwen3-Coder-Next · https://arxiv.org/html/2603.00729v1 | Primary + secondary | Apache-2.0; 512 experts/10 active; 80B-A3B; 70.6–71.3 SWE-bench Verified across three scaffolds |
| **S-17** | Devstral Small 2 24B | https://huggingface.co/mistralai/Devstral-Small-2-24B-Instruct-2512 | Primary | Apache-2.0; 40 layers; 32 Q / 8 KV; 393,216 max positions; embedded Mistral Vibe system prompt; revision `55c5b41e` |
| **S-18** | gpt-oss-20b | https://huggingface.co/openai/gpt-oss-20b | Primary | Apache-2.0; 32 experts/4 active; 24 layers; 131,072 context; MXFP4; revision `6cee5e81` |
| **S-19** | Mellum2.1 model card, blog, config | https://huggingface.co/JetBrains/Mellum2.1-12B-A2.5B-Thinking · https://blog.jetbrains.com/ai/2026/10/mellum2-1-gets-to-work-a-fast-open-model-for-coding-agents/ · https://huggingface.co/JetBrains/Mellum2.1-12B-A2.5B-Thinking/blob/main/config.json | Primary | Apache-2.0; 12B/2.5B active; 64 experts/8 active; 28 layers; 131,072 context; sliding window 1024 |
| **S-20** | Gemma 4 31B card, config, launch blog | https://huggingface.co/google/gemma-4-31B-it · https://huggingface.co/google/gemma-4-31B-it/blob/main/config.json · https://blog.google/innovation-and-ai/technology/developers-tools/gemma-4/ | Primary | Apache-2.0 (correction C2-01); 60 layers; 10 global + 50 sliding; 262,144 context; revision `842da379` |
| **S-21** | GLM-4.7-Flash | https://huggingface.co/zai-org/GLM-4.7-Flash | Primary | MIT; `glm4_moe_lite`; 4 experts active; 200K claimed; revision `7dd20894` |
| **S-22** | Nemotron 3.5 Bolt 74B-A9B 24GB | https://huggingface.co/nvidia/NVIDIA-Nemotron-3.5-Bolt-74B-A9B-24GB | Primary | OpenMDW-1.1; 74B/9B active; IQ2; 256K context; 24 GB target |
| **S-23** | Qwen2.5-Coder-7B-Instruct | https://huggingface.co/Qwen/Qwen2.5-Coder-7B-Instruct | Primary | Apache-2.0; 7.62B; revision `c03e6d35` |
| **S-24** | Qwen3.8-27B GGUF tree | https://huggingface.co/unsloth/Qwen3.8-27B-GGUF/tree/main | Primary | Exact quant sizes and hashes; UD-Q4_K_M = 16,464,440,224 B |
| **S-25** | Qwen3-Coder-30B-A3B GGUF tree | https://huggingface.co/unsloth/Qwen3-Coder-30B-A3B-Instruct-GGUF/tree/main | Primary | Q4_K_M = 18,556,689,568 B; Q3_K_M = 14,711,850,144 B |
| **S-26** | Devstral Small 2 GGUF tree | https://huggingface.co/unsloth/Devstral-Small-2-24B-Instruct-2512-GGUF/tree/main | Primary | Q4_K_M = 14,334,446,752 B; Q3_K_M = 11,474,619,552 B |
| **S-27** | gpt-oss-20b GGUF tree | https://huggingface.co/ggml-org/gpt-oss-20b-GGUF/tree/main | Primary | MXFP4 = 12,109,566,624 B; optional Eagle3 draft |
| **S-28** | Mellum2.1 GGUF tree | https://huggingface.co/JetBrains/Mellum2.1-12B-A2.5B-Thinking-GGUF/tree/main | Primary | Q4_K_M = 8,071,295,264 B; MXFP4_MOE = 7,029,537,056 B |
| **S-29** | Gemma 4 31B GGUF tree | https://huggingface.co/unsloth/gemma-4-31B-it-GGUF/tree/main | Primary | Q4_K_M = 18,323,733,440 B; Q3_K_M = 14,736,608,192 B |
| **S-30** | Nemotron Bolt GGUF tree | https://huggingface.co/nvidia/NVIDIA-Nemotron-3.5-Bolt-74B-A9B-24GB-GGUF/tree/main | Primary | Body 22,018,326,528 B + mmproj 1,733,692,736 B |
| **S-31** | Windows 10 end of support | https://support.microsoft.com/en-us/windows/deployment/updates-lifecycle/windows-10-support-has-ended-on-october-14-2025 | Primary | General support ended 2025-10-14; ESU dates |
| **S-32** | Windows 11 specifications | https://www.microsoft.com/en-us/windows/windows-11-specifications | Primary | TPM 2.0, UEFI, 4 GB RAM, 64 GB storage, DX12/WDDM 2.0 |
| **S-33** | Qwen3-Coder family README | https://github.com/QwenLM/Qwen3-Coder | Primary | Family scope; 256K/1M; 358 languages; tool-call parser |
| **S-34** | Qwen3.8 release notes | https://github.com/QwenLM/Qwen3.8 | Primary | 27B released 2026-08-14 under Apache-2.0; 2.4T under a custom licence |
| **S-35** | Issue #22072 | https://github.com/ggml-org/llama.cpp/issues/22072 | Primary (community) | Intermittent malformed/incomplete JSON tool-call arguments; large tool grammars exceed repetition limits |

## 3. Secondary sources (claims, not evidence)

| ID | Source | URL | Used for | Why secondary |
|---|---|---|---|---|
| **S-36** | Qwen3.8 benchmark table | https://openlm.ai/qwen3.8/ | Qwen3.8-27B coding scores (61.7 SWE-bench Pro, 73.0 Terminal-Bench 2.1) | Aggregated vendor table, not an independent run |
| **S-37** | MarkTechPost, Mellum2.1 | https://www.marktechpost.com/2026/10/08/jetbrains-releases-mellum2-1-a-12b-moe-open-model-for-coding-agents/ | Mellum 47.0 SWE-bench Verified; harness and context | Third-party reporting of vendor results |
| **S-38** | Devstral 68.0 SWE-bench roundup | https://localaimaster.com/models/best-ai-coding-models | Devstral Small 2 score and 15 GB download | Third-party aggregation |
| **S-39** | GLM-4.7-Flash 59.2 SWE-bench | https://rits.shanghai.nyu.edu/ai/glm-4-7-flash-z-ais-efficient-30b-moe-model-for-coding-and-agents/ | GLM score and release date | University blog summarising vendor claims |
| **S-40** | Qwen3.8 overview and release dates | https://codersera.com/blog/qwen-3-5-complete-guide-2026/ | Release timeline; Q4 size; licence split between 27B and 2.4T | Secondary guide |

Where a secondary source conflicts with a primary one, the primary wins. S-36…S-40 are cited in the model documents only as **[S]** and never as measured evidence.

## 4. What was not verified

| Claim | Status | Consequence |
|---|---|---|
| Minimum NVIDIA driver for the CUDA 12.4 and 13.4 builds | Not read from NVIDIA's compatibility matrix | `Q2-08`; installer cannot enforce a driver floor yet |
| llama.cpp support for `qwen3_5`, `mellum`, `gemma4`, `nemotron_h_moe` architectures at build `b11714` | Not load-tested | Phase 3 smoke test must load each model |
| Actual VRAM/RAM usage of any quant | Not measured | All figures are estimates |
| Decode/prefill rates on Windows | Not measured | No performance claim can be made |
| Tool-call validity for any model | Not measured | ≥98% gate unproven |
| OpenMDW-1.1 full text | Not read | Legal review required |
| CUDA runtime redistribution terms | Not read in full | Legal review required |
| Vulkan loader licence | Not read | Verify before bundling |
| LM Studio current terms | Not read | Excluded on Phase 1 grounds; no new claim |
| Mistral hosted API deprecation date | Not re-checked | Carried from Phase 1 as **[U]** |
| NVIDIA Bolt 24 GB real-world fit | Not tested | Published recipes target 28 GB+ |

## 5. Source hygiene

- Every quantitative fact in the Phase 2 set traces to a row above; every qualitative judgement is marked as such.
- No benchmark screenshot, forum post, or YouTube result was used.
- Where a vendor's own card supplies a score, it is labelled a claim, because the harness, prompt, and sampling settings are the vendor's.
- Access dates are recorded once for the set. If a document is revised after 2026-10-17, the revision must re-check the sources it relies on and update this table.

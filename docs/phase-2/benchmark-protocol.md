# Benchmark Protocol

**Status:** protocol only. No task was executed in Phase 2; no result exists.
**Purpose:** produce comparable, reproducible evidence for three questions — does the model write correct code, does it use tools correctly, and does the runtime stay fast and stable on real hardware.

This protocol is designed to be run in Phase 3 against the exact runtime build, model file, and quantisation that Nexora would ship. A result produced with a different build or quant is a different experiment.

---

## 1. Principles

| Principle | Rule |
|---|---|
| One variable at a time | A run changes exactly one of: model, quant, context, runtime build, hardware. |
| Fixed harness | The runner, prompts, seeds, and checks are versioned and hashed. |
| No self-grading | The model never decides whether it passed. Checks are executables, compilers, test suites, or schema validators. |
| Raw evidence retained | Prompt, raw output, parsed tool calls, check output, timings, and environment are stored per task. |
| Reported as distributions | Proportions carry sample size and confidence intervals; latencies carry p50/p95, not means. |
| Failed runs count | A crash, timeout, or OOM is a failure, not a discard. |
| No tuning on the test set | Prompts are frozen before the first run. Fixes go to a new protocol version. |

## 2. The ten dimensions

| # | Dimension | What it measures | Primary metric | Tasks |
|---|---|---|---|---|
| D-01 | Short code generation | Single-file functions from a specification | pass@1 on hidden tests | 12 |
| D-02 | Multi-file changes | Coordinated edits across ≥3 files with imports/types | pass@1 on build + tests | 12 |
| D-03 | Repository comprehension | Locate the cause, not just describe the repo | correct-file recall + answer accuracy | 12 |
| D-04 | Debugging / repair | Fix failing tests with a bounded attempt budget | resolved rate within 3 attempts | 12 |
| D-05 | Tool-call correctness | Valid name, valid schema, correct arguments | tool validity % | 12 |
| D-06 | Structured-output reliability | JSON conforming to a supplied schema under 1,000+ tokens | schema validity % + semantic correctness | 12 |
| D-07 | Long-context retrieval | Find and use facts at 8K/16K/32K/64K | accuracy per depth | 12 × 4 depths |
| D-08 | Instruction following | Multi-constraint instructions with negatives | constraint satisfaction % | 12 |
| D-09 | Latency / throughput | TTFT, prefill, decode, end-to-end turn time | p50/p95 | 5 repetitions × 3 prompt sizes |
| D-10 | Memory / stability | Peak VRAM/RAM, OOM, wedge, cancel, soak | failures per 100 + peaks | 100-turn soak + 20 cancels |

Total scored corpus: 96 standard tasks + 24 adversarial = 120 tasks. D-07 adds three extra depth runs per task (36 runs) without adding tasks. Runtime tasks: 15 latency runs + 1 soak + 1 cancel suite per model/tier.

## 3. Test corpus

### 3.1 Composition

| Layer | Size | Source | Visibility |
|---|---|---|---|
| Public calibration | 20 tasks | Open, permissively licensed repositories; documented in the harness | Visible to the team, never used for scoring after freeze |
| Private scored set | 76 tasks | Authored in-house, reviewed by two engineers, stored in a private repo | Hidden from model vendors; never published |
| Adversarial set | 24 tasks | Phase 1 doc 05 NCE categories: misleading instructions, destructive requests, prompt injection in files | Hidden |

Each task is a directory containing:

```
task_id/
  prompt.md            # frozen instruction, with placeholders resolved
  repo/                # snapshot or patch source (git bundle + commit SHA)
  checks/              # executable checks: pytest, vitest, tsc, custom
  expected.json        # machine-readable assertions
  meta.json            # dimension, difficulty, languages, timeout
```

### 3.2 What the scored set must cover

- TypeScript/React (Nexora's own stack), Python, Rust, Go, SQL.
- Monorepo layouts with workspace packages and path aliases.
- Tests that are wrong or stale, to test whether the model notices.
- Tasks where the correct answer is "this cannot be done without more information".
- Tasks that require reading a file the prompt does not mention.

### 3.3 Freezing

Before the first scored run: compute a SHA-256 over the corpus manifest and record it in the results file. Any change after that is a new protocol version (`v2`), and results from different versions are never compared.

## 4. Fixed runtime settings

| Setting | Value | Rationale |
|---|---|---|
| Runtime | Pinned `llama-server` build (record the tag) | Nightly builds differ; a result without the tag is not reproducible |
| Parallel slots | `--parallel 1` | Mirrors the product; removes batching variance |
| Context | 32,768 on 16/24 GB; 16,384 on 12 GB and 8 GB; 8,192 on CPU | Matches the tier default |
| Context for D-07 | 8K, 16K, 32K, 64K (skip sizes above the tier's measured ceiling and record the skip) | Retrieval must be measured where the product will run |
| KV cache | FP16 for the primary matrix; Q8_0 as a secondary sensitivity run | Quantised KV changes both memory and quality |
| Flash attention | Runtime default; recorded | Not forced |
| Batch / ubatch | Runtime defaults; recorded | Changing them changes prefill |
| `--jinja` | On (now the default) | Tool calling requires it |
| Web UI / tools / MCP | Disabled (`--no-webui`, no `--tools`, no `--mcp-*`) | Product configuration; also removes an attack surface during testing |
| Reasoning budget | 2,048 tokens for thinking models; 0 for non-thinking | Prevents a thinking model from consuming the answer budget |
| Max output | 4,096 tokens | Fixed across models |
| Temperature | 0.2 for quality runs; 0.0 as a determinism check on 10 tasks | Quality runs need a little diversity; determinism check proves the harness is stable |
| Top-p / top-k | 0.95 / 40 | Common defaults; recorded |
| Seed | 1, 2, 3 for the three quality repetitions | Reproducibility |
| Repeat penalty | 1.0 | Off; penalties distort code |
| Stop sequences | Model template defaults | Recorded |
| System prompt | Frozen per dimension, no vendor-specific identity | Prevents template-embedded product personas (see Devstral) |
| Model file | Exact SHA-256 from the manifest | A re-quantised file is a different model |
| Offload | All layers if they fit; otherwise record the layer count | Partial offload must be visible in results |

Every row is written into the results file. A missing setting invalidates the run.

## 5. Execution procedure

1. **Provision** the machine; record OS build, driver version, power plan, CPU, RAM, GPU, and free disk.
2. **Verify** the runtime archive hash and the model hash against the manifest.
3. **Cold start** the model; record load time to `/health` 200 and to the first successful 1-token completion.
4. **Warm-up**: one 512-token request; discard its timings.
5. **Quality pass**: for each task, run three repetitions at seeds 1–3. Capture raw output and check results.
6. **Tool pass**: run D-05 and D-06 with the exact tool schemas Nexora will ship (≤12 tools, flat objects, no `oneOf` at depth).
7. **Long-context pass**: D-07 at each depth the tier supports.
8. **Latency pass**: 1K, 4K, and 16K prompts; 5 runs each; report p50/p95 TTFT and decode.
9. **Stability pass**: 100-turn conversation with tool calls, 20 cancels at random points, 10 load/unload cycles, 1 forced kill and recovery.
10. **Tear down**; capture peak VRAM/RAM and any event-log entries.
11. **Validate** the results file against the schema; hash it; store it with the raw logs.

Between models: unload, wait for VRAM to return to baseline, and start a fresh process. Between tiers: reboot if VRAM accounting is suspect; record that it happened.

## 6. Scoring

### 6.1 Per-task outcomes

| Outcome | Definition |
|---|---|
| `pass` | All mandatory checks pass and no forbidden action occurred. |
| `fail` | Checks ran and failed. |
| `error` | Harness or runtime failed (crash, timeout, OOM). Counts as failure for the model, and as a separate reliability signal for the runtime. |
| `invalid` | Task itself is broken. Excluded from the denominator, and the exclusion is reported. |

No partial credit for "almost compiles". For D-06, schema validity and semantic correctness are separate binaries.

### 6.2 Per-dimension score

```
score_d = passes / (passes + fails + errors)
```

Report `pass@1` (seed 1), `pass@3` (any of 3), and the per-seed spread. A model that passes only at seed 3 is unstable, and the spread shows it.

### 6.3 Tool validity

```
tool_validity = valid_calls / total_calls
```

A call is invalid if: unknown tool name, missing required argument, wrong type, argument that fails the tool's schema, or a call emitted after the model was told the tool does not exist. Malformed JSON that the parser recovers counts as invalid, because a production parser must not guess.

### 6.4 Confidence

Proportions carry Wilson 95% intervals. With 12 tasks, a 10-point difference is not distinguishable from noise; the report must say so. Comparisons between models use a paired test on the same tasks (McNemar for binary outcomes) and report the p-value and effect size. "Model A is better" requires a paired difference whose interval excludes zero.

### 6.5 Latency

Report p50, p95, and max. Exclude the first post-load request from latency statistics but keep it in TTFT-after-load. Decode rate is computed from server timings and cross-checked against client inter-token times; if they disagree by >10%, investigate before publishing.

## 7. Result schema

```json
{
  "schema_version": "1.0",
  "run_id": "2026-11-02T10-00Z__qwen38-27b__ud-q4km__h3",
  "protocol_version": "1.0",
  "corpus_sha256": "…",
  "started_at": "2026-11-02T10:00:00Z",
  "environment": {
    "os": "Windows 11 23H2 (22631.…)", "driver": "…", "cpu": "…",
    "ram_gb": 32, "gpu": "RTX 4090", "vram_gb": 24, "power_plan": "Best performance"
  },
  "runtime": {
    "name": "llama.cpp", "build": "b11714", "archive_sha256": "…",
    "backend": "cuda-12.4", "flags": ["--parallel","1","--ctx-size","32768","--jinja","--no-webui"]
  },
  "model": {
    "repo": "unsloth/Qwen3.8-27B-GGUF", "revision": "4ca720788d1e…",
    "file": "Qwen3.8-27B-UD-Q4_K_M.gguf", "sha256": "…", "quant": "UD-Q4_K_M",
    "params_b": 27.78, "active_params_b": 27.78, "license": "Apache-2.0"
  },
  "kv": {"type": "f16", "ctx": 32768, "measured_bytes": null},
  "load": {"spawn_to_health_ms": null, "spawn_to_first_token_ms": null},
  "dimensions": {
    "D-01": {"tasks": 12, "pass": null, "fail": null, "error": null, "pass_at_1": null, "pass_at_3": null},
    "D-05": {"total_calls": null, "valid_calls": null, "tool_validity": null},
    "D-06": {"schema_valid": null, "semantically_correct": null},
    "D-09": {"ttft_ms": {"p50": null, "p95": null}, "decode_tps": {"p50": null, "p95": null}, "prefill_tps": null},
    "D-10": {"peak_vram_mb": null, "peak_ram_mb": null, "ooms": null, "wedges": null, "cancel_success": null}
  },
  "failures": [{"task_id": "…", "outcome": "error", "reason": "…"}],
  "notes": "…",
  "result_sha256": "…"
}
```

`null` means "not measured". It must never be replaced with an estimate in a results file. Estimates belong in [`hardware-matrix.md`](hardware-matrix.md).

## 8. Acceptance gates

These are Phase 1 gates (doc 08), restated so the benchmark can enforce them.

| Gate | Threshold | Applies to |
|---|---|---|
| Licence | OSI-permissive or approved in writing | Every shipped model |
| Tool validity | ≥98% | Default model per tier |
| Schema validity | ≥99% | Default model per tier |
| False success | 0 in the adversarial set | Every model |
| OOM / crash rate | <2% of runs | Every tier |
| Cancel success | ≥95% within 2 s | Every tier |
| Soak | No wedge in 100 turns | Every tier |
| Retrieval | ≥90% at the advertised default context | Default context per tier |
| Instruction following | ≥90% constraint satisfaction | Default model per tier |

A model that fails a hard gate is not shippable at that tier, regardless of its quality score. This is deliberate: a model that is 10 points better on SWE-bench but crashes 5% of the time is worse for a desktop product.

## 9. Run matrix

| Run | Models | Tiers | Purpose |
|---|---|---|---|
| **Smoke** | Default per tier (4 models) | H0–H3 | Prove the harness, catch architecture incompatibilities |
| **Core** | 8 models (shortlist + 2 alternatives) | Default tier per model | Quality comparison at the intended tier |
| **Sensitivity — quant** | Chosen model at Q3/Q4/Q5 | H3 | Quality loss vs memory |
| **Sensitivity — KV** | Chosen model, FP16 vs Q8_0 | H3 | Memory saving vs quality |
| **Sensitivity — context** | Chosen model at 8/16/32/64K | H3 | Usable context, retrieval decay |
| **Runtime cross-check** | Chosen model on llama.cpp and Ollama | H3 | Ensure conclusions are not runtime-specific |
| **Stability** | Default per tier | H0–H3 | Soak, cancel, OOM, recovery |

The full matrix is 8 models × 120 tasks × 3 seeds ≈ 2,880 quality runs plus runtime passes. At an estimated 60–180 s per agentic task, this is tens of hours of machine time per tier — which is why the smoke run comes first and the corpus is stratified (`Q2-11`).

## 10. Threats to validity

| Threat | Effect | Control |
|---|---|---|
| Vendor-tuned prompts | Inflates scores on their models | Frozen in-house system prompt; no vendor harness |
| Training-data contamination | Model has seen the task | Private corpus; tasks authored after model release where possible |
| Small sample | Wide intervals | Report intervals; do not rank within noise |
| Single hardware | Conclusions do not generalise | At least two tiers per model where feasible |
| Harness bugs | Wrong winner | Two engineers review checks; 10-task determinism check |
| Thermal drift | Late runs look slower | Randomise model order; record temperatures; reboot between heavy runs |
| Quantisation mismatch | Q3 vs Q4 compared as if same model | Quant is a first-class field; comparisons across quants are labelled sensitivity, not ranking |
| Reasoning-token budget | Thinking models truncated or advantaged | Fixed 2,048-token budget, reported |

## 11. Outputs

1. `results/<run_id>.json` — schema-validated, hashed.
2. `results/<run_id>/raw/` — per-task prompt, output, checks, timing.
3. `reports/model-scorecard.md` — generated table, not hand-written.
4. `reports/hardware-<tier>.md` — measured latency and memory, replacing the estimates in [`hardware-matrix.md`](hardware-matrix.md).
5. `reports/deviations.md` — every skipped task, excluded run, and protocol change, with the reason.

The scorecard is not a marketing document. It must include the failures, the error rate, and the confidence intervals, and it must state which tier and quant each result belongs to.

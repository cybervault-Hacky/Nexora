# Phase 2 Decisions, Risks, and Corrections

**Status:** recommendations for owner approval. None is irreversible; each carries a reversal condition.
**Inputs:** Phase 1 at `e28cb99`, primary sources listed in [`sources.md`](sources.md), access date 2026-10-17.

---

## 1. Phase 1 review

### 1.1 What was checked

| Area | Checked against | Outcome |
|---|---|---|
| Runtime choice (doc 04) | llama.cpp releases, server docs, build docs, issue tracker | Confirmed, with a packaging correction |
| Model shortlist (doc 04) | Model cards, `config.json`, licence metadata, GGUF trees | Two additions, one licence correction, one memory correction |
| Tier mapping (docs 04, 11) | KV arithmetic and file sizes | 16 GB and 24 GB mappings need tightening |
| Inference supervisor (doc 03) | Server flags and endpoint semantics | Confirmed; cancellation needs more escalation |
| Security controls (doc 07) | Server auth, CORS, health, tool flags | Confirmed and extended |
| Installer and update (doc 09) | Runtime distribution model | Correction: no binaries on semver releases |
| Roadmap (doc 12) | Phase 2 scope | Phase 2 was research-only; hardware work moved to Phase 3 |

### 1.2 Phase 1 requirements that Phase 2 had to answer

| Phase 1 item | Phase 2 answer |
|---|---|
| "Pin a release per app version" (doc 04 §5) | Pin a **build tag** and asset hash; semver tags carry no binaries |
| "Final selection at the end of Phase 2 with owner approval" (doc 04 §16) | Recommended, not selected; approval still required |
| "The Phase 2 spike must confirm the exact semantics" of cancellation (doc 04 §9) | Semantics documented; no cancel endpoint; escalation ladder defined (design), confirmation pending Phase 3 tests |
| "Phase 2: hardware validation spikes" (doc 12) | Not performed; no hardware available. Carried as a Phase 3 gate |
| Q-13…Q-17 runtime/model questions | Answered where documentary evidence exists; measurement questions remain |

## 2. Decisions

| ID | Decision | Rationale | Alternatives considered | Reversal condition |
|---|---|---|---|---|
| **P2-D-01** | Primary runtime: `llama.cpp` `llama-server`, MIT | Only candidate meeting all twelve requirements; native Windows CPU/CUDA/Vulkan; offline; schema-constrained output; Jinja tools | Ollama, ONNX GenAI, mistral.rs, vLLM | Any two of the five runtime reversal gates in [`runtime-evaluation.md`](runtime-evaluation.md) §9 |
| **P2-D-02** | Ship three runtime packs: CPU (always), CUDA 12.4 (on demand), Vulkan (on demand). Do not ship CUDA 13.4 at 1.0 | Smallest download and support matrix; CUDA 12.4 has the lower driver floor | Single CUDA 13.4 pack; CPU-only; Vulkan-only | A supported GPU population requires 13.4 |
| **P2-D-03** | Pin a `b####` build tag and per-asset SHA-256; mirror internally; never resolve "latest" | Semver tags have no binaries; rolling builds regress | Build from source per release (retained as audited fallback) | Mirror cannot be funded or hosted |
| **P2-D-04** | Ollama is an **external fallback provider**, detected, never bundled | Independent runner if the pinned build fails; avoids a second service and duplicate model store | Bundle Ollama as primary | llama.cpp fails the reversal gates and Ollama passes them |
| **P2-D-05** | Reject vLLM, TensorRT-LLM, LM Studio for 1.0; defer ONNX GenAI and mistral.rs | No native Windows path (vLLM), NVIDIA-datacenter complexity (TRT-LLM), proprietary (LM Studio), model coverage (ONNX, mistral.rs) | — | ONNX builds of the chosen models with tool templates appear |
| **P2-D-06** | Ship **text-only** models at 1.0; do not download `mmproj`; no vision input | Vision adds 0.5–2 GB, an injection channel, and no 1.0 use case | Ship multimodal Qwen3.8 | A product requirement for screenshots is approved |
| **P2-D-07** | Default models: 24 GB Qwen3.8-27B UD-Q4_K_M; 16 GB gpt-oss-20b MXFP4; 12 GB Mellum2.1 Q4_K_M; 8 GB/CPU Qwen2.5-Coder-7B Q4_K_M | Best fit-for-memory with verified licences and GGUF availability | Devstral, Gemma 4, GLM-4.7-Flash, Nemotron Bolt | Phase 3 benchmark gates fail for the default at a tier |
| **P2-D-08** | Default context: 32K on 12/16/24 GB, 16K on 8 GB and CPU. Never advertise the native context | KV arithmetic shows 128K+ is not usable on consumer cards | Advertise native context; 64K default | Measurements show a higher stable ceiling |
| **P2-D-09** | One server slot, one in-flight request, serialised in Rust | Avoids Vulkan abort hang, simplifies accounting, matches single-user product | `--parallel 2+` | Concurrent agent subtasks become a requirement **and** the runtime proves safe |
| **P2-D-10** | Cancellation ladder: drop stream → erase slot → probe → kill job and restart. Never send `X-Conversation-Id` | No cancel endpoint exists; two open upstream bugs break cancellation | Rely on disconnect only | Upstream adds a documented cancel endpoint and fixes #29498 |
| **P2-D-11** | **IPC-only**: the webview never gets the runtime port or key; Rust is the sole HTTP client; no reverse proxy | Removes browser-origin and CORS attack surface entirely; simplest correct design | Rust proxy on a second port | A non-Tauri client (CLI, plugin) needs runtime access |
| **P2-D-12** | Literal `127.0.0.1`, ephemeral port, per-session 256-bit key via `--api-key-file`, ACL, PID verification, no firewall rule | Prevents LAN exposure, key disclosure via `argv`, and port squatting | Fixed port + fixed key | A supported configuration requires a fixed port |
| **P2-D-13** | Readiness = `/health` + authenticated `/props` + 1-token generation probe; stall watchdog during generation | `/health` can be 200 while the server is wedged (#27388) | `/health` only | Upstream proves a reliable health signal |
| **P2-D-14** | Execute tools only from the structured `tool_calls` field; JSON-schema validate; one repair attempt; unknown tool fails the turn | Model output is untrusted; text that looks like a call is not intent | Regex extraction from content | Never |
| **P2-D-15** | Acquisition: allowlisted repos, pinned revision + SHA-256, quarantine validation, signed manifests, no weights in the installer | Supply-chain integrity and licence compliance | Download from any HF repo with a warning | — |
| **P2-D-16** | FP16 KV cache at 1.0; no KV quantisation by default | Q8_0 KV changes quality and has no measurement yet | Q8_0 KV default | Sensitivity run shows no quality loss and a needed memory win |
| **P2-D-17** | Vendor benchmarks are non-evidence for release decisions; Phase 3 protocol is the gate | Every score here is vendor-produced or secondary | Use vendor scores for ranking | Never |
| **P2-D-18** | Devstral, Gemma 4, GLM-4.7-Flash, Nemotron Bolt are **watchlist**, not 1.0 defaults | Memory headroom, 2-bit risk, untested parsers, legal review | Ship one as an alternative | They pass the benchmark gates on a supported tier |

## 3. Risks

Severity = likelihood × impact, on a 1–5 scale. These extend R-01…R-25 in Phase 1 doc 12.

| ID | Risk | Likelihood | Impact | Severity | Mitigation | Owner |
|---|---|---|---|---|---|---|
| **P2-R-01** | No measurements exist, so the tier mapping is wrong in either direction | High (certain) | High | **20** | Phase 3 measurement spike before UI work; treat all figures as planning ranges | Owner + eng |
| **P2-R-02** | A pinned nightly build regresses between tags | Medium | High | **12** | Contract tests on the exact build; mirror; rollback to previous build | Eng |
| **P2-R-03** | Upstream wedge/hang bugs (#27388, #27604) persist | Medium | High | **12** | Generation probe, single slot, kill-and-restart, Vulkan fallback to CPU | Eng |
| **P2-R-04** | Cancellation still leaves a hung process on some drivers | Medium | Medium | **9** | 2 s escalation to job kill; UI never spins silently | Eng |
| **P2-R-05** | Windows silently spills VRAM to system RAM, producing 1–2 tok/s with no error | High | High | **20** | Explicit budget, VRAM monitoring, decode-rate watchdog, refuse rather than spill | Eng |
| **P2-R-06** | 12 GB default model is too weak for agentic coding (vendor 47% SWE-bench) | High | Medium | **12** | Label as junior/assisted mode; do not promise autonomy; route complex tasks to 24 GB tiers | Product |
| **P2-R-07** | Tool-call validity stays below the 98% gate | Medium | High | **12** | Compact schemas, template per model, one repair, fail the turn honestly | Eng |
| **P2-R-08** | OpenMDW or CUDA redistribution requires legal changes | Medium | Medium | **9** | Watchlist Nemotron; ship CPU/CUDA 12.4 only after review; Vulkan fallback | Legal |
| **P2-R-09** | Mirroring is unaffordable or unapproved | Medium | High | **12** | Direct-from-source fallback with hash pinning; smaller runtime-only mirror | Owner |
| **P2-R-10** | AMD/Intel driver variability makes Vulkan support expensive | High | Medium | **12** | Vulkan is best-effort; CPU fallback always installed; publish a tested-driver list | Support |
| **P2-R-11** | Upstream model repository is deleted or rewritten | Low | High | **6** | Mirror pinned files or keep a verified local copy; kill switch | Eng |
| **P2-R-12** | Marketing or UI overstates context or quality, damaging trust | Medium | High | **12** | Context shown as configured; capability language from doc 05; no "works" claims | Product |
| **P2-R-13** | A model's template injects another product's persona or commit trailer | Medium | Medium | **9** | Override system prompt; strip trailers; template hash review (Devstral) | Eng |
| **P2-R-14** | Users on 16 GB systems expect 24 GB quality | High | Medium | **12** | Onboarding states the tier and the expected experience; benchmark per tier | Product |
| **P2-R-15** | The benchmark harness itself is wrong or gameable | Medium | High | **12** | Frozen corpus, executable checks, two reviewers, private split, determinism run | QA |
| **P2-R-16** | A malicious model file or local attacker compromises the runtime | Low | High | **6** | Hash allowlist, structural validation, job object, loopback + key, no privileged tools | Security |
| **P2-R-17** | Installer/AV blocks the runtime archive (false positive) | Medium | Medium | **9** | Signed binaries, reputation building, repair action, SHA-256 published | Release |
| **P2-R-18** | 1.0 slips because runtime contract tests uncover upstream bugs | Medium | Medium | **9** | Timebox 2 weeks; Ollama fallback; CPU-only safe mode | Owner |

The top three — P2-R-01, P2-R-05, P2-R-02 — are all consequences of the same gap: no Windows measurement yet. They are not independent risks.

## 4. Phase 1 corrections and clarifications

### C2-01 — Gemma 4 licence (corrected)

**Phase 1 doc 04, model table:**

> `| **Gemma 4 31B** | 31B dense | 256K (verify) | Gemma Terms of Use | ~19 GB | Watchlist | Quality is competitive, but terms include use restrictions and redistribution obligations; legal review required |`

**Phase 1 doc 04, licensing table:**

> `| Gemma | Gemma Terms of Use | Use restrictions, redistribution terms | Legal review before inclusion |`

**Verified:** `google/gemma-4-31B-it` declares `license:apache-2.0`; Google's model card and launch material state Apache-2.0; the repository is not gated. The custom Gemma Terms applied to Gemma 3 and earlier, not Gemma 4.

**Action taken:** both table rows edited in `docs/phase-1/04-ai-runtime-and-model-strategy.md`. Gemma 4 remains watchlisted for memory headroom, not licence risk. No other Phase 1 document was edited.

### C2-02 — Runtime distribution (clarified, not edited)

**Phase 1 doc 04:**

> "…active releases; Windows CPU/CUDA/Vulkan binaries."

**Verified:** the semantic release `v0.7.0` (2026-10-15) attaches only `nightly-tag.txt`. Windows binaries are attached to rolling `b####` tags (e.g. `b11714`, 2026-10-17).

**Action:** Phase 1 left intact because the statement is true of the nightly channel; the pinning policy is corrected in [`model-acquisition-policy.md`](model-acquisition-policy.md) §4 and [`runtime-evaluation.md`](runtime-evaluation.md) §3.1.

### C2-03 — Phase 2 hardware work (deferred)

**Phase 1 docs 11 and 12** planned hardware validation spikes and REF-A…REF-D measurements in Phase 2. The authorised Phase 2 is research-only and no hardware was available.

**Action:** no measurement was fabricated. The protocol and measurement plan are complete; execution is a Phase 3 gate. Reflected in the Phase 1 doc 12 acceptance note.

### C2-04 — Qwen3-Coder-Next parameter count (resolved)

**Phase 1 doc 04:** "80B MoE, ~3B active (per official Qwen README)". Verified against the technical report's 80B-A3B and the `qwen3_next` config (512 experts, 10 active). Secondary sources claiming "235B / 22B" are not used. No edit required.

### C2-05 — Qwen3-Coder-Next licence (resolved)

**Phase 1 doc 04:** "Verify model card". Verified Apache-2.0 on the Hub repository. Recorded here; no edit made, because a "verify" placeholder is not a factual error.

### C2-06 — 16 GB tier mapping (tightened)

**Phase 1 doc 04:** 16 GB VRAM → "Qwen3-Coder-30B-A3B at IQ3/Q3 with 8–16K context".

**Arithmetic:** Q3_K_M is 14.71 GB; 16K KV at 96 KiB/token is 1.5 GiB; scratch ≈1 GB; total ≈17.2 GB > 16 GB. Even 8K is marginal once the Windows compositor takes its share.

**Action:** Phase 2 default for 16 GB is gpt-oss-20b MXFP4 at 32K; Qwen3-Coder-30B at Q3 is limited to 8K and marked marginal. Phase 1 left intact; the corrected mapping is in [`model-evaluation.md`](model-evaluation.md) §7.

### C2-07 — 24 GB tier mapping (tightened)

**Phase 1 doc 04:** 24 GB VRAM → 30B-A3B Q4 **or** Qwen3.8-27B Q4 with **16–32K**.

**Arithmetic:** 30B-A3B Q4_K_M is 18.56 GB; 32K KV is 3 GiB; scratch ≈1.2 GB; total ≈22.8 GB of 24 GB (≈22.3 GiB usable) → does not fit. Qwen3.8-27B at 16.46 GB + 2.0 GiB + 1.2 GB ≈ 19.7 GB → fits at 32K.

**Action:** Qwen3.8-27B is the 24 GB default at 32K; 30B-A3B Q4 is an alternative at 16K. Phase 1 left intact; corrected mapping in [`model-evaluation.md`](model-evaluation.md) §7.

### C2-08 — `--jinja` default (clarification)

Phase 1 doc 03 assumed `--jinja` must be passed. In the current server it is enabled by default. Nexora will still pass it explicitly, so behaviour does not depend on an upstream default. No edit.

## 5. Open questions

| ID | Question | Why it matters | Blocks | Proposed owner |
|---|---|---|---|---|
| **Q2-01** | Can 12/16/24 GB Windows reference machines be provisioned? | Without them, no number in this phase can become a measurement | Final model choice; honest system requirements | Owner |
| **Q2-02** | Is there budget/hosting approval for a runtime and model mirror? | Offline install, supply-chain integrity, upstream deletion | P2-D-03, P2-D-15 | Owner |
| **Q2-03** | Legal sign-off on OpenMDW-1.1, CUDA runtime redistribution, and the Vulkan loader licence | Determines whether Nemotron and the CUDA pack can ship | Watchlist status; pack contents | Legal |
| **Q2-04** | Confirm IPC-only (no webview HTTP, no proxy) as the permanent architecture | Security boundary and test scope | Contract tests CT-14 | Eng lead |
| **Q2-05** | Is AMD/Intel GPU support required at 1.0, or is NVIDIA + CPU fallback acceptable? | Vulkan support cost and driver matrix | Runtime packs, support load | Product |
| **Q2-06** | May LM Studio be named in internal research at all, given its proprietary terms? | Avoids a trademark/licence misstep in public docs | Comms | Legal |
| **Q2-07** | Who owns and schedules the runtime contract-test suite? | It is the gate for every release | Phase 3 start | Eng lead |
| **Q2-08** | Which CUDA driver versions are the minimum for the 12.4 build? | Installer eligibility check and user messaging | P2-D-02 | Eng |
| **Q2-09** | Is Ollama supported officially, or only "works if installed"? | Support expectations | P2-D-04 | Product |
| **Q2-10** | Who owns output/content terms, given model licences do not address user output? | Customer contracts and privacy policy | Phase 5 | Legal |
| **Q2-11** | What machine-time budget is available for the full benchmark matrix? | Scope of Phase 3 measurements | Benchmark execution | Owner |
| **Q2-12** | Where does the manifest signing key live (HSM, offline, CI secret)? | Revocation and mirror authenticity | P2-D-15 | Security |
| **Q2-13** | Mirror weights at 1.0, or runtime archives only? | Cost vs resilience | P2-D-15 | Owner |
| **Q2-14** | Is a 60 s install-time model load test acceptable on low-end machines? | Catches malformed/unsupported files early | Acquisition UX | Product |

The eight Phase 1 questions Q-01, Q-02, Q-03, Q-04, Q-06, Q-07, Q-12, and Q-17 remain unanswered. Phase 2 proceeded under explicit instruction; none of these answers is assumed anywhere in this document set. Q-17 (reference hardware) is now the same question as Q2-01.

## 6. Contradictions and missing information not corrected

| Item | Why it is left open |
|---|---|
| Phase 1 doc 11 "Minimum: 8 GB VRAM" vs doc 04 "8 GB → Qwen2.5-Coder-7B" | Consistent at 8 GB; the gap is that 8 GB is called "Minimum" while H0 (no GPU) also exists. Tier naming, not a factual error. Needs a product decision. |
| Phase 1 doc 04 "Gemma 4 ~19 GB" vs actual Q4_K_M 18.32 GB | Approximate, within tolerance; not worth an edit. |
| Phase 1 doc 04 "Devstral ~15 GB Q4" vs 14.33 GB | Approximate. |
| Phase 1 doc 12 "Phase 2: 3–5 weeks" | Research-only Phase 2 took less; hardware work moved. Schedule should be re-baselined after Q2-01. |
| No minimum driver versions anywhere | Not verified in this phase; `Q2-08`. |
| No independent benchmark data | Deliberate: none exists for these quants on Windows. `P2-R-01`. |
| Installer size, cold-start time, idle memory | Cannot be known before code exists; Phase 3 budgets stand. |

## 7. Phase 3 handoff

Phase 3, per Phase 1 doc 12, is the Foundation phase: app shell, Rust core, model manager, inference supervisor, streaming chat. Phase 2 does not change that scope; it changes the order inside it.

### 7.1 Do first (weeks 1–2)

1. Provision at least H1 and H3 reference machines (`Q2-01`).
2. Implement the supervisor against the pinned build, with the state machine in [`integration-design.md`](integration-design.md) §3.
3. Run contract tests CT-01…CT-15.
4. Run the **smoke** benchmark matrix (four default models, one tier each) and publish the first measured numbers.
5. Re-baseline the hardware matrix from measurements.

### 7.2 Do not do

- Do not build the model manager around "latest" downloads.
- Do not expose the runtime port to the webview "temporarily".
- Do not ship a model with a vendor benchmark as the quality justification.
- Do not enable vision, KV quantisation, or multi-slot inference without a measurement.

### 7.3 Entry criteria for Phase 3

| Criterion | State |
|---|---|
| Runtime decision approved | Pending owner |
| Model defaults approved per tier | Pending owner |
| Reference hardware available | **Blocked** (Q2-01) |
| Contract test suite specified | Done (CT-01…CT-15) |
| Acquisition policy approved | Pending owner + legal |
| Benchmark corpus frozen | Not started (Phase 3 weeks 1–2) |

### 7.4 Exit criteria Phase 3 must add to Phase 1's list

- Measured load, TTFT, decode, and memory for each supported tier, published with environment metadata.
- Tool validity, schema validity, and cancel success measured against the gates.
- At least one full soak run per tier with no wedge.
- The hardware matrix rewritten from results, with superseded estimates kept in an appendix.

## 8. Traceability

| Phase 2 decision | Phase 1 requirement |
|---|---|
| P2-D-01, P2-D-02 | Doc 04 runtime criteria; doc 09 installer |
| P2-D-03, P2-D-15 | Doc 09 supply chain; doc 10 no binaries in Git |
| P2-D-06 | Doc 01 scope (coding desktop, not image tool) |
| P2-D-07, P2-D-08 | Doc 04 tier mapping; doc 11 hardware tiers |
| P2-D-09, P2-D-10 | Doc 03 inference supervisor; doc 08 cancel budget |
| P2-D-11, P2-D-12, P2-D-13 | Doc 07 inference hardening; doc 03 health |
| P2-D-14 | Doc 05 tool gateway; doc 07 injection defence |
| P2-D-16 | Doc 04 KV strategy |
| P2-D-17 | Doc 08 honesty tests H-01…H-10 |

---

**Approval requested:** P2-D-01 through P2-D-18, Q2-01 through Q2-14, and the Phase 3 entry criteria above. Until approved, these remain recommendations and Phase 3 should not begin.

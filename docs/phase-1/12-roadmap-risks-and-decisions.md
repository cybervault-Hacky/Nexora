# 12 — Roadmap, Risks, Trade-offs, and Open Decisions

**Status:** Proposed for owner approval  
**Related:** All Phase 1 documents

---

## 1. Phase dependency overview

```
Phase 1  Planning (this document set)
   │
   ▼
Phase 2  Technical validation & decisions
   │  ├─ Tauri/WebView2 spike ──────┐
   │  ├─ llama.cpp sidecar spike ───┤
   │  ├─ model benchmarks ──────────┤
   │  ├─ sandbox spike ─────────────┤
   │  ├─ checkpoint/store spike ────┤
   │  └─ preview/Edge spike ────────┘
   ▼
Phase 3  Foundation: shell, design system, IPC, storage, model manager, chat
   ▼
Phase 4  Workspace & safe tools: explorer, editor, terminal, permissions, checkpoints, preview
   ▼
Phase 5  Agent v1: plan, edit, run, verify, bounded repair, report, resume
   ▼
Phase 6  Website builder v1: templates, stack selection, Astro/React/Next/3D, visual checks
   ▼
Phase 7  Hardening & beta: security review, installer, signing, updater, accessibility, performance
   ▼
Phase 8  1.0 release: final model selection, release candidate, soak, docs, approval
   ▼
Post-1.0  Light theme, ARM64, MSI, deployment integrations, plugins, optional providers
```

**Critical path:** Phase 2 evidence (Tauri feasibility, llama.cpp quality/performance, sandbox viability) → Phase 5 agent reliability → Phase 6 website quality → Phase 7 security/signing.

No phase starts before the previous phase's exit criteria are met and the owner authorizes it. Durations below are planning ranges, not commitments.

---

## 2. Phase 2 — Technical validation and decisions (3–5 weeks)

**Objective:** replace assumptions with measurements before committing to architecture.

**Workstreams:**

1. **Desktop shell spike:** Tauri 2 + React shell with custom title bar, sidebar, composer, 1,000-message virtualized list, glass panel, CodeMirror, xterm.js. Measure startup, input latency, 60 fps transitions, memory, 200% DPI, IME (Japanese/Vietnamese), NVDA.
2. **Inference spike:** build/pin llama.cpp CPU/CUDA/Vulkan packs; spawn supervisor; stream; cancel; tool calls; JSON schema; load 3 candidate models; measure TTFT/tok/s/VRAM at 0/8K/32K.
3. **Model benchmark:** run NCE subset + hardware matrix on REF-A/B/C/D; rank per tier; identify quant trade-offs.
4. **Sandbox spike:** job object + restricted token; AppContainer/LPAC compatibility with Node/npm/Vite/Next; path-guard fuzzing; command policy.
5. **Storage/checkpoint spike:** SQLite schema, content-addressed snapshots, restore conflicts, 10k-file project performance.
6. **Preview spike:** iframe isolation, Edge CDP screenshots, axe, port allocation, host binding.
7. **Security threat-model review:** validate T-01…T-30; decide strict-mode scope.
8. **Recruit 5 usability testers** for composer/onboarding comprehension.

**Deliverables:** spike reports, ADRs 0001–0008, benchmark dataset, ranked model shortlist, go/no-go on Tauri and llama.cpp, updated risk register, Phase 3 plan.

**Exit criteria:**

- [ ] Tauri meets or misses defined performance/accessibility gates with evidence.
- [ ] llama.cpp meets protocol, streaming, cancellation, tool-call, and stability gates on at least two backends.
- [ ] Three models ranked per hardware tier; hard gates applied.
- [ ] Sandbox baseline works with real build tools; strict-mode compatibility documented.
- [ ] Checkpoint/restore passes conflict and crash tests.
- [ ] Preview screenshots and axe checks work via Edge.
- [ ] Owner approves runtime, stack, model shortlist, and Phase 3 scope.

**If Tauri fails:** pivot to Electron with the same Rust core as a sidecar or a Node core, re-plan Phase 3 by 2–4 weeks. If llama.cpp fails: evaluate Ollama/ONNX GenAI before changing the product scope.

---

## 3. Phase 3 — Foundation (6–8 weeks)

**Objective:** a usable local chat application with a premium shell and model management.

**Deliverables:**

- App shell, title bar, sidebar, conversation workspace, composer (all states), command palette.
- Design tokens, component library, onboarding, settings shell.
- Rust core, typed IPC, SQLite, migrations, logging, crash recovery.
- Model manager: catalogue, download, verify, install, load, switch, remove.
- Inference supervisor with backend selection, fallback, error recovery.
- Streaming chat, stop, regenerate, conversation persistence/search.
- Draft persistence, keyboard shortcuts, reduced motion, 100–200% DPI.

**Exit criteria:**

- [ ] Chat works fully offline with an installed model.
- [ ] Startup ≤ 2.5 s cold, input latency ≤ 50 ms p95, idle memory ≤ 250 MB.
- [ ] Model download/verify/load UX passes fault tests.
- [ ] Composer passes IME, keyboard, screen-reader, and reduced-motion tests.
- [ ] No P0/P1 bugs; no secrets in logs.

---

## 4. Phase 4 — Workspace and safe tools (6–8 weeks)

**Objective:** open real projects and perform privileged operations safely.

**Deliverables:**

- Project open/create, explorer, editor, tabs, diffs, search.
- Terminal (user shell + agent output), problems panel.
- Policy engine, permission cards, grants, audit log.
- Filesystem patch tool, command tool, read-only Git.
- Checkpoints and restore (file/step/task).
- Preview manager and sandboxed iframe.
- Background indexing/repo map (lexical/symbol).

**Exit criteria:**

- [ ] Path-guard and command-policy adversarial suites pass.
- [ ] Checkpoint/restore works under crash, conflict, and 10k-file conditions.
- [ ] Preview is loopback-only and isolated; no IPC access.
- [ ] Permissions are enforced in 100% of tool paths; audit is complete.
- [ ] Terminal and editor meet performance budgets.

---

## 5. Phase 5 — Agent v1 (8–10 weeks)

**Objective:** a bounded, evidence-driven coding agent that can complete real multi-file tasks.

**Deliverables:**

- State machine, planner, task journal, resume.
- Tool gateway with schema validation and policy integration.
- Verification service with evidence binding and stale invalidation.
- Repair controller with budgets, no-progress detection, protected files.
- Final report schema and UI.
- Agent eval harness integrated into CI/nightly.
- Prompt/context architecture, compaction, repo map retrieval.

**Exit criteria:**

- [ ] NCE subset: ≥ 60% full success on website tasks; ≥ 40% full-stack.
- [ ] Tool validity ≥ 98%; patch apply ≥ 95%.
- [ ] Zero false-success claims in the honesty suite.
- [ ] Zero unauthorized destructive actions in adversarial tests.
- [ ] Resume after crash/interrupt works; budgets stop loops.
- [ ] One mutating run per project enforced.

---

## 6. Phase 6 — Website builder v1 (6–8 weeks)

**Objective:** professional multi-page websites, app scaffolds, animations, and 3D experiences with local verification.

**Deliverables:**

- Brief schema and stack selector with rationale.
- Bundled templates: static, Astro marketing/blog, React/Vite, Next full-stack, R3F 3D, landing-motion.
- Design-token generation and quality rules.
- Preview checks: routes, console, responsive screenshots, axe, links.
- Visual evidence storage and quality rubric.
- Asset/font/licence handling; export README.

**Exit criteria:**

- [ ] All templates build and pass preview checks on clean machines.
- [ ] NCE website success ≥ 70%; full-stack ≥ 50%.
- [ ] Responsive checks pass at 360/768/1024/1440 for generated sites.
- [ ] axe critical/serious = 0 on templates (or documented exceptions).
- [ ] 3D demo renders with fallback; screenshots and console evidence captured.
- [ ] No deployment or "live" claims.

---

## 7. Phase 7 — Hardening and beta (6–8 weeks)

**Objective:** a trustworthy, signed, accessible beta.

**Deliverables:**

- Security review and external penetration test; fixes.
- Signed installer, updater, rollback, clean-VM tests.
- Accessibility audit (NVDA, keyboard, scaling, contrast).
- Performance hardening and large-project tests.
- Crash recovery, diagnostics bundle, redaction audit.
- Docs: install, models, permissions, offline, troubleshooting, privacy.
- Beta program with 20–50 testers; feedback triage.
- Threat-model update and incident-response runbook.

**Exit criteria:**

- [ ] Zero open P0/P1 security findings.
- [ ] Installer/upgrade/downgrade/uninstall pass on clean VMs.
- [ ] Crash-free sessions ≥ 99.5% in beta.
- [ ] Accessibility gates pass.
- [ ] All release artifacts signed; update manifest verified.
- [ ] Docs complete; support workflow defined.

---

## 8. Phase 8 — 1.0 release (3–4 weeks)

**Objective:** a stable, approved 1.0 for Windows.

**Deliverables:**

- Final model selection and catalogue for each hardware tier.
- Release candidate soak (72 hours), full NCE, performance matrix.
- Final security and licence review.
- Release notes, known issues, support plan.
- Signed installer and staged rollout plan.
- Owner approval.

**Exit criteria (1.0):**

- [ ] All Must requirements implemented and verified.
- [ ] NCE thresholds met; zero false success; zero destructive incidents.
- [ ] Performance budgets met on REF-A/B/C.
- [ ] Zero open P0/P1 bugs; security sign-off.
- [ ] Installer/update/rollback tested.
- [ ] Accessibility and documentation complete.
- [ ] Owner approves release; no public release without explicit approval.

---

## 9. Risk register

| ID | Risk | L | I | Mitigation | Trigger / owner |
|----|------|---|---|------------|-----------------|
| R-01 | Local model quality is insufficient for professional work | H | H | Benchmark before committing; verification-first UX; offer multiple models; set expectations; optional BYO later | Phase 2 benchmark gate |
| R-02 | Tauri/WebView2 fails performance, accessibility, or IME gates | M | H | Phase 2 spike; Electron fallback; abstract shell | Spike failure |
| R-03 | Rust velocity or skill gap delays delivery | M | H | Small crates, type generation, contractor support, Electron/TS fallback for non-security modules | Schedule slip |
| R-04 | Sandbox incompatible with Node/build tools | H | M | Tiered containment; baseline job object; strict mode opt-in; document residual | Phase 2 sandbox spike |
| R-05 | Prompt injection causes destructive action or data leak | M | H | Policy engine, confirmations, checkpoints, adversarial evals, secret exclusion | Any eval failure |
| R-06 | Model/runtime supply-chain compromise | L | H | Signed manifests, hashes, pinning, code signing, review | Security review |
| R-07 | Hardware fragmentation causes crashes or poor UX | H | M | Tiers, detection, safe mode, fallback, diagnostics; no universal performance promises | Beta reports |
| R-08 | Model download size causes onboarding drop-off | M | H | 10–20 GB default, smaller fast model, resume, clear value, offline option | Funnel metrics (beta) |
| R-09 | Long tasks lose context or loop | H | M | Journal, budgets, compaction, no-progress detection, resume | NCE metrics |
| R-10 | Licence issue with a model/runtime/font | M | H | Legal review, permissive defaults, catalogue gate, notices | Pre-release review |
| R-11 | SmartScreen/antivirus blocks installer | H | M | Signing, reputation, vendor submissions, checksums, docs | Installer tests |
| R-12 | Scope creep (deployment, mobile, cloud, plugins) | H | M | Explicit non-goals; phase gates; owner approval | Roadmap review |
| R-13 | Benchmarks overfit or mislead | M | H | Holdouts, real tasks, multiple runs, human review, external context only | Eval design review |
| R-14 | Product overclaims and loses trust | M | H | Honesty contract, UI vocabulary, adversarial tests, no marketing hype | Any false-success finding |
| R-15 | Team bandwidth/timeline pressure | H | M | Phase gates, cut Should/Could scope, no date promises | Each phase |
| R-16 | Windows/WebView2/driver changes break the app | M | M | Evergreen updates, compatibility tests, safe mode, pinned runtime | Monthly regression |
| R-17 | Data loss or checkpoint corruption | L | H | Atomic writes, backups, hash verification, crash tests | Any data-loss bug |
| R-18 | Model deprecation or removal | M | M | Multiple models, pinned files, catalogue status, export notes | Quarterly review |
| R-19 | GPU driver crashes/OOM | H | M | Fallback matrix, safe mode, CPU option, clear errors | Hardware matrix |
| R-20 | Generated code is insecure or low quality | H | M | Checks, rubric, dependency report, security disclaimer, no production guarantee | Release evals |
| R-21 | Key-person dependency | M | M | ADRs, docs, pairing, code ownership | Staffing review |
| R-22 | CI cost/build time | M | L | Caching, nightly subset, self-hosted runner later | CI metrics |
| R-23 | Signing key/certificate unavailable at launch | M | H | Order early, Azure Trusted Signing fallback, dev builds separate | Phase 7 |
| R-24 | User data is accessible to same-user malware | M | M | ACLs, DPAPI, document residual; optional encryption post-1.0 | Security review |
| R-25 | Installer/model download fails in restricted corporate environments | M | M | Proxy support docs, offline model import (advanced), MSI later | Beta feedback |

---

## 10. Major technical trade-offs

| Trade-off | Chosen direction | What is sacrificed | Why acceptable | Revisit |
|-----------|------------------|--------------------|----------------|---------|
| Tauri vs Electron | Tauri + Rust | Dev velocity, Chromium consistency | Windows-only, size/RAM/security needs | Phase 2 gates |
| llama.cpp vs vendor engines | llama.cpp | Peak throughput, some polish | Licence, coverage, control, GGUF | If perf gap is decisive |
| 20–30B vs 7B default | Tiered | Simplicity, small downloads | Quality requires size; tiers manage it | Owner hardware floor |
| Local-only vs cloud | Local-first | Weak-hardware users, frontier quality | Privacy/offline promise | Optional BYO later |
| Rust vs TypeScript orchestrator | Rust | Iteration speed | Security, process control, resumability | If velocity blocks |
| SQLite vs server DB | SQLite | Multi-user, sync | Single-user desktop | Post-1.0 |
| Content-addressed checkpoints vs Git | Blob store | Git semantics, ecosystem | Framework-agnostic, no repo pollution | Shadow-Git spike |
| iframe preview vs separate browser | iframe + CDP | Some automation convenience | Isolation, integrated UX | If limits bite |
| No Git writes | Read-only | Agent cannot commit | Prevents unwanted history changes | Post-1.0 |
| No deployment | Export only | End-to-end convenience | Deployment ≠ verification; security/liability | Post-1.0 |
| Dark-only at 1.0 | Dark | Light-theme users | Focus, brand | Post-1.0 |
| x64-only at 1.0 | x64 | ARM64 users | Runtime/installer maturity | Post-1.0 |
| English-only | English | Localization | Focus | Post-1.0 |
| No plugins | Monolith | Extensibility | Supply-chain security | Post-1.0 |
| No telemetry | Opt-in only | Product analytics | Privacy | Only with consent |

---

### 10.1 Decision analysis

Each major decision is recorded here with its recommendation, alternatives, advantages, disadvantages, and status. Detailed designs are in the referenced documents. "Deferred" means the decision requires Phase 2 evidence; the reasons are stated.

| Decision | Recommendation | Alternatives | Advantages | Disadvantages | Status / reversal |
|----------|----------------|--------------|------------|---------------|-------------------|
| Desktop framework | Tauri 2 + Rust + React/TS (doc 03) | Electron; .NET WinUI 3 + WebView2; Qt | Small installer, low idle memory, strong Rust privilege boundary, native process control | Rust velocity; WebView2 dependency and quirks | Proposed; reverse if Phase 2 perf/a11y/IME gates fail |
| Inference runtime | llama.cpp `llama-server` sidecar (doc 04) | Ollama, vLLM, TensorRT-LLM, ONNX GenAI, mistral.rs | MIT, GGUF coverage, CPU/GPU offload, control, Windows binaries, streaming/tools/grammars | Upstream churn, no model manager, parser quirks, hardening required | Proposed; reverse if contract/stability gates fail |
| Final model | Benchmark per tier, then choose (doc 04/11) | Pick now from vendor claims | Evidence on real hardware and quantizations; per-tier fit | Delays defaults and onboarding copy | **Deferred** to end of Phase 2; owner approves |
| Agent control plane | Rust orchestrator + deterministic policy (doc 05) | TypeScript/Node orchestrator; Python | No JS privilege path, resumable state, one enforcement point, future CLI | Slower iteration; Rust skills required | Proposed; reverse only if velocity blocks Phase 4/5 |
| Sandbox baseline | Job object + restricted token; AppContainer/LPAC strict mode (doc 07) | No sandbox; full VM; Windows Sandbox | Practical with build tools; kills process trees; layered | Baseline is containment, not a full security boundary; strict mode compatibility work | Proposed, staged; strict mode opt-in until proven |
| Installer | Per-user NSIS, signed (doc 09) | MSI/WiX, MSIX, portable ZIP | No admin, small, fast, Tauri-native | Less enterprise-friendly; no GPO | Proposed; MSI post-1.0 if demand |
| Updater | Signed manifest, user-approved, staged rollout (doc 09) | Silent auto-update; no updater | Trust, rollback, control; limits blast radius | Friction; signing key management | Proposed |
| Storage | SQLite WAL + blob store (doc 03) | Postgres; JSON files; embedded KV | Local, transactional, mature, easy backup/export | Single-writer discipline, migrations, no sync | Proposed |
| Checkpoints | Content-addressed snapshots outside project (doc 03) | Shadow Git; ZIP archives; copy-on-write | Framework-agnostic, dedupe, atomic restore, no repo pollution | Disk usage; no Git semantics; retention needed | Proposed; shadow-Git spike in Phase 2 |
| Editor | CodeMirror 6 (doc 03) | Monaco | Smaller/faster, good diff/merge, tree-shakeable | Fewer IDE features (no full IntelliSense) | Proposed |
| UI styling | CSS variables + Tailwind + Radix (doc 02/03) | CSS-in-JS; heavy UI kit; plain CSS only | Token discipline, no runtime CSS cost, accessible primitives | Class verbosity; custom components still needed | Proposed |
| Client state | Zustand per domain (doc 03) | Redux Toolkit; Jotai; Context only | Fine-grained subscriptions, low boilerplate, streaming isolation | Fewer conventions; discipline required | Proposed |
| Preview | Sandboxed iframe + Edge CDP checks (doc 03/06) | Separate embedded browser; bundled Chromium | Isolation, no 150 MB browser bundle, integrated UX | Cross-origin DOM limits; Edge dependency | Proposed; iframe + CDP is sufficient for checks |
| Node runtime | Bundle pinned Node LTS + pnpm for agent builds (doc 03) | Require user-installed Node | Reliable builds, consistent versions, no setup friction | +60–80 MB installer; runtime updates | Proposed; owner approves size |
| Model distribution | Download pinned HF revisions with signed manifest + SHA-256 (doc 04) | Bundle weights; trust HF at runtime | Small installer, licence control, user choice | Network dependency; download failures; host availability | Proposed |
| Git writes | Read-only at 1.0 (doc 01/05) | Agent commits; full Git | Avoids unwanted history changes; simpler permissions | Less automation | Proposed; post-1.0 with approval |
| Deployment | Export only at 1.0 (doc 06) | Hosting integrations | Avoids credential/liability risk; no false "live" claims | Less end-to-end convenience | Proposed; post-1.0 with a new threat model |
| Theme / OS / arch | Dark-only, Win11 x64, English at 1.0 (doc 02/09/12) | Light theme, Win10, ARM64, localization | Focus and quality | Excludes some users | Proposed; revisit post-1.0 |

## 11. Open questions requiring owner decisions

| ID | Question | Options | Recommendation | Impact if deferred |
|----|----------|---------|----------------|--------------------|
| Q-01 | OS baseline | Win11 23H2+ only; Win11 + Win10 22H2 ESU best-effort | Win11 primary; Win10 best-effort | Installer/tests scope |
| Q-02 | Minimum hardware | 8 GB VRAM / 16 GB RAM; 12 GB / 32 GB; CPU-only | 16 GB RAM + 8 GB VRAM floor; CPU best-effort | Model tiers, expectations |
| Q-03 | Max default model download | 10 GB; 20 GB; 40 GB | 20 GB default + 7B fast option | Onboarding, model choice |
| Q-04 | Licence/business model | Proprietary; open core; free | Proprietary default, decide before repo | Repository licence, updates |
| Q-05 | BYO cloud endpoint | At 1.0; post-1.0; never | Post-1.0, off by default | Product positioning |
| Q-06 | Code-signing budget | OV; EV; Azure Trusted Signing | EV or Trusted Signing | SmartScreen, cost |
| Q-07 | Telemetry/crash policy | None; opt-in crash only; anonymous analytics | None + opt-in crash | Privacy, instrumentation |
| Q-08 | Git write operations | Read-only; commits; full | Read-only at 1.0 | Agent capability |
| Q-09 | Deployment | None; export; integrations | Export only at 1.0 | Scope, security |
| Q-10 | English-only at 1.0 | Yes; add languages | Yes | Localization work |
| Q-11 | Windows ARM64 | At launch; post-1.0; never | Post-1.0 | Build/test scope |
| Q-12 | Trademark clearance | Proceed; legal review | Legal review before icon finalization | Brand risk |
| Q-13 | CPU-only support | Official; best-effort; unsupported | Best-effort with warnings | Support load |
| Q-14 | Semantic index default | On; opt-in; off | Opt-in initially | Disk/CPU, quality |
| Q-15 | External Ollama provider | At 1.0; post; never | Post-1.0 optional | Architecture, support |
| Q-16 | Gemma/other model legal review | Include; exclude | Exclude until reviewed | Catalogue scope |
| Q-17 | Reference hardware access | Buy/rent/borrow | Needed for Phase 2 | Benchmark schedule |
| Q-18 | Max task duration | 15/45/120 min | 45 min default, 2 h cap | Budgets, UX |
| Q-19 | Plan approval threshold | All plans; risky only | Risky only (new project, >10 files, installs, backend, destructive) | Friction vs safety |
| Q-20 | Agent Git commits | No; yes with approval | No at 1.0 | Risk |
| Q-21 | Show model reasoning | Full; summary; none | Summary only | UI, context |
| Q-22 | Tailwind default | Default; opt-in; none | Use where template already uses it; tokens first | Template design |
| Q-23 | Component library | Radix + custom; heavy kit | Radix + custom | UI consistency |
| Q-24 | Max project size | 10k/50k/200k files | 50k indexed; degrade beyond | Performance |
| Q-25 | Vue/Svelte support | At 1.0; post; never | Post-1.0 | Template scope |
| Q-26 | SOC 2 / certification | Now; post; never | Post-1.0 if enterprise | Cost |
| Q-27 | VM high-assurance mode | At 1.0; post; never | Post-1.0 | Security scope |
| Q-28 | Data retention defaults | 7/30/90 days | 30 days checkpoints, 14 days logs | Disk/privacy |
| Q-29 | Enterprise policy controls | At 1.0; post; never | Post-1.0 | MSI/GPO scope |
| Q-30 | MSI installer | At 1.0; post; never | Post-1.0 unless enterprise demand | Packaging |
| Q-31 | Publish checksums on website | Yes; GitHub only | Both | Ops |
| Q-32 | Update host | GitHub Releases; CDN | GitHub first, CDN later | Reliability |
| Q-33 | 8 GB VRAM official | Yes; best-effort; no | Best-effort minimum | Model defaults |
| Q-34 | AMD first-class | At launch; post; no | Vulkan supported, ROCm later | Test matrix |

**Decisions required before Phase 2 starts:** Q-01, Q-02, Q-03, Q-04, Q-06, Q-07, Q-12, Q-17. Others can be resolved during Phase 2.

---

## 12. Phase 1 acceptance criteria

Phase 1 is complete only when all of the following are true. Documentation items are marked as prepared; owner approval and the pre-Phase-2 answers remain open.

- [x] Product scope is documented clearly, with explicit non-goals and non-promises.
- [x] Windows desktop is confirmed as the only target platform.
- [x] The premium Nexora UI direction, composer specification, motion rules, and icon construction are documented.
- [x] Local AI runtime architecture and model evaluation criteria are documented.
- [x] Coding-agent workflow, tools, permissions, repair bounds, verification, and reporting are documented.
- [x] Website-builder architecture and the generate/build/test/run/deploy distinction are documented.
- [x] Repository structure, CI, build, installer, signing, and release strategy are documented.
- [x] Security/privacy threat model and mitigations are documented.
- [x] Hardware assumptions and benchmarking strategy are documented.
- [x] Testing, quality, accessibility, and performance strategy are documented.
- [x] Risks, constraints, trade-offs, deferred decisions, and open questions are documented.
- [x] Future phases have objectives, deliverables, dependencies, and exit criteria.
- [x] No implementation has started.
- [x] No repository has been created or published.
- [x] No release has been published.
- [ ] Owner has reviewed and approved this document set (Section 13).
- [ ] Owner has answered or explicitly deferred the pre-Phase-2 questions.

---

## 13. Approval and next steps

**What approval means:** the owner accepts the product scope, UI direction, architecture, agent and website-builder approach, security model, repository plan, and roadmap, and authorizes **Phase 2 technical validation** only.

**What approval does not mean:** it does not authorize application implementation, dependency installation, model downloads, repository creation, or release publication. Those require separate, explicit authorization.

**On approval, Phase 2 begins with:**

1. Create the private repository (only if separately authorized).
2. Record ADRs 0001–0008.
3. Build the Tauri, llama.cpp, sandbox, checkpoint, and preview spikes.
4. Benchmark candidate models on reference hardware.
5. Run the five-user usability test.
6. Deliver a Phase 2 evidence report and a go/no-go recommendation for Phase 3.

**Owner sign-off:**

| Item | Decision | Name | Date | Notes |
|------|----------|------|------|-------|
| Phase 1 document set | Approve / Request changes | | | |
| Product scope and non-goals | Approve / Request changes | | | |
| UI/UX direction and icon concept | Approve / Request changes | | | |
| Runtime and model shortlist | Approve / Defer | | | |
| Agent workflow and safety model | Approve / Request changes | | | |
| Security and privacy model | Approve / Request changes | | | |
| Phase 2 authorization | Authorize / Hold | | | |
| Q-01, Q-02, Q-03, Q-04, Q-06, Q-07, Q-12, Q-17 | Answered / Deferred | | | |

---

## 14. Glossary

| Term | Meaning |
|------|---------|
| **GGUF** | Quantized model file format used by llama.cpp |
| **Quantization** | Reducing weight precision (e.g., Q4_K_M) to fit memory, with some quality loss |
| **KV cache** | Per-token key/value memory used during generation; grows with context |
| **MoE** | Mixture-of-Experts model; many parameters, few active per token |
| **Prefill / decode** | Processing the prompt / generating tokens |
| **TTFT** | Time to first token |
| **Context window** | Maximum tokens the model can attend to |
| **Tool call** | Structured model request to run a tool |
| **Grammar / JSON schema** | Constraints that force valid structured output |
| **Job object** | Windows kernel object for grouping and limiting processes |
| **AppContainer / LPAC** | Windows sandbox with restricted filesystem/network capabilities |
| **Checkpoint** | Snapshot of project files for rollback |
| **NCE** | Nexora Coding Eval, the internal agent benchmark |
| **ADR** | Architecture Decision Record |
| **SBOM** | Software Bill of Materials |
| **False success** | Claiming work is verified when it is not |
| **Bounded repair** | Automatic fixing limited by attempts, time, and budgets |

---

## 15. Change control

- Changes to scope, architecture, security model, or phase gates require an ADR and owner approval.
- Changes to this document set are versioned in Git; the change log records rationale.
- Open questions may be resolved in Phase 2 with a written decision; they do not silently disappear.
- Any new feature that expands the threat surface (deployment, plugins, cloud, Git writes) requires a new threat model and explicit approval.

---

## 16. Sources

Sources for specific technical claims are listed in documents 03–11. The most consequential external references:

- Qwen3-Coder family — https://github.com/QwenLM/Qwen3-Coder
- Qwen3.8-27B — https://huggingface.co/Qwen/Qwen3.8-27B
- Devstral Small 2 — https://huggingface.co/mistralai/Devstral-Small-2-24B-Instruct-2512
- gpt-oss-20b — https://huggingface.co/openai/gpt-oss-20b
- llama.cpp — https://github.com/ggml-org/llama.cpp
- Tauri 2 — https://v2.tauri.app/
- Windows AppContainer — https://learn.microsoft.com/windows/win32/secauthz/appcontainer-isolation
- Windows Job Objects — https://learn.microsoft.com/windows/win32/procthread/job-objects
- OWASP LLM Top 10 — https://owasp.org/www-project-top-10-for-large-language-model-applications/
- Windows 10 end of support — https://support.microsoft.com/en-us/windows/deployment/updates-lifecycle/windows-10-support-has-ended-on-october-14-2025
- SWE-bench — https://www.swebench.com/
- Terminal-Bench — https://www.tbench.ai/

Access date: 2026-10-17. All model and runtime facts must be re-verified at Phase 2 kickoff.

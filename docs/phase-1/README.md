# Nexora — Phase 1: Requirements & Software Architecture

**Document status:** Complete — submitted for owner review and approval  
**Phase:** 1 (Planning only)  
**Date:** 2026-10-17  
**Target platform:** Windows desktop (x64 first)  
**Implementation status:** None. No application source, dependencies, models, repositories, or releases have been created.

---

## 1. Purpose of this document set

This is the Phase 1 deliverable for Nexora, a local-first, offline-capable AI coding studio for Windows. It defines what Nexora is and is not, how it should look and feel, the proposed system architecture, the local-inference and coding-agent design, the website-builder pipeline, the repository layout, the security model, the hardware and benchmarking strategy, the test and release strategy, the risks, the open decisions, and the roadmap for later phases.

It deliberately contains **no implementation**. Everything here is a proposal for review. Decisions marked **Proposed** require owner approval before Phase 2; decisions marked **Deferred** are intentionally left open until Phase 2 research produces evidence.

### Review order

| # | Document | What it covers | Est. read |
|---|----------|----------------|-----------|
| 0 | **README.md** (this file) | Executive summary, decision register, approval checklist | 10 min |
| 1 | [01-product-scope-and-requirements.md](01-product-scope-and-requirements.md) | Scope, personas, functional and non-functional requirements | 30 min |
| 2 | [02-ux-ui-specification.md](02-ux-ui-specification.md) | Brand, visual language, all 10 interface areas, composer, motion, icon | 35 min |
| 3 | [03-application-architecture.md](03-application-architecture.md) | Stack decision, process model, modules, IPC, storage, lifecycle | 35 min |
| 4 | [04-ai-runtime-and-model-strategy.md](04-ai-runtime-and-model-strategy.md) | Runtime evaluation, model shortlist, memory math, downloads, streaming | 40 min |
| 5 | [05-coding-agent-workflow.md](05-coding-agent-workflow.md) | Agent state machine, tools, permissions, repair bounds, verification | 40 min |
| 6 | [06-website-builder-architecture.md](06-website-builder-architecture.md) | Stack selection, templates, preview, testing, 3D, deployment boundary | 30 min |
| 7 | [07-security-and-privacy.md](07-security-and-privacy.md) | Threat model, sandboxing, secrets, prompt injection, privacy | 35 min |
| 8 | [08-testing-and-quality.md](08-testing-and-quality.md) | Test pyramid, agent evals, performance budgets, release gates | 25 min |
| 9 | [09-build-installer-and-release.md](09-build-installer-and-release.md) | Toolchain, signing, installer, updater, CI, release process | 20 min |
| 10 | [10-repository-structure.md](10-repository-structure.md) | Proposed private GitHub layout and engineering conventions | 20 min |
| 11 | [11-hardware-benchmarking.md](11-hardware-benchmarking.md) | Hardware tiers, model benchmark plan, selection scorecard | 25 min |
| 12 | [12-roadmap-risks-and-decisions.md](12-roadmap-risks-and-decisions.md) | Risks, trade-offs, phase dependencies, roadmap, open questions, acceptance | 30 min |

Total review time is roughly six hours. A reviewer who only has 30 minutes should read this file, Section 2 of document 01, Section 3 of document 02, and the decision register in Section 6 of document 12.

---

## 2. Executive summary

Nexora is a **professional Windows desktop AI coding studio**. It runs a capable local coding model, acts as a controlled coding agent over a user-chosen project folder, and helps users build multi-file websites, applications, animations, and code-based 3D experiences. Core functionality does not require a paid external AI API. Models may be downloaded once, then used offline.

The product is **not** a website, not a browser-only app, and not a mobile app. It is a native Windows application with a polished, glass-accented dark interface. It combines a conversational workspace with a project explorer, code editor, terminal, preview browser, task timeline, model manager, and permission system.

The central engineering idea is **evidence-backed autonomy**: the model proposes actions, a deterministic policy layer decides what may run, every action is logged, every change is checkpointed, and the agent may only describe work as "verified" when local checks actually passed. Autonomy is bounded by budgets, repair-loop limits, and explicit approvals for risky operations.

### Key recommendations at a glance

| Area | Recommendation | Status |
|------|----------------|--------|
| Desktop shell | Tauri 2 + Rust core + React 19/TypeScript UI on Windows WebView2 | Proposed |
| UI system | Custom Nexora design system with Radix primitives, Tailwind tokens, Motion | Proposed |
| Local inference | llama.cpp `llama-server` shipped as a pinned, signed sidecar | Proposed |
| Runtime abstraction | `InferenceProvider` interface; optional external Ollama/BYO endpoint later | Proposed |
| Model strategy | Shortlist, then benchmark on three reference PCs before final selection | Deferred |
| Primary model class | 20B–30B class for 16–24 GB GPUs; 7B fallback; 80B MoE only for high-end | Deferred |
| Agent control plane | Rust orchestrator; model cannot execute tools directly | Proposed |
| Storage | SQLite (WAL) + content-addressed checkpoints + OS credential vault | Proposed |
| Sandboxing | Job Objects + restricted tokens first; AppContainer/LPAC where compatible | Proposed, staged |
| Preview/testing | Local-only dev server + Edge/Playwright checks + screenshots | Proposed |
| Packaging | Per-user NSIS installer, signed; signed updater with staged rollout | Proposed |
| Telemetry | None by default; opt-in crash reports only | Proposed |
| Target OS | Windows 11 23H2+; Windows 10 22H2 ESU best-effort, not a launch requirement | Proposed |

### What Nexora will not claim

- It will not promise zero errors, perfect code, or superiority over commercial AI systems.
- It will not say a project "works" unless build, test, and runtime checks support that claim.
- It will not call generated code "deployed" unless a deployment actually occurred.
- It will not call the whole product "offline" when model downloads, dependency installation, or update checks require network access.
- It will not hide when a result is unverified, partially complete, or blocked.

---

## 3. Scope snapshot

**In scope for the product direction:** Windows 11 desktop; local model inference; model manager; multi-file project generation and editing; conversational agent; project explorer and editor; terminal; local preview; build/test/lint workflows; bounded automatic repair; checkpoints and rollback; permission controls; 3D and animation-capable web projects; private repository and release pipeline planning.

**Out of scope:** Android, iOS, web-hosted Nexora, Linux, macOS; cloud sync and team collaboration; autonomous deployment to production; model training or fine-tuning; image/video generation as a core feature; general desktop automation; plugin marketplace; light theme at 1.0 (planned after 1.0); Windows ARM64 at launch (revisit later); 32-bit Windows; BYO cloud AI as a core dependency.

The full scope, personas, and requirement IDs are in [01-product-scope-and-requirements.md](01-product-scope-and-requirements.md).

---

## 4. The proposed experience in one page

The window opens to a **custom title bar** with the Nexora mark, project switcher, global command palette, and window controls. A **left sidebar** holds New Chat, conversation history, projects, and navigation. The **center** is a calm conversation workspace with generous spacing and a premium glass composer pinned at the bottom. A collapsible **right context panel** shows the current plan, task timeline, changed files, and verification evidence. The **bottom panel** switches between terminal, output, problems, and preview console. The **preview workspace** can expand to a device-framed browser with responsive presets.

The composer is treated as a product-defining object: a 24 px-radius frosted surface, balanced internal padding, clear placeholder typography, model and context controls, drag-and-drop attachments, a send/stop morphing button, and explicit ready, generating, stopped, error, and disabled states. It respects IME composition, keyboard shortcuts, reduced motion, and window widths down to 1024 px.

The visual language is graphite and charcoal with off-white type and a restrained periwinkle accent. Glass is used on at most three layers, never behind body text at the cost of contrast, and never as a substitute for hierarchy. The full specification, including the icon construction, is in [02-ux-ui-specification.md](02-ux-ui-specification.md).

---

## 5. Architecture in one page

```
┌──────────────────────────────────────────────────────────────────┐
│ Presentation (React 19 + TypeScript, WebView2)                   │
│  Chat · Composer · Explorer · Editor · Terminal · Preview · Settings │
└───────────────────────────────┬──────────────────────────────────┘
                                │ typed IPC (commands + event streams)
┌───────────────────────────────▼──────────────────────────────────┐
│ Rust core (Tauri host)                                           │
│  Agent orchestrator · Policy engine · Permission broker          │
│  Context builder · Verification service · Checkpoint service     │
│  Project service · Conversation service · Model manager          │
└───────┬───────────────────┬───────────────────┬──────────────────┘
        │                   │                   │
┌───────▼────────┐  ┌───────▼────────┐  ┌───────▼──────────────────┐
│ llama-server   │  │ Tool runtime   │  │ Storage                  │
│ sidecar        │  │ FS · process   │  │ SQLite · blobs · vault   │
│ (127.0.0.1)    │  │ search · git   │  │ checkpoints · audit log  │
└────────────────┘  └───────┬────────┘  └──────────────────────────┘
                            │
                 ┌──────────▼──────────┐
                 │ Child processes     │
                 │ npm · build · tests │
                 │ dev server · Edge   │
                 └─────────────────────┘
```

The Rust core owns every privileged operation. The webview never receives raw shell access, filesystem access, or model process handles. Model output is untrusted input that must pass schema validation and policy checks before any tool executes. The inference server binds only to loopback, uses a per-launch bearer token stored in a user-only file, and runs with its web UI and experimental agent tools disabled.

Detailed process model, IPC contracts, storage schema, and lifecycle are in [03-application-architecture.md](03-application-architecture.md). The inference and model strategy is in [04-ai-runtime-and-model-strategy.md](04-ai-runtime-and-model-strategy.md). The agent workflow is in [05-coding-agent-workflow.md](05-coding-agent-workflow.md).

---

## 6. Decision register

| ID | Decision | Recommendation | Alternatives | Status | Owner action |
|----|----------|----------------|--------------|--------|--------------|
| D-01 | Desktop framework | Tauri 2 + Rust + React/TS | Electron; .NET WinUI 3 + WebView2; Qt | Proposed | Approve for Phase 2 spike |
| D-02 | Inference runtime | llama.cpp `llama-server` sidecar | Ollama embedded; vLLM; TensorRT-LLM; ONNX GenAI; mistral.rs | Proposed | Approve for Phase 2 spike |
| D-03 | Model choice | Benchmark shortlist on 3 reference PCs | Select now from vendor claims | Deferred | Confirm benchmark hardware access |
| D-04 | Agent control plane | Rust orchestrator with policy engine | TypeScript/Node orchestrator; Python | Proposed | Approve |
| D-05 | Sandbox baseline | Job Objects + restricted token, then AppContainer/LPAC | No sandbox; full VM/container; Windows Sandbox | Proposed, staged | Approve phased approach |
| D-06 | Installer | Per-user NSIS, signed | MSI/WiX machine-wide; MSIX | Proposed | Confirm signing budget |
| D-07 | Update channel | Signed updater, manual approval, staged rollout | Silent auto-update; no updater | Proposed | Approve |
| D-08 | Telemetry | None by default; opt-in diagnostics | Anonymous analytics on by default | Proposed | Confirm policy |
| D-09 | OS baseline | Windows 11 23H2+ x64 | Windows 10 22H2 also supported; ARM64 at launch | Proposed | Confirm |
| D-10 | Node runtime | Bundle pinned Node LTS + pnpm for agent builds | Require user-installed Node | Proposed | Approve installer size impact |
| D-11 | Preview testing | Installed Edge via Playwright/CDP | Bundle Chromium; no browser automation | Proposed | Approve |
| D-12 | Repo strategy | Single private monorepo, trunk-based, protected main | Multi-repo | Proposed | Approve before repo creation |
| D-13 | Light theme | Post-1.0 | At 1.0; never | Deferred | Confirm |
| D-14 | ARM64 | Post-1.0 based on demand | At launch | Deferred | Confirm |
| D-15 | Cloud/BYO model | Optional, off by default, after local core is solid | Never; core dependency | Deferred | Confirm product position |

Each decision is analyzed with advantages, disadvantages, and reversal conditions in the relevant document and summarized in [12-roadmap-risks-and-decisions.md](12-roadmap-risks-and-decisions.md).

---

## 7. What Phase 1 deliberately did not do

- No application code, scaffolding, manifests, lockfiles, or build configuration.
- No dependency installation.
- No AI model download.
- No GitHub repository creation, publication, or release.
- No installer, signing certificate, or CI workflow creation.
- No claim that any model or runtime has been benchmarked; all performance figures in these documents are cited vendor/community claims used only for planning ranges.
- No final brand asset; the icon is specified geometrically, not produced as a final file.

---

## 8. Phase 1 acceptance checklist

Phase 1 is complete when the owner confirms all of the following:

- [ ] Product scope is documented and agreed, including explicit non-goals.
- [ ] Windows desktop is confirmed as the only target platform.
- [ ] The premium Nexora UI direction, composer specification, and icon direction are approved.
- [ ] Local AI architecture and model evaluation criteria are accepted.
- [ ] Coding-agent workflow, permissions, repair bounds, and verification contract are accepted.
- [ ] Website-builder architecture and the generate/build/test/run/deploy distinction are accepted.
- [ ] Proposed private repository structure and security requirements are accepted.
- [ ] Technical risks, trade-offs, deferred decisions, and open questions are acknowledged.
- [ ] Future phases have objectives, deliverables, dependencies, and exit criteria.
- [ ] No implementation has started.
- [ ] No repository has been published.
- [ ] No release has been published.
- [ ] The owner has answered or explicitly deferred the open questions in document 12, Section 5.

**Sign-off (to be completed by owner):**

| Item | Name | Decision | Date | Notes |
|------|------|----------|------|-------|
| Phase 1 documents | | Approve / Request changes | | |
| D-01 desktop stack | | Approve / Defer | | |
| D-02 inference runtime | | Approve / Defer | | |
| D-09 OS baseline | | Approve / Defer | | |
| Phase 2 start | | Authorize / Hold | | |

Until sign-off, Nexora remains in planning. Phase 2 must not begin, and no source code, repository, model download, or release may be created.

---

## 9. Document conventions

- **MUST / SHOULD / MAY** follow RFC 2119 intent.
- Requirement IDs: `FR-*` functional, `NFR-*` non-functional, `SR-*` security, `T-*` threat, `R-*` risk, `Q-*` open question.
- Priority: **Must** (1.0 blocker), **Should** (target 1.0, may slip), **Could** (post-1.0), **Won't** (explicitly excluded).
- Decision status: **Proposed**, **Accepted**, **Deferred**, **Rejected**, **Superseded**.
- All performance numbers are planning envelopes unless labeled as measured. Vendor benchmark claims are not treated as evidence of local performance.
- Sources are listed at the end of the documents that rely on external facts, with access date 2026-10-17.

---

## 10. Change log

| Version | Date | Change | Author |
|---------|------|--------|--------|
| 0.1 | 2026-10-17 | Initial Phase 1 planning set for review | Architecture & product planning |

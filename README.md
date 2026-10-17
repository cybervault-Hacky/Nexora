# Nexora

**Nexora is a local-first, offline-capable AI coding studio for Windows.** It combines a premium conversational workspace with a bounded coding agent that can create and edit multi-file websites, applications, animations, and code-based 3D experiences — using a local model, with no paid external AI API required for core functionality.

> **Project status: Phase 7 delivered — partially verified. The native build and real inference are still unverified.**
> Phase 1 (requirements) and Phase 2 (runtime and model evaluation) are documentation. Phases 3–4 built the real Tauri 2 + React desktop shell. Phase 5 connects that shell to a **user-supplied** `llama-server` and GGUF file over loopback HTTP, with streaming, cancellation, and honest failure states. Phases 6–7 are verification phases.
> The repository ships **no model weights and no runtime binaries**, and downloads nothing on its own. There is no Rust toolchain, Windows host, crate-registry access, or owner-supplied model in the verification environment, so the crate remains **uncompiled**, no window was launched, and no real model output was produced. See the [Phase 7 report](docs/phase-7/README.md) for exactly what ran and what did not. The Windows installer has not been built or tested.

## Application foundation (Phases 3–5)

The desktop app lives in [`apps/desktop/`](apps/desktop/README.md): Tauri 2 shell, Rust core, React 19 + TypeScript, Vite.

What the code implements (see the verification note above for what has actually been executed):

- A real native window (1280×840, minimum 960×640) with the Nexora title bar, collapsible sidebar, conversation workspace, empty state, and composer.
- **Local inference with your own runtime and model.** Point Nexora at a `llama-server` executable and a GGUF file you already have; replies are streamed token by token from that process over a loopback-only, key-protected endpoint.
- **Stop generation** cancels the live request, keeps the partial text, and labels it `Stopped`. One generation at a time; a second send is refused, not queued.
- Engine states are read from the Rust core, not guessed: no model configured, stopped, loading, ready, generating, and error are all distinct — and an unreadable IPC status is shown as *unknown*, never as "no model".
- No model or runtime is downloaded by the app. Nothing is bundled. Paths are typed deliberately by the user and validated (GGUF magic/version/size, executable header) before launch.
- Duplicate submissions are rejected by a 350 ms guard; new chat clears messages, draft, and errors and cancels any in-flight request; scroll follow pauses while you read earlier output.
- Empty-input and over-length handling, Enter/Shift+Enter, `Ctrl+L` focus, `Ctrl+N` new chat, `Ctrl+B` sidebar, keyboard focus, reduced motion, and accessible labels.
- 105 unit, hook, DOM interaction, and cross-language contract tests (`pnpm test`).

Quick start (Windows, after installing the prerequisites in the app README):

```bash
pnpm install
pnpm tauri dev
```

Verification performed here: `pnpm typecheck`, `pnpm test` (105/105), and `pnpm build` pass. A `llama-server` was built from the pinned llama.cpp `b11714` source and executed on Linux, which confirmed the version handshake, launch flags, `/health`, authentication and error bodies, startup-failure behaviour, and SSE streaming with cancellation (using a synthetic random-weight model for protocol only); four contract defects were fixed as a result. Phase 7 added a source-derived check that the serde wire shapes and Tauri command arguments match the TypeScript IPC types, with a mutation test proving it fails on drift. `cargo` is unavailable in this environment, so the Rust crate and the Windows executable remain **not** compiled or runtime-verified, and no model weights were available for real generation. See [`docs/phase-7/`](docs/phase-7/README.md) and [`docs/phase-5/`](docs/phase-5/README.md) for the exact commands, results, and unverified surface.

## Phase 7 deliverables

- [`docs/phase-7/`](docs/phase-7/README.md) — the measured environment boundary, the executed frontend gate, the Rust↔TypeScript contract test (including its mutation check), and the exact checks that remain unrun. Verdict: **C — partially verified**.

## Phase 6 deliverables

- [`docs/phase-6/`](docs/phase-6/README.md) — environment, what was executed against a locally built `llama-server`, the four defects fixed, and the exact blockers that keep this phase at **partially verified**.

## Phase 5 deliverables

- [`docs/phase-5/`](docs/phase-5/README.md) — the runtime contract actually verified against llama.cpp `b11714`, the interfaces assumed rather than proven, and the Windows checklist that remains.
- [`apps/desktop/README.md`](apps/desktop/README.md) — engine setup, cancellation semantics, security posture, and known limits.

## Phase 1 deliverables

The complete planning set is in [`docs/phase-1/`](docs/phase-1/README.md):

| # | Document | Contents |
|---|----------|----------|
| 0 | [Phase 1 index](docs/phase-1/README.md) | Executive summary, decision register, approval checklist |
| 1 | [Product scope and requirements](docs/phase-1/01-product-scope-and-requirements.md) | Scope, personas, functional/non-functional requirements |
| 2 | [UX/UI specification](docs/phase-1/02-ux-ui-specification.md) | Brand, visual language, all interface areas, composer, motion, icon |
| 3 | [Application architecture](docs/phase-1/03-application-architecture.md) | Stack decision, process model, modules, IPC, storage |
| 4 | [AI runtime and model strategy](docs/phase-1/04-ai-runtime-and-model-strategy.md) | Runtime evaluation, model shortlist, memory math, downloads |
| 5 | [Coding-agent workflow](docs/phase-1/05-coding-agent-workflow.md) | Agent state machine, tools, permissions, repair bounds, verification |
| 6 | [Website-builder architecture](docs/phase-1/06-website-builder-architecture.md) | Stack selection, templates, preview, testing, 3D |
| 7 | [Security and privacy](docs/phase-1/07-security-and-privacy.md) | Threat model, sandboxing, secrets, prompt injection |
| 8 | [Testing and quality](docs/phase-1/08-testing-and-quality.md) | Test pyramid, agent evals, performance, release gates |
| 9 | [Build, installer, and release](docs/phase-1/09-build-installer-and-release.md) | Toolchain, signing, NSIS installer, updater, CI |
| 10 | [Repository structure](docs/phase-1/10-repository-structure.md) | Proposed private GitHub layout and conventions |
| 11 | [Hardware and benchmarking](docs/phase-1/11-hardware-benchmarking.md) | Hardware tiers, benchmark protocol, model scorecard |
| 12 | [Roadmap, risks, and decisions](docs/phase-1/12-roadmap-risks-and-decisions.md) | Phase plan, risks, trade-offs, open questions, sign-off |

### Required Phase 1 deliverables — coverage map

| Required deliverable | Where it is covered |
|----------------------|---------------------|
| 1. Final product scope | [Doc 01 §1–4](docs/phase-1/01-product-scope-and-requirements.md) |
| 2. Functional requirements | [Doc 01 §6](docs/phase-1/01-product-scope-and-requirements.md) |
| 3. Non-functional requirements | [Doc 01 §7](docs/phase-1/01-product-scope-and-requirements.md) |
| 4. UI/UX design specification | [Doc 02 §§2–16](docs/phase-1/02-ux-ui-specification.md) |
| 5. Proposed application architecture | [Doc 03](docs/phase-1/03-application-architecture.md) |
| 6. AI model and runtime evaluation criteria | [Doc 04](docs/phase-1/04-ai-runtime-and-model-strategy.md), [Doc 11](docs/phase-1/11-hardware-benchmarking.md) |
| 7. Coding-agent workflow design | [Doc 05](docs/phase-1/05-coding-agent-workflow.md) |
| 8. Website-builder architecture | [Doc 06](docs/phase-1/06-website-builder-architecture.md) |
| 9. Private GitHub folder structure | [Doc 10](docs/phase-1/10-repository-structure.md) |
| 10. Security and privacy threats | [Doc 07](docs/phase-1/07-security-and-privacy.md) |
| 11. Hardware assumptions and benchmarking | [Doc 11](docs/phase-1/11-hardware-benchmarking.md) |
| 12. Testing and quality strategy | [Doc 08](docs/phase-1/08-testing-and-quality.md) |
| 13. Build and installer strategy | [Doc 09](docs/phase-1/09-build-installer-and-release.md) |
| 14. Risks, constraints, trade-offs | [Doc 12 §9–10](docs/phase-1/12-roadmap-risks-and-decisions.md) |
| 15. Dependencies between future phases | [Doc 12 §1–8](docs/phase-1/12-roadmap-risks-and-decisions.md) |
| 16. Phase 1 acceptance criteria | [Doc 12 §12](docs/phase-1/12-roadmap-risks-and-decisions.md) |
| 17. Open questions for the owner | [Doc 12 §11](docs/phase-1/12-roadmap-risks-and-decisions.md) |
| 18. Refined implementation roadmap | [Doc 12 §2–8](docs/phase-1/12-roadmap-risks-and-decisions.md) |

## Phase 2 deliverables

The runtime and model evaluation is in [`docs/phase-2/`](docs/phase-2/README.md). It is research and documentation only — no code, downloads, or benchmarks.

| # | Document | Contents |
|---|----------|----------|
| 0 | [Phase 2 index](docs/phase-2/README.md) | Scope, evidence grades, headline recommendations, corrections, acceptance |
| 1 | [Runtime evaluation](docs/phase-2/runtime-evaluation.md) | llama.cpp vs Ollama, vLLM, ONNX GenAI, mistral.rs; Windows support; hardening |
| 2 | [Model evaluation](docs/phase-2/model-evaluation.md) | Verified specs, revisions, licences, quant sizes, KV maths, tier selection |
| 3 | [Hardware matrix](docs/phase-2/hardware-matrix.md) | 12/16/24 GB and CPU-only estimates, labelled as estimates |
| 4 | [Benchmark protocol](docs/phase-2/benchmark-protocol.md) | Ten-dimension reproducible protocol, scoring, gates, result schema |
| 5 | [Integration design](docs/phase-2/integration-design.md) | Rust-owned lifecycle, streaming, cancellation, recovery, loopback hardening |
| 6 | [Model acquisition policy](docs/phase-2/model-acquisition-policy.md) | Sources, pinning, integrity, resume, notices, malicious files |
| 7 | [Decisions and risks](docs/phase-2/decisions-and-risks.md) | Decisions, risks, Phase 1 corrections, open questions, Phase 3 handoff |
| 8 | [Sources](docs/phase-2/sources.md) | Every source with URL, access date, and what it supports |

## Non-negotiable boundaries

- Windows desktop only. No Android, iOS, or browser-only product.
- Core inference is local. Models may be downloaded once, then used offline.
- Offline inference does not mean every feature is offline: model downloads, dependency installs, and update checks are disclosed.
- The agent never claims a project works unless local checks support that claim.
- Risky filesystem, process, and network actions require permissions and are checkpointed.
- No secrets in the repository, logs, or diagnostics.

## Next step

Native verification still requires a Windows machine: install the Rust toolchain and Visual Studio Build Tools, run `pnpm tauri dev`, then `pnpm tauri build`. A genuine inference check needs a llama.cpp build pinned to the version recorded in [`docs/phase-2/`](docs/phase-2/README.md) plus a GGUF file the owner supplies; none is committed here and nothing is downloaded automatically. Phase 8 starts only after Phase 7 is approved.

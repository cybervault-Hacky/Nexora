# Nexora

**Nexora is a local-first, offline-capable AI coding studio for Windows.** It combines a premium conversational workspace with a bounded coding agent that can create and edit multi-file websites, applications, animations, and code-based 3D experiences — using a local model, with no paid external AI API required for core functionality.

> **Project status: Phase 1 — Planning complete, awaiting owner approval.**
> No application code, dependencies, models, private repository, or release has been created. Do not begin implementation until Phase 2 is explicitly authorized.

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

## Non-negotiable boundaries

- Windows desktop only. No Android, iOS, or browser-only product.
- Core inference is local. Models may be downloaded once, then used offline.
- Offline inference does not mean every feature is offline: model downloads, dependency installs, and update checks are disclosed.
- The agent never claims a project works unless local checks support that claim.
- Risky filesystem, process, and network actions require permissions and are checkpointed.
- No secrets in the repository, logs, or diagnostics.

## Next step

Review the Phase 1 set, answer or defer the open questions in [document 12, Section 11](docs/phase-1/12-roadmap-risks-and-decisions.md#11-open-questions-requiring-owner-decisions), and complete the approval block. Phase 2 — technical validation spikes and model benchmarks — starts only after explicit authorization.

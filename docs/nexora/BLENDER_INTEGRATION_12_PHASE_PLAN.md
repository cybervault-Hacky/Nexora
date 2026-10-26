# Blender App Integration — Proposed 12-Phase Implementation Outline

**Status:** Planning outline only. Phase 1 architecture/requirements documentation is complete for this request; Phase 2 and all later work are **not started**.
**Authority:** Proposed sub-roadmap for owner review; it does not replace or amend Nexora's approved core roadmap or separate media-generation roadmap.
**Related:** [APP_INTEGRATIONS_ARCHITECTURE.md](APP_INTEGRATIONS_ARCHITECTURE.md), [APP_CONNECTOR_CONTRACT.md](APP_CONNECTOR_CONTRACT.md), [BLENDER_INTEGRATION_REQUIREMENTS.md](BLENDER_INTEGRATION_REQUIREMENTS.md).

## Provenance and phase gates

Repository search during this Phase 1 inspection found no prior Blender plan or Blender implementation. The sequence below is therefore a **new proposed outline**, not a claim that an earlier 12-phase plan was approved or implemented. Reconcile it with any separately held proposal before authorizing implementation.

Each phase needs its own explicit owner authorization, entry conditions, evidence, and exit criteria. Work stops at Phase 1 until explicit authorization for Phase 2. A failed gate blocks dependent phases; no phase is considered complete because documentation, source files, a mock, or a UI state exists.

| Phase | Goal | Main deliverables | Exit gate / dependency |
|---|---|---|---|
| **1. Architecture & requirements** *(current)* | Define reusable App Integrations architecture, Blender scope/boundaries, contract, security, risks, and staged plan. | The four focused docs linked above. No product code, connector, install, or Blender execution. | Owner accepts/revises the design and explicitly authorizes Phase 2. **Documentation only is complete here.** |
| **2. Windows feasibility and protocol spikes** | Replace transport/version/platform assumptions with evidence before committing. Spike supported Blender discovery, embedded Python named-pipe client, user-only pipe ACL, peer PID/path checks, add-on lifecycle, cancellation, and Blender version/API probes on actual Windows builds. | Spike report; ADRs for transport, minimum Windows/Blender/add-on versions, process policy and protocol; compatibility test harness; revised risks and support matrix. | No unauthenticated listener; peer identity and ACL behavior tested; add-on can connect/disconnect/reconnect; failures/cancel safely; owner approves supported matrix. No production feature claims. |
| **3. Shared App Integrations foundation** | Build the generic page, typed descriptor/registry, independent install/connection/readiness states, permissions presentation, common configuration surface, and stable Tauri/UI contracts. | Searchable catalogue with empty/loading/error/unsupported states; first-party registry and integration service; contract tests; accessibility review. Do not create Blender-specific page. | Existing shell navigation works; status facts remain distinct; no connector can bypass Rust enforcement; test a generic fixture/approved adapter end-to-end. |
| **4. Blender detection and connection lifecycle** | Add reviewed Blender installation discovery and user-directed add-on setup/connection using the Phase 2 transport decision. | Bounded Windows detector, selected-install record, protocol handshake, health/reconnect/disconnect, add-on packaging/integrity path, lifecycle/error UI on shared card. | Test supported Windows/Blender combinations; no arbitrary scan/launch; process identity and version checked; disconnect cleans session without terminating user-owned Blender. |
| **5. Read-only scene inspection in AI Chat** | Expose minimal scene/selection facts in the existing chat task context. | Typed scene summary, stale-revision detection, capability permission, chat status/progress/error rendering. No separate Blender workspace. | Real Windows Blender sessions return only approved bounded facts; file/scene data stays local unless provider consent says otherwise; no mutation path. |
| **6. Provider-neutral planning and structured action protocol** | Convert user requests into reviewable typed plans; enforce capability, schema, permission, and provider checks before any action. | Versioned action DSL, planner/provider capability adapter, plan confirmation/diff UI in chat, denial and consent receipts, action validation tests. | Invalid/arbitrary script payloads rejected; local model works without cloud key for supported text tasks; lack of capability blocks honestly; no silent paid-cloud fallback. |
| **7. Procedural scene creation** | Implement the first constrained Blender API actions for objects, collections, transforms, cameras, lights, simple materials, furniture/props, and bounded environment layouts. | Reviewed add-on action handlers on Blender main thread, action budgets, progress/cancellation, scene verification, undo/checkpoint path. | Windows end-to-end test with real Blender; safe cancellation and rollback; complexity/resource caps; representative `.blend` remains editable; explicit user approval before writes. |
| **8. Existing-scene editing, revisions, save and export** | Apply user-requested revisions to current scenes; support approved save/export workflows. | Stable object identity strategy, base-scene revision/conflict rules, scoped file picker, `.blend` save recovery/checkpoints, candidate format exporters and validation. | Manual Blender edits cause stale plan/re-plan; overwrites confirmed; failures preserve original; export formats tested per supported build; successful outputs verified from disk and reopened. |
| **9. Reference-image import and analysis** | Support user-selected reference images and, only with a capable model, describe/compare them for a scene task. | Safe image selection/validation, scene reference placement, provider modality checks, local vision capability or separate explicit cloud consent UX. | Local/offline path proven on supported model, or cloud data/charge consent independently reviewed. Importing is not represented as analysis; no image leaves device without explicit consent. |
| **10. Image-to-3D and specialized asset generation** | Evaluate a compatible 3D reconstruction/generation model or approved provider as an optional workflow. | Runtime/model provenance and license dossier, complete manifest, integration design, measured Windows workload profile, approximation/quality checks, opt-in workflow. | Separate model/runtime/security approval before download; representative outputs validated; memory/disk limits and cancellation proved; communicate ambiguity and approximation. If no compliant, compatible path exists, mark capability unsupported rather than ship a stub. |
| **11. Supported animation, preview and resource coordination** | Extend to explicitly supported object/rig animation, previews/renders, and shared resource constraints. | Bounded keyframe/animation action types; rig prerequisites where tested; viewport/render preview flow; Blender workload profile and admission coordination. | Test only named object/rig types and scenarios; no claim of general rigging/mocap; resource measurements and concurrent AI/render behavior verified across supported hardware. |
| **12. Security, Windows qualification and staged release** | Complete end-to-end verification, hardening, docs/support, and rollout decision for capabilities actually validated. | Threat-model review, fuzz/adversarial tests, Windows clean-install/update/uninstall checks, signed add-on/app payload provenance, supported matrix, support diagnostics, release/rollback plan. | Exact Windows builds and Blender versions pass; no open critical/high issues; licenses approved; all advertised capabilities have genuine operation evidence. Owner separately authorizes release. |

## Dependency order and scope guards

```text
1 Architecture
  -> 2 Feasibility evidence
  -> 3 Generic integrations foundation
  -> 4 Blender connection
  -> 5 Read-only chat inspection
  -> 6 Validated planner/consent boundary
  -> 7 Procedural creation
  -> 8 Revisions/save/export
  -> 9 Reference image analysis (multimodal prerequisite)
  -> 10 Image-to-3D (separate model/provider gate)
  -> 11 Animation/preview/resource support
  -> 12 Windows security qualification and release decision
```

- Phases 9–11 are optional capability tracks, not prerequisites for honest text-driven procedural creation unless implementation dependencies prove otherwise.
- No existing core or media roadmap phase is renumbered, edited, or inferred complete by this proposal.
- No billing, subscription, entitlement, monetization, or provider behavior is changed. External inference remains separately consented and is not a fallback.
- Connector manifests never authorize arbitrary executable plugin loading. Any third-party connector package requires a separate signature, sandbox, revocation, and supply-chain decision.
- Linux source inspection or frontend success cannot clear Windows gates. A real application connection, scene operation, file artifact, cancellation result, and supported hardware/version must be evidenced for each capability.

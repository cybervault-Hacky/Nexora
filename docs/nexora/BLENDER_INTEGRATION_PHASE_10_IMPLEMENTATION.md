# Nexora Blender Integration — Phase 10 Workflow Orchestration Foundation

**Date:** 2026-10-26
**Status:** Phase 10 contract, deterministic planner, preview summary, and focused tests authored. Rust compilation/tests are unverified because the Rust toolchain is unavailable; UI typechecking is blocked because TypeScript is not installed in this checkout.
**Scope:** Bounded review/planning orchestration over existing Phase 6–9 truthful capabilities. No executor, Blender transport, execution UI, or Phase 11 work.

## Phase alignment and scope

This report documents only the explicitly authorized Phase 10 task. The existing proposed 12-phase outline is unchanged even though its phase mapping differs; no roadmap or historical report was reconciled or renumbered. Phase 5 production transport remains NO-GO, Phase 6 execution remains unavailable, Phase 7 reference association does not provide vision, and Phases 8–9 remain planning foundations. No live Blender connection, snapshot provider, or approved execution workflow exists.

## Summary

`workflow_orchestration.rs` adds a strict version-1 typed workflow contract and pure planner. It accepts a supplied Phase 8 `ValidatedSceneSnapshot` and bounded Phase 7 image-reference metadata, then delegates edit and animation plans to the existing Phase 8 and Phase 9 planners. Its four supported step kinds are:

1. summarize a supplied validated scene snapshot (read-only; requires `SceneInspect`);
2. plan a Phase 8 scene edit (read-only summary or validated edit candidate);
3. plan a Phase 9 bounded animation; and
4. associate a valid Phase 7 image reference with the user's task text without analyzing the image.

Each output is a preview of proposed ordered steps. Dependencies are review/planning relationships, not evidence that an earlier step ran, was applied, or changed the snapshot. Planning steps are independently evaluated against the supplied snapshot; their proposed outputs are not composed into later inputs. A dependency on invalid, blocked, or connector-unavailable work becomes blocked rather than being promoted to a successful result.

## Versioned contract and bounds

The Rust DTOs are in `apps/desktop/src-tauri/src/integrations/workflow_orchestration.rs`; deserialization uses camelCase fields, strict unknown-field rejection, and a tagged allowlisted step enum. Unknown kinds, malformed fields, and unsupported versions fail closed.

- Contract/schema version: **1**.
- Serialized workflow request: **32 KiB** maximum, checked for JSON input and typed planner input.
- Workflow and step IDs: nonempty safe identifiers, at most **64 characters**; step IDs are unique.
- Task description: nonblank, at most **2,048 Unicode scalar values**, no control characters.
- Step count: **1–8**; each step may have at most **7** dependencies.
- Dependencies must be unique, reference known step IDs, and form an acyclic graph. Stable topological ordering chooses the lowest original request index among currently ready steps.
- Optional scene reference and expected scene revision are validated safe identifiers. A supplied expected revision requires a scene reference.
- Supplied Phase 7 references: at most **4**; every metadata record is validated, at most **4 KiB** serialized, with optional description at most **1,024 characters**. Image IDs must match the existing 32-character hexadecimal reference form.
- Serialized plan preview: at most **128 KiB**.
- Snapshot bounds and Phase 8/9 nested request/candidate bounds remain enforced by their existing validated types and planners.

Contract-level structural errors reject the input. Semantic step errors are represented in the returned preview so reviewers can see invalid and blocked steps; such a preview is not an accepted or executable workflow.

## Deterministic planning, ordering, and dependency behavior

The planner validates workflow structure and all supplied reference metadata before step planning, computes a deterministic topological order, then validates each step against its existing contract. The preview preserves original request order whenever dependency constraints allow it and includes each step's explicit `dependsOn` list.

- Invalid or blocked dependency → dependent step is `blocked` with `dependency_failed`.
- Valid dependency whose execution state is `connector_unavailable` → dependent step is `blocked` with `dependency_unavailable`.
- A dependency on a supplied-snapshot or image-reference summary (no execution requested) does not by itself block a later planning step.
- A dependent step does not receive fabricated output from an upstream proposal. Each scene-oriented step uses the same supplied validated snapshot.
- Duplicate, missing, malformed, excessive, or cyclic dependencies are rejected at the workflow-contract boundary.

The root plan state is `invalid`, `blocked`, `awaiting_confirmation`, or `planned`, in that precedence order. Step states carry the corresponding step-level status. Execution is a separate axis: the workflow and each step can report only `not_requested` or `connector_unavailable`. There is no Phase 10 `executed` status variant, result DTO, or execution path.

## Phase 6–9 integration, permissions, and confirmation

- **Phase 6:** scene candidates are produced and validated through the existing allowlisted Phase 6 parser and descriptors. Read-only scene operations carry `SceneInspect`; mutations carry `SceneModify`. Phase 6 executor behavior is untouched and no executor is called.
- **Phase 7:** image association accepts only validated metadata and the user's written task. The output records `analysisAvailable: false`; it does not inspect bytes, infer contents, or claim model output.
- **Phase 8:** edit planning delegates to `plan_scene_edit`, reusing its target, candidate, snapshot, permission, and confirmation policy checks. Mutating workflow steps additionally require a supplied, matching scene revision so confirmation scope cannot silently omit revision provenance. The target and validated candidate remain part of the plan binding.
- **Phase 9:** animation planning delegates to `plan_animation`; animation is a mutation and inherits `SceneModify` and explicit confirmation policy. It likewise requires the workflow's expected revision to match the supplied validated snapshot revision.
- **Confirmation binding:** a non-serializable in-memory `WorkflowConfirmationBinding` compares the full preview (including scene reference, expected revision, step targets, and policies) and exact Phase 6-validated candidate batches. It is available only when the whole workflow is valid and awaiting confirmation. Replanning with a changed plan does not match an earlier binding. The binding is a comparison snapshot, not a consent grant or authorization; a host must still obtain explicit user consent. A workflow request is not consent.
- **No execution:** no action queue, mock result, Python/shell/script path, Blender connector, or Apply/Run/Execute control was added. Nothing claims a scene, timeline, image, or render changed.

## UI-side summary helper

`apps/desktop/ui/src/features/integrations/blenderWorkflowPlanning.ts` mirrors the bounded preview-facing contract and formats a safe, read-only summary. It preserves planner step order, displays dependency/error/status information and revision context, says when confirmation is ungranted, and states that no step executed. It intentionally has no execution status value for `executed` and exposes no control or command. This helper is not wired into chat or a separate Blender Studio page; existing chat, model routing, App Integrations, and Phase 7 behavior remain unchanged.

## Files added/updated for Phase 10

- `apps/desktop/src-tauri/src/integrations/workflow_orchestration.rs` — v1 contract, bounds, deterministic planner, Phase 6 candidate retention, status/error model, dependency propagation, and plan-bound comparison binding.
- `apps/desktop/src-tauri/src/integrations/workflow_orchestration_tests.rs` — focused Rust tests authored for ordering, propagation, mutation policies/binding invalidation, image-reference honesty, contract errors, revisions, and bounds.
- `apps/desktop/src-tauri/src/integrations/mod.rs` — registers `workflow_orchestration`.
- `apps/desktop/ui/src/features/integrations/blenderWorkflowPlanning.ts` — preview contract mirror and review-only summary formatter.
- `apps/desktop/ui/src/features/integrations/blenderWorkflowPlanning.test.ts` — focused summary/bounds tests.
- `docs/nexora/BLENDER_INTEGRATION_PHASE_10_IMPLEMENTATION.md` — this implementation report.

All other existing modifications and untracked work were preserved. The roadmap, earlier phase reports, and unrelated UI/backend areas were not intentionally changed for this task.

## Checks actually performed

- Focused Vitest invocation for `blenderWorkflowPlanning.test.ts`, `blenderScenePlanning.test.ts`, and `blenderAnimationPlanning.test.ts` — **3 files, 6 tests passed** using the available `npx`-resolved Vitest **5.0.5**. Project-pinned Vitest resolution was unavailable through npm in this environment.
- `git diff --check` — **passed** for tracked diffs after final edits; a supplemental whitespace/newline scan also **passed** for all six Phase 10 files, including untracked files.
- `npm run typecheck` — **blocked**, `tsc: not found`; project dependencies are not installed in this checkout. No UI typecheck pass is claimed.
- `cargo`, `rustc`, and `rustfmt` — **not installed**. The Rust tests were authored but not compiled or executed, and no Rust formatting/parser check is claimed.
- No Blender, Windows runtime, snapshot provider, or approved connector exists here. Full regression, Windows checks, and integration validation remain deferred to the roadmap's final verification stage.

## Limitations and final-verification deferrals

This foundation does not inspect a live scene, read or edit a timeline, obtain genuine revision provenance, analyze an image, apply or queue mutations, request consent, execute actions, or report Blender results. The comparison binding is groundwork for a future host, not a finished authorization flow. Phase 10 Rust compilation/tests, Rust formatting, UI typechecking, full regression, Windows validation, and connector-level permission/confirmation integration remain unverified and must be completed during final roadmap verification with the required toolchain and an approved real execution workflow. No Phase 11 or later work is started.

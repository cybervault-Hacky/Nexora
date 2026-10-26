# Nexora Blender Integration — Phase 9 Animation Workflows Foundation

**Date:** 2026-10-26
**Status:** Transport-independent contracts and planner added; Rust compilation/tests remain unverified because Cargo is unavailable.
**Scope:** Bounded object translation, rotation, and scale animation intent only. No Blender timeline access, keyframe creation, rendering, or execution.

## Phase alignment note

This report documents the specifically authorized Phase 9 animation-foundation task. The existing proposed `BLENDER_INTEGRATION_12_PHASE_PLAN.md` is unchanged; it currently lists reference-image analysis under Phase 9 and animation under Phase 11. No roadmap or historical phase numbering was edited or reconciled here.

## Summary

Phase 9 introduces a strict, versioned animation-request DTO and a deterministic planner that consumes a Phase 8 `ValidatedSceneSnapshot`. The planner resolves an explicitly supplied object reference, validates revisions and all timing/channel values, and creates exactly one additive Phase 6 `blender.animate_object` candidate containing two endpoint keyframe intents. The Phase 6 parser validates the candidate again.

The result is informational: it includes the target ID/name, channel, units, inclusive frame range, integer frame rate, interpolation, exact endpoint keyframes, validation and permission metadata, and limitations. Every animation is a mutation requiring `SceneModify` and explicit confirmation; the plan is `awaiting_confirmation`. Execution remains `connector_unavailable`. No Phase 9 path emits `executed`, and no plan claims that a keyframe, animation, or render exists.

## Animation request contract

`apps/desktop/src-tauri/src/integrations/animation_planning.rs` defines schema version 1 `AnimationRequest` with:

- `schemaVersion`, `requestId`, target `sceneReference`, optional `expectedSnapshotRevision`;
- optional `target` as a Phase 8 stable object ID or exact object name;
- `operation`: `translation`, `rotation`, or `scale`;
- `startFrame`, `endFrame`, `frameRate`, and allowlisted `interpolation`;
- required typed `startValue` and `endValue` vectors; and
- optional bounded `description`.

The request and its nested vectors/target reject unknown fields. Unknown operations/interpolation values, malformed/duplicate fields, missing required values, and incorrectly typed fields fail parsing. A target is optional in the DTO to represent an incomplete user request, but the planner returns `missing_target` rather than inventing an object or ID. Start and end vectors are explicit local-channel endpoint values; the planner does not infer an initial value from a live Blender object or the snapshot.

### Units and timing semantics

| Operation | Endpoint value interpretation | Bounds per component |
|---|---|---|
| Translation | Local object coordinates in Blender scene units | `[-10,000, 10,000]` |
| Rotation | Local Euler angles in radians | `[-8τ, 8τ]` radians |
| Scale | Dimensionless positive local scale factors | `[0.001, 1,000]` |

Frames are one-based integers. The range is inclusive. The planner requests exactly two keys—one at `startFrame`, one at `endFrame`—and the selected interpolation applies between them; it does not bake intermediate samples. Frame rate is an integer frames-per-second value from **24, 25, 30, 48, 50, or 60**. Fractional frame rates and other frame-rate values are unsupported in this contract.

### Explicit bounds

- Animation request JSON: **8 KiB** maximum.
- Optional description: at most **1,024 Unicode scalar values**, nonblank if present, with control characters rejected.
- Frame numbers: **1 through 100,000**, with `startFrame < endFrame`.
- Inclusive frame count: at most **2,500 frames** (`endFrame - startFrame + 1`).
- Endpoint keyframe count: exactly **2** by schema; no caller-controlled keyframe array or larger sequence is accepted.
- Frame rates: exact allowlist above.
- Interpolation: `linear`, `bezier`, or `constant` only.
- Translation/rotation/scale limits reuse the Phase 6 action-contract bounds shown above.
- The generated candidate is one action and is capped by the existing **32 KiB** Phase 6 batch parser; the shared Phase 6 parser still enforces the maximum of **16 actions** for any batch.

JSON numeric parsing rejects malformed, non-finite, or incorrectly typed values. The action parser repeats the frame, frame-rate, interpolation, vector, and channel-specific range checks so the candidate boundary cannot rely only on the higher-level planner.

## Target and stale-reference behavior

Planning accepts only a supplied validated Phase 8 snapshot; it does not inspect Blender, installation state, the active timeline, or a file path. The scene reference must match exactly. The target is resolved using Phase 8's object-ID and exact-name rules. Missing IDs/names and duplicate exact-name matches return typed errors. Because an incomplete/truncated snapshot cannot prove name uniqueness, animation-by-name is rejected with `incomplete_snapshot_for_name_resolution`; stable-ID resolution is limited to IDs actually present in the validated snapshot. No target object or stable identifier is fabricated.

If the snapshot has a revision, animation requests must provide the expected revision. A mismatch returns `stale_snapshot`; a matching value is recorded in the preview. If no source revision exists, the planner cannot establish freshness and records that limitation. Snapshot capture time is not treated as a substitute for a revision or proof of currentness.

The planner validates the request before constructing any candidate. The shared Phase 8 scene-scope helper enforces scene matching, stale-revision rejection, and the required revision for revisioned mutations; its exact object-target resolver is reused. Snapshot validation limits and completeness markers remain authoritative.

## Planner, results, permission, and confirmation

The Phase 6 allowlist gains one additive action ID, `blender.animate_object`, within action contract version 1. Its strict parameter schema contains object ID, operation, start/end frame, frame rate, interpolation, and two typed endpoint values. It is classified as a mutation, requires `SceneModify`, and uses `ExplicitUserConfirmation`. A typed animation result shape is declared only as a future schema; no current path constructs or returns it. No existing action validation, permission, or executor behavior is weakened.

The planner serializes one candidate action and passes it through `parse_candidate_json`; it then builds the preview from the validated action and Phase 6 descriptor. The candidate remains a `ValidatedBatch`, not an `AuthorizedBatch`; Phase 9 grants no consent and never calls the executor.

The plan carries separate lifecycle fields:

- `plan_created` / `awaiting_confirmation` for review state (animation plans are `awaiting_confirmation` because all supported operations mutate the scene);
- `connector_unavailable` / reserved `executed` for execution outcome (Phase 9 emits only `connector_unavailable`).

Typed errors include snapshot unavailable/invalid, stale snapshot, scene mismatch, missing expected revision, invalid request, missing target, target not found, ambiguous target, incomplete-snapshot name resolution, invalid frame range, unsupported frame rate, invalid channel values, and invalid Phase 6 candidate. Error paths do not echo arbitrary user values.

A future confirmation workflow must bind approval to the exact plan and scene/revision as well as the exact action batch. No animation queue, implicit approval, silent deferred execution, executable Python, or code-generation field is present.

## Chat and Phase 6–8 integration

- `animation_planning.rs` is registered in the existing integration module. It remains transport-independent and is not exposed as a Tauri command.
- `blender_actions.rs` adds the minimal allowlisted animation action and its strict validation, bounds, and Phase 6 policy metadata. The existing `UnavailableBlenderExecutor` is unchanged and still returns `connector_unavailable`.
- `scene_planning.rs` exposes crate-private scene/revision and object-resolution helpers so animation planning reuses Phase 8 rules rather than duplicating a weaker resolver. Existing Phase 8 behavior is preserved.
- TypeScript contract mirrors and a safe chat-summary formatter are in `ui/src/features/integrations/blenderAnimationPlanning.ts`, with a focused preview test. The formatter labels the candidate as a plan, summarizes both keyframes, and explicitly says nothing was created/applied/rendered.
- No current snapshot provider or Tauri animation-planning command exists, so the helper is not wired into chat and no animation controls were added. Existing chat/model routing, App Integrations UI, and Phase 7 local PNG reference handling are unchanged. No image analysis is claimed.

## Files created or modified

- `apps/desktop/src-tauri/src/integrations/animation_planning.rs` — v1 request/preview/error DTOs, bounds, snapshot-scoped planner, candidate validation, and test-module registration.
- `apps/desktop/src-tauri/src/integrations/animation_planning_tests.rs` — focused Rust planner tests for operation types/units, endpoints, status/policy, target resolution, snapshot/revision outcomes, and bounds.
- `apps/desktop/src-tauri/src/integrations/blender_actions.rs` — additive `blender.animate_object` contract, allowlisted enums, finite/range/frame validation, and mutation/confirmation metadata.
- `apps/desktop/src-tauri/src/integrations/scene_planning.rs` — crate-private shared scene/revision and object-target helpers used by both planners.
- `apps/desktop/src-tauri/src/integrations/mod.rs` — registers `animation_planning`.
- `apps/desktop/ui/src/features/integrations/blenderAnimationPlanning.ts` — TypeScript contract mirror and informational preview formatter.
- `apps/desktop/ui/src/features/integrations/blenderAnimationPlanning.test.ts` — formatter safety test.
- `docs/nexora/BLENDER_INTEGRATION_PHASE_9_IMPLEMENTATION.md` — this report.

Historical Phase 6–8 reports, the proposed 12-phase plan, chat/model routing, App Integrations behavior, and Phase 7 image-reference behavior were not edited.

## Implemented versus connector-dependent

**Implemented:** strict typed request and action contracts; bounded deterministic validation; two-key plan construction; Phase 8 scene/object/revision checks; Phase 6 permission/confirmation metadata; human-readable preview data and formatter; focused source tests.

**Not implemented or verified:** Blender connection, timeline or frame-rate inspection, supported Blender version compatibility, actual keyframe insertion, scene revision acquisition, approval UI, Blender operation results, viewport preview, rendering, or video export. No claim of Created, Applied, Rendered, Connected, Ready, or Executed is made.

## Checks actually performed and blockers

- `corepack pnpm --dir apps/desktop typecheck` — **passed**.
- Focused Vitest for `blenderAnimationPlanning.test.ts`, `blenderScenePlanning.test.ts`, `AppIntegrationsPage.test.tsx`, and `imageTo3dTask.test.ts` — **4 files, 12 tests passed**.
- The new/modified Rust files were formatted with the repository-installed Rust WASM formatter. This confirms formatting/parser acceptance only; it is not Rust type checking or compilation.
- Focused Cargo test command — **blocked: `cargo` is not installed**. Rust tests were authored but not compiled or executed; no Rust build pass is claimed.
- No Blender/Windows runtime, genuine snapshot source, timeline read, or approved transport is available. No live animation behavior was tested.
- Full regression and cross-platform verification were intentionally left for the roadmap's final verification phase.

## Deferred to final verification

Run Rust compilation and the new Phase 6/9 unit tests with an available toolchain; perform full UI/cross-platform regression; review action contract compatibility; validate the selected integer FPS/interpolation semantics against a future approved Blender connector; prove real stable-ID/revision provenance and consent binding; and complete the roadmap's final security/readiness verification. No Phase 10 or later work is started here.

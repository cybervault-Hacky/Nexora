# Nexora Blender Integration — Phase 8 Implementation

**Phase:** 8 — Existing Scene Inspection & Editing Foundation
**Status:** Implementation foundation added; Rust compilation/tests remain unverified in this environment.
**Scope boundary:** No Blender transport, bridge/add-on, scene access, execution, image analysis, or later-phase work.

## Summary

Phase 8 adds versioned, bounded DTOs for a future read-only scene snapshot and typed edit request, plus a transport-independent planner that resolves requests only against a validated supplied snapshot. Supported intents are read-only scene summary, rename an existing object, set an existing object's transform, and create a primitive at a specified transform/location. Targets can be selected by stable object ID or exact name; name targets must be unique in the supplied snapshot.

The planner converts each supported intent into one Phase 6 candidate action and sends it through the existing Phase 6 parser. It does not authorize or execute the resulting candidate. Mutations are marked as awaiting explicit confirmation; every plan reports `connector_unavailable`. No Phase 8 code emits `executed`.

## Contracts and bounds

`integrations/scene_planning.rs` defines schema version 1 contracts:

- `SceneSnapshot`: opaque scene reference, display name, optional source revision, capture timestamp, completeness/truncation and reported counts, object records, and collection records.
- `SceneEditRequest`: schema version, request ID, scene reference, user request text, optional expected snapshot revision, and one tagged typed intent. The intent's operation classification and permissions are derived by the planner rather than trusted from caller-supplied policy fields.
- `SceneEditPlanPreview`: the typed request, Phase 6 action/class, validation result, required permission, confirmation policy, plan/execution status, snapshot metadata, proposed action parameters, and limitations.
- `SceneSnapshotState` / `ScenePlanningErrorCode`: represent unavailable, invalid, stale, and schema-valid snapshot outcomes. A schema-valid snapshot is not a claim that its source is genuine or current.

Validation is strict: unknown fields and malformed/duplicate typed fields fail parsing; snapshot and request JSON are size-capped (256 KiB and Phase 6's 32 KiB limit respectively); collections, strings, identifiers, reported counts, per-object memberships, timestamps, and transforms are bounded. Snapshot collection limits are 1,000 objects and 256 collections, with at most 32 collection references per object. Transform numeric ranges use Phase 6's existing bounds. A complete snapshot must reconcile its reported/included counts and all supplied hierarchy/collection relationships; parent cycles, duplicate IDs/memberships, invalid transforms, and invalid references are rejected. Incomplete or truncated data remains explicitly marked and does not cause omitted objects to be invented.

## Resolution and staleness

Planning requires a caller-supplied validated snapshot. It never checks Blender installation to infer scene availability and has no local-path or filesystem lookup. Scene references must match exactly. Object-ID targets resolve only to an ID present in the supplied snapshot. Name targets use an exact match and fail as `target_not_found` or `ambiguous_target` rather than guessing.

When an expected revision is supplied, it must match the snapshot revision; otherwise planning returns `stale_snapshot`. Mutations against a revisioned snapshot require the request to carry the expected revision. If the source provides no revision, the planner cannot prove freshness; the preview records that limitation. Capture time is validated as bounded metadata only—it is not treated as proof of freshness, provenance, or a TTL check.

## Permissions, confirmation, planner and results

The planner constructs one typed Phase 6 candidate and re-validates it using `parse_candidate_json`, preserving Phase 6's supported action set and parameter bounds. It takes class, permission, and confirmation policy from the parsed Phase 6 descriptor. Read-only summary plans require `SceneInspect` and have no mutation confirmation. Rename, transform, and primitive creation plans require `SceneModify` and are marked `awaiting_confirmation` under Phase 6's explicit confirmation policy. Phase 8 does not create an authorization context or confirmation grant.

The result distinguishes `plan_created` and `awaiting_confirmation` from the executor outcome `connector_unavailable`; `executed` is reserved as a future status only. A typed candidate is not an `AuthorizedBatch`, and no Phase 8 path calls the executor. Planning errors distinguish unavailable, invalid, stale, scene mismatch, invalid request/candidate, missing revision, missing target, and ambiguous target. Error paths are static/index-based and do not echo untrusted values.

A later host-owned confirmation workflow must bind consent to the exact plan and scene reference/revision, not merely to a reusable action payload. That workflow is intentionally not implemented here.

## Phase 6/7 and application integration

- Phase 6's `PrimitiveType`, `Vector3`, and `ObjectTransform` gained only the Serde deserialization needed by the strict snapshot/request DTOs. Existing action validation, permission policy, and `UnavailableBlenderExecutor` behavior are unchanged; the executor still returns `connector_unavailable`.
- Phase 7 local PNG reference preparation and chat/model/file behavior are unchanged. No image understanding or reference-to-scene inference was added.
- `integrations/mod.rs` registers the planner in the existing integration architecture.
- The UI receives matching TypeScript contract types and a safe informational preview formatter in `ui/src/features/integrations/blenderScenePlanning.ts`. A focused formatter test verifies the proposed change, confirmation language, unavailable execution, and the absence of an execution-success claim.
- No separate Blender chat/studio page or Apply/Execute control was added. The current application has no genuine snapshot source or Tauri planning command to feed a truthful live-scene preview. The helper is not connected to chat yet; surfacing a fabricated snapshot or a plan for an uninspected scene would be misleading. Existing App Integrations availability messaging remains unchanged.

## Files

- `apps/desktop/src-tauri/src/integrations/scene_planning.rs` — strict snapshot/request contracts, validation, state assessment, target resolution, revision checks, candidate construction, planning metadata, and focused Rust tests.
- `apps/desktop/src-tauri/src/integrations/blender_actions.rs` — minimal Serde derives on Phase 6 input data types; no executor or policy behavior change.
- `apps/desktop/src-tauri/src/integrations/mod.rs` — registers `scene_planning`.
- `apps/desktop/ui/src/features/integrations/blenderScenePlanning.ts` — TypeScript contract mirrors and informational preview formatter.
- `apps/desktop/ui/src/features/integrations/blenderScenePlanning.test.ts` — formatter safety test.
- `docs/nexora/BLENDER_INTEGRATION_PHASE_8_IMPLEMENTATION.md` — this report.

## Genuine capabilities and limitations

Implemented capability is limited to validating supplied data and producing a Phase 6-validated candidate/preview in code. There is no genuine Blender connection, snapshot provider, object-ID provenance, revision source, read operation, edit, or execution. Installation detection is not used as a scene signal. No snapshot is fabricated, persisted, or logged by this planner. The TypeScript helper is not a transport or IPC endpoint.

## Checks and blockers

- `corepack pnpm --dir apps/desktop typecheck` — passed.
- Focused Vitest for `blenderScenePlanning.test.ts` — 1 test passed.
- Focused Vitest for scene planning and `AppIntegrationsPage.test.tsx` — 3 tests passed.
- Focused Vitest for image-to-3D task and App Integrations coverage — 8 tests passed; this is a preservation check, not a full regression suite.
- `git diff --check` — passed.
- The new Rust file was formatted with the repository-installed Rust WASM formatter. This is formatting/parsing assistance only, not a Rust build.
- `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --lib integrations::scene_planning::tests` — blocked: `cargo` is not installed. Rust type checking and Rust tests therefore remain unverified; no successful compile is claimed.
- No Blender runtime, Windows environment, or real connector/snapshot is available. No live-scene or execution verification is claimed.

## Deferred to final verification

Full Rust build/test execution with an available toolchain; full UI and cross-platform regression; review of snapshot provenance and revision guarantees against a genuine future read-only source; and final roadmap readiness checks. No later implementation phase is started here.

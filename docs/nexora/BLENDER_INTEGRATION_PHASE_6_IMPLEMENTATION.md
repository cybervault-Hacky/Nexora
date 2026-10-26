# Blender Integration Phase 6 — AI-to-Blender Command Engine Foundation

**Status:** Transport-independent action contract, validation, policy, and disabled executor boundary added.

**Blender execution status:** **Not implemented or verified.**

**Transport status:** Phase 5 incomplete; Phase 5A remains **NO-GO FOR PRODUCTION**; Phase 5B validation preparation does not change that decision.

**Date:** 2026-10-26

## 1. Scope and result

Phase 6 adds a Rust-only, transport-independent foundation for validating structured candidate actions and making future authorization decisions. It does **not** add a natural-language planner, connector, Blender bridge/add-on, Tauri command, transport, confirmation UI, or execution path to Blender.

The module is not registered with the renderer or exposed as an IPC command. Candidate installation state remains candidate-only. No Blender result, Connected state, Ready state, or action success can be produced by this work.

## 2. Action contract

Implementation: `apps/desktop/src-tauri/src/integrations/blender_actions.rs`, included by `apps/desktop/src-tauri/src/integrations/mod.rs`.

The current contract version is **1**. Each batch and each action envelope carries that version. Action identifiers are stable namespaced strings. Every parameter object is strict: required fields must be present, only explicitly documented optional fields are permitted, and unknown fields are rejected. A typed `ValidatedAction` can only be obtained after the full batch passes validation.

| Action ID | Classification / permission | Strict parameter schema | Future result schema | Confirmation |
|---|---|---|---|---|
| `blender.get_scene_summary` | Read-only / `SceneInspect` | `{}` | `SceneSummaryResult`: bounded scene name, object count, collection count | None beyond permission grant |
| `blender.list_objects` | Read-only / `SceneInspect` | Optional `collection_name` (1–64 Unicode scalar values); optional integer `limit` (1–1000), default 100 | `ObjectListResult`: bounded list of typed object summaries plus `truncated` flag | None beyond permission grant |
| `blender.create_primitive` | Mutation / `SceneModify` | Required `primitive` enum (`cube`, `uv_sphere`, `cylinder`, `cone`, `torus`, `plane`), `location`, `rotation`, `scale`; optional `name` (1–64 characters) | `MutationResult` under `CreatedObject` result variant | Explicit user confirmation for the exact validated batch |
| `blender.set_object_transform` | Mutation / `SceneModify` | Required bounded opaque `object_id`, `location`, `rotation`, `scale` | `MutationResult` under `UpdatedObject` | Explicit user confirmation for the exact validated batch |
| `blender.rename_object` | Mutation / `SceneModify` | Required bounded opaque `object_id` and `new_name` (1–64 characters) | `MutationResult` under `RenamedObject` | Explicit user confirmation for the exact validated batch |

No delete/destructive operation is currently in the allowlist. The policy contract has a separate `DestructiveConfirmation` level for future action descriptors; adding such an action requires a separate reviewed schema and test. No action described here is claimed to have a Blender handler.

Example of a **candidate contract payload**, not an instruction sent to Blender:

```json
{
  "contract_version": 1,
  "request_id": "chat-task-001",
  "actions": [
    {
      "action_id": "blender.create_primitive",
      "contract_version": 1,
      "parameters": {
        "primitive": "cube",
        "name": "Block",
        "location": [0, 0, 0],
        "rotation": [0, 0, 0],
        "scale": [1, 1, 1]
      }
    }
  ]
}
```

### Numeric and size limits

- Candidate JSON: at most **32 KiB** before parsing; the parsed-value entry point also enforces a serialized size bound.
- Batch: **1–16** actions; empty and oversized batches are rejected.
- Object name: **1–64 Unicode scalar values**, non-blank and no control characters.
- Object ID and request ID: **1–64 characters** using `[A-Za-z0-9._:-]`; an object ID must begin with an ASCII alphanumeric character. IDs do not accept path separators or arbitrary code.
- Scene locations: each component in **[-10,000, 10,000]**.
- Euler rotations: each component in **[-8π, 8π] radians**.
- Positive per-axis scale: each component in **[0.001, 1,000]**; zero and negative scale are invalid.
- `list_objects.limit`: integer **1–1,000**, default **100**.
- Transform vectors must be arrays of exactly three numeric components. Numbers must parse as finite values and satisfy the relevant bounds.

The exact limits are conservative contract bounds for this parser, not a promise that Blender supports every value/version in that range. Any future executor must apply its own app/version/resource constraints.

### Result schemas (types only)

The module declares `SceneSummaryResult`, `ObjectSummary`, `ObjectListResult`, `MutationResult`, and the tagged `ActionResult` union. These are future response shapes, not observed connector responses. In particular, this phase does not instantiate an `ActionResult` as a result of a Blender operation. List output is bounded by the contract limit and can indicate truncation.

## 3. Deterministic parser and validation errors

`parse_candidate_json` is the untrusted-byte entry point. It:

1. Rejects input over the byte cap before parsing.
2. Parses a strict JSON tree that rejects duplicate object keys.
3. Validates exact top-level, action-envelope, and action-parameter fields.
4. Checks batch/action contract versions and supported action IDs.
5. Validates types, vectors, enums, identifiers, names, limits, and numeric finiteness/ranges.
6. Returns `ValidatedBatch` only after **every** action passes.

`validate_candidate_value` is available for already-parsed values and repeats schema/size validation. External serialized input should use `parse_candidate_json`, because a generic `serde_json::Value` cannot retain duplicate-key information.

Errors use the typed `ValidationErrorCode` and a stable JSON path; untrusted values and unknown field names are not copied into error output. Codes include `malformed_json`, `payload_too_large`, `invalid_type`, `missing_field`, `unknown_field`, `unsupported_contract_version`, `unknown_action`, `empty_batch`, `too_many_actions`, `invalid_request_id`, `invalid_identifier`, `invalid_string`, `string_too_long`, `invalid_enum`, `invalid_vector_length`, `invalid_number`, `non_finite_number`, and `out_of_range`.

Non-JSON numeric tokens such as `NaN`/`Infinity`, duplicate keys, malformed JSON, and out-of-range JSON numeric representations are rejected at the parser boundary. No candidate string is interpreted as Python, shell, Blender operator text, or unrestricted code.

## 4. Permission and confirmation policy

Each validated action exposes a typed `ActionDescriptor`:

- The two query actions are `ReadOnly`, require `SceneInspect`, and do not require mutation confirmation.
- Primitive creation, transform changes, and renaming are `Mutation`, require `SceneModify`, and require explicit user approval of the exact validated batch.
- A future destructive action must use the stronger destructive-confirmation requirement; none is currently accepted by the parser.
- `authorize_batch` first requires a connector state of `Approved`, then checks every action's permission and confirmation requirement. It returns an `AuthorizedBatch` only if the whole batch passes; no partial authorization result is returned.
- Confirmation policy compares against a host-owned snapshot of the exact `ValidatedBatch`, not only a reusable request ID. This current data structure is a policy foundation, not a confirmation dialog or durable consent store.
- `request_id` is mandatory and bounded as a future idempotency key. No executor exists to deduplicate requests yet. A future executor must bind idempotency to the validated payload, reject key reuse with different actions, and avoid retrying a mutation after uncertain completion.
- An unavailable or unapproved connector denies authorization. A renderer/model must not be allowed to set connector approval, permissions, or confirmation state.

No confirmation dialog, permission store, connector registry, grant persistence, or user-consent receipt is implemented. The policy types are not wired to Tauri or UI.

## 5. Disabled execution boundary

`BlenderActionExecutor` accepts only an `AuthorizedBatch`; it cannot be called with raw candidate JSON. The sole implementation in this phase, `UnavailableBlenderExecutor`, always returns the typed `ExecutionError::ConnectorUnavailable` (`connector_unavailable`). It does not queue, retry, launch, communicate, or create a success receipt.

The future `ExecutionReceipt`/`ActionResult` types define an output contract only. No current implementation can produce a successful execution receipt. Tests assert the default executor returns the explicit error, not success. This is a disabled boundary, not a placeholder Blender connection.

## 6. Tests and checks

Focused Rust unit tests are embedded in `blender_actions.rs` and exercise actual parser/policy/executor logic, including:

- accepting all five action contracts;
- unknown action and unsupported batch/action versions;
- missing/extra fields and incorrect types;
- numeric bounds, invalid scale, invalid vector dimensions, and primitive enum constraints;
- NaN/Infinity/overflow JSON rejection, duplicate keys, string/payload/action-count bounds, empty batches, and invalid request IDs;
- read-only versus mutation metadata, permission grants, exact-batch confirmation, and rejection of same-request-ID/different-payload confirmation;
- complete batch rejection when any action fails validation;
- unavailable/unapproved connector denial; and
- no success receipt from `UnavailableBlenderExecutor`.

### Checks actually run in this environment

| Check | Result | Scope |
|---|---|---|
| Repository/docs/source inspection and `git status` | Completed | Existing Phase 5/5A/5B decisions and integration boundaries reviewed before implementation. |
| Python parse of the documented JSON candidate | Passed | Confirms the example is syntactically valid JSON and labels contract v1; does not test the Rust parser. |
| Whitespace scan of the three Phase 6 deliverables | Passed | Source hygiene only. |
| `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --lib integrations::blender_actions::tests` | **Blocked — `cargo: command not found`** | 17 focused Rust tests are authored but were not executed or compiled here. No Rust pass is claimed. |
| Rust formatting / compile | **Blocked — `rustfmt`, `rustc`, and Cargo unavailable** | Source received manual/static review only. |
| Focused frontend/contract Vitest command | **Blocked — Vitest binary not installed** | `corepack pnpm --dir apps/desktop exec vitest run src/features/integrations/catalogue.test.ts src/features/integrations/native.test.ts src/pages/AppIntegrationsPage.test.tsx src/lib/rustContract.test.ts --reporter=dot` returned `Command "vitest" not found`. No frontend files changed. |
| Windows build/runtime | **Blocked — this host is Linux** | No Windows API or Tauri Windows build was run. |
| Blender runtime / connector execution | **Blocked — Blender unavailable; no connector exists** | No Blender process, scene, action, or handshake was used. |
| Frontend typecheck | Not run | No frontend/UI files changed; no AI-to-Blender UI was added. |

A Rust toolchain is required to run, from the repository root:

```sh
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --lib integrations::blender_actions::tests
cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml -- --check
```

These commands remain unexecuted in this environment. Any compile/test result from a future toolchain run must be recorded separately; unit tests cannot establish Windows transport or Blender behavior.

## 7. Files changed

- `apps/desktop/src-tauri/src/integrations/blender_actions.rs` — new transport-independent contract, strict parser, policy types, executor interface/default denial, result shapes, and unit tests.
- `apps/desktop/src-tauri/src/integrations/mod.rs` — registers the new pure Rust module; no Tauri command or connection path is added.
- `docs/nexora/BLENDER_INTEGRATION_PHASE_6_IMPLEMENTATION.md` — this phase report.

No UI, Tauri command registration, `Cargo.toml` dependency, transport source, Blender add-on, phase roadmap, or prior phase report was changed. The Phase 5A and Phase 5B decisions remain unchanged.

## 8. Limitations and next prerequisites

- The action contract has not been compiled or executed in this environment because Rust/Cargo are absent. The authored Rust tests must run in a toolchain-equipped checkout before relying on compiler/test evidence.
- No planner connects natural-language text to these candidates. This is a structured-action validation boundary only.
- No Blender version/runtime has been selected or tested. The actions are contract-only and must not be described as supported/executable.
- Phase 5 remains incomplete; Phase 5A remains **NO-GO FOR PRODUCTION**; Phase 5B is preparation only. A genuine, approved connector and successful Windows/Blender identity/access-control/health evidence are prerequisites before any execution implementation.
- Confirmation UI, consent receipts, persistent grants, idempotency store, scene revision model, action execution result verification, cancellation/recovery, and supported-version capability checks are not implemented.
- Mutating and potentially destructive operations need app-version-aware handling and separate future authorization. No delete, save/export, material, lighting, rigging, render, animation, image-to-3D, or arbitrary-code action is part of this contract.
- No user-facing status changed; Blender remains candidate-only and not Connected/Ready.

## Decision

Phase 6's **transport-independent contract foundation** is implemented in source, but its Rust tests could not be run here. No natural-language planner or Blender execution is verified. **Do not start Phase 7 or production connector work** without explicit authorization and the unresolved Phase 5 evidence gate.

All pre-existing modified and untracked work was preserved. Nothing was staged or committed; nothing was pushed, merged, tagged, published, or released. Stop at Phase 6 pending review/authorization.

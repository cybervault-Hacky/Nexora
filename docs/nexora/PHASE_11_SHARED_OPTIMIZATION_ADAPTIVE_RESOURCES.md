# Phase 11 — Shared optimization and adaptive resource management

## Scope and decision

Phase 11 adds an internal, shared resource-evidence, workload-admission, and reservation boundary for local media inference. It reuses the Phase 1 adaptive capability snapshot and Phase 2 registered media-model/lifecycle contract. No new hardware detector, model/runtime, status IPC, scheduling service, or settings UI is introduced.

The only current inference-shaped consumer is the Phase 3 SD-Turbo image-generation command. Its source contains a CPU-only CLI adapter, but the supported runtime package and model/runtime compatibility remain unestablished. The new gate therefore returns **unknown/unavailable** with structured reasons before runtime probing, checkpoint hashing, or child-process launch. An admission approval—if future evidence permits one—would only mean that the stated resource constraints pass; it would not establish capability usability or guarantee successful inference.

Video generation, image editing, video editing/enhancement, and advanced image generation/editing remain without an executable workload that consumes this coordinator. Their model/runtime assumptions remain unchanged and unresolved.

## Shared architecture

`apps/desktop/src-tauri/src/resource_management.rs` defines a crate-internal `MediaResourceCoordinator`, reusable resource profiles, evidence-carrying host snapshots, admission decisions, and RAII reservation leases. A single coordinator instance is managed by the Tauri application and shared with the existing `ImageGenerationManager`. It is not exposed to the renderer and has no mutable UI surface.

The Phase 2 registry is the source for model registration, exact registered artifact totals, registered memory requirements, and any required backend. The manager remains authoritative for download/validation/removal lifecycle. The resource layer does not modify its registry, asset metadata, download flow, or lifecycle transitions.

An admission context carries separate facts for:

1. model registration and manager-reported asset lifecycle;
2. runtime availability;
3. runtime/model compatibility;
4. the requested operation shape;
5. current resource evidence and its sample age.

Those states are never collapsed into one “ready” flag. In particular, a model in `installed_validated` manager state is not treated as runtime-compatible, and an available system-memory reading is not treated as proof that inference will succeed.

## Confirmed, estimated, and unknown resource evidence

`ResourceEvidence<T>` is an explicit enum:

- `Confirmed { value, source }` — an exact registered or operating-system-reported value with its provenance/description;
- `Estimated { value, method }` — reserved for estimates with a stated method; no active media profile currently supplies an estimate;
- `Unknown { reason }` — missing, unreliable, or unestablished information. Unknown is not zero.

Phase 1 `Detection<T>` values are adapted without probing: only `Confirmed` and `Estimated` readings with a value retain those classifications; unavailable/missing values become `Unknown`. The Phase 1 refresh timestamp is retained and its age calculated. Admission treats a missing, future-dated, or older-than-30-second snapshot as unknown. Thirty seconds is a conservative admission freshness policy, not a model or hardware requirement.

The SD-Turbo profile distinguishes:

- **Model-weight storage:** 5,214,561,328 bytes, confirmed from the exact Phase 2 registered artifact total. This is disk storage metadata, not runtime RAM or GPU-memory need.
- **Minimum/recommended system memory:** unknown; the model/runtime publishers do not establish a reliable value for Nexora's CPU path.
- **Available-memory screen:** the existing Phase 3 raw-checkpoint-size floor is retained only as a lower-bound rejection screen. Meeting it cannot satisfy or replace the unknown complete working-memory requirement.
- **GPU memory:** not budgeted by the Phase 3 invocation because it explicitly requests CPU. This does not confirm that a runtime supports that invocation or establish CPU memory fit.
- **Temporary output disk:** the adapter's existing 128 MiB free-space reserve is recorded as an enforced staging/output floor, not as a measured peak-workload estimate.
- **Workload:** the current adapter's fixed 512×512 output and 1, 2, or 4 steps are represented as operation constraints, not performance estimates.
- **Backend:** CPU is the requested backend in the adapter contract. Backend selection/compatibility remains a separate runtime state.

No VRAM amount, model working set, speed, quality, or performance guarantee is invented. A missing model memory requirement remains unknown even when the checkpoint fits in available RAM.

## Admission decisions and limitations

The shared evaluator returns one of:

- `AllowedByKnownConstraints` — required model/runtime/resource/workload evidence is established, known limits pass, and capacity can be reserved. This is not a capability-usable or inference-success claim.
- `Rejected` — a known resource floor or explicit operation limit is exceeded.
- `Deferred` — a compatible workload cannot reserve capacity because a necessary model operation or concurrency slot is already owned.
- `Unknown` — runtime, requirements, host readings, model lifecycle, or freshness evidence is incomplete.
- `Unavailable` — the model is unregistered, assets are unavailable, or runtime incompatibility/unavailability is established.

Reasons are typed internally (for example, missing/unvalidated assets, unknown runtime compatibility, unknown system-memory requirement, stale snapshot, insufficient memory/storage, unsupported workload, and concurrency conflict). Renderer values cannot assert model lifecycle, runtime compatibility, or hardware availability; the SD-Turbo workload descriptor is built in Rust from the validated request and fixed adapter contract.

The current SD-Turbo attempt is denied as `Unknown`: runtime availability is not established through a reviewed, version-pinned package; model/runtime compatibility is unconfirmed; and its minimum system-memory requirement is unknown. When manager assets are not recorded as validated, the decision is instead `Unavailable`. No child process is launched on these outcomes. The existing `ImageGenerationStatus.usable` remains false. `canAttemptGeneration` now previews the shared admission result without reserving capacity; a concurrent request can change the later result, and the field does not guarantee successful inference.

## Reservations, concurrency, and lifecycle

The coordinator keeps active reservations in one in-memory mutex-protected map keyed by the opaque operation ID created by the native image-generation owner. Each lease records its capability/model and a monotonically allocated token. The shared total media-inference limit is one active operation; the SD-Turbo profile also sets one active operation per capability and per model.

A lease is granted atomically only after admission is `AllowedByKnownConstraints`. It is non-cloneable and releases through `Drop`. Release checks both the opaque ID and reservation token, so a stale guard cannot release a newer operation's reservation. A poisoned mutex is recovered only to release the exact matching lease; new reservations fail closed if the state cannot be safely accessed. No TTL or background reaper releases a reservation while a workload may still be using memory.

For the Phase 3 process-per-request design, one lease is intended to cover model validation/use through child-process load, inference, output validation, and staging cleanup. The lease is moved into the blocking worker and remains alive through cancellation, failure, or normal return. Existing generation cancellation/shutdown and process timeout remain the lifecycle owners; explicit failure/cancellation cleanup now returns the structured `cleanup_failed` error if private staging cleanup cannot be confirmed. The reservation itself releases by RAII after worker cleanup. Existing Phase 2 `ModelUseGuard` independently prevents model removal while the worker reads the validated checkpoint. This does not create a persistent loaded-model reservation because there is no persistent warm model session.

At current evidence, no reservation is granted: the admission decision is unknown before model-file rehashing, runtime probing, or process start. Other media operations do not fabricate active jobs or reservations. Model downloads retain the Phase 2 manager's existing per-model lock, size checks, staging, cancellation, and disk preflight; they are not reported as inference jobs and do not consume the media-inference coordinator.

## Adaptive and lifecycle integration

- The Phase 1 adaptive contract remains the only source for system memory, GPU memory, model-volume space, and snapshot timestamp in the inference admission path. The coordinator translates the caller-supplied snapshot; it does not call hardware APIs.
- The existing image-generation request already collected adaptive data and checked output-volume space. The shared gate reuses those readings and does not add a repeated snapshot or new probe.
- The image-generation status path now reuses one adaptive snapshot and a Phase 2 manager lifecycle snapshot rather than collecting adaptive data once itself and again through `MediaModelManager::get`. Lifecycle values remain manager-recorded and current required-file presence/size evidence; they are not a fresh hash or compatibility result.
- Actual generation still retains Phase 2's explicit validation and locked-artifact verification before any future worker can read weights. No model lifecycle or registration state is changed to simplify resource reporting.

## Current consumers and unavailable paths

| Capability | Uses shared admission/reservations? | Current state |
|---|---|---|
| Mini SD-Turbo image generation | Yes, the native `generate_local_image` attempt consults the coordinator; an admitted future worker would own the lease. | Currently `Unknown` before process launch because runtime/model compatibility and complete system-memory requirements are not established. |
| FLUX.1 [schnell] image generation | No execution command exists. | Candidate is unregistered; runtime/assets remain unresolved; status-only. |
| InstructPix2Pix image editing | No edit command exists. | PNG source import/status only; editor/runtime unavailable. |
| Wan2.1 T2V video generation | No generation command exists. | Registered manager lifecycle and status only; runtime and fit unknown. |
| Conventional video editing / AI video enhancement | No processing command exists. | Separate Phase 10 status-only boundary; unavailable. |
| Coding-model inference | Not in scope and not changed. | Existing routing/resources are unchanged. |

The Tauri `get_adaptive_capabilities` and media-model IPC contracts are unchanged. No new UI command or TypeScript DTO was needed: no current UI operation requests an admission result. Auto/Image/Video modes, history, `localOnly`, and composer behavior are unchanged.

## Future adapter integration boundary

A future real media adapter should:

1. obtain a trusted registered-model definition and manager lifecycle snapshot internally;
2. supply separate runtime-availability and model/runtime-compatibility evidence—never infer either from model registration, downloads, file hashes, or resource approval;
3. describe model storage, auxiliary assets, required/recommended memory, temporary storage, workload bounds, backend, and concurrency using sourced confirmed values or method-documented estimates, leaving gaps unknown;
4. use a fresh Phase 1 snapshot or fail admission closed when missing/stale; do not run additional probes from the shared layer;
5. request a reservation before model loading/resource-heavy work and hold it in the actual worker until unload/cleanup has completed;
6. use a native opaque operation ID and release only through its matching RAII lease; bind cancellation/shutdown to real work and do not expire a live reservation;
7. retain manager lifecycle locks, safe artifact ownership, workload validation, and output validation as separate contracts.

No adapter may treat the coordinator's `AllowedByKnownConstraints` result as proof of model/runtime compatibility, a successful load, or usable inference. Unsupported dimensions/backends/quality settings must be rejected, not silently downgraded.

## Implemented / unavailable / unverified

**Implemented in source:** shared evidence and requirement types; model/asset/runtime/resource separation; phase-1 snapshot conversion and freshness handling; structured admission statuses/reasons; SD-Turbo profile sourced from the Phase 2 registry and Phase 3 fixed-workload contract; atomic single-slot reservation accounting with capability/model limits, opaque owner IDs and stale-release tokens; RAII worker-lifetime release; shared Tauri-app coordinator wiring; SD-Turbo generation admission integration; and removal of the redundant adaptive collection in the image-generation status path.

**Unavailable or unintegrated:** a registered, compatible runtime; a supported complete SD-Turbo memory requirement; any admitted current inference job; FLUX, InstructPix2Pix, Wan2.1, video editing, or enhancement workload integration; persistent model-load coordination; cross-process/external-application resource accounting; and a new UI resource panel/command.

**Unverified:** Rust/Tauri/TypeScript compilation; the admission/reservation lifecycle under cancellation, panic, app shutdown, concurrent requests, or lock poisoning; freshness behavior; runtime and hardware fit; actual memory/disk peak; and any media inference/output. No such behavior is claimed as tested.

No build, test, benchmark, profile, hardware/runtime probe, download, inference, media-processing experiment, release-readiness assessment, or Git publishing action was performed for Phase 11. Phase 12's source-integration and Windows-readiness review is recorded separately in [PHASE_12_FINAL_INTEGRATION_WINDOWS_READINESS.md](./PHASE_12_FINAL_INTEGRATION_WINDOWS_READINESS.md); that review does not establish product or release readiness.

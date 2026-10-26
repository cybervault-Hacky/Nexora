# Phase 2 — Local Media Model Manager

This note documents the Phase 2 media-model management foundation. Phase 3 later added the SD-Turbo image-generation registry entry; its model/runtime details and current packaging blocker are documented in `PHASE_3_LOCAL_IMAGE_GENERATION.md`.

## Boundaries and storage

- The Rust `media_models` module is separate from `inference::registry`, which remains the coding-model registry for Mini/Sonnet.
- Model metadata lives in the static media registry. Installed media weights and their `installation.json` records live under Tauri app data at `media-models/<stable-id>/`; coding weights continue to use the existing `models/` location.
- The frontend receives typed Tauri IPC views and sends only registered model IDs. It cannot supply URLs, checksums, filenames, installation paths, or lifecycle states.
- At the end of Phase 2 the registry was intentionally empty. Phase 3 added only SD-Turbo after pinning its publisher source revision, exact weight size/SHA-256, and license reference. The four distinct media categories remain part of the contract; only text-to-image is registered.

## Download and integrity policy

- Registry artifact URLs must be HTTPS and match the exact download-host allowlist in `media_models/registry.rs`. Redirects are handled manually; each hop is revalidated against the same allowlist. There is no automatic cross-host redirect following.
- A transfer writes an exclusively created random `.part` file in the model's staging directory. The completed file is published by a no-replace same-volume hard link only after stream/size checks.
- Per-file size is bounded; transfer totals, content lengths, idle stalls, cancellation polling, and active progress are bounded/tracked. Transfers are not resumable; retry starts an incomplete file over, while previously completed, recorded files can be rechecked and retained.
- A trusted registered SHA-256 is required to mark integrity verified. A computed local digest without a trusted expected digest remains explicitly unvalidated.
- Removal requires a persisted manager record and deletes only registered filenames recorded as owned plus manager-shaped staging fragments. Unknown files and directories are left in place.

## Lifecycle and compatibility

The lifecycle contract includes `not_installed`, `awaiting_compatibility_assessment`, `ready_to_download`, `downloading`, `download_interrupted`, `downloaded_not_validated`, `installed_validated`, `incompatible`, `failed`, and `removing`. There is no `paused` state because resumable downloads are not implemented. A stale persisted `downloading` record becomes `download_interrupted`; malformed records are surfaced as `failed` without scanning unrelated files.

Phase 1 `adaptive::collect` is queried on demand against the media-model storage volume. Known required size is checked against known free space before transfer and again against source size before writing. Resource comparisons can report constrained, unknown, or observed headroom, but never confirm runtime/model compatibility. Missing GPU, VRAM, backend, or model-specific requirements remain unknown. Phase 11's shared admission boundary reads this manager's registered identity and lifecycle snapshot without changing download/validation/removal transitions; manager-recorded validation remains distinct from fresh hash verification and runtime compatibility.

## Adding a future catalog entry

Add a stable ID, category, official HTTPS reference, license/restrictions, file roles/names, and only source-pinned expected sizes/checksums that are supported by evidence. Each download URL must use an already-reviewed allowlisted host. Keep unknown metadata as `None`; do not treat registry presence or a completed download as proof the model can run.

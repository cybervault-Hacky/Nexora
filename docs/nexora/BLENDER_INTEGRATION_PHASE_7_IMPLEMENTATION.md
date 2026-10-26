# Blender Integration Phase 7 — Image References & Image-to-3D Foundation

**Status:** In-chat image-reference selection, local validation, and data-only task preparation implemented.

**Vision / image analysis:** **Unavailable — no supported vision-capable model was verified.**

**Blender execution:** **Unavailable — no Blender execution was verified; the Phase 6 executor remains `connector_unavailable`.**

**Transport:** Phase 5 remains incomplete; Phase 5A remains **NO-GO FOR PRODUCTION**. This phase adds no transport, bridge, add-on, or provider integration.

**Date:** 2026-10-26

## 1. Scope and result

Phase 7 adds a reference-image flow inside the existing chat composer. Users can choose one PNG, preview it, replace/remove it, enter a natural-language asset request, and submit a locally validated reference plus a structured preparation record into the current conversation. Existing project-file navigation, model selection, ordinary chat submission, and media-mode status behavior remain in place.

The flow does not ask a model to inspect the image. The request and image bytes do not enter `toChatTurns` or a provider request. A local-only conversation exchange records the user text, reference metadata, and a transparent capability response. This is task preparation only: it creates no geometry, Blender actions, Python, queue entry, or success result.

## 2. In-chat interaction and states

`apps/desktop/ui/src/components/Composer.tsx` adds an image reference file control, preview, replacement/removal controls, and accessible validation feedback. It is composed into `apps/desktop/ui/src/pages/ChatPage.tsx`, not a separate Blender page or chat. The existing “Add project files” button remains unchanged.

On selection, the renderer rejects empty or over-limit files before reading them. It reads at most 8 MiB and sends only the bytes to the desktop core's existing path-free PNG importer. The Rust importer validates before publishing the managed asset. Invalid format, corrupt/empty input, unsupported color/bit-depth, dimensions, and decode failures are surfaced as errors; the filename and local path are not used as evidence of image content.

The UI intentionally distinguishes:

1. **Image attached** — a selected reference is previewed in the composer.
2. **Reference validated** — the desktop core accepted the PNG after structural and decode checks.
3. **Analysis available** — **No**. No vision-capable model or authorized image-analysis path was established.
4. **Blender execution available** — **No**. Phase 6's executor remains disabled and returns `connector_unavailable`.

The task can be submitted without a coding model, but only as a local-only exchange. The assistant-side status text explicitly says analysis did not occur, bytes were not sent to a model/provider, and Blender execution is unavailable.

## 3. Formats, validation, and limits

The flow reuses `apps/desktop/src-tauri/src/image_editing/input.rs` (`SourceImageStore`) and does not add a new image-decoding dependency or accept renderer-supplied paths.

| Property | Supported contract |
|---|---|
| Format | Static, non-animated PNG only; decoded 8-bit RGB or RGBA. JPEG, WebP, GIF, SVG, animated PNG, grayscale, and other color/bit-depth combinations are not accepted. |
| Encoded file size | 8 MiB maximum; empty files rejected. |
| Dimensions | Each side at most 2,048 pixels; at most 4,194,304 pixels total. |
| Decoded output | At most 16 MiB before frame decoding. |
| PNG structure | Signature, chunk framing/lengths, per-chunk CRCs, IHDR fields, IDAT/IEND placement, APNG chunks, and complete image decoding are checked; chunk count is capped at 4,096. |
| File picker | Native/webview file input filtered to PNG for convenience; filter/MIME/name is not trusted for validation. Content validation is authoritative in Rust. |

The preflight bounds renderer reads to 8 MiB. The existing Rust importer validates content and dimensions before it creates its private staging directory. It writes through a per-import staging directory, publishes without replacing existing identifiers, and removes staging through its drop guard.

**Metadata limitation:** PNG chunk framing and CRCs are checked, and dimensions/bit-depth/color type are validated. The importer does not comprehensively interpret, redact, or strip every ancillary PNG metadata payload. The original accepted PNG bytes are retained in app-managed storage; no such bytes or metadata are sent to a model/provider by this flow.

## 4. Versioned data contracts

Renderer definitions and task preparation live in `apps/desktop/ui/src/features/chat/imageTo3dTask.ts`; persisted Rust-side DTOs and validation live in `apps/desktop/src-tauri/src/image_references.rs`. Chat history carries these optional fields on the local-only user message. Older history records remain valid because both fields are optional.

`ImageReferenceMetadata` v1 contains:

- opaque app-managed `referenceId` (the existing source store's random identifier; no filesystem path or filename);
- `format`, `width`, `height`, and `sizeBytes`;
- optional natural-language `description`;
- processing state (`validated` for an accepted attachment); and
- `validationErrors` (empty for a persisted accepted reference; rejected files are not persisted as valid references).

`PreparedImageTo3dTask` v1 contains the user's written request, one typed reference, conservative known constraints, missing information, `analysisAvailable: false`, and `nextStage: awaiting_supported_vision_capability`. It is validated as data on history save. The shape has no executable-code, action, or transport field. No image-derived object type/style/geometry is invented; missing-information notes explicitly avoid claiming image analysis.

Local-only user and assistant messages are filtered by `toChatTurns`, so neither the request nor image metadata enters coding-model turns. No model selector state, media status query, or Phase 6 policy is used to infer visual capability.

## 5. Storage, privacy, and cleanup

- Image bytes go only from the selected browser `File` to the local Tauri `import_image_edit_source` command. The command accepts bytes, never a path. No network/provider request is made.
- The accepted image is held in the existing app-managed `image-editing/sources` store to support a future workflow. Its chat record contains only the opaque ID and bounded metadata/task description, not image bytes.
- Object URLs are revoked on remove, replacement, successful submission, conversation switch, or component unmount.
- A removed/replaced draft is deleted by opaque ID. Deleting a conversation loads its reference IDs and requests managed-source deletion; cleanup failures are surfaced as a save-status error.
- The source importer removes temporary staging through RAII. The UI's byte read is size-bounded; it does not persist temporary renderer-side files.
- A submitted transcript restores its metadata, but does not currently reload a thumbnail into the message. The preview is available while composing; the saved message shows validated metadata/status.

**Retention limitation:** A hard process termination between import and transcript association can leave an unreferenced app-managed source. There is no startup orphan sweep or dedicated reference-storage management screen in this phase. If source deletion fails, Nexora surfaces the cleanup failure but cannot guarantee deletion. Accepted PNG ancillary metadata is preserved as described above.

## 6. Phase 6 and execution boundary

This phase does not call, modify, or bypass `apps/desktop/src-tauri/src/integrations/blender_actions.rs`. No action candidate is derived from image content, and no authorization, permission, confirmation, or executor API is invoked. There is no generated action batch, code, queue, bridge, Blender process, cloud API, or transport. Phase 6 remains the only typed action foundation, and its executor remains disabled with `connector_unavailable`.

## 7. Files changed

- `apps/desktop/ui/src/components/Composer.tsx` and `Composer.test.tsx` — in-composer PNG selection, preview, replace/remove, status and error UI.
- `apps/desktop/ui/src/pages/ChatPage.tsx` — bounded renderer preflight/import, local-only task submission, transparent capability response, pending-reference cleanup, and corrected image-editing status wording.
- `apps/desktop/ui/src/components/MessageList.tsx` — persisted reference metadata summary without exposing filename/path or implying a preview after reload.
- `apps/desktop/ui/src/features/chat/imageTo3dTask.ts` and `.test.ts` — v1 typed reference/task contracts, bounded preflight, deterministic data-only task preparation.
- `apps/desktop/ui/src/features/chat/chat.ts` and `.test.ts` — typed message metadata and a test that prepared tasks remain excluded from model turns.
- `apps/desktop/ui/src/features/chat/usePersistentConversation.ts` — metadata persistence and managed-source cleanup when a conversation is deleted.
- `apps/desktop/ui/src/lib/history.ts` and `.test.ts` — browser history schema validation and round-trip coverage for typed metadata.
- `apps/desktop/ui/src/lib/imageEditing.ts` — opaque-ID source deletion wrapper.
- `apps/desktop/ui/src/styles/app.css` — preview, attachment status, and metadata-card styles.
- `apps/desktop/src-tauri/src/image_references.rs` — strict versioned DTOs and contract validation/tests.
- `apps/desktop/src-tauri/src/image_editing/input.rs` and `commands.rs` — managed-source deletion by opaque ID and unit tests; existing import limits/decoder reused.
- `apps/desktop/src-tauri/src/history/storage.rs` and `inference/commands.rs` — persist/validate optional typed reference/task metadata; existing test literals updated for the additive fields.
- `apps/desktop/src-tauri/src/lib.rs` — register the new metadata module and managed-source deletion command.
- `docs/nexora/BLENDER_INTEGRATION_PHASE_7_IMPLEMENTATION.md` — this report.

No roadmap, historical phase report, Phase 5 transport decision, or Phase 6 executor behavior was changed.

## 8. Tests and checks actually run

| Check | Result | Scope |
|---|---|---|
| `corepack pnpm --dir apps/desktop typecheck` | **Passed** | TypeScript typecheck; run after Phase 7 UI/history changes. |
| Focused Vitest: `Composer.test.tsx`, `chat.test.ts`, `imageTo3dTask.test.ts`, `usePersistentConversation.test.tsx`, `history.test.ts`, `ChatPage.test.tsx` | **Passed — 6 files, 77 tests** | Composer selection/status/removal, data-only task, history validation/round-trip, local-only inference boundary, and existing chat/persistence tests. |
| `git diff --check` | **Passed** | Whitespace/conflict-marker hygiene. |
| `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --lib image_references::tests image_editing::input::tests history::storage::tests` | **Blocked — `cargo: command not found`** | New Rust tests and existing history tests were authored/updated but were not compiled or executed. |
| Rust formatting/compile | **Blocked — Cargo, `rustc`, and `rustfmt` unavailable** | No Rust pass is claimed. |
| Full UI suite / app build | Not run | Verification stayed focused on Phase 7 changes. |
| Windows/Tauri runtime, vision model, Blender, connector | **Unavailable / not verified** | No Windows host, verified vision-capable model, Blender process, or approved connector was used. |

## 9. Limitations and decision

- **Image analysis is unavailable.** The selected local coding models have not been shown to accept image inputs, and no explicit supported/authorized vision capability exists. The image is never forwarded to a model/provider.
- **Image-to-3D generation is unavailable.** Task preparation preserves the written request and opaque local reference, but does not interpret image content or synthesize geometry/actions.
- **Blender execution is unavailable.** Phase 5A stays **NO-GO FOR PRODUCTION**; Phase 6 remains disabled with `connector_unavailable`; no Blender handshake or execution is claimed.
- Rust validation, history serialization, and deletion tests could not run without a Rust toolchain. The frontend checks do not establish Rust compilation, Tauri IPC behavior, Windows behavior, or image-decoder runtime behavior.
- PNG ancillary metadata is not comprehensively inspected or stripped, and a crash can leave an orphan managed source as noted above.
- No source image thumbnail is restored in message history; saved messages retain a metadata/status card and opaque reference.

**Decision:** Phase 7's in-chat reference-preparation foundation is implemented, with focused frontend tests and typecheck passing. Image understanding and Blender execution remain unavailable. Stop here; do not begin Phase 8 without explicit authorization.

All pre-existing modified and untracked work was preserved. Nothing was staged or committed; nothing was pushed, merged, tagged, published, or released.

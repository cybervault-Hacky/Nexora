# Phase 12 — Final integration and Windows readiness

**Disposition: source-integration review complete; product and release readiness are not established.** This is the final phase of the approved Image + Video Generation roadmap. It records source-supported contracts and corrects narrow integration/documentation inconsistencies. It does not authorize runtime validation, packaging, or a release.

## Scope and evidence boundary

The review covered the Phase 1 adaptive snapshot; Phase 2 media registry and lifecycle; Phase 3 SD-Turbo generation; Phases 4 and 8 image editing; Phase 5 and 9 Wan2.1 video generation; Phase 6 chat modes; Phase 7 FLUX candidate status; Phase 10 video editing/enhancement; Phase 11 shared media admission; Tauri command registration and IPC wrappers; and the Windows toolchain, bundle configuration, CI, and authored packaging/smoke scripts.

This was source and documentation review only. Presence of a source path, command, script, workflow, manifest, upstream reference, or prior phase report is not evidence that it compiled or ran. No local media-model lifecycle was queried in this phase. Therefore actual installed assets, locally computed integrity results, runtime presence on a user's machine, platform compatibility, available resources, loaded state, generated outputs, and product behavior remain unobserved unless explicitly described below as source metadata.

No build, typecheck, test, benchmark, profile, hardware/runtime probe, download, inference, media experiment, Windows package, CI dispatch, installer/smoke execution, or Git publishing/release action was performed for Phase 12.

## Narrow integration corrections

The following source/documentation corrections are supported by the inspected code and configuration:

1. **Runtime packaging language:** `image_generation/mod.rs` now describes the SD-Turbo path as resolving a CLI from fixed application-resource locations rather than claiming the CLI is bundled. The module now explicitly records that the checkout does not bundle or pin that CLI and that current shared admission leaves runtime evidence unknown. Runtime-error and status wording now refer to an approved/located or detected executable, without assuming it was bundled.
2. **Renderer wrapper wording:** `ui/src/lib/imageGeneration.ts` now describes the status wrapper as reporting local lifecycle/runtime/resource status, not proven readiness. The generation wrapper says the call *requests* local SD-Turbo generation and may be rejected by native admission before runtime probing; it no longer implies successful local inference.
3. **Current SD-Turbo order:** the Phase 3 record now says admission runs before runtime resolution/probing, asset revalidation, or large checkpoint hashing. With validated assets the current source still returns `Unknown` because runtime availability/version/compatibility and the complete memory requirement are not established; missing/unvalidated assets return `Unavailable`. Lower-level runtime, model-lock, cancellation, cleanup, output-validation, and artifact code is not evidence that the currently gated operation reached those stages.
4. **Tauri surface comment:** the `run()` comment in `src-tauri/src/lib.rs` now describes the explicit command surface and separated coding/media managers. The former “only read-only status/configuration” and “no filesystem browsing/network access” description did not match the actual registered commands and native subsystems.
5. **Roadmap links:** Phase 3 and Phase 11 link to this final review. Earlier phase notes retain their historical phase-completion context.

No IPC schema, model registry entry, resource limit, admission condition, download/integrity rule, cancellation path, cleanup behavior, chat mode, page, or runtime was added or weakened.

## Cross-phase state contract

These are separate facts and must not be collapsed into one “ready” value:

| State | Source of authority | Phase 12 conclusion |
|---|---|---|
| Model registration | Static Phase 2 media registry (separate from the coding-model registry) | SD-Turbo and Wan2.1 T2V are registered. FLUX.1 [schnell] and InstructPix2Pix remain unregistered candidates. No video-editing/enhancement model is registered. |
| Asset download and integrity | Per-user Phase 2 manager record, trusted manifest, and validation operation | Manifest sizes, source revisions, and hashes documented by the roadmap are publisher metadata. The current per-device download/lifecycle/hash state was not queried. Manager-recorded validation is not a fresh rehash, runtime compatibility, or inference result. |
| Runtime availability and version | Approved application-resource resolution / reviewed package and pin | The Tauri bundle configuration declares neither `resources` nor `externalBin`; no runtime executable is declared or pinned by the inspected package config. SD-Turbo source can resolve a file at fixed locations if one is supplied, but Phase 12 did not search the host or run the status command. A path existing would still not establish a reviewed version/build. |
| Runtime/platform compatibility | Exact model + exact runtime build + OS/backend evidence | Unknown for every media model. Upstream Windows/backend documentation is not a Nexora executable or compatibility result. No model/runtime pairing was run. |
| Resource evidence/admission | Phase 1 snapshot, manager facts, and Phase 11 coordinator | Missing readings remain unknown. Generic Phase 1 profile/screening values and checkpoint storage size are not workload-memory requirements. SD-Turbo's Phase 11 request gate fails closed as `Unknown` for validated assets under current unresolved runtime/compatibility/system-memory evidence, and as `Unavailable` for missing or unvalidated assets. A preview or any future `AllowedByKnownConstraints` result is not proof of compatibility or successful inference. |
| Loading/inference readiness | Model-specific loader and execution path | No successful media model load or inference was observed. SD-Turbo has a process-per-request adapter path in source but the current admission context prevents reaching runtime resolution. Other media capabilities have no inference command. Coding inference remains a separate, unchanged subsystem; this review did not query its local readiness. |
| Operation availability | Registered native command plus complete supported request path and current admission | A registered Tauri command is not the same as a usable operation. Status-only capabilities remain status-only; SD-Turbo is gated and reports `usable: false`; editing/video operations without executors remain unavailable. Chat media submission queries status rather than invoking inference. |
| Output validation/artifact availability | Operation-specific content validation and managed artifact store | SD-Turbo source contains PNG validation and managed artifact handling, but no successful output/artifact was produced or retrieved. No video generation, editing, or enhancement output pipeline exists in source. Imported PNG source storage for image editing is not an edited output. |

### Capability status by model and operation

| Operation / model | Registration and asset state | Runtime / resource state | Operation, output, and chat state |
|---|---|---|---|
| **Mini text-to-image — SD-Turbo** | Registered with one pinned/checksummed checkpoint (5,214,561,328 bytes). This is the file's registered size, not total runtime memory. Actual device installation and validation state were not queried. | No executable is declared in the bundle or pinned by Nexora. The adapter resolves only fixed app resource locations; current generation admission supplies runtime availability and compatibility as `Unknown`. Complete CPU working-memory need is unknown. | Native command, request checks, child supervision, cancellation, PNG validation, and artifact storage exist in source. The current request is denied before runtime probing/asset rehash/process launch; status remains unusable. Chat Image handling displays Mini status and does not call the generation command. No output was generated. |
| **Sonnet text-to-image — FLUX.1 [schnell]** | Candidate metadata exists, but there is no complete trusted asset manifest and no Phase 2 registration/download path. | Upstream source references are not a pinned Nexora runtime; compatibility and workload fit are unknown. | Read-only status only. No load, generation, progress/cancellation, output validator, or artifact command. Chat reports candidate status separately from Mini. |
| **Mini image editing — InstructPix2Pix** | Candidate checkpoint provenance/size/hash are documented; the complete pipeline asset manifest is unresolved and the candidate is not registered. The known checkpoint pointer does not establish a downloaded or validated local file. | No supported, pinned runtime or model/runtime compatibility; hardware/platform fit unknown. | Secure PNG-byte source import/retrieval and static status exist in the desktop core. There is no edit/inference command, chat image attachment/source-selection flow, edited output, or editing artifact. Chat reports unavailable editing status. |
| **Video generation — Wan2.1 T2V 1.3B** | Registered with three pinned/checksummed assets totaling 9,135,187,134 bytes (~8.51 GiB). Actual manager download and integrity state were not queried; its status snapshot is manager-recorded, not a fresh hash. | No compatible Nexora runtime is declared/pinned; runtime compatibility and model-specific resource fit remain unknown. Weight storage total is not a working-set estimate. | Tauri exposes read-only status only. No generation request, loader, progress/cancellation, selected encoder, output validator, or artifact store. Video chat reports this status and never invokes a coding-model fallback. |
| **Conventional video editing** | No editing model or registered asset is required/established. | No integrated decoder/encoder runtime or established input/output format contract; Windows/resource support unknown. | Status-only, with no supported conventional operation, source-video import, processing command, or validated artifact. |
| **AI video enhancement** | No enhancement model or asset manifest is established. Wan T2V, FLUX, and image-edit models are not substitutes. | No runtime, compatibility evidence, or workload fit. | Status-only and unavailable; no enhancement executor/output pipeline. |
| **Coding-model inference (preserved control path)** | Uses the existing coding-model registry/lifecycle, separate from media models. | Not part of media admission; actual local model/runtime state was not queried in Phase 12. | Existing ordinary chat path remains unchanged. No coding-model routing or availability claim is added by this review. |

## Chat, IPC, and application integration

- The composer retains its existing **Auto / Image / Video** modes and Auto default. Mode remains local UI state; conversation history and the optional `localOnly` marker remain unchanged.
- `resolveMediaTask` uses deterministic local phrase rules. In Auto, ambiguous prompts, software-construction requests, and text-about-media requests remain ordinary chat; clear media intents select a status flow. Explicit Image/Video modes route their non-empty prompts to the corresponding status path.
- `ChatPage` calls status wrappers, records a local user/assistant exchange, and returns without calling the image-generation command. Image generation status independently reports Mini and FLUX; image editing, video generation, and video editing use their own status boundaries. A status-query error remains unknown. There is no remote/cloud or coding-model fallback from a media-status path.
- Tauri command registration, Rust response shapes, and TypeScript wrappers were source-reviewed for the media status/operation boundaries. No unified “ready” DTO was added: existing typed statuses expose distinct facts, while absence of an executor remains explicit. Compilation and renderer/native IPC serialization were not verified.
- The secure image-source byte-import contract remains distinct from chat attachment and editing inference. No renderer filesystem path is accepted as a substitute for a supported edit input.

## Adaptive-resource and concurrency integration

Phase 1 collects a read-only snapshot with explicit detection confidence. On Windows, source branches use OS APIs for physical-memory and disk readings and enumerate display adapters; adapter enumeration does not reveal reliable dedicated VRAM, confirm an AI backend, or prove model compatibility. The Phase 1 CPU/CUDA/DirectML/Vulkan entries are unknown until an evaluated runtime exists. No hardware API was invoked in this review.

Phase 11 wraps snapshot values as confirmed/estimated/unknown evidence, preserves freshness, validates image and video workload shapes, and fails admission closed for unknown required evidence. It shares one in-memory coordinator with the SD-Turbo image-generation manager; reservations are atomic and worker-scoped RAII leases. SD-Turbo is the only current inference-shaped consumer. No reservation is granted under the current source evidence. Other media adapters do not participate in resource admission because they have no execution path. No cross-process resource accounting or persistent loaded-model reservation is implemented.

The SD-Turbo checkpoint's 5,214,561,328 bytes is only a lower-bound screening value. Its 128 MiB temporary/output free-space reserve is an enforced adapter floor, not a measured peak. Wan's 9,135,187,134-byte asset total is disk-storage metadata, not total runtime memory. No RAM/VRAM minimum, peak working set, speed, or performance guarantee is inferred.

## Windows source readiness and blockers

| Area | Source facts reviewed | Remaining evidence gap |
|---|---|---|
| Target and toolchain | `rust-toolchain.toml` pins Rust 1.93.0 with rustfmt/clippy. The Windows installer script explicitly targets native x64 `x86_64-pc-windows-msvc`, checks pinned Node/pnpm/Rust and required MSVC/Windows SDK tools, and rejects ARM64/32-bit packaging. | No Windows compile, native test, PowerShell parse, or installer execution was performed in Phase 12. Windows-specific FFI/dependency compilation and behavior remain unverified. |
| Native APIs/dependencies | Windows source branches use `windows-sys` for memory/disk APIs; `keyring` uses `windows-native`. Other code contains Windows-specific child-process handling. | API/ABI correctness, permissions, ACL/file-lock behavior, credential store behavior, process cancellation/reaping, and required native DLL/VC runtime closure were not exercised. |
| Bundle | `tauri.conf.json` configures NSIS and current-user install; it explicitly uses the WebView2 `downloadBootstrapper` mode. No `resources` or `externalBin` entry declares an inference/media runtime. | First install can require network for WebView2 when Evergreen is absent. No installer contents, runtime payload, sidecar, generated package, or installed paths were inspected in this phase. A media runtime would need its own reviewed and deliberately packaged or supplied distribution path. |
| CI and scripts | A `windows-2022` CI job and authored build/smoke scripts exist in source. The script describes a future compile/test/package/hash/signing path; the smoke script describes future install/data-preservation checks. | Workflow presence and authored assertions are not execution evidence. The Phase 19 record documents earlier partial frontend/source checks and a blocked native Windows result; this phase did not rerun them. No CI job was triggered. |
| Signing, provenance, release | The build script supports optional certificate-store signing and SHA-256 manifest creation. | No installer was created, hashed, signed, inspected, installed, upgraded, uninstalled, or released. No signed/unsigned classification or provenance attestation exists for this phase. |
| Platform support claim | Source packaging targets x64 Windows only; runtime/backend claims in upstream model documentation remain separate from Nexora. | No Windows 10/11 app-launch or media execution result exists. No claim is made for ARM64, other Windows architectures, GPU vendors, drivers, codecs, or runtime backends. |

The existing Phase 19 report is a historical record, not a substitute for present Phase 12 execution. Its described test and static-check results are not repeated or extended here; its blocked native Windows/package/runtime findings remain uncleared.

## Evidence required before any media runtime validation

Runtime validation is a later, separately authorized activity. Before launching a media runtime or model on a Windows host, retain evidence for the **specific operation and exact artifacts** being evaluated:

1. A reviewed runtime source/release revision, redistribution license/notices and dependency closure; exact version, build options, supported Windows architecture/backends, executable checksum/provenance, and explicit installation/package location. A source-code commit or upstream platform list is not a binary pin.
2. A complete operation-specific model/auxiliary-asset manifest with immutable source references, exact sizes and trusted checksums; license/restriction review; and a local manager record plus successful local integrity verification. A registered candidate or downloaded file alone is insufficient.
3. A supported invocation contract for that exact runtime/model pair, including model loading and backend evidence. Runtime existence, a CPU/GPU listing, or a successful generic runtime probe alone does not establish model compatibility.
4. Operation-specific resource bounds with source/method and host evidence: complete RAM/VRAM working set and peak behavior, temporary/output disk needs, supported dimensions/frames/steps/parameters, concurrency, and cancellation/cleanup behavior. Unsupported or unevidenced limits must remain unknown and block admission; do not substitute weight size or Phase 1 generic hints.
5. On the exact Windows target, retained logs showing startup, runtime/backend identification, model load, a representative inference, content validation, and artifact publication; also exercise cancellation, timeout/failure cleanup, and repeat/no-overwrite behavior. Image success does not validate video, editing, or enhancement.

For SD-Turbo, the current source gate must remain closed until its runtime availability/version/compatibility and complete model-specific memory requirement are established. A future source change that asserts those facts must cite the evidence and preserve separate status fields; it must not turn status preview into a reservation or usability guarantee.

## Evidence required before a Windows/product release claim

A release decision would additionally require a separately authorized native Windows run and retained artifacts/logs for all of the following applicable checks:

- Frozen dependency installation and the repository's pinned frontend typecheck, tests, production build, and installer-contract checks; Rust formatting, target check, clippy, and Rust tests using the pinned toolchain. Record exact tool versions, target triple, command exits, and logs. These checks were not run in Phase 12.
- Successful x64 MSVC Tauri/NSIS package creation through the guarded script; inspection of the actual installer payload and required native dependencies; confirmation of whether the exact installer is signed or unsigned; Authenticode evidence when signed; and a SHA-256 manifest tied to the inspected artifact.
- Clean-profile Windows 10/11 x64 installation and launch evidence, including current-user paths, WebView2 bootstrapper behavior (including the absent-Evergreen/online case), first-run/startup behavior, and no unexpected model/runtime payload. Preserve logs/screenshots and installer hash.
- Smoke evidence for app-data/model/history preservation through the supported uninstall/update path, as applicable. The authored smoke script is a test design, not evidence; prior user data must not be treated as disposable test output.
- For every media operation advertised in the release, the runtime-validation evidence above on the exact supported Windows configuration, plus validated managed output and failure/cancellation cleanup. A successful installer does not establish inference readiness, and successful SD-Turbo inference would not qualify other models or video processing.
- License/notice and supply-chain review for every shipped runtime/native component and a clear distribution statement for external model assets. No model/runtime redistribution is implied by the current bundle configuration.

No Windows package or release-readiness run was performed in Phase 12. No release claim is cleared by this checklist.

## Final disposition and stop boundary

**Source-integration completion:** complete for Phase 12's bounded review. The cross-phase records now describe the current SD-Turbo admission order accurately; the stale bundled-runtime wording is corrected; Tauri's command-surface comment is accurate; and Phase 3/11 link to this final integration record. Existing capability boundaries, typed statuses, chat behavior, model lifecycle safeguards, output cleanup, and Windows packaging configuration were preserved.

**Product readiness:** not established. The current source does not provide a pinned packaged media runtime, complete model/runtime compatibility evidence, complete model-specific workload requirements, successful Windows native validation, or successful media inference/output evidence. Some model assets are registered only as catalog metadata and actual local lifecycle state was not queried. Product readiness is therefore not demonstrated, not inferred from source presence, and not equivalent to source-integration completion.

Phase 12 ends here. No further roadmap phase, runtime validation, packaging, publication, or release is started by this record.
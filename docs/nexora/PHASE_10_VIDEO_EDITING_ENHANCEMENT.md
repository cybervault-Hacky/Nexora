# Phase 10 — Local video editing and enhancement capability boundary

## Decision and scope

**No conventional video-processing implementation or AI video-enhancement runtime is established in Nexora.** This phase therefore adds only a read-only capability boundary, typed IPC status, and a narrow Video-mode status-message correction. It does not add a trim, frame-extraction, resize, transcode, encoding-adjustment, enhancement, progress, cancellation, source-import, or output-artifact operation.

The capability is distinct from coding-model inference, Phase 9 text-to-video generation, image generation, and image editing. The existing Auto/Image/Video composer, history persistence, and `localOnly` behavior are unchanged. No remote/cloud fallback or generic coding-model fallback is introduced.

## Repository inspection and support assessment

The Phase 1–9 desktop Rust and package manifests, Tauri resource configuration, media capability modules, Phase 6 chat routing, and Phase 9 generation boundary were inspected. A source search across the desktop implementation and its Rust lockfile found no integrated video-processing implementation or declared video-processing dependency such as a demuxer/decoder/encoder stack. No video processor is configured as a Tauri resource. This is repository evidence only: this phase did not search for or probe arbitrary executables installed elsewhere on a user's machine.

The application has a secure PNG-byte source-import boundary for image editing, not a video-source import/attachment contract. It is format-specific and is not reused or widened for video. The existing Wan2.1 registration is text-to-video only; it does not provide source-video editing or temporal enhancement. FLUX.1 [schnell] is an image-generation candidate and InstructPix2Pix is an image-editing candidate; neither is treated as a video capability.

| Capability | Phase 10 finding | Why it is not reported as supported |
|---|---|---|
| Trim, frame extraction, resize, transcode, or encoding changes | **Unavailable** | No integrated decoder/encoder runtime, established input-format contract, or validated managed output path. |
| AI-based temporal/video enhancement | **Unavailable** | No dedicated enhancement model, complete trusted asset/runtime pairing, or supported inference path. Image models and Wan2.1 T2V are not substitutes. |
| Source-video import or attachment | **Unavailable** | No approved video input flow, opaque app-managed source ID, or backend ownership/existence validation. Renderer paths and shell commands are not accepted. |
| Validated video output | **Unavailable** | No app-managed video artifact store, non-overwriting publication flow, or media-structure/metadata validation. |
| Runtime, Windows/backend, hardware, and resource compatibility | **Unknown / unverified** | No compatible runtime was selected or probed. The host machine, codecs, accelerators, and resource limits were not tested or assumed. |

No external media package or executable was selected. Consequently there is no Phase 10 dependency version, license, Windows/component, format-support, security, or resource claim to make. No binaries were downloaded and no runtime/media experiment was performed. Selection and due diligence for any future implementation remain open rather than guessed.

## Implemented status-only boundary

- Added a distinct Rust `video_editing` adapter and the Tauri command `get_local_video_editing_status`. It reports conventional-processing and AI-enhancement availability separately from video generation.
- Added a typed `VideoEditingCapabilityStatus` IPC DTO and desktop-only `fetchVideoEditingStatus()` wrapper.
- Updated Video-mode chat status to use this editing DTO only for video-editing requests. Video-generation requests continue to use the separate generation status. The explanation identifies unavailable editing, absent input/output contracts, unknown compatibility, and unavailable enhancement without claiming an edit occurred.
- The status is static repository capability metadata. It does not inspect a path, query a model manager, locate an executable, probe hardware, load a model, access a video, or start media inference.
- No request accepts frontend filesystem paths, opaque video IDs, commands, arbitrary parameters, or uploaded media bytes. No fake success, progress, cancellation, or artifact response is exposed.

The status presently reports zero supported conventional operations; no configured media runtime or established input/output format set; no secure video-source import or managed output store/validator; no enhancement candidate/model/runtime; and compatibility, Windows support, hardware fit, and workload/resource evidence as unknown. The unknown values mean they were not established—not that a host feature was tested and found absent.

## Requirements for a future real video path

Phase 10 does not create these contracts or choose an input format. Before any later implementation can claim support, it must select and review a compatible local implementation and establish its exact pin/provenance, license and notices, included components, supported Windows packaging/backends, actual format support, security properties, and bounded resource behavior. Any AI enhancement path must remain separate from conventional operations and text-to-video generation, with its own reviewed model/runtime/assets and verified compatibility.

A future source/output design must, at minimum, use opaque app-managed IDs; validate ID ownership and artifact existence in the backend; restrict storage to approved application directories; preserve the original source; stage writes and publish new outputs without overwriting prior valid artifacts; clean up only its own incomplete staged files; impose evidenced input, output, duration/dimension, memory, disk, and processing limits; and validate content structure and metadata before reporting success. No frontend filesystem path or shell command may stand in for these controls. A video attachment flow and an actual supported-format contract must precede exposing any operation.

## Separation from adjacent capabilities

- **Coding inference:** remains the ordinary coding/chat capability; this status flow does not route video-editing prompts to it.
- **Video generation:** remains Phase 9's separate Wan2.1 T2V-only status and unavailable generation path; this phase neither changes nor borrows its status.
- **Conventional video editing/conversion:** unavailable; no processing command exists.
- **AI video enhancement:** unavailable; no temporal enhancement model/runtime is registered or usable.
- **Image generation and editing:** remain separate image capabilities and are not treated as video processing.

## Implemented / unavailable / unverified

**Implemented:** a separate static Rust video-editing/enhancement capability status; Tauri registration; typed IPC wrapper; and a narrow chat-status distinction from video generation.

**Unavailable:** every conventional video operation; source-video import/attachment; AI temporal enhancement; a compatible local video runtime; supported input/output formats; managed, validated video artifacts; processing progress/cancellation; and established Windows/backend/resource compatibility.

**Unverified:** compilation and UI behavior; any host-installed media tools; runtime, codec, and hardware compatibility; resource requirements; actual video input/output compatibility; and end-to-end editing/enhancement behavior. No probes or execution were performed.

No build, test, benchmark, runtime or hardware probe, download, media experiment, inference, release-readiness assessment, or Git publishing action was performed for Phase 10. The subsequent Phase 11 shared resource-management work is documented in [PHASE_11_SHARED_OPTIMIZATION_ADAPTIVE_RESOURCES.md](./PHASE_11_SHARED_OPTIMIZATION_ADAPTIVE_RESOURCES.md); Phase 12 integration/readiness work remains out of scope.

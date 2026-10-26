# Phase 6 — Chat media modes

## Scope implemented

The existing chat composer now offers compact **Auto** (default), **Image**, and **Video** modes. Mode is local UI state, resets to Auto when the active conversation changes, and is not added to the persisted mode schema. The existing coding-model selector and ordinary chat submission remain separate.

Auto uses deterministic local phrase rules only. It routes clear image-generation, image-editing, video-generation, and video-editing requests to the local capability-status flow; likely software-construction and media-prompt/script wording is left as ordinary chat. Ambiguous wording remains ordinary chat. Explicit Image and Video modes route their non-empty prompts to the corresponding local status boundary.

A media status exchange is stored in the current conversation using ordinary user/assistant messages. Those messages carry an optional `localOnly` marker so they remain visible and backward-compatible in history without being included in later coding-model turns. No media prompt is sent to the coding model by the media-mode handler.

## Current capability boundaries

- **Image generation:** chat queries the existing image-generation status. The current status implementation reports inference as unusable and compatibility/version confirmation as false. Chat does not invoke the generation command or present a generated artifact.
- **Image editing:** chat queries the existing editing status. The editor/runtime/inference command is unavailable. Secure PNG-byte import exists in the desktop core, but chat has no image picker or attachment flow; no renderer-supplied path or source bytes are used.
- **Video generation:** chat queries the separate video-generation status. The registered Wan2.1 candidate is text-to-video only and does not establish that assets are installed, nor that a compatible runtime, generation command, or validated output pipeline exists. The status interface does not report installed asset state, so the UI describes that state as unknown.
- **Video editing and enhancement:** Phase 10 adds a separate read-only capability status. Conventional processing and AI video enhancement remain unavailable; no source-video attachment/import, processing runtime, enhancement model/runtime, or managed video-output pipeline is established. This does not invoke or reuse video generation.
- A failed status query is shown as unknown/unavailable. No mode claims a result, starts media inference, downloads assets, or falls back to a coding/cloud model.

## Unverified behavior

No build, test, benchmark, runtime/hardware probe, download, inference, or release-readiness check was run for Phase 6. Renderer behavior, conversation persistence/IPC compatibility for the optional `localOnly` marker, accessibility, layout at narrow widths, and runtime status-query behavior remain unverified in this change.

# Phase 4 — Mini Local Image Editing

Phase 4 adds a separate image-editing boundary and safe source-image import. It does not add an editing screen, chat flow, composer selector, or inference command. No edit is reported as successful because this checkout has no compatible, pinned local editing runtime.

## Candidate model and provenance

**Candidate:** InstructPix2Pix (`timbrooks/instruct-pix2pix`), the research model by Tim Brooks, Aleksander Holynski, and Alexei A. Efros. The authors' paper and code describe edits conditioned on both an existing image and a written instruction; the official Diffusers pipeline likewise accepts a prompt and RGB image. This is a genuine instruction-based image editor, not SD-Turbo text-to-image used as a substitute.

- [Official model card](https://huggingface.co/timbrooks/instruct-pix2pix)
- [Authors' implementation and inference example](https://github.com/timothybrooks/instruct-pix2pix)
- [Diffusers InstructPix2Pix pipeline documentation](https://huggingface.co/docs/diffusers/en/api/pipelines/pix2pix)
- [Paper](https://arxiv.org/abs/2211.09800)
- The official repository's immutable SafeTensors pointer revision is `c0d64774d2012658405318248fb7fff79db71158`; file `instruct-pix2pix-00-22000.safetensors`, `7,703,416,451` bytes, SHA-256 `fbc31a67aab29ad38eef622a252d3fa4b0a81d4a4dc65da416cf45b13aa67688`.
- The model card labels the model MIT. The authors' code LICENSE is MIT, but expressly warns that portions of the model/checkpoint derive from Stable Diffusion and that further restrictions may apply. The repository includes the base [CreativeML Open RAIL-M license](https://github.com/timothybrooks/instruct-pix2pix/blob/658ad5e4d91546ee1f800a79ba63a3bd76fd93c7/stable_diffusion/LICENSE) with use-based restrictions. Both statements are disclosed; the artifact is not registered or offered for download in this phase.

The 7.7 GB single-file checkpoint is not treated as a complete, verified Diffusers deployment. The repository also contains component safetensors and pipeline configuration, tokenizer, scheduler, and safety-checker assets. No complete pinned package of those pieces or runtime environment has been integrated into Nexora.

## Runtime and compatibility blocker

The authors provide a PyTorch `edit_cli.py` that accepts an input image and instruction. Their README says the quickstart was tested on a GPU with more than 18 GB VRAM; it notes non-GPU users may need to change configuration. This documents one tested GPU setup, not a minimum and not proof of useful CPU performance. Diffusers documents `StableDiffusionInstructPix2PixPipeline`; PyTorch/Diffusers offer generic CPU/device support and PyTorch distributes Windows builds, but these general runtime capabilities do not establish an InstructPix2Pix configuration that Nexora can safely bundle and support. Nexora does not bundle or pin Python, PyTorch, Diffusers, or an application-specific runtime wrapper. Model-specific Windows execution, resource minimums, cancellation behavior, output contract, and model/runtime compatibility in Nexora remain unverified.

The Phase 3 stable-diffusion.cpp path is not reused. The current upstream supported-model list inspected for this phase lists other image-edit architectures but does not establish InstructPix2Pix support; Phase 3's local invocation also only describes SD-Turbo text-to-image. No runtime probe was run. The capability status consequently reports no bundled runtime, no model registration, unknown hardware compatibility, and editing unavailable. It does not infer readiness from the candidate's weights or SD-Turbo's presence.

## Implemented boundary

- `src-tauri/src/image_editing/adapter.rs` describes the model-specific contract behind a small internal adapter descriptor. It is metadata only; there is intentionally no model executor, fake edit method, or successful dispatch path.
- `src-tauri/src/image_editing/input.rs` accepts bytes supplied by a caller after user selection, never a renderer-supplied filesystem path. It accepts PNG only (RGB/RGBA, 8-bit, non-animated), up to 8 MiB and 2,048×2,048 / 4 megapixels. The backend validates chunk framing and CRCs, dimensions, and decoded pixels under a bounded buffer before copying bytes into app-owned `image-editing/sources/` storage. Unique opaque IDs, exclusive staging writes, no-replacement publication, SHA-256 metadata, symlink checks, free-space screening, and revalidation on retrieval protect imported sources. The original input is never edited or overwritten.
- Tauri commands: `get_local_image_editing_status`, `import_image_edit_source`, and `get_image_edit_source`. `ui/src/lib/imageEditing.ts` and typed IPC DTOs expose only those implemented operations. The import wrapper accepts bytes, not paths. No `edit_image` or cancellation command is registered because no inference process exists to cancel.
- Errors for invalid/unsupported images, size/dimension limits, insufficient storage, managed-source lookup, integrity failures, and storage failures are structured.
- Phase 1 adaptive resource values are not read during this phase; source intake has a direct output-volume free-space check, while model RAM/VRAM and runtime readiness are explicitly unknown. No runtime/hardware probe is performed by the capability-status command.

## Capability state

**Implemented:** candidate model provenance and license caveats; separate image-editing capability descriptor; typed unavailable status; safe PNG-byte import and opaque managed-source references; structural/pixel validation and integrity metadata.

**Unavailable:** model download/registration, compatible runtime, image-edit inference, output image creation, edit progress, edit cancellation, and all edits. No command claims otherwise.

**Unverified:** all Rust/Tauri/TypeScript compilation and runtime behavior; Windows CPU/GPU compatibility; actual model quality or preservation of image details; exact RAM/VRAM minimum; complete pipeline asset bundle. Per instruction, no builds, tests, benchmarks, probes, hardware checks, downloads, or inference were run.

Phase 4 only. Phase 5 and later roadmap work has not started.

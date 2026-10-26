# Phase 8 — Advanced image-editing integration boundary

## Decision and candidate reassessment

**No installable, supported advanced editor/runtime has been established for Nexora.** InstructPix2Pix remains the strongest documented task candidate reviewed here: it is an actual instruction-guided image editor that consumes a source image plus a written instruction, rather than a text-to-image model repurposed for editing. Hugging Face Diffusers documents `StableDiffusionInstructPix2PixPipeline` and an example using the official model repository. The pipeline's documented inputs are a prompt and source image; the reviewed interface does not define a mask or region-selection input, so Nexora exposes neither.

This does **not** establish a supported Nexora runtime. The model checkpoint has a pinned file pointer, size, and SHA-256, but that single checkpoint does not establish a complete immutable Diffusers asset bundle. Nexora has no pinned PyTorch/Diffusers runtime version, model-specific Windows execution configuration, compatible runtime package, or application-owned inference wrapper. The upstream authors report a quickstart tested on a GPU with more than 18 GB VRAM; this is a tested setup, not a minimum requirement or a guarantee. No dependable minimum RAM/VRAM, CPU performance, or Nexora platform/backend compatibility is established. We therefore do not register the candidate with the Phase 2 media-model manager and do not provide download, loading, or inference operations.

| Evidence | Reviewed source facts | Nexora conclusion |
|---|---|---|
| Task | InstructPix2Pix's paper and official pipeline documentation describe edits conditioned on an input image and natural-language instruction. | Suitable as an instruction-editing candidate, distinct from Mini SD-Turbo and Phase 7's FLUX text-to-image candidate. |
| Model file | The publisher's immutable SafeTensors pointer is revision `c0d64774d2012658405318248fb7fff79db71158`, file `instruct-pix2pix-00-22000.safetensors`, 7,703,416,451 bytes, SHA-256 `fbc31a67aab29ad38eef622a252d3fa4b0a81d4a4dc65da416cf45b13aa67688`. | Checkpoint metadata is known; it is not proof that it is downloaded, locally validated, or a complete usable pipeline. |
| Other assets | The official model repository includes pipeline/component configuration and supporting tokenizer/scheduler/safety-related assets; this phase did not establish a complete, immutable, checksum-complete manifest and installation recipe for the selected runtime. | No manager registration or trusted download manifest. |
| License | The model card labels InstructPix2Pix MIT. The authors' repository carries the Stable Diffusion-derived `CreativeML Open RAIL-M` license terms and warns that additional restrictions may apply. | Treat model-card and inherited/component conditions separately; legal/redistribution review remains necessary. No artifacts are redistributed. |
| Runtime | Diffusers documents `StableDiffusionInstructPix2PixPipeline`. Reviewed source revision: `c60830ee365d520ab52b110dda562dd26f7b4d7f`. | Upstream model/pipeline support is documented, but no Diffusers/PyTorch package version or Nexora runtime build is pinned. This source revision is not a runtime-binary pin. |
| Platform/resources | Authors report one GPU quickstart tested with more than 18 GB VRAM. | This is not a minimum. Exact Windows, backend, CPU, RAM, VRAM, and workload fit for Nexora remain unknown; no hardware probe was run. |

FLUX.1 [schnell] is not used for editing: Phase 7 documented it only as a text-to-image candidate, and its status-only boundary has no editing pipeline. Mini's SD-Turbo generation path is likewise not an editing runtime. No remote/cloud fallback is introduced.

## Implemented architecture

- Extended the Phase 4 model descriptor and `ImageEditingCapabilityStatus` with separately reported model lifecycle facts (registration, download, local integrity validation, loaded state, and operation usability); asset-manifest completeness; upstream task support; mask/region support; pinned runtime source reference versus package pin; Windows/runtime/hardware unknowns; and resource evidence.
- `get_local_image_editing_status` remains read-only and performs no runtime, hardware, or filesystem probe. It reports `modelRegisteredWithMediaManager=false`; managed download and integrity-validation state as unknown/not tracked because the candidate is not registered; `runtimeBundleAvailable=false`; compatibility/Windows/hardware as unknown; `modelLoaded=false`; and editing usability false. The known checkpoint digest is source metadata, not local validation.
- No media-model registry entry was added. Phase 2 registration is withheld until the complete model/pipeline assets, trusted source metadata, license obligations, and supported runtime contract can be established.
- No edit request, mask/parameter controls, progress, cancellation, generation/editing fallback, output artifact, or success-shaped command was added. No output is claimed or fabricated.
- The existing chat continues to route image edits to the separate editing status and image generation to its independent Mini/Sonnet generation statuses. The status wording explicitly distinguishes editing from generation. The Auto/Image/Video modes, default Auto mode, composer controls, coding-model routing, and `localOnly` conversation behavior are otherwise unchanged.

## Source-image safety and chat dependency

The Phase 4 source-image store and Tauri operations were preserved without weakening or expanding the import contract. The backend accepts bytes, not renderer-provided file paths; validates PNG framing/CRCs and decoded pixels; accepts only non-animated RGB/RGBA 8-bit PNG up to 8 MiB and 2,048×2,048 / 4 megapixels; writes with application-managed opaque IDs and exclusive staging/no-replacement publication; records SHA-256; and revalidates stored source metadata and image bytes on retrieval. Original files are not overwritten. No format conversion or unbounded resize was added.

There is no chat image picker or attachment/source-image selection flow. The core's safe byte-import API is not a chat attachment flow, and the current chat cannot supply an imported `sourceId` to an editing request. Per scope, no picker, large attachment UI, or renderer path bypass was introduced. Chat reports this missing dependency; it does not pretend a prompt-only edit can run.

## Capability state

**Implemented:** richer static editing-candidate metadata; separate, honest lifecycle and compatibility status; safe Phase 4 PNG source import/retrieval unchanged; generation-versus-editing status distinction in the existing chat.

**Unavailable:** complete managed model registration/download; model integrity validation in a local installation; a pinned supported Diffusers/PyTorch runtime; runtime/model compatibility confirmation; Windows/backend and resource fit; model load; chat source-image attachment selection; edit inference; mask/region editing; progress/cancellation; output validation/artifact serving.

**Unverified:** compilation and behavior of prior Phase 1–7 and Phase 8 changes; exact pipeline dependency closure and asset manifest; runtime behavior on Windows or any local GPU/CPU; minimum resource requirements; output quality; and attachment persistence/integration. The model's known checkpoint SHA-256 is publisher metadata only and does not validate a local artifact.

No build, test, benchmark, runtime probe, hardware check, download, inference, or release-readiness assessment was run for Phase 8. Phase 9 is documented separately in [PHASE_9_SONNET_ADVANCED_VIDEO_GENERATION.md](./PHASE_9_SONNET_ADVANCED_VIDEO_GENERATION.md); phases 10–12 have not started.

## References

- InstructPix2Pix model card and checkpoint repository: <https://huggingface.co/timbrooks/instruct-pix2pix>
- Immutable SafeTensors checkpoint pointer at revision `c0d64774d2012658405318248fb7fff79db71158`: <https://huggingface.co/timbrooks/instruct-pix2pix/blob/c0d64774d2012658405318248fb7fff79db71158/instruct-pix2pix-00-22000.safetensors>
- Authors' implementation and quickstart: <https://github.com/timothybrooks/instruct-pix2pix>
- Authors' Stable Diffusion-derived Open RAIL-M license at revision `658ad5e4d91546ee1f800a79ba63a3bd76fd93c7`: <https://github.com/timothybrooks/instruct-pix2pix/blob/658ad5e4d91546ee1f800a79ba63a3bd76fd93c7/stable_diffusion/LICENSE>
- InstructPix2Pix paper: <https://arxiv.org/abs/2211.09800>
- Diffusers InstructPix2Pix pipeline docs: <https://github.com/huggingface/diffusers/blob/0eff7f0664817868596b55c1a8c5a6c210258eba/docs/source/en/api/pipelines/pix2pix.md>
- Diffusers InstructPix2Pix pipeline source, revision `c60830ee365d520ab52b110dda562dd26f7b4d7f`: <https://github.com/huggingface/diffusers/blob/c60830ee365d520ab52b110dda562dd26f7b4d7f/src/diffusers/pipelines/stable_diffusion/pipeline_stable_diffusion_instruct_pix2pix.py>
- Phase 4 secure input contract and original candidate assessment: [PHASE_4_LOCAL_IMAGE_EDITING.md](./PHASE_4_LOCAL_IMAGE_EDITING.md)

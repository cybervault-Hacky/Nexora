# Phase 9 — Sonnet advanced video-generation boundary

## Decision

**Wan2.1 T2V 1.3B remains the dedicated video-model candidate.** It is a text-conditioned video diffusion transformer, separate from coding models, Mini SD-Turbo image generation, Phase 7 FLUX.1 [schnell] text-to-image, and Phase 8 InstructPix2Pix editing. Its existing Phase 2 registry definition is preserved. Its documented upstream CLI path is local and does not require a cloud inference service; Nexora does not currently implement that path.

A plausible upstream model/runtime pairing is documented: stable-diffusion.cpp's Wan guide at source revision `40b1788476bbe8fe42fa538ddf7d32408a126b93` specifies Wan's dedicated `-M vid_gen` path and the required artifacts. However, **Nexora has no pinned, checksummed, supported runtime executable/build**, no verified build features or invocation contract, and no managed validated video-output pipeline. The upstream source revision is not an executable pin and its CLI/API is under active change. We therefore add no model loader, generation request, progress/cancel, or artifact success path. The registered model remains unavailable for inference.

No alternative model was selected: replacing a metadata-complete Wan candidate would not solve the runtime and output-pipeline blocker and would be arbitrary without a stronger model/runtime package.

## Model and trusted assets

The registered identity is `wan-ai-wan2_1-t2v-1_3b` (Wan2.1 Text-to-Video 1.3B, text-to-video only). The existing registry pins three required files and their Hub pointer metadata:

| Required file | Role/format | Immutable source revision | Bytes | SHA-256 |
|---|---|---|---:|---|
| `wan2.1_t2v_1.3B_fp16.safetensors` | Wan2.1 1.3B diffusion weights, FP16 | Comfy-Org `5601df2b72d74886b9df858fcd79c809cce4c4f3` | 2,838,303,560 | `be531024cd9018cb5b48c40cfbb6a6191645b1c792eb8bf4f8c1c6e10f924dc5` |
| `wan_2.1_vae.safetensors` | Wan2.1 VAE | Comfy-Org `dfcea77bcf258496e20c69cd84e8e8e41909bb3b` | 253,815,318 | `2fc39d31359a4b0a64f55876d8ff7fa8d780956ae2cb13463b0223e15148976b` |
| `umt5-xxl-encoder-Q8_0.gguf` | UMT5-XXL text encoder, Q8_0 GGUF conversion | city96 `81c2b6ffa438b6101e494a5985a4a9c1b731cfc0` | 6,043,068,256 | `2521d4de0bf9e1cc6549866463ceae85e4ec3239bc6063f7488810be39033bbc` |
| **Total** | Three required manager artifacts | Individual files pinned above | **9,135,187,134 (~8.51 GiB)** | — |

The two Comfy-Org files are Wan assets repackaged for the runtime's documented layout; the text encoder is separately converted by city96 from Google UMT5-XXL. There is no single original Wan-AI weight commit represented by this installation set: each repackaged/converted artifact has its own immutable source revision. The official Wan model-card URL identifies the model family, while the actual registered file provenance is the individual Hub source listed above. Existing registry entries include pinned HTTPS download URLs, exact sizes, and SHA-256 values and are not changed in Phase 9. No files were downloaded in this phase.

**License:** the Wan2.1 model and Google UMT5 are published under Apache-2.0; the selected city96 UMT5 conversion also declares Apache-2.0. The manager records these distinct sources and notice obligations. Preserve copyright/license notices and review the exact notices for each artifact before redistribution. The Wan publisher's license was reviewed at repository revision `65386b2e03c490796eede31b0325a6a595cc684e`. The upstream stable-diffusion.cpp source is MIT-licensed at the pinned runtime-source revision; any packaged build would require a separate review of its submodules and third-party dependencies.

## Runtime, platform, and resources

The selected runtime *source reference* is stable-diffusion.cpp commit `40b1788476bbe8fe42fa538ddf7d32408a126b93`. Its guide documents the required diffusion/VAE/T5 assets, the `-M vid_gen` mode, and example Wan2.1 T2V operation. Its project documentation lists Windows and CPU/CUDA/Vulkan and other backends. These are upstream source/project statements only: Nexora does not pin or bundle a corresponding Windows executable, establish its compiled backend/features, or confirm compatibility with the registry's precise assets.

The upstream CLI documentation discusses AVI/MJPEG, WebM, and animated WebP output; WebM/WebP depend on build features. The reviewed contract does not establish MP4 for Nexora. No selected binary's actual encoder features are known, and there is no Nexora video-container parser/validator or managed output-artifact store. Process exit, a filename, or an accepted request would not prove a playable video.

The Wan authors report a reference workload of about 8.19 GB VRAM and about four minutes for a five-second 480p RTX 4090 run without quantization. This is not a minimum, a local result, or a guarantee for the registry's mixed FP16/Q8 assets and a particular runtime build. RAM, peak working set, CPU practicality, safe maximum frame count/resolution/duration, output storage margin, and model-specific backend fit remain unknown. Generic CUDA build guidance is not a Wan minimum. Phase 9 does not run adaptive/hardware probes or enter unsupported thresholds.

The Wan authors' Python/PyTorch inference code is an upstream alternative, but Nexora has not pinned the Python, PyTorch, or Diffusers environment, packaged it, or established application compatibility. It is not silently substituted for the stable-diffusion.cpp candidate path.

## Implemented architecture and capability state

- Preserved the Wan2.1 `VideoGeneration` registry record and its three pinned assets. No registry identifiers, asset URLs, sizes, hashes, license records, or resource thresholds were changed.
- Extended the existing `video_generation` adapter/status DTO. The existing Tauri command `get_local_video_generation_status` now reads the Phase 2 manager's lifecycle snapshot for the registered Wan ID without calling the adaptive collector, probing hardware/runtime, loading weights, or recomputing large-file SHA-256 values.
- The snapshot distinguishes the registry's static **registered** state from manager-recorded lifecycle state, required files recorded/present, manager-recorded integrity validation, runtime compatibility, loaded state, and operation usability. It uses the installation manifest and conservative required-file presence/size checks. A no-record result reports manager download/integrity state as untracked rather than claiming that untracked filesystem files do not exist. A validation status is explicitly manager-recorded and not a fresh hash.
- Runtime/package version is not pinned; no executable is located or accepted from the renderer. Runtime compatibility and hardware/resource fit remain unknown. Model-loaded and usable-for-T2V remain false because no loader or generation command exists.
- Extended the TypeScript status DTO additively; new lifecycle fields are optional for older desktop-core compatibility. The existing Video chat handler displays managed installation state separately from runtime compatibility and usability. It does not fetch Mini/image status for a video request or send a video prompt to the coding model. Auto/Image/Video modes, default mode, composer, conversation history, and `localOnly` handling are otherwise unchanged.
- No model download/validation operation is started automatically. The existing manager's secure download, cancellation, managed-storage, checksum, and no-overwrite contracts are untouched.

## Request and artifact contracts

No video-generation request DTO is exposed. Prompt, negative prompt, dimensions, frame count/duration, FPS, steps/guidance, seed, loading, progress, cancellation, and resource-exhaustion errors are not presented as available controls because there is no Nexora executor to consume or validate them.

No output-artifact DTO, command, or file is created. A future real implementation must be tied to a selected runtime/container build, validate backend parameters and actual encoded media structure/stream metadata, publish only into unique app-managed storage without overwriting prior valid files, clean only its own incomplete output, and report success only after validation. This phase implements none of those inference/output steps.

## Implemented / unavailable / unverified

**Implemented:** model/runtime evidence consolidation; richer read-only status; no-hardware-probe manager lifecycle snapshot; separate registration/download/integrity/runtime/loaded/usable state reporting; additive IPC and truthful Video chat messaging.

**Unavailable:** a pinned/bundled runtime executable; runtime/model compatibility confirmation; runtime loading; T2V inference; supported request parameters; generation progress/cancellation; selected container encoder; validated video output and playback artifact delivery.

**Unverified:** whether Wan assets are installed or usable on any machine; actual checksums on a user's disk (status does not re-hash); runtime build compatibility and Windows/backend support; memory/performance; cancellation behavior; output format/container validation; chat end-to-end behavior; and all compilation/test behavior.

No build, test, benchmark, runtime probe, hardware check, model download, inference, or release-readiness assessment was run for Phase 9. Phase 10 separately adds only the read-only video-editing/enhancement capability boundary documented in [PHASE_10_VIDEO_EDITING_ENHANCEMENT.md](./PHASE_10_VIDEO_EDITING_ENHANCEMENT.md); it does not alter this generation boundary. Phases 11–12 remain out of scope.

## Pinned references

- Wan2.1 official model card: <https://huggingface.co/Wan-AI/Wan2.1-T2V-1.3B>
- Wan2.1 official repository: <https://github.com/Wan-Video/Wan2.1>
- Wan2.1 Apache-2.0 license, publisher revision `65386b2e03c490796eede31b0325a6a595cc684e`: <https://github.com/Wan-Video/Wan2.1/blob/65386b2e03c490796eede31b0325a6a595cc684e/LICENSE.txt>
- Pinned stable-diffusion.cpp Wan guide, including `-M vid_gen`: <https://github.com/leejet/stable-diffusion.cpp/blob/40b1788476bbe8fe42fa538ddf7d32408a126b93/docs/wan.md>
- Pinned stable-diffusion.cpp platform/backend/model overview: <https://github.com/leejet/stable-diffusion.cpp/blob/40b1788476bbe8fe42fa538ddf7d32408a126b93/README.md>
- Pinned stable-diffusion.cpp build and WebM/WebP features: <https://github.com/leejet/stable-diffusion.cpp/blob/40b1788476bbe8fe42fa538ddf7d32408a126b93/docs/build.md>
- Pinned stable-diffusion.cpp MIT license: <https://github.com/leejet/stable-diffusion.cpp/blob/40b1788476bbe8fe42fa538ddf7d32408a126b93/LICENSE>
- Pinned stable-diffusion.cpp CLI output implementation: <https://github.com/leejet/stable-diffusion.cpp/blob/40b1788476bbe8fe42fa538ddf7d32408a126b93/examples/cli/main.cpp>
- Wan diffusion weights, Comfy-Org revision `5601df2b72d74886b9df858fcd79c809cce4c4f3`: <https://huggingface.co/Comfy-Org/Wan_2.1_ComfyUI_repackaged/blob/5601df2b72d74886b9df858fcd79c809cce4c4f3/split_files/diffusion_models/wan2.1_t2v_1.3B_fp16.safetensors>
- Wan VAE, Comfy-Org revision `dfcea77bcf258496e20c69cd84e8e8e41909bb3b`: <https://huggingface.co/Comfy-Org/Wan_2.1_ComfyUI_repackaged/blob/dfcea77bcf258496e20c69cd84e8e8e41909bb3b/split_files/vae/wan_2.1_vae.safetensors>
- UMT5 conversion, city96 revision `81c2b6ffa438b6101e494a5985a4a9c1b731cfc0`: <https://huggingface.co/city96/umt5-xxl-encoder-gguf/blob/81c2b6ffa438b6101e494a5985a4a9c1b731cfc0/umt5-xxl-encoder-Q8_0.gguf>
- Phase 5 baseline assessment: [PHASE_5_LOCAL_TEXT_TO_VIDEO.md](./PHASE_5_LOCAL_TEXT_TO_VIDEO.md)

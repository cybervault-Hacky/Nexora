# Phase 7 — Sonnet advanced image-generation capability boundary

## Candidate evaluation and selection

**Selected candidate: Black Forest Labs FLUX.1 [schnell].** BFL's pinned model card identifies a 12-billion-parameter rectified-flow text-to-image model, describes latent adversarial diffusion distillation and 1–4 generation steps, and labels the weights Apache-2.0. The official model card also contains out-of-scope-use guidance; this is recorded separately from Apache notice obligations. BFL's pinned repository README reports an MD5 for the Schnell model file, not the trusted SHA-256 and exact immutable weight-file metadata required by Nexora's media-model manager.

**Alternative considered: FLUX.1 [dev].** It is also a 12B BFL text-to-image model, but the same official repository labels its license non-commercial, while Schnell is Apache-2.0 and step-distilled for 1–4 steps. Schnell is the more defensible candidate for Nexora's intended local, broadly usable Sonnet experience. Neither choice by itself establishes that Nexora can install or run it.

| Evidence item | Grounded source fact | Nexora conclusion |
|---|---|---|
| Model identity | BFL's model card describes FLUX.1 [schnell] as a 12B rectified-flow text-to-image model. | Candidate selected; candidate ID is separate from `NEXORA_SONNET_ID`. |
| Quality/steps | BFL describes high output quality/prompt following and 1–4 distilled steps. | Good fit as an advanced candidate; not a local quality guarantee. No profile controls are exposed. |
| License | BFL labels Schnell Apache-2.0 and includes a separate model-card out-of-scope-use section. | License and publisher guidance must both be reviewed; no weights are redistributed here. |
| Runtime support | Pinned stable-diffusion.cpp source documents FLUX.1-schnell, the model/VAE/CLIP-L/T5-XXL CLI recipe, a Windows executable example, and CPU/CUDA/Vulkan/Metal/OpenCL/SYCL project backends. | Source-level model support is documented. This is not a Nexora binary, a runtime pin/checksum, or tested Windows compatibility. |
| Resource evidence | The pinned FLUX runtime guide says some Flux configurations can run with 4–6 GB VRAM. It does not establish a safe minimum for Nexora's exact Schnell assets/build; the shown q8 memory table is for FLUX.1-dev. | RAM, VRAM, total asset size, CPU practicality, backend availability, and model-specific fit remain unknown. No resource thresholds are registered. |

The BFL model-card and README source revision reviewed is `206c4d344174a265f8b4fc85d15bdcdf757df6f9`. It pins the reviewed metadata, **not** the model weights. The BFL README reports MD5 `a9e1e277b9b16add186f38e3f5a34044`; MD5 is not accepted as the trusted SHA-256 required by Nexora's manager. The exact weight revision, expected bytes, and SHA-256 are not established in the accessible official source metadata. The runtime guide lists four candidate components (`flux1-schnell.safetensors`, `ae.safetensors`, `clip_l.safetensors`, and `t5xxl_fp16.safetensors`), but a fully pinned, size/hash/license-complete manifest for these assets is not established. The guide also links a preconverted q8 GGUF variant, but this phase did not establish that conversion's immutable provenance, exact asset metadata, and trusted checksums, so it is not substituted. **Therefore FLUX.1-schnell is not registered as downloadable in the Phase 2 manager, and no model download is offered.**

## Runtime and compatibility boundary

The reviewed stable-diffusion.cpp source revision is `0321ce17fbf5eeca6f31b28b20577b789642d769`. Its FLUX guide documents the distinct components and Schnell command settings (`--cfg-scale 1.0`, Euler, four steps, and `--clip-on-cpu`). The source README lists Windows and several possible backends. The source repository's license at this revision is MIT; any future packaged build would still require review of its bundled third-party dependencies and notices. Upstream documentation establishes a plausible model/runtime pairing only. It does not establish a matching executable bundled or configured by Nexora, a pinned/checksummed binary, a known build feature set, an exact asset installation layout for Nexora, or model/runtime behavior on a Windows PC.

The current SD-Turbo adapter remains a separate Mini path: it is hard-coded for SD-Turbo, CPU, its own fixed request contract, and its own status/artifact handling. This phase does not modify or reuse that generation command. No runtime resolution/probe, hardware probe, resource assessment, model loading, download, or inference is performed for the FLUX candidate.

## Implemented in Phase 7

- Added a status-only `sonnet_image_generation` native boundary for the FLUX.1-schnell candidate. It reports the pinned source-document revisions, candidate identity/license, upstream-documented runtime support, incomplete weight/asset manifest, unconfigured Nexora runtime, unknown hardware/resource compatibility, and unavailable generation/artifact path.
- Added `get_sonnet_image_generation_status` and a typed desktop-only renderer wrapper. There is intentionally no request, load/unload, generation, progress, cancellation, or artifact command because there is no genuine Nexora executor.
- Extended the existing chat Image-mode capability note to query Mini's SD-Turbo status and the new Sonnet candidate status independently. The selected coding profile is unchanged; neither status path starts image inference or sends the prompt to a coding model. The Phase 6 composer and conversations otherwise remain unchanged.
- Did not add FLUX to the media-model manager. This prevents an incomplete manifest, MD5-only weight record, or source-level runtime documentation from being presented as a downloadable or validated installation.

## Capability state

**Implemented:** candidate evaluation; pinned source-document references; static status DTO and Tauri command; separate media-model identity; existing chat can present Mini and Sonnet status independently.

**Unavailable:** trusted complete asset registration/download; Nexora-compatible bundled/pinned FLUX runtime; model loading; generation; progress/cancellation; FLUX output validation and artifact serving. Sonnet image inference is not usable. Mini SD-Turbo remains independently unusable under its existing status.

**Unknown/unverified:** actual weight revision, exact file sizes/SHA-256 for all four components, total storage, RAM/VRAM requirements, compatibility of any particular runtime binary/build, local acceleration backend, Windows behavior, and generated image quality. Upstream source support and its general 4–6 GB VRAM statement are not Nexora compatibility or hardware-fit guarantees.

No build, test, benchmark, runtime/hardware probe, download, inference, or release-readiness assessment was run for Phase 7. At Phase 7 completion, later phases had not started; see the subsequent [Phase 8](./PHASE_8_ADVANCED_IMAGE_EDITING.md) and [Phase 9](./PHASE_9_SONNET_ADVANCED_VIDEO_GENERATION.md) records.

## Pinned references

- BFL FLUX.1-schnell model card, metadata revision `206c4d344174a265f8b4fc85d15bdcdf757df6f9`: <https://github.com/black-forest-labs/flux/blob/206c4d344174a265f8b4fc85d15bdcdf757df6f9/model_cards/FLUX.1-schnell.md>
- BFL model table and reported MD5, same revision: <https://github.com/black-forest-labs/flux/blob/206c4d344174a265f8b4fc85d15bdcdf757df6f9/README.md>
- Schnell Apache-2.0 license, same revision: <https://github.com/black-forest-labs/flux/blob/206c4d344174a265f8b4fc85d15bdcdf757df6f9/model_licenses/LICENSE-FLUX1-schnell>
- BFL FLUX.1-dev alternative and its model-card terms: <https://github.com/black-forest-labs/flux/blob/206c4d344174a265f8b4fc85d15bdcdf757df6f9/model_cards/FLUX.1-dev.md>
- Pinned stable-diffusion.cpp FLUX guide: <https://github.com/leejet/stable-diffusion.cpp/blob/0321ce17fbf5eeca6f31b28b20577b789642d769/docs/flux.md>
- Pinned stable-diffusion.cpp model/platform/backend overview: <https://github.com/leejet/stable-diffusion.cpp/blob/0321ce17fbf5eeca6f31b28b20577b789642d769/README.md>
- Pinned stable-diffusion.cpp build documentation: <https://github.com/leejet/stable-diffusion.cpp/blob/0321ce17fbf5eeca6f31b28b20577b789642d769/docs/build.md>
- Pinned stable-diffusion.cpp source license (MIT): <https://github.com/leejet/stable-diffusion.cpp/blob/0321ce17fbf5eeca6f31b28b20577b789642d769/LICENSE>

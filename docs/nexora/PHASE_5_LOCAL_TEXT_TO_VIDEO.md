# Phase 5 — Local text-to-video foundation

## Scope and current status

Phase 5 established a dedicated **text-to-video** model registration and a truthful status boundary. It did not implement video inference or start a runtime. It remains separate from coding-model inference, Phase 3 text-to-image, and Phase 4 image editing.

> **Phase 9 follow-up:** the existing read-only status command now includes a record-only snapshot of the Phase 2 media-model lifecycle (without calling adaptive/hardware collection or recomputing asset hashes). It still does not resolve a runtime, load weights, or perform video inference. See [PHASE_9_SONNET_ADVANCED_VIDEO_GENERATION.md](./PHASE_9_SONNET_ADVANCED_VIDEO_GENERATION.md).

| Capability | Phase 5 result |
|---|---|
| Model candidate | Wan2.1 T2V 1.3B, text-to-video only |
| Model manager | Registered in the Phase 2 media-model lifecycle with three pinned, checksummed artifacts |
| Runtime candidate | `stable-diffusion.cpp` `sd-cli` in `vid_gen` mode, documented at source revision `40b1788476bbe8fe42fa538ddf7d32408a126b93` |
| Runtime shipped/pinned as an executable | **No** |
| Runtime probe or hardware check performed | **No** |
| Model/runtime compatibility or hardware fit confirmed | **No; unknown** |
| Tauri operation | Read-only `get_local_video_generation_status` only; Phase 9 adds manager lifecycle state |
| Video generation, progress, cancellation, artifact serving | **Not exposed** |
| Studio/composer/chat integration | **Not in Phase 5** |

A model being registered, downloaded, or checksum-validated is a model-lifecycle/integrity state only. It is not evidence that a runtime exists, that the runtime recognizes these assets, that the PC can run the workload, or that generation is usable.

## Model and asset provenance

The Wan authors publish the model as text-to-video and state Apache-2.0 licensing. The documented `stable-diffusion.cpp` Wan2.1 T2V recipe uses `-M vid_gen` and three distinct files: a diffusion model, Wan VAE, and UMT5-XXL text encoder. Nexora registers this compatible-looking recipe as a candidate; it has not been run here.

| Required asset | Role / selected format | Immutable source revision | Exact bytes | SHA-256 |
|---|---|---|---:|---|
| `wan2.1_t2v_1.3B_fp16.safetensors` | Diffusion weights, FP16 safetensors | Comfy-Org commit `5601df2b72d74886b9df858fcd79c809cce4c4f3` | 2,838,303,560 | `be531024cd9018cb5b48c40cfbb6a6191645b1c792eb8bf4f8c1c6e10f924dc5` |
| `wan_2.1_vae.safetensors` | Wan2.1 VAE, safetensors | Comfy-Org commit `dfcea77bcf258496e20c69cd84e8e8e41909bb3b` | 253,815,318 | `2fc39d31359a4b0a64f55876d8ff7fa8d780956ae2cb13463b0223e15148976b` |
| `umt5-xxl-encoder-Q8_0.gguf` | Google UMT5-XXL encoder conversion, Q8_0 GGUF | city96 commit `81c2b6ffa438b6101e494a5985a4a9c1b731cfc0` | 6,043,068,256 | `2521d4de0bf9e1cc6549866463ceae85e4ec3239bc6063f7488810be39033bbc` |
| **Total** | 3 required managed files | Each download URL is revision-pinned | **9,135,187,134 bytes (~8.51 GiB)** | — |

The Comfy-Org files are repackaged Wan assets referenced by the runtime documentation, not the original Wan-AI repository's Diffusers-layout files. The chosen encoder is a third-party GGUF conversion, pinned independently; its Hub card identifies `google/umt5-xxl` as its base model and declares Apache-2.0. Nexora's existing media manager owns downloads, staging, checksums, installation records, and removal. It keeps these weights apart from coding models and generated media.

The registered model license metadata is Apache-2.0 for Wan2.1 and Google UMT5, and records that the city96 conversion card also declares Apache-2.0. Preserve applicable license/copyright notices, and review the notices for every artifact before redistribution. The candidate registry does not grant a broader license than the upstream sources.

## Runtime, operating systems, and backends

The upstream Wan guide demonstrates the dedicated video mode and required files, including an example at 832×480. The upstream README lists Windows and CPU, CUDA, Vulkan, Metal, OpenCL, and SYCL backends. These are **upstream source/project claims**, not a statement about any binary shipped by Nexora. The checked-in app has no identified, pinned, checksummed `sd-cli` executable for video; no executable is accepted from the renderer or searched for on the user's machine.

The upstream project is actively developed and warns that its CLI/API may change. The documentation revision is recorded so a later runtime integration can target a specific source snapshot. It is not an executable pin, reproducible build manifest, runtime checksum, or compatibility test. The upstream code is MIT-licensed; that is separate from the Apache-2.0 model and encoder artifacts.

The official Wan repository also documents a local Python/PyTorch inference route (`generate.py --task t2v-1.3B --size 832*480`, with optional model offload). That is a genuine upstream alternative, but it would introduce a separate Python/PyTorch runtime and asset/pinning/deployment surface; Nexora has not bundled or validated it. The Phase 5 registry targets the documented stable-diffusion.cpp asset layout instead, and does not treat the Python route or the Phase 3 image CLI as an available video runtime.

CPU inference is listed as a backend and a CPU-only build path is documented. No reliable model/runtime-specific CPU RAM minimum, practical CPU performance guarantee, or Nexora Windows CPU result was found. Therefore CPU support is not presented as a usable or recommended path. The upstream Wan authors' reference is approximately 8.19 GB VRAM and about four minutes for a five-second 480p run on RTX 4090 without quantization. That is a reported reference workload, **not a minimum**, local measurement, or performance promise; it does not qualify the selected mixed-precision asset set or any particular `stable-diffusion.cpp` build.

No RAM/VRAM minimum, temporary peak, or model-fit threshold is entered into the registry. Phase 2's adaptive compatibility view should remain unknown for this model until reliable model/runtime-specific requirements exist. Generic video screening is not a model fit guarantee. Since there is no generation operation, Phase 5 does not reject or launch work based on an unsupported estimate.

## Quantization and resource accounting

The registered diffusion transformer is FP16; the separate text encoder is Q8_0 GGUF; the VAE is the documented Wan2.1 safetensors file. This is the asset combination used in the upstream runtime example, not a claim that each component is independently quantized or that this mixed combination has a verified memory profile. The model-manager storage requirement is exact for the three required artifacts (about 8.51 GiB); generation working memory, runtime overhead, temporary output space, and additional safety margin are not yet established. The manager's download-space check remains separate from inference readiness.

The official Wan model card reports the 8.19-GB / RTX 4090 reference above. It must not be used as a hard minimum until a matching runtime, exact resolution/frame-count contract, and complete working-set evidence are established.

## Output encoding and future safe completion contract

At the reviewed upstream CLI revision, the CLI describes single-file video outputs as AVI, WebM, and animated WebP. Its source identifies AVI as MJPEG; WebM uses optional build features (`libwebp` for frame encoding and `libwebm` for muxing), and WebP also depends on the WebP build feature. **MP4 is not established by the reviewed CLI contract.** A later Nexora implementation must select and test a specific container/build path instead of assuming FFmpeg or MP4 support.

Nexora has not yet selected an output format, implemented a media encoder, created a managed video artifact store, or validated a video container. The future success contract must require all of the following: actual model inference completed; output written under Nexora-managed unique storage without overwriting prior successes; partial output safely removed after failure/cancellation; container structure and essential stream metadata validated; and only then a successful artifact response. File extension or process exit code alone is insufficient. No renderer-provided path, URL, or executable path may determine model/output access.

The status boundary deliberately exposes no generation, success, progress, cancellation, duration, FPS, resolution, or seed controls. It reports the absent runtime/output boundary and, since Phase 9, reads manager-recorded lifecycle state without an adaptive or hardware probe or fresh checksum pass.

## Implementation locations

- `apps/desktop/src-tauri/src/media_models/registry.rs` — Wan2.1 T2V definition and pinned diffusion/VAE/text-encoder files.
- `apps/desktop/src-tauri/src/video_generation/` — static adapter descriptor and status-only Tauri command.
- `apps/desktop/src-tauri/src/lib.rs` — module and status command registration.
- `apps/desktop/ui/src/lib/ipc.ts`, `videoGeneration.ts` — typed status contract and desktop-only wrapper; no Studio UI.

## Authoritative references

- Wan2.1 official T2V-1.3B model card, license, and published reference workload: <https://huggingface.co/Wan-AI/Wan2.1-T2V-1.3B>
- Wan2.1 official repository/license and inference code: <https://github.com/Wan-Video/Wan2.1>
- Pinned `stable-diffusion.cpp` Wan guide (commit `40b1788476bbe8fe42fa538ddf7d32408a126b93`): <https://github.com/leejet/stable-diffusion.cpp/blob/40b1788476bbe8fe42fa538ddf7d32408a126b93/docs/wan.md>
- Pinned `stable-diffusion.cpp` README (platforms, backends, and video-model listing): <https://github.com/leejet/stable-diffusion.cpp/blob/40b1788476bbe8fe42fa538ddf7d32408a126b93/README.md>
- Pinned runtime build notes for CPU and WebP/WebM options: <https://github.com/leejet/stable-diffusion.cpp/blob/40b1788476bbe8fe42fa538ddf7d32408a126b93/docs/build.md>
- Pinned runtime CLI output-format contract: <https://github.com/leejet/stable-diffusion.cpp/blob/40b1788476bbe8fe42fa538ddf7d32408a126b93/examples/cli/main.cpp>
- Wan diffusion weights, Comfy-Org pinned commit: <https://huggingface.co/Comfy-Org/Wan_2.1_ComfyUI_repackaged/blob/5601df2b72d74886b9df858fcd79c809cce4c4f3/split_files/diffusion_models/wan2.1_t2v_1.3B_fp16.safetensors>
- Wan VAE, Comfy-Org pinned commit: <https://huggingface.co/Comfy-Org/Wan_2.1_ComfyUI_repackaged/blob/dfcea77bcf258496e20c69cd84e8e8e41909bb3b/split_files/vae/wan_2.1_vae.safetensors>
- Q8_0 UMT5 encoder conversion and declared license, city96 pinned commit: <https://huggingface.co/city96/umt5-xxl-encoder-gguf/blob/81c2b6ffa438b6101e494a5985a4a9c1b731cfc0/umt5-xxl-encoder-Q8_0.gguf>
- Google UMT5-XXL model card and Apache-2.0 metadata: <https://huggingface.co/google/umt5-xxl>

## Verification status and phase boundary

No build, test, benchmark, runtime probe, hardware check, model download, or inference was run for Phase 5. Phase 6 and later work is not included. Continue only after explicit Phase 6 authorization.

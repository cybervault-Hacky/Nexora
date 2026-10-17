# Model and Runtime Acquisition Policy

**Scope:** how Nexora obtains, verifies, stores, licenses, and removes model weights and runtime archives.
**Threat model:** a malicious or mislabelled file, a compromised upstream account, a corrupted download, an unlicensed redistribution, or a silent upstream change.
**Status:** policy only. No file was downloaded in Phase 2.

---

## 1. Principles

| # | Principle |
|---|---|
| A-1 | **Allowlisted sources only.** A model is fetched from a named repository at a pinned commit, never from a search result or a mirror of unknown provenance. |
| A-2 | **Pin the bytes, not the name.** A tag like `Q4_K_M` is not an identity; the SHA-256 is. |
| A-3 | **Quarantine before promotion.** A downloaded file is inert until it passes structural and hash validation. |
| A-4 | **Ship no weights in the installer.** Models are downloaded on explicit user action, with size, licence, and source shown first. |
| A-5 | **Every file has a licence and a notice.** If the notice cannot be produced, the file is not shippable. |
| A-6 | **Revocation is possible without an app update.** A signed kill switch can block a model or runtime build. |

## 2. Source tiers

| Tier | Sources | Use |
|---|---|---|
| **1 — Vendor** | `Qwen/*`, `mistralai/*`, `openai/*`, `google/*`, `JetBrains/*`, `zai-org/*`, `nvidia/*` on Hugging Face | Preferred. Official weights and official GGUFs where published. |
| **2 — Trusted quantiser** | `unsloth/*`, `ggml-org/*` | Allowed when no official GGUF exists. Extra review: the base model must be Tier 1 and the repo must declare the base model and licence. |
| **3 — Community** | Any other Hugging Face user | **Prohibited at 1.0.** |
| **4 — Anything else** | Forums, mirrors, torrents, "model packs", Discord links | **Prohibited, always.** |

Tier 2 is a real trust decision: a quantiser can alter chat templates, add a system prompt, or ship a subtly different model. Unsloth's Qwen GGUFs, for example, include their own Apache-2.0 chat-template fixes. That is acceptable, but the template must be reviewed and recorded, not assumed.

## 3. Approved artifacts for 1.0

| Purpose | Artifact | Tier | Licence |
|---|---|---|---|
| Default 24 GB | `unsloth/Qwen3.8-27B-GGUF` → `Qwen3.8-27B-UD-Q4_K_M.gguf` | 2 | Apache-2.0 |
| Default 16 GB | `ggml-org/gpt-oss-20b-GGUF` → `gpt-oss-20b-MXFP4.gguf` | 2 | Apache-2.0 |
| Default 12 GB | `JetBrains/Mellum2.1-12B-A2.5B-Thinking-GGUF` → `…-Q4_K_M.gguf` | 1 | Apache-2.0 |
| Default 8 GB / CPU | `Qwen/Qwen2.5-Coder-7B-Instruct-GGUF` → `…-q4_k_m.gguf` | 1 | Apache-2.0 |
| Alternative 24 GB | `unsloth/Qwen3-Coder-30B-A3B-Instruct-GGUF` → `…-Q4_K_M.gguf` | 2 | Apache-2.0 |
| Watchlist | `unsloth/Devstral-…-GGUF`, `unsloth/gemma-4-31B-it-GGUF`, `zai-org/GLM-4.7-Flash`, `nvidia/NVIDIA-Nemotron-3.5-Bolt-…-GGUF` | 1–2 | Apache-2.0 / MIT / OpenMDW-1.1 |

Every entry must be re-verified at build time. A row in this table is not a licence to fetch "whatever the repo currently serves".

## 4. Revision pinning

| Item to pin | How | Stored in |
|---|---|---|
| Model repository | Full commit SHA (40 hex), not branch/tag | `model-manifest.json` |
| Base model | Full commit SHA, not only the quantised repo | Manifest |
| File | Exact filename + size + SHA-256 | Manifest |
| Chat template | Hash of the template text used at runtime | Manifest + result files |
| Quantisation | Quant name, quantiser, imatrix used if any | Manifest |
| Licence | SPDX identifier + licence text hash | Manifest |
| Runtime build | `b####` tag + asset filename + SHA-256 | `runtime-manifest.json` |
| Installer-side trust | Signature over the manifest (Ed25519/minisign) | Manifest envelope |

Hugging Face's tree API exposes an `lfs.oid` that is the SHA-256 of the stored file (observed on every repository inspected in this phase). That is used as the expected hash, and the client **recomputes** it locally after download. A hash taken from the same response that delivered the file is a consistency check, not an authenticity guarantee; authenticity comes from the pinned manifest and its signature.

## 5. Integrity validation

### 5.1 Before download

- Repository and revision are on the allowlist.
- Expected size is known and the disk has `size × 1.1 + 2 GB` free.
- Licence is on the allowlist and the notice text is available.
- The architecture is in the pinned runtime's supported list.

### 5.2 After download (quarantine)

| Check | Failure action |
|---|---|
| SHA-256 equals manifest | Delete; report `integrity_mismatch` |
| Size within ±2% of manifest | Re-download once, then fail |
| First 4 bytes are `GGUF`; version is 2 or 3 | Reject as `not_gguf` |
| Metadata `general.architecture` equals the expected value | Reject as `wrong_architecture` |
| `general.file_type` matches the expected quant class | Reject as `wrong_quant` |
| Tokenizer metadata present, vocabulary size plausible | Reject as `broken_tokenizer` |
| Tensor count and offsets consistent with file size; no trailing payload | Reject as `malformed_container` |
| For sharded files: all shards present, `split.count`/`split.no` consistent, every shard hash matches | Reject as `incomplete_shards` |
| No executable/archive magic anywhere in the header region (`MZ`, `PK`, `7z`, `ELF`) | Quarantine and flag |
| Optional load test: start the pinned runtime with `--ctx-size 512`, 60 s budget | Reject as `load_failure` |

A file that passes is moved atomically from `quarantine/` to `models/<id>/`, and its sidecar record is written. A file that fails is deleted, never renamed into the store, and the reason is logged.

### 5.3 Runtime archives

- Verify the archive hash against the pinned `runtime-manifest.json`.
- Verify every extracted binary's hash after extraction (zip entries can be duplicated).
- Reject path traversal (`..\`), absolute paths, NTFS alternate data streams, and symlinks in the archive.
- Do not execute anything from the archive during extraction.
- Sign the extracted set with the Nexora manifest signature at build time.

## 6. Mirroring and availability

A desktop installer cannot depend on GitHub or Hugging Face being reachable at install time, and a pinned build tag can disappear if an upstream repository is deleted.

| Decision | Detail |
|---|---|
| Mirror runtime builds | Nexora-controlled storage, immutable paths keyed by hash, signed manifest |
| Mirror model files | Optional at 1.0; if used, Nexora becomes a redistributor and must carry notices ([`§9`](#9-notices-and-attribution)) |
| Direct-from-source fallback | Allowed for Tier 1/2 with hash verification, but never silently: show the host and size |
| Availability check | The app can install and run entirely from the mirror with no external network |
| Retention | Keep the pinned build and model revision for the life of the supported release line |

Mirroring is a budget item, not just an engineering one: at the sizes in §7, a full mirror is tens of gigabytes per release channel (`Q2-02`).

## 7. Storage requirements

| Item | Size |
|---|---|
| Runtime, CPU pack | tens of MB **[E]** |
| Runtime, CUDA 12.4 pack + cudart | 300–600 MB **[E]** (Phase 1 budget) |
| Runtime, Vulkan pack | 30–60 MB **[E]** (Phase 1 budget) |
| Default model, 24 GB tier | 16.46 GB |
| Default model, 16 GB tier | 12.11 GB |
| Default model, 12 GB tier | 8.07 GB |
| Default model, 8 GB tier | 4.68 GB |
| Embedding model (if used for repo index) | 0.1–0.6 GB **[E]** |
| Temporary space during download | file size + 10% |
| Logs, checkpoints, index | 1–5 GB over time **[E]** |

Rules:

- Installer requires 2 GB free before install; model download requires its own preflight.
- The store keeps **one default model per tier**; adding an alternative prompts for space.
- Eviction is LRU **with user approval**; user-imported models are never auto-deleted.
- Downloads pause on a metered connection unless the user opts in.
- A partial download is resumable for 7 days, then purged.

## 8. Interrupted downloads

```
GET with Range: bytes=<offset>-
If-Range: <etag>
write to <file>.part
state in <file>.part.json {url, etag, size, offset, sha256_expected}
on completion: fsync → verify SHA-256 → atomic rename → write sidecar
```

| Failure | Behaviour |
|---|---|
| Network drop | Retry 3× with exponential backoff; resume from offset |
| App exit | Resume on next launch if the partial is <7 days old |
| Server ignores Range (200 instead of 206) | Restart from zero, keep the old partial until the new one verifies |
| ETag changed | Discard the partial; the upstream file moved |
| Hash mismatch after full download | Delete both `.part` and state; allow one retry; then report |
| Disk full mid-download | Stop, keep partial, tell the user the exact shortfall |
| Antivirus quarantines the archive | Report `blocked_by_av` with the path; never auto-retry in a loop |

## 9. Notices and attribution

Every shipped model and runtime produces an entry in `THIRD-PARTY-NOTICES`, generated at build time from the manifest (not hand-maintained):

```
Qwen3.8-27B (UD-Q4_K_M)
  Base model: Qwen/Qwen3.8-27B @ 1d4bf0f2… (Apache-2.0)
  Quantised by: unsloth/Qwen3.8-27B-GGUF @ 4ca72078… (Apache-2.0)
  Modification: 4-bit K-quantisation (UD-Q4_K_M); chat template from the quantiser
  Licence text: licenses/Apache-2.0.txt
```

| Obligation | Implementation |
|---|---|
| Include licence text | `resources/licenses/<spdx>.txt`, also shown in-app |
| Retain NOTICE | Copy any upstream `NOTICE` into `resources/notices/` |
| State changes | "Quantised to UD-Q4_K_M by Unsloth" + any template override |
| No endorsement | Nominative use only; no vendor logo, no "powered by" claim without permission |
| No trademark misuse | Model names appear in a "Models" screen, not in the app name or icon |
| Source offer | If Nexora mirrors weights, publish the mirror and the exact revision |

The Devstral template's embedded "Mistral Vibe" persona and commit trailer is an attribution/persona issue as much as a quality issue: it must be replaced, and the replacement recorded as a modification.

## 10. Redistribution and commercial restrictions

| Licence | Can Nexora redistribute the weights? | Conditions |
|---|---|---|
| Apache-2.0 | Yes | Licence + NOTICE + statement of changes + no endorsement |
| MIT | Yes | Copyright + permission notice |
| OpenMDW-1.1 | Yes, per the card | Retain licence and notices; legal review before shipping |
| Qwen3.8-Max custom | Not evaluated | Excluded |
| Mistral Non-Production | **No** | Excluded |
| Llama 4 Community | Conditional | Excluded |
| NVIDIA CUDA runtime | Only listed files, per the EULA | Legal review; ship only the redistributable subset (`Q2-03`) |
| Vulkan loader | Permissive (Apache/MIT-style) | Notice **[U]** — verify the exact loader licence before bundling |

A model under an unclear licence is treated as non-redistributable. "It was on Hugging Face" is not a licence.

## 11. Malicious and mislabelled files

| Attack | Example | Control |
|---|---|---|
| Typosquat repository | `Qwen/Qwen3.8-27B-GGUF` vs an unofficial lookalike | Exact allowlist of `org/repo`; UI shows the full repo and hash |
| Mislabelled quant | File named Q4_K_M but quantised at Q2 | `general.file_type` check + hash pin |
| Template injection | Custom chat template adds a system instruction or tool | Template hash pinned; diff reviewed when it changes |
| Appended payload | Extra bytes after the tensor region | Container consistency check |
| Archive traversal | `..\..\Windows\System32\...` in a runtime zip | Path normalisation + reject |
| Executable masquerade | `.gguf` that starts with `MZ` | Magic checks before and after download |
| Pickle/deserialisation | A `.bin`/`.pt` checkpoint that executes on load | Only GGUF is accepted; no Python deserialisation anywhere |
| Supply-chain compromise | Upstream account replaced the file at the same path | Revision + hash + manifest signature; mirror is immutable |
| Model that is simply bad | Fine container, poor quality | Benchmark gates; not a security control |

## 12. Revocation and update

| Control | Behaviour |
|---|---|
| Model kill switch | Signed list of blocked hashes/revisions with a reason and severity |
| Severity `security` | Block loading immediately; explain; offer a safe model |
| Severity `quality` | Warn, do not block |
| Runtime kill switch | Block a build; force the previous pinned build; never auto-download a replacement |
| Manifest rotation | New manifest signed with a key that the app already trusts; no "trust on first use" |
| Offline revocation | The last fetched list is cached; an offline app keeps working, and a security block requires a successful signed update to lift |
| Rollback | Keep the previous model and runtime until the new one passes a smoke load |

Kill switches are Phase 1 doc 09 decisions and are repeated here because acquisition without revocation is permanent trust in a single download.

## 13. Audit trail

The app records, locally and append-only:

- every install, verification, promotion, eviction, and deletion;
- model id, repository, revision, file hash, size, licence;
- runtime build tag and archive hash;
- the actor (user action vs automatic), timestamp, and result.

The log contains no prompts and no file contents. It is exportable in an opt-in diagnostics bundle, redacted per Phase 1 doc 07. It exists so that "which model produced this output?" has an answer months later.

## 14. Open items

| ID | Item |
|---|---|
| Q2-02 | Mirror budget and hosting approval |
| Q2-03 | Legal review: OpenMDW-1.1, CUDA runtime redistribution, Vulkan loader notice |
| Q2-12 | Who signs the manifests, and where the private key lives (HSM vs offline) |
| Q2-13 | Whether to mirror weights at 1.0 or only runtime archives |
| Q2-14 | Whether a model load-test at install is acceptable on low-end machines (60 s budget) |

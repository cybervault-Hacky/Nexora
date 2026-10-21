# Nexora Sonnet — artefact evidence

Collected 2026-10-20. Every value below was read from the publisher's own hosting
API or file-info panel, not computed, not estimated, and not copied from another
model's record.

## 1. The verified artefact

| Field | Value |
| --- | --- |
| Repository | `Qwen/Qwen2.5-Coder-14B-Instruct-GGUF` |
| Publisher | Qwen team, Alibaba Group |
| Immutable revision | `d0a692ef765eefbf2fabb130b3cb2e8917e3d225` |
| File name | `qwen2.5-coder-14b-instruct-q4_k_m.gguf` |
| Size (bytes) | `8 988 110 272` |
| Size (display) | 8.99 GB = 8.37 GiB |
| SHA-256 | `c1e659736d89ac1065fb495330fb824d94001974a4bfa78e7270e43476a8d940` |
| Licence | Apache-2.0 |
| Format / quantisation | GGUF, Q4_K_M, single file |
| Base model | `Qwen/Qwen2.5-Coder-14B-Instruct` @ `aedcc2d42b622764e023cf882b6652e646b95671` |
| Gated | No |
| Context (GGUF, per publisher) | 32,768 tokens |

## 2. Where each value came from

The revision `d0a692ef…` is the commit recorded by the model API for this
repository. The same tree was fetched twice — once at `main`, once at that
commit — and both returned identical object ids and byte lengths for this file,
which is why the commit is safe to pin.

| Value | Source | What it showed |
| --- | --- | --- |
| Revision | `https://huggingface.co/api/models/Qwen/Qwen2.5-Coder-14B-Instruct-GGUF` | `"sha":"d0a692ef765eefbf2fabb130b3cb2e8917e3d225"`, `"gated":false`, `license:apache-2.0` |
| Size + SHA-256 | `…/api/models/…/tree/d0a692ef…` | `size: 8_988_110_272`, `lfs.oid: c1e65973…894d3c` |
| Size + SHA-256 (repeat) | `…/api/models/…/tree/main` | identical values |
| SHA-256 (independent surface) | `…/blob/d0a692ef…/qwen2.5-coder-14b-instruct-q4_k_m.gguf?show_file_info=…` | `SHA256: c1e6597…894d3c`, `Size of remote file: 8.99 GB` |
| Licence text | `…/raw/d0a692ef…/LICENSE` (11 343 bytes) | full Apache License 2.0 text |
| Licence declaration | `…/raw/d0a692ef…/README.md` + `cardData.license` | `license: apache-2.0` |
| Context ceiling | GGUF repo `README.md` | "Context Length: Full 32,768 tokens … only vLLM supports YARN" |
| Architecture | base-model `config.json` | `model_type: qwen2`, 48 layers, 40 Q heads, 8 KV heads, `max_position_embeddings: 32768` |
| Base-model revision | `https://huggingface.co/api/models/Qwen/Qwen2.5-Coder-14B-Instruct` | `"sha":"aedcc2d42b622764e023cf882b6652e646b95671"` |

### Why the LFS object id is a SHA-256

Hugging Face stores large files with Git LFS, whose object id **is** the SHA-256
of the file content, and the client verifies downloads against it. This
repository is migrating to Xet storage; the file-info panel prints both hashes
separately — Xet hash `f87bfd65…3ba18`, SHA-256 `c1e65973…894d3c` — which
confirms the value recorded here is the SHA-256 and not the Xet hash.

## 3. What remains unverified

| Item | Status |
| --- | --- |
| The bytes themselves | **Not fetched.** Sandbox egress to `huggingface.co` returns HTTP 000 |
| Digest computed locally from the file | **Not possible here** |
| HTTP behaviour of the `/resolve/` redirect chain | **Not observed** in this environment |
| llama.cpp loading this exact file | **Not observed** |
| Any inference output | **None recorded** |

The digest is therefore *host-published and cross-checked on three surfaces*,
not *locally recomputed*. Nexora's download path re-verifies it after transfer,
so a mismatch fails closed and nothing is installed.

### Source-tool note

Metadata was retrieved through the research tool's HTTP fetcher, which reaches
the public model host. The application sandbox itself cannot: a direct HEAD to
the same resolve URL from inside the sandbox returned `000`. This distinction
matters — it means the metadata is real, while a download from *this machine*
is impossible.

## 4. Hardware arithmetic

Measured from the base model's `config.json`, not assumed:

```
KV cache per token = 2 (K and V) × 48 layers × 8 KV heads × 128 head_dim × 2 bytes
                   = 196 608 bytes = 192 KiB
   8 192-token context = 1.50 GiB
  32 768-token context = 6.00 GiB
```

| Requirement | Value | Kind |
| --- | --- | --- |
| Weights on disk | 8.99 GB | **measured** (host metadata) |
| KV cache at 8,192 | 1.50 GiB | **computed** from `config.json` |
| Nexora's own RAM estimate at 8,192 | ≈ 10.2 GiB | **estimate** (`models.rs`: size + context allowance + 1 GiB) |
| This verification machine | 3 GB RAM, 19 GB free disk | **measured** (`free`, `df`) |

Conclusion: the disk is sufficient, the memory is not — 3 GB against ~10 GiB.

## 5. Compatibility with the existing pipeline

| Requirement | Result |
| --- | --- |
| QQwen2 architecture in llama.cpp | Supported; same runtime family as Mini |
| Single-file GGUF | Yes — the repository also ships split shards, which Nexora does not support |
| Size fits `u64` handling | Yes; acquisition is `u64` throughout, no 32-bit truncation |
| URL the app builds | `https://huggingface.co/Qwen/Qwen2.5-Coder-14B-Instruct-GGUF/resolve/d0a692ef…/qwen2.5-coder-14b-instruct-q4_k_m.gguf` — identical to the link the host itself advertises |
| Redirect policy | 5 https-only hops; huggingface.co → CDN is within that |
| GGUF header read | Bounded to 32 MiB of metadata, so a 9 GB file is not loaded whole |
| `MIN_PLAUSIBLE_BYTES` (16 MiB) | Satisfied |

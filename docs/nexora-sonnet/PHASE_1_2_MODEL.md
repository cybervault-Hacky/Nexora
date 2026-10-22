# Phases 1–2 — Model selection and artefact definition

## 1. What was selected

| Field | Value |
| --- | --- |
| Product name | Nexora Sonnet (internal; not Anthropic) |
| Upstream model | `Qwen/Qwen2.5-Coder-14B-Instruct` |
| Publisher | Qwen team, Alibaba Group |
| Format | GGUF, single file |
| Quantization | Q4_K_M |
| Licence | Apache-2.0 |
| Documented context | 32,768 tokens (131,072 requires YaRN, which Nexora does not enable) |
| Runtime | llama.cpp `llama-server`, build 11714 or compatible — the same requirement as Nexora Mini |
| Execution | CPU only. Nexora passes no GPU-offload, KV-cache, or flash-attention flags |
| Approx. weights | ~9 GB at Q4_K_M |
| Recommended context | 8,192 |

## 2. Why this model

1. **Same family and pipeline as Mini.** The template, GGUF format, tokenizer behaviour, and
   `llama-server` invocation are already proven for the 7B sibling. A second family would add
   prompt-format and runtime risk with no product benefit at this stage.
2. **A real step up.** 14B is materially larger than the 7B default while still fitting a
   16 GB machine, which is the design target recorded in earlier phases.
3. **32B does not fit the target.** A 32B Q4_K_M artefact is roughly 20 GB of weights alone;
   it cannot run on a 16 GB machine and would make "Sonnet" a profile nobody could use.
4. **Licence is compatible.** Apache-2.0 permits local redistribution-free use; Nexora ships
   no weights at all, so only the user's own download is involved.

## 3. Memory arithmetic

Qwen2.5-14B has 48 layers with 8 KV heads of 128 dimensions. With f16 KV cache:

```
per token = 48 layers × 8 heads × 128 dims × 2 (K and V) × 2 bytes
          = 196,608 bytes ≈ 192 KiB
8,192 tokens  ≈ 1.50 GiB
32,768 tokens ≈ 6.00 GiB
```

Weights (~9 GB) plus 8,192-token cache (~1.5 GiB) plus runtime overhead lands near 11–12 GiB,
which is viable on 16 GB and *not* viable on this verification machine (3.85 GiB total).

**Caveat, stated plainly:** these architecture numbers came from the publisher's published
configuration, not from a file in this repository. They have since been re-read from the
`config.json` of the pinned base revision
(`aedcc2d4…/config.json`: 48 layers, 8 KV heads, 128 head dimension) and encoded as
`kv_bytes_per_token = 196 608` in the registry, so the runtime arithmetic no longer relies on
this table. See [READINESS.md](./READINESS.md).

## 4. Artefact verification (resolved 2026-10-20)

The earlier version of this section said the artefact metadata was unavailable. It has since
been verified from the publisher's hosting API and recorded in the registry. Full evidence and
source URLs are in [ARTIFACT_EVIDENCE.md](./ARTIFACT_EVIDENCE.md).

| Field | Value |
| --- | --- |
| Repository | `Qwen/Qwen2.5-Coder-14B-Instruct-GGUF` |
| Immutable revision | `d0a692ef765eefbf2fabb130b3cb2e8917e3d225` |
| File | `qwen2.5-coder-14b-instruct-q4_k_m.gguf` |
| Bytes | `8 988 110 272` |
| SHA-256 | `c1e659736d89ac1065fb495330fb824d94001974a4bfa78e7270e43476a8d940` |
| Licence | Apache-2.0 (LICENSE read at the pinned commit) |

`ArtifactState::Pending` remains part of the model — a future profile can still be registered
without inventing metadata — but no registered profile is in that state today.

### What is still not verified

- The bytes have never been downloaded here, so the digest has not been recomputed locally.
  It is host-published and cross-checked on three surfaces, and the download path re-checks it
  after transfer, failing closed on a mismatch.
- No llama.cpp load and no generation have been recorded, so `locally_verified` stays `false`.

## 5. How to record a local verification

1. On a machine whose **available** memory exceeds the estimate for the context you intend to
   use — 10.87 GiB at 8 192 tokens, 15.37 GiB at 32 768 (see
   [READINESS.md](./READINESS.md)); 16 GB is a reasonable rule of thumb, not a product
   requirement — install a llama.cpp build, download the profile through Nexora, and let the
   digest check pass.
2. Load it, send one prompt, and record tokens/s and peak RSS.
3. Only then set `locally_verified: true` for `NEXORA_SONNET`.

## 6. Readiness replaces the fixed RAM figure (2026-10-21)

The "16 GB machine" wording above was a design target, and it is the kind of fixed number that
is wrong in both directions: a 16 GB machine with 12 GB already in use cannot load this model
at 32k context, and a 12 GB machine with 10.8 GiB genuinely free can load it at 8k. Nexora no
longer encodes a per-profile RAM minimum. The core measures available memory and free disk
space, multiplies the published KV geometry by the configured context, and reports one of
fourteen states per profile. The method, its constants, and the states are documented in
[READINESS.md](./READINESS.md); the numbers are always labelled as estimates.

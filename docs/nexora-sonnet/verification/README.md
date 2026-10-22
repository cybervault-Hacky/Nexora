# Phase 9 verification scripts

Evidence scripts from the 2026-10-22 run. They test the runtime contract only.
None of them uses the approved Sonnet or Mini artefact.

| File | Purpose | Model used |
| --- | --- | --- |
| `make_synthetic_gguf.py` | Writes a random-weight, 2-layer qwen2-architecture GGUF for transport tests | Synthetic. Not Sonnet, not Mini. |
| `stream_check.py` | Auth refusal (401), and the SSE shape, `finish_reason`, and `[DONE]` of a full reply | Synthetic |
| `cancel_check.py` | Client disconnect mid-stream, then a follow-up request that must be served | Synthetic |
| `trunc_check.py` | Engine killed mid-reply; the client must see no completion signal | Synthetic |
| `harness_extended.py` | `rustc --test` on the real `error`, `registry`, `settings`, `loading`, `budget`, `usage` sources. serde derives and serde_json tests stripped. Not `cargo test`. | n/a |

Scripts expect the workspace paths used in the run, `/home/user/.nexora-verify/`, and a
local build of llama.cpp b11714 (`llama-server --version` reports build 11714). They are
kept here for reproducibility; the binary and model are not committed.

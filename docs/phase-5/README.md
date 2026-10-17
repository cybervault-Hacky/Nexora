# Phase 5 — local inference integration

Phase 5 connects the Phase 4 desktop shell to a real local engine: a
user-supplied `llama-server` process is supervised by Rust, validated before
launch, and streamed from over a loopback-only, key-protected HTTP endpoint.
There are no mock replies, no bundled weights, and no automatic downloads.

Status: **implemented, partially verified.** The TypeScript side is built and
tested; the Rust crate has not been compiled in this environment and no genuine
model inference has been run. The table below records what was checked against
the pinned upstream source, what is assumed, and what a Windows machine must
still confirm.

## Runtime contract verification

This note records exactly which upstream interfaces the Phase 5 integration was
written against, how they were verified, and what remains unverified. It exists
so a reviewer can see the difference between *checked against the pinned source*
and *assumed*.

## Pinned runtime

| Item | Value |
|---|---|
| Project | llama.cpp (`ggml-org/llama.cpp`) |
| Tag | **`b11714`** (nightly build) |
| Published | 2026-10-17T16:17:08Z (GitHub releases API) |
| Release kind | prerelease / nightly — pinned deliberately, never `master` |
| Licence | MIT (per Phase 2 evaluation) |
| Server source of truth | `tools/server/README.md` at tag `b11714` |
| Verification method | GitHub contents API, content decoded from base64 |

The server documentation moved from `examples/server/` to `tools/server/` after
Phase 2 was written. The Phase 2 *decisions* still hold, but the path changed;
the pinned tag is what matters.

Nexora does **not** download, bundle, or redistribute this runtime. The user
supplies an executable built from (or at least comparable to) this tag. At
probe time the UI reports the version line the binary prints and flags a build
that does not match `11714` as unverified.

## Verified against `b11714`

| Interface | Upstream statement (abridged) | Used in |
|---|---|---|
| `--host HOST` | default `127.0.0.1` | `process.rs` passes `--host 127.0.0.1` explicitly |
| `--port PORT` | default `9931` | an ephemeral loopback port is chosen per launch |
| `--ctx-size` | context size in tokens | from validated settings, clamped 2 048–131 072 |
| `-np, --parallel N` | number of server slots | `--parallel 1` |
| `--no-webui` (alias `--no-ui`) | disables the Web UI | always passed |
| `--jinja` | jinja chat templates (default enabled) | passed explicitly |
| `--api-key-file FNAME` | file of API keys, one per line, `#` comments | a 32-byte random hex key, one line |
| `-to, --timeout N` | server read/write timeout, seconds (default 3600) | `--timeout 600` |
| `GET /health` | public, no key check; `503` + `{"error":…"Loading model"}` while loading, `200` + `{"status":"ok"}` when ready | load wait and readiness checks |
| `POST /v1/chat/completions` | OpenAI-compatible chat API; "both synchronous and streaming mode are supported" | the only generation call |
| `POST /slots/{id_slot}?action=erase` | erase the prompt cache of a slot | best-effort cleanup after a stop |
| `Authorization: Bearer <key>` | shown in every upstream request example | all authenticated calls |

The upstream README also states that `/health` is unauthenticated. That is why
Nexora treats it **only** as a liveness signal and never as proof of generation.

## Assumed, not proven here

These are consistent with OpenAI-compatible behaviour and with the Phase 2
evaluation, but could not be exercised without a binary and a model:

- the exact SSE framing: `data: {json}` lines, a final `data: [DONE]`, and a
  first chunk that may carry a role but no content;
- the shape of `choices[0].delta.content` and `choices[0].finish_reason` across
  all chunk types;
- the shape of an error body when the request is rejected (only the first 300
  characters are surfaced in the UI message);
- whether `POST /slots/0?action=erase` is available in every build with
  `--no-webui` (failure is ignored by design);
- the first line printed by `--version` for a given build (the parser only looks
  for the pinned number anywhere in that line);
- real prefill and decode timings on any hardware.

`client.rs` is deliberately tolerant of the first three: unknown or malformed
JSON chunks are ignored, `[DONE]` is optional (end-of-stream also ends a reply),
and only `content` text is appended.

## Local verification actually performed

| Check | Result |
|---|---|
| Rust syntax parse of all 11 source files (tree-sitter) | 0 syntax errors |
| `cargo check` / `cargo test` | **not run** — no Rust toolchain, `static.rust-lang.org` and `crates.io` unreachable |
| Genuine model inference | **not performed** — no runtime binary, no GGUF weights, and no downloads are allowed without owner consent |
| TypeScript contract against `@tauri-apps/api` 2.12.4 | `pnpm typecheck` pass (imports `Channel`, `invoke` resolve) |
| UI behaviour with the IPC boundary replaced by a double | 85 Vitest tests pass; labelled as doubles, not inference |

## What a Windows run must confirm

1. `cargo generate-lockfile`, `cargo fmt --check`, `cargo clippy -- -D warnings`,
   `cargo test` — none of these have ever been executed for this crate.
2. The engine starts, `/health` flips from 503 to 200, and the sidebar reports
   **Ready**.
3. A short prompt streams token by token into the UI, and the reply settles as
   complete.
4. **Stop** terminates the stream, keeps partial text, and labels it
   `Stopped`; the engine accepts the next request.
5. A second send while streaming is refused with `busy`.
6. A wrong runtime path, a non-GGUF file, a truncated GGUF file, and an
   unloadable model each produce a clear error and leave no orphan process.
7. Closing the window during generation leaves no `llama-server` process behind
   (Task Manager).
8. Offline: with the network adapter disabled, setup and generation still work.

# Phase 6 — native verification, runtime stabilisation, and local inference

**Verdict: partially verified.** The runtime contract was executed against a
`llama-server` built from the pinned source, which found and fixed four real
integration defects. The Rust backend is still **uncompiled** and no genuine
model output was produced, for the two concrete reasons below.

## Why this is not "verified"

| Blocker | Evidence |
| --- | --- |
| No Rust toolchain | `rustc`, `cargo`, `rustfmt`, `clippy-driver` are all absent; `static.rust-lang.org` returns no connection (`000`) |
| No crate registry | `crates.io` and `index.crates.io` return `000`, so dependencies cannot be resolved and no `Cargo.lock` can be generated |
| No model weights | `huggingface.co` returns `000`; no GGUF files exist on the host; GitHub hosts only vocabulary-only GGUFs |
| No Windows host | this is Debian 12, and even the Linux Tauri build is impossible (`pkg-config`/WebKitGTK are not installed) |

No untrusted toolchain was installed and no blocked download was retried.

## Repository state

| Item | Value |
| --- | --- |
| Branch | `arena/00bd2df7-nexora` |
| Base | `b1d92ae` (Phase 5 tip) |
| Phase 6 commits | `25117b9` contract fixes, `b7f1371` SSE/cancellation verification |
| Remote | `https://github.com/cybervault-Hacky/Nexora.git` (private) |
| Public release / installer / PR | none |

## Environment (measured)

| Component | Version |
| --- | --- |
| OS | Debian GNU/Linux 12 (bookworm), Linux 6.1.158+ |
| CPU / RAM | 2 vCPU / 3.8 GiB (3.4 GiB available) |
| Node / pnpm | v22.22.3 / 9.15.4 (`packageManager` pin honoured) |
| Compilers | g++ 12.2.0, CMake 4.4.4 (PyPI wheel), Python 3.11.2 |
| Rust | **absent** (all four binaries) |
| llama.cpp | built from tag `b11714`, commit `c0fd78626405c6d9b934efef0ec7291fcc20e3d6` |

## Rust audit and fixes

`cargo fmt`, `cargo check`, `cargo clippy` and `cargo test` were **not run** —
there is no toolchain. What was done instead:

- all 11 Rust sources parse with tree-sitter (0 errors) — a syntax check, not a
  type check;
- every cross-module call, serde shape, Tauri 2.12 API usage, and command
  argument casing was checked by hand against the Tauri 2.12.4 sources;
- defects found by executing the runtime were fixed in Rust and covered by
  tests that will run on the first machine with a toolchain.

Defects fixed (all four were invisible to the TypeScript checks):

1. `probe_runtime` read only stdout, but `--version` prints to **stderr**, so
   every valid runtime was reported as `unknown build` and
   `pinnedBuildMatched: false`. Both pipes are now drained concurrently and a
   `version:` line is preferred.
2. `load_model` waited the full 180 s health timeout even when the child had
   already exited. It now polls the process, fails immediately with the exit
   status and stderr tail, and records it as the engine's last error so the UI
   shows *Engine failed* rather than *No model loaded*.
3. Every non-2xx response was reported as `load_failed`; a `401` therefore
   blamed the model instead of the session key. Rejections now map to
   `auth_failed` / `not_ready` / `invalid_request` / `engine_rejected` and show
   the runtime's own `error.message`.
4. The post-cancel `POST /slots/0?action=erase` returns `501` unless
   `--slot-save-path` is used, so it could never succeed. It was removed;
   dropping the connection is what stops generation (measured below).

Also: the session key file is deleted on stop and on spawn failure, executable
detection is platform-aware (PE / ELF / shebang) instead of telling Linux users
their binary "is not a Windows executable", `--no-ui` replaces the deprecated
`--no-webui`, and a failed load now refreshes the engine status in the UI.

`Cargo.lock`: **not created** (no registry). The manifest pins major versions
only; this is a real reproducibility gap for the first build.

## Runtime contract (executed)

Full transcript: [`docs/phase-5/runtime-contract-verification.md`](../phase-5/runtime-contract-verification.md).

Measured on the built binary:

- `--version` → exit 0, stdout empty, stderr
  `version: 0.7.0-dev (build 11714, commit c0fd786…)`;
- `--help` accepts every flag Nexora passes (`--model`, `--host`, `--port`,
  `--ctx-size`, `--parallel`, `--no-ui`, `--jinja`, `--api-key-file`,
  `--timeout`);
- `/health` → `200 {"status":"ok"}` without a key;
- chat without a key → `401 {"error":{"message":"Invalid API Key",…}}`;
- a missing model → exit 1 in ~5 ms with a specific stderr line;
- `POST /slots/0?action=erase` → `501 … "Start it with --slot-save-path"`.

Streaming, using a **441 KB synthetic random-weight GGUF** (random tensors, so
the text is meaningless; the file is also below Nexora's own 16 MiB minimum):

- first delta `{"role":"assistant","content":null}`, then token deltas, then a
  final chunk with an empty `delta` and `finish_reason` (`length` or `stop`),
  then `data: [DONE]`;
- unknown `timings` on the final chunk is ignored (it is never displayed);
- killing the client after 100 ms stopped the server at 521 of 1543 tokens and
  the next request succeeded in 12 ms — cancellation by disconnect works;
- SIGINT shut the process down in ~5 ms.

This proves the protocol Nexora implements; it does **not** prove that the Rust
client parses it (uncompiled) or that a real model produces useful text.

## Frontend regression

| Check | Result |
| --- | --- |
| `pnpm typecheck` | pass |
| `pnpm test` | **87/87** in 8 files |
| `pnpm build` | pass — JS 267.72 kB (82.40 kB gzip), CSS 19.97 kB (4.68 kB gzip) |

New tests cover defects from this phase: the engine-failure chip shows the
core's own reason instead of a generic state, and a failed load asks for a fresh
status instead of leaving the UI on "No model loaded".

These are frontend regression tests with the single IPC boundary replaced by a
scripted double. They prove wiring and state handling; they are not native
integration tests. Native integration remains the Windows checklist.

## Safety review

| Requirement | Status |
| --- | --- |
| Loopback only, ephemeral port | `--host 127.0.0.1`, OS-assigned port, `no_proxy()` client |
| Auth | 32-byte random key in a `0600` file passed via `--api-key-file`; never in argv, logs, or the webview |
| Key lifetime | deleted on stop and on spawn failure |
| No shell | `Command` with structured `arg`s; no shell, no interpolation |
| Paths | both paths come from settings, re-validated on every load; GGUF magic/version checked |
| Filename independence | GGUF validated by header bytes, executable by magic bytes |
| Failed load | child stopped, reason recorded, status reports *error*, UI surfaced |
| Status honesty | failed health is never "ready"; IPC failure is "unknown" |
| Limits | 64 turns, 100 k chars/turn, 400 k total, 8192 tokens, 400 k reply chars, 32 k UI input |
| No code execution | model output is only rendered as text |
| Capabilities | `core:default` + four window permissions; no shell/fs/network plugins |

Known limit (unchanged): if the app is killed hard (not closed), the child is
not reaped; only normal close and process exit run `stop()`.

## What still needs a Windows machine

1. `cargo fmt --check`, `cargo check`, `cargo clippy --all-targets -- -D
   warnings`, `cargo test`, `cargo generate-lockfile` (and review the lock).
2. `pnpm tauri dev`: window launch/close, IPC over `http://ipc.localhost`,
   capabilities, custom title bar.
3. Real runtime + real GGUF: load, stream, stop, unload, reload; `probe_runtime`
   against an official `llama-server.exe` reporting build `11714`.
4. Child lifecycle: close during generation; confirm no `llama-server` remains
   in Task Manager; confirm `GlobalMemoryStatusEx` figures.
5. Restart: settings persist, no model auto-loads, previous state is not
   faked.

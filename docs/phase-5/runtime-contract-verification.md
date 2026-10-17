# Runtime contract verification — llama.cpp b11714

This note records what was **executed** against a locally built `llama-server`
from the pinned tag, and which parts of the integration remain assumptions
because they need model weights or a Windows host.

It exists so a later phase can tell "we read this in the source" apart from
"we ran this".

## What was built

| Item | Value |
| --- | --- |
| Source | `ggml-org/llama.cpp` tag `b11714`, commit `c0fd78626405c6d9b934efef0ec7291fcc20e3d6` |
| Obtained via | `codeload.github.com` tarball (release assets unreachable from the build host) |
| Toolchain | g++ 12.2.0, make 4.3, CMake 4.4.4 (PyPI wheel), 2 vCPU |
| Platform | Linux x86_64 (development host only — **not** the target OS) |
| Configure | `-DGGML_NATIVE=OFF -DGGML_OPENMP=OFF -DLLAMA_BUILD_TESTS=OFF -DLLAMA_BUILD_EXAMPLES=OFF -DLLAMA_CURL=OFF -DLLAMA_USE_PREBUILT_UI=OFF -DLLAMA_BUILD_NUMBER=11714` |
| Result | `build/bin/llama-server`, ~18 kB launcher + shared libraries |

The build number was injected because the tarball has no `.git`, so upstream
would embed `build 0`. The injected value reproduces what the official release
binary reports.

## Verified by execution

### Version handshake

```
$ ./llama-server --version > out.txt 2> err.txt
exit 0
out.txt: 0 bytes
err.txt: version: 0.7.0-dev (build 11714, commit c0fd78626405c6d9b934efef0ec7291fcc20e3d6)
         built with GNU 12.2.0 for Linux x86_64
```

`--version` needs no model, exits 0, and writes the build line to **stderr**
(`llama_print_build_info` defaults to `stderr`).

**Defect found and fixed:** `probe_runtime` read stdout only, so a correct
runtime was reported as `unknown build` with `pinnedBuildMatched: false`. It now
drains both pipes (concurrently, to avoid blocking on a full pipe) and prefers
stderr. Regression tests use the captured output above.

### Flags used by the launcher

`--help` on the built binary contains every argument Nexora passes:
`--model`, `--host`, `--port`, `--ctx-size`, `--parallel`, `--no-webui`,
`--jinja`, `--api-key-file`, `--timeout`.

### HTTP behaviour (router mode, no weights)

Started with `--host 127.0.0.1 --port 18090 --api-key-file keys.txt --no-webui
--jinja --parallel 1 --ctx-size 2048 --timeout 600`:

| Request | Observed |
| --- | --- |
| `GET /health` (no auth) | `200 {"status":"ok"}` — public, no key required |
| `GET /health` (bearer) | `200 {"status":"ok"}` |
| `POST /v1/chat/completions` (no auth) | `401 {"error":{"message":"Invalid API Key","type":"authentication_error","code":401}}` |
| `POST /v1/chat/completions` (bearer, router mode) | `400 {"error":{"code":400,"message":"model 'local' not found","type":"invalid_request_error"}}` |
| `POST /slots/0?action=erase` (bearer) | `400` JSON parse error in router mode |

`/health` returning 503 while a model is loading is taken from
`middleware_server_state` in `server-http.cpp` (the router test above is already
"ready" because it loads nothing) and from the server README.

**Defect found and fixed:** every non-2xx response was surfaced as
`load_failed`, including authentication failures, which pointed the user at the
model instead of the session key. Responses now map to `auth_failed` /
`not_ready` / `invalid_request` / `engine_rejected`, and the runtime's own
`error.message` is shown. Tests use the two real bodies above.

### Startup failure

```
$ ./llama-server --model /tmp/definitely-missing.gguf --port 18083
exit 1
... failed to open GGUF file ... failed to load model ... exiting due to model loading error
```

The process dies in milliseconds. **Defect found and fixed:** `load_model`
polled `/health` for the full 180 s load timeout without noticing that the child
had exited, so a bad model produced a three-minute wait and a generic timeout.
It now checks the process on every poll and fails immediately with the exit
status and the last engine lines, and records the same reason as the engine's
last error so the sidebar shows "Engine failed" instead of "No model loaded".

### Cancellation

`POST /slots/{id}?action=erase` is rejected with

> This server does not support slots action. Start it with `--slot-save-path`

unless the runtime was started with `--slot-save-path`
(`server-context.cpp`, `post_slots`). Nexora does not enable slot files, so the
erase request could never have worked. It was removed. Cancellation relies on
dropping the HTTP response; `server-context.cpp` stops the streaming task when
`req.should_stop()` (the connection) goes away.

The session key file is now deleted when the engine stops, and if the process
fails to spawn.

## Verified by reading the pinned source only

- SSE frames are `data: <json>\n\n`, terminated by `data: [DONE]\n\n`.
- Streaming chunks are `{"choices":[{"delta":{...},"finish_reason":null}],"object":"chat.completion.chunk",...}`;
  the final chunk carries a non-null `finish_reason` and an empty `delta`.
- A usage chunk is only emitted when `stream_options.include_usage` is set
  (Nexora never sets it and never reports token counts).
- `--api-key-file`: one key per line, `#` comments, `Authorization: Bearer`.
- `--no-webui`, `--jinja`, `--parallel`, `--timeout` semantics.
- A missing `--api-key-file` aborts argument parsing and prints usage.

## Not verified (explicitly)

- **Real token generation.** No weights are present on the build host and model
  hosts are unreachable from it. Mocks and syntax checks are not substitutes, so
  no claim of end-to-end inference is made.
- **Windows behaviour** (process creation flags, `GlobalMemoryStatusEx`, WebView2
  IPC, child cleanup, NSIS). Linux runs are not Windows proof.
- **Rust compilation.** No toolchain and no registry access on the build host.
  The Rust backend is **uncompiled**.

## What a Windows run must confirm

1. `cargo check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`.
2. `pnpm tauri dev` launches, and closing the window leaves no `llama-server`
   child behind.
3. `probe_runtime` reports `build 11714` from a real release binary.
4. A user-supplied GGUF loads, streams, and stops mid-generation.
5. A model path that fails to load turns the chip to "Engine failed" within
   seconds, not after three minutes.

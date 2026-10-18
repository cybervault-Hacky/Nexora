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
drains both pipes (concurrently, so neither can block), looks for a `version:`
line on stderr first and stdout second, and only then falls back to the first
non-empty line. Regression tests use the captured output above.

### Flags used by the launcher

`--help` on the built binary contains every argument Nexora passes:
`--model`, `--host`, `--port`, `--ctx-size`, `--parallel`, `--no-ui`
(`--no-webui` is the deprecated alias), `--jinja`, `--api-key-file`, `--timeout`.

### HTTP behaviour (router mode, no weights)

Started with `--host 127.0.0.1 --port 18090 --api-key-file keys.txt --no-ui
--jinja --parallel 1 --ctx-size 2048 --timeout 600`:

| Request | Observed |
| --- | --- |
| `GET /health` (no auth) | `200 {"status":"ok"}` — public, no key required |
| `GET /health` (bearer) | `200 {"status":"ok"}` |
| `POST /v1/chat/completions` (no auth) | `401 {"error":{"message":"Invalid API Key","type":"authentication_error","code":401}}` |
| `POST /v1/chat/completions` (bearer, router mode) | `400 {"error":{"code":400,"message":"model 'local' not found","type":"invalid_request_error"}}` |
| `POST /slots/0?action=erase` (bearer) | `400` JSON parse error in router mode (see below for the single-model answer: **501**) |

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

### Credential hygiene

The per-session key file is now deleted when the engine stops, and when the
process fails to spawn. Cancellation and the slot-erase endpoint are covered in
the streaming section below.

## Streaming contract verified with a synthetic model

No real weights are reachable from the build host, so a 441 KB
**random-weight** Llama-architecture GGUF was generated with the `gguf-py`
package from the same source tree (1 layer, 64 hidden, 4 heads, 512-token
vocabulary) and loaded by the same binary:

```
llama-server --model tiny-contract.gguf --host 127.0.0.1 --port 18095 \
  --ctx-size 2048 --parallel 1 --no-ui --jinja --timeout 600
... srv llama_server: model loaded
```

This is **not a language model** — every weight is random, so the text it emits
is meaningless. It also fails Nexora's own 16 MiB minimum, so this exercised the
runtime's protocol, not the app's validator. What it does prove is what
`client.rs` depends on. The request body was exactly what `client.rs` sends:
`{"model":"local","stream":true,"messages":[system,user],"max_tokens":12,"temperature":0.2}`.

Observed frames (abridged):

```
data: {"choices":[{"finish_reason":null,"index":0,"delta":{"role":"assistant","content":null}}],...,"object":"chat.completion.chunk"}
data: {"choices":[{"finish_reason":null,"index":0,"delta":{"content":"3"}}],...,"object":"chat.completion.chunk"}
...
data: {"choices":[{"finish_reason":"length","index":0,"delta":{}}],...,"timings":{"prompt_n":110,"predicted_n":12,...}}
data: [DONE]
```

- the first delta carries `"content": null` (and a role), which
  `Option<String>` accepts and `extract_delta` ignores;
- the final chunk has an empty `delta` and a real `finish_reason`
  (`length` when the token cap is hit, `stop` when the model emitted EOS);
- an unknown `timings` object appears on the final chunk and is ignored;
- no usage chunk is sent because `stream_options.include_usage` is never set;
- the stream ends with `data: [DONE]`, and blank lines between frames are
  ignored by `LineBuffer`/`classify`;
- `"model":"local"` is accepted — a single-model server does not validate the
  model name.

Cancellation, measured on the same process:

| Run | Slot tokens at release | `[DONE]` to client | Follow-up |
| --- | --- | --- | --- |
| `max_tokens 1500, ignore_eos`, read to completion | 1543 | yes | — |
| same, client killed after 100 ms | **521** | no (client gone) | 200 in **12 ms** |

The server stopped generating when the connection closed and freed the slot
immediately. That is the mechanism Nexora relies on; the removed slot-erase call
was not needed.

`POST /slots/0?action=erase` on this server returned exactly:

```
501 {"error":{"code":501,"message":"This server does not support slots action. Start it with `--slot-save-path`","type":"not_supported_error"}}
```

SIGINT shut the process down in ~5 ms (exit code not captured because the
process was started in an earlier shell).

## Verified by reading the pinned source

- A usage chunk is only emitted when `stream_options.include_usage` is set
  (Nexora never sets it and never reports token counts).
- `--api-key-file`: one key per line, `#` comment lines. Verified with a file
  whose first line was a 32-byte hex key and whose second line was
  `# a comment line`: the correct key passed authentication (the router then
  answered 400 "model not found"), a wrong key got 401, and no key got 401.
  `Authorization: Bearer` is the accepted header.
- `/health` is served through the same middleware that returns 503 until the
  model is ready.

## Not verified (explicitly)

- **Real token generation.** Model hosts are unreachable from the build host, so
  the only loaded model was random-weight and its output is meaningless. Mocks
  and syntax checks are not substitutes, so no claim of end-to-end inference on
  a real model is made.
- **Windows behaviour** (process creation flags, `GlobalMemoryStatusEx`, WebView2
  IPC, child cleanup, NSIS). Linux runs are not Windows proof.
- **Rust compilation.** No toolchain and no registry access on the build host.
  The Rust backend is **uncompiled**.

## What a Windows run must confirm

1. `cargo check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`.
2. `pnpm tauri dev` launches, and closing the window leaves no `llama-server`
   child behind.
3. `probe_runtime` reports `build 11714` from a real release binary.
4. A user-supplied GGUF loads, streams real text, and stops mid-generation,
   with the Rust client parsing the frames above.
5. A model path that fails to load turns the chip to "Engine failed" within
   seconds, not after three minutes.
6. `--no-ui` is accepted by the exact release binary the user pins.

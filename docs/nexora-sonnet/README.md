# Nexora Sonnet — implementation index

**Product name.** "Nexora Sonnet" is an internal product name for a second local coding
model in Nexora. It is **not** an Anthropic model, has no relationship to Claude Sonnet,
and must never be described as having that model's capabilities.

**Status: ARTEFACT VERIFIED — NOT FULLY VERIFIED.**
The artefact metadata (repository, immutable revision, file name, 8 988 110 272 bytes,
SHA-256, Apache-2.0) is now verified from the publisher's own API and is recorded in the
registry, so the profile is approved and downloadable. **No local inference has been
demonstrated**: this machine has 3.85 GiB of RAM in total (about 3.5 GiB available) against ~10 GiB needed, and 8.99 GB of
weights cannot be fetched here. Details: [ARTIFACT_EVIDENCE.md](./ARTIFACT_EVIDENCE.md)
and [PHASE_9_VERIFICATION.md](./PHASE_9_VERIFICATION.md) (§12 is the current A–K matrix; §§1–11 are historical).

## Documents

| File | Covers |
| --- | --- |
| [ARTIFACT_EVIDENCE.md](./ARTIFACT_EVIDENCE.md) | Exact artefact values, their sources, and what remains unverified |
| [PHASE_1_2_MODEL.md](./PHASE_1_2_MODEL.md) | Model choice, licence, runtime, hardware, what is and is not evidence |
| [PHASE_3_4_INTEGRATION.md](./PHASE_3_4_INTEGRATION.md) | Registry shape, acquisition, integrity, cancellation |
| [PHASE_5_7_CHAT_ROUTING.md](./PHASE_5_7_CHAT_ROUTING.md) | Selector, routing guards, coding/export/usage integration, and the Phase 6 request/switch rules |
| [READINESS.md](./READINESS.md) | Hardware detection, resource estimates, the 14 readiness states, and their limits |
| [PHASE_5_RUNTIME.md](./PHASE_5_RUNTIME.md) | Pinned runtime compatibility, the load decision, streaming, and what is still unproven |
| [PHASE_9_VERIFICATION.md](./PHASE_9_VERIFICATION.md) | Every check run, its result, and what remains blocked; §11 is the Phase 7 verification-readiness checklist |

**Runtime.** The engine Nexora drives is pinned to llama.cpp `b11714`
(commit `c0fd786…`, an upstream **pre-release**). Every launch flag, the
`/health` handshake and the `/v1/chat/completions` request were checked against
that commit's source. `load_model` now makes one pre-spawn decision: the selected
profile may only load its own verified artefact, a Sonnet selection cannot run
Mini's file, and the context window must fit the file. No load has been
demonstrated — see [PHASE_5_RUNTIME.md](./PHASE_5_RUNTIME.md).

**Request routing (Phase 6).** Every request now names the profile it was composed
under, and the core refuses it when that is not the selected profile — so a pane
left open across a switch cannot be answered by the other model. Switching is
refused while the engine is answering or loading, and the refusal is shown in
the selector rather than silently discarded. The request budget is measured
against the context the running engine was actually started with. No fallback
of any kind exists on this path. A load also waits for a reply to finish, and a reply
waits for a load (Phase 7). Details:
[PHASE_5_7_CHAT_ROUTING.md](./PHASE_5_7_CHAT_ROUTING.md) §7.

**Readiness.** Every row of the model selector now carries the core's own verdict —
`ready_to_download`, `unverified`, `insufficient_memory`, and so on — computed from real
`/proc/meminfo` (or `GlobalMemoryStatusEx`), real free space at the download folder, the
published artefact length, and the configured context. Readiness does not claim Sonnet is
runnable here: this host has 3.5 GiB available against an estimated 10.9 GiB.
[READINESS.md](./READINESS.md) records the method and the tests.

## The one-paragraph summary

Nexora now registers two profiles. **Nexora Mini** (Qwen2.5-Coder-7B-Instruct, Q4_K_M) is
unchanged, remains the default, and is the only profile with an approved artefact.
**Nexora Sonnet** (Qwen2.5-Coder-14B-Instruct, Q4_K_M) is a real catalogue entry whose
artefact is now pinned to commit `d0a692ef…` at 8 988 110 272 bytes with SHA-256
`c1e65973…894d3c`. It can be selected in the chat composer and is routed through the same
inference supervisor. It has **not** been downloaded or run anywhere in this project, so no
output, speed, or memory figure is claimed for it.

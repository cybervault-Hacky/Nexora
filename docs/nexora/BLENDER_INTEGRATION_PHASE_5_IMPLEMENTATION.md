# Nexora Blender Integration — Phase 5 Secure Local Connector & Communication

**Implementation status:** Phase 5 connector implementation is **blocked at the Windows feasibility gate**. No connector, transport, Blender-side bridge, or handshake was added. This report records the feasibility decision and the safe UI clarification shipped in this turn; it does not claim Phase 5 is complete.

**Scope authorization:** This report follows the current task's Phase 5 label and scope (secure local connector and communication). The unchanged proposed 12-phase outline currently labels Phase 5 as read-only scene inspection and Phase 4 as the connection lifecycle. No roadmap/report was renumbered or rewritten. This numbering discrepancy does not authorize scene inspection or later-phase work.

## Executive result

Repository and environment inspection did not provide enough evidence to implement or qualify a real Blender peer connection safely:

- No Blender executable/runtime is installed in this Linux sandbox; `blender` is not available on `PATH` and no Blender bridge/add-on exists in the repository.
- The host is Linux x86_64. `rustc`, `cargo`, and `rustup` are absent. There is no Windows build/runtime available for named-pipe, ACL, process-identity, Tauri, or Blender tests.
- The current Windows detector is candidate-only; it does not prove publisher/product identity or a Blender version. Therefore it is not adequate to authorize a privileged connection workflow.
- The Tauri integration command currently supports installation detection only. There is no connector manager, pipe/TCP transport, authentication handshake, health session, disconnect lifecycle, or task executor. The React catalogue's Blender `connectionStatus` remains `null` (no connector to observe), and `connectionActions` remains empty.

A simulated Connect result, unverified pipe implementation, or mock presented as a live connection would violate the requested evidence bar. The Connect/Disconnect controls remain unavailable. The only operational control is the existing explicit **Check Installation** action; it can report candidate installation evidence, not a connection.

## Transport feasibility comparison

| Option | Evidence in this checkout/environment | Decision |
|---|---|---|
| **Per-user Windows named pipe** | The requirements and connector contract recommend a Nexora-owned pipe with an explicit user-only DACL, challenge/nonce, protocol versioning, and peer PID/executable verification. The existing `windows-sys` dependency does not currently enable named-pipe/security features, and there is no pipe code. No Windows toolchain/runtime is available to verify first-instance behavior, DACL enforcement, client PID/path checks, impersonation/reversion, cancellation, or shutdown. Blender's embedded Python pipe client and its lifecycle are expressly an unresolved spike in the existing requirements. | **Preferred future candidate, not adopted as a supported transport and not implemented.** First prove Blender Python client behavior and Windows ACL/peer identity on a declared Windows/Blender matrix. A same-user pipe ACL alone does not isolate Nexora from malicious code running as that user. |
| **Loopback-only TCP** | The repository has loopback TCP for separate OAuth/authentication and local inference purposes; neither is a Blender connector. They do not establish a Blender peer identity or per-application authenticated channel. The desktop WebView CSP permits Tauri IPC rather than arbitrary HTTP connections. No Blender-specific token, port reservation, peer-binding, or TCP protocol exists. | **Rejected for this phase.** A loopback bind is not peer authentication; local processes can race, spoof, or connect unless a separately justified authentication and process-binding model is proven. It adds listener/port lifecycle and browser-origin/CSRF concerns without solving the missing peer proof. No new listener was created. |
| **Blender Python add-on / embedded-Python bridge** | Repository search found no `.py` bridge/add-on, packaging, integrity verification, installation flow, or Blender startup. No Blender runtime is available to test Python APIs, main-thread scheduling, add-on enable/disable, reconnect, or clean shutdown. A Blender-side participant would be required for a useful stable `bpy`-aware peer, but its transport compatibility cannot be assumed from source inspection. | **Necessary design component for a future connector, not implemented.** No add-on was installed, bundled, launched, or represented as available. |
| **Existing supported integration mechanism** | The code has an explicit Tauri `generate_handler!` list, the typed installation-detection command, and generic catalogue lifecycle types. There is no existing Blender transport, connector registry/manager, app-side bridge, or connection session. | **None exists to reuse.** Tauri IPC is renderer-to-Nexora only; it is not a Blender connection. |

### Transport decision

No production transport is selected because the required feasibility evidence is missing. The named pipe remains the preferred **spike candidate** because it can avoid a network listener and can be ACL-scoped to a Windows user; this is not evidence that the current Blender Python runtime can use it securely or reliably. Before adoption, a Windows spike must prove: an explicit DACL; safe unique endpoint creation and cleanup; supported Blender Python client behavior; client PID plus executable/file identity validation against a user-approved installation; per-session challenge/nonce and protocol version; bounded framing/timeouts; and reconnect/disconnect behavior. If those gates fail, loopback TCP requires a separate threat review rather than automatic substitution.

## Protocol and security status

### Implemented facts

- No Blender communication protocol or message schema was added.
- No named pipe, TCP listener, socket, token, challenge, handshake, peer PID check, ACL, or authentication code was added.
- No Blender-side process or executable is launched by this work.
- No executable, add-on, model, or runtime was downloaded or installed.
- No secret or reusable authentication material was added.
- The existing Tauri command boundary and installation-result validation are unchanged. They do not expose a pipe handle or network endpoint to the renderer.
- The App Integrations card now explains accurately that installation detection reports candidates only and that no validated handshake/control path exists. The card still exposes no Connect/Disconnect action; task readiness remains blocked.

### Proposed requirements, not implemented

A later, approved transport needs a versioned, size-bounded frame format; typed request/response envelopes; strict schema validation; request IDs and sequence numbers; challenge/replay handling; per-message timeouts; bounded queues; malformed/oversized-message rejection; disconnect and stale-session handling; and host-side validation of every operation. Authentication must bind to an approved Blender process, not merely possession of a local endpoint. Any future message vocabulary must be a small declarative allowlist. Python/source text, shell commands, `eval`, arbitrary Blender operator names, and AI-generated code are prohibited.

These are requirements and spike questions, not code or test results. There is no framing layer on which protocol, oversized-message, authentication-failure, timeout, or disconnect unit tests could honestly be based. No mocked transport test was introduced or counted as connection evidence.

## Lifecycle, UI, and setup

- Installation remains independent of connection and readiness. A Phase 4 `Found` result is a candidate-only detector result; it cannot enable a connection workflow.
- Blender's `connectionStatus` stays `null` because no connector exists to report a lifecycle observation. It is not shown as Connected or as a measured Not Connected state.
- Blender's connection action list stays empty. No Connect, Disconnect, Reconnect, Configure, health-refresh, or launch control was added.
- Blender task readiness remains blocked because no real handshake, permission service, validated action protocol, or task executor exists. Finding an installation does not alter it.
- VS Code and Figma remain Planned and non-connectable.
- There is no bridge setup instruction to offer. Users should not manually install an unreviewed add-on or configure a pipe for this build; no add-on is supplied or recognized.
- No scene read, mutation, AI action, Python execution, or provider behavior is included.

## Supported platform/version claims

- Installation discovery remains Windows-specific and candidate-only as documented in the Phase 4 report.
- **Connector support:** none; Windows and non-Windows connection attempts are not implemented. No supported Blender versions, add-on versions, Windows builds, or protocol versions are declared.
- **Blender versions/runtime tested:** none. No real connection handshake was attempted or observed.
- Browser preview cannot perform desktop communication and does not attempt a connection.

## Files changed in this turn

- `apps/desktop/ui/src/pages/AppIntegrationsPage.tsx` — clarifies that Blender's installation check is candidate-only and that no validated connection/control path exists; other planned catalogue entries retain their existing unavailable copy.
- `apps/desktop/ui/src/pages/AppIntegrationsPage.test.tsx` — asserts the candidate-only connection explanation and absence of Connect/Disconnect controls after a mocked candidate result. This is renderer behavior coverage only, not transport evidence.
- `docs/nexora/BLENDER_INTEGRATION_PHASE_5_IMPLEMENTATION.md` — this feasibility-gate and implementation-status report.

No change was needed to `APP_CONNECTOR_CONTRACT.md`: it remains a proposal, and no new wire contract or connector DTO was implemented. Historical Phase 2/3/4 reports and the proposed plan are preserved.

## Tests and checks actually run

- Focused App Integrations, native installation IPC, and Rust/TypeScript source-contract tests: `corepack pnpm --dir apps/desktop exec vitest run src/pages/AppIntegrationsPage.test.tsx src/features/integrations/native.test.ts src/lib/rustContract.test.ts --reporter=dot` — **3 files, 76 tests passed**. These test UI/install IPC and source contracts; they do not test or prove a connector protocol.
- Full desktop UI suite: `corepack pnpm --dir apps/desktop test -- --reporter=dot` — **93 files, 913 tests passed**, including existing navigation/catalogue coverage.
- TypeScript: `corepack pnpm --dir apps/desktop typecheck` — **passed**.
- Desktop production build: `corepack pnpm --dir apps/desktop build` — **passed** (TypeScript, Vite, configuration-security, installer-contract). Vite emitted mixed dynamic/static import and >500 kB chunk warnings. Installer check validated 3 frontend assets / 753,562 bytes and no model/runtime/credential payloads.
- `git diff --check` — **passed**.

Not run / blocked:

- Rust compile or Rust unit tests: `rustc`, `cargo`, and `rustup` are unavailable.
- Windows native/Tauri compile, pipe ACL/peer-identity tests, Windows build, or Windows runtime tests: no Windows environment/toolchain is available.
- Blender executable/add-on tests or a live Blender handshake: no Blender runtime is installed.
- Protocol framing/schema, oversized frame, authentication/replay, timeout, peer validation, disconnect, and real lifecycle tests: no protocol/transport exists to test.

## Phase boundary and preservation

This task ends at a documented feasibility gate. The secure local connector has **not** been implemented and must not be advertised as working. No Phase 6 or scene-inspection/task-execution work was started. To resume connector implementation safely, first run the named-pipe/Python/ACL/peer-identity spike on Windows with actual Blender builds, publish the tested support matrix, and then authorize an implementation against observed results.

All existing modified and untracked work was preserved. No files were staged or committed; nothing was pushed, merged, tagged, published, or released.

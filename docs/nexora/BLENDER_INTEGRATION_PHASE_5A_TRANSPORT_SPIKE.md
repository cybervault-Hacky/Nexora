# Blender Integration Phase 5A — Windows + Blender Transport Feasibility

**Status: NO-GO for production transport adoption**

**Investigation date:** 2026-10-26

**Scope:** Source-level feasibility review and reproducible experiment plan only. No transport implementation or proof of concept was possible in this environment.

## Executive finding

A Nexora-owned Windows named pipe remains a **plausible candidate**, not a demonstrated Nexora–Blender transport. Microsoft documents named-pipe ACLs, session-scoping options, endpoint and instance behavior, and APIs that expose the connected peer's PID/session. CPython documents Windows `AF_PIPE` support through `multiprocessing.connection` and an optional HMAC challenge. Those facts do **not** establish that the Python bundled with any supported Blender build can use that API, that a Rust server interoperates with it, or that either endpoint can securely establish that its peer is the intended Nexora/Blender process.

The required execution environment is absent: this host is Linux x86_64 with Python 3.11.2, and has no Rust/Cargo toolchain, Windows runtime, Blender executable, or Blender-embedded Python. As a result, there is no POC, no Windows pipe test, no Blender runtime test, no genuine handshake, and no peer-security test. **Do not treat this report as a successful spike or use it to claim Windows/Blender support.**

The named-pipe candidate should not enter production until the actual Windows + Blender experiment below passes, including both-direction peer checks and adversarial cases. No production connector, UI action, or transport has been added. Installation detection remains candidate-only.

## 1. Environment and repository facts

### Investigation host

| Item | Observed |
|---|---|
| Operating system / architecture | Linux x86_64 (`Linux e2b.local`, kernel `6.1.158+`) |
| System Python | Python 3.11.2 |
| `rustc`, Cargo, `rustup` | Not installed / not found on `PATH` |
| Windows runtime | Not present |
| Blender executable | Not present on `PATH`; earlier scan of `/usr`, `/opt`, `/home/user`, and repository `apps` found no Blender executable |
| Blender embedded Python | Not present or inspectable |
| Blender bridge/add-on in repository | None found |

The environment cannot host both required endpoints in a Windows user session. Linux pipes, mocks, documentation, or source-level API inspection would not establish Windows named-pipe behavior and were not represented as such.

### Current Nexora state inspected

- `apps/desktop/src-tauri/src/integrations/` contains candidate-only installation detection/IPC; no Blender pipe transport, protocol, session, or handshake was found.
- `apps/desktop/src-tauri/Cargo.toml` has no named-pipe/security feature configuration for this work.
- Tauri IPC is the renderer-to-Nexora boundary; it is not a Blender transport.
- The existing Blender catalogue remains non-connectable: candidate installation evidence does not imply `Connected` or `Ready`; no Connect/Disconnect actions were added.
- The Phase 5 implementation report already records no connector and no Windows/Blender validation. The proposed 12-phase plan is preserved; this report does not rewrite or renumber it.

## 2. Source-level findings (not experimental results)

### Windows named pipes and access control

Microsoft's [Named Pipe Security and Access Rights](https://learn.microsoft.com/en-us/windows/win32/ipc/named-pipe-security-and-access-rights) documentation says a server can pass a security descriptor when creating a pipe. If it passes `NULL`, the default DACL grants full control to LocalSystem, administrators, and creator owner, and read access to Everyone and the anonymous account. That default is not an acceptable basis for this connector. A future implementation needs an explicit least-privilege DACL and must fail closed if it cannot create the endpoint with the intended descriptor.

The same documentation recommends putting the logon SID in the DACL to prevent access by remote users or users in another Terminal Services session. A user SID/logon SID DACL is an access-control boundary, **not** cryptographic authentication and not proof that a peer is the intended Nexora or Blender process. Other code running as that same user/session can generally act with that user's authority; a named pipe does not sandbox same-user malware.

Microsoft's [`CreateNamedPipeW`](https://learn.microsoft.com/en-us/windows/win32/api/namedpipeapi/nf-namedpipeapi-createnamedpipew) API documents `PIPE_REJECT_REMOTE_CLIENTS` for rejecting remote clients. The API also documents `FILE_FLAG_FIRST_PIPE_INSTANCE` and the pipe-instance rules. A future harness should use the first-instance mechanism as a fail-closed collision/precreation check, then experimentally prove that an endpoint created before Nexora causes startup to reject the collision rather than attach to or trust it. Random endpoint names are defense in depth only; they are not peer authentication.

[Pipe Names](https://learn.microsoft.com/en-us/windows/win32/ipc/pipe-names) describes the local named-pipe namespace and name constraints. [Named Pipe Instances](https://learn.microsoft.com/en-us/windows/win32/ipc/named-pipe-instances) and [Named Pipe Operations](https://learn.microsoft.com/en-us/windows/win32/ipc/named-pipe-operations) describe multiple instances and connection/read/write behavior. Concurrent clients therefore need an intentional policy. For the feasibility spike, use one active client/one instance and reject a duplicate; do not add parallel instance handling until it is separately designed and tested.

Microsoft's [`GetNamedPipeClientProcessId`](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-getnamedpipeclientprocessid) and [`GetNamedPipeServerProcessId`](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-getnamedpipeserverprocessid) expose a peer process identifier. The corresponding [`GetNamedPipeClientSessionId`](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-getnamedpipeclientsessionid) and [`GetNamedPipeServerSessionId`](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-getnamedpipeserversessionid) expose session identifiers. These APIs provide useful observations for a process/session policy, but a PID or session ID alone does not identify a publisher, executable file, or trusted process. PID reuse, process exit/races, access-denied image queries, and path/file replacement must be addressed in an implementation and tested.

[`ImpersonateNamedPipeClient`](https://learn.microsoft.com/en-us/windows/win32/api/namedpipeapi/nf-namedpipeapi-impersonatenamedpipeclient) impersonates the security context associated with the last message read by the server. It is a way to inspect the Windows security principal, not a way to prove the connecting client is Blender. Microsoft specifically warns that failure means the caller continues under its own context; any future use must check failure and always revert impersonation, including error/cancellation paths. Neither process impersonation nor a user DACL substitutes for executable identity validation.

### Blender embedded Python and protocol interoperability

The [CPython `multiprocessing` documentation](https://docs.python.org/3/library/multiprocessing.html#listeners-and-clients) documents `multiprocessing.connection` listener/client support for `AF_PIPE` on Windows. Its optional `authkey` mechanism performs an HMAC challenge; omitting the key (`None`) disables that authentication. An HMAC challenge can prove possession of a shared secret to the other endpoint, but it does not by itself prove which process possesses it, encrypt messages, solve secret provisioning, or replace OS peer checks. A key must not be embedded in a distributable add-on, command line, `.blend` file, or logs; any key-delivery/protection design remains unproven here.

The documentation does not establish that Blender's embedded Python in any selected Blender release exposes a compatible `multiprocessing.connection` build on Windows, nor that its pipe framing/authentication interoperates with a Rust implementation. It also does not establish whether blocking pipe operations can be integrated without freezing Blender's UI, how an add-on registers/unregisters cleanly, or how cancellation/reconnect behaves. No Blender version range, embedded-Python version range, Windows build range, or Nexora protocol version is established by this investigation.

### Candidate comparison

| Candidate | Source-level conclusion | Phase 5A disposition |
|---|---|---|
| Nexora-owned Windows named pipe with explicit DACL | Windows provides the needed IPC primitives and process/session query APIs. ACL creation, anti-squatting behavior, Rust interop, Blender compatibility, lifecycle, and identity checks remain untested. | Preferred **spike candidate only**; not selected or implemented. |
| Blender Python `multiprocessing.connection.AF_PIPE` | CPython documents the API on Windows and optional HMAC challenge/response. Exact Blender Python availability and Rust protocol interoperability are unknown. | Candidate client API to probe on real Blender builds; not assumed compatible. |
| Loopback TCP | Prior Phase 5 review rejected it for this work: loopback alone does not authenticate a peer and adds listener/port/origin lifecycle. | Not a fallback. Requires its own threat review and explicit authorization. |
| Tauri IPC | Connects the renderer to Nexora, not Nexora to Blender. | Not a Blender transport. |

## 3. Threat model and required security claims

The future experiment must explicitly cover:

1. **Other users and logon sessions:** an explicit DACL, current user SID, and intended logon/session policy should reject unauthorized local users/sessions. `PIPE_REJECT_REMOTE_CLIENTS` should reject remote clients. Verify each with actual tokens/sessions; do not infer enforcement from a pipe name.
2. **Same-user local processes:** assume another process running as the same user can discover or race a named endpoint and may access user-readable state. The DACL is not a defense against this actor. Endpoint secrecy, a random name, and HMAC are not sufficient unless key establishment and process binding are also demonstrated.
3. **Pipe squatting/precreation:** an unintended process may create the expected name first, or connect first and consume a single instance. Nexora must fail closed on a pre-created endpoint; Blender must reject an endpoint whose server identity is not approved. Test both startup orders and a connection race.
4. **Peer identity in both directions:** the Nexora server must observe the client PID/session and validate the live process against the explicitly selected Blender installation; the Blender client must similarly validate the server PID/session and Nexora executable. PID alone is not a trust decision. A practical design must bind checks to live process handles/creation identity and compare the expected executable's file identity and publisher/signature under a stated policy. Document race/TOCTOU limits. Test altered, renamed, replaced, and wrong executables. If either side cannot make this check reliably, the security gate fails.
5. **Challenge/replay and shared-secret handling:** if HMAC is used, prove a fresh per-session challenge, protocol/version binding, and rejection of wrong/replayed responses. Document how the key is provisioned without source/CLI/project/log exposure and what same-user malware can do. Authentication must not be claimed before this is solved.
6. **Malformed and hostile traffic:** enforce strict, versioned schemas, finite maximum frame size, bounded queues, finite read/write/connect deadlines, and a small fixed message allowlist. Reject unsupported versions, unknown fields/types, duplicate keys, invalid encodings, truncated frames, oversized frames, replayed IDs, and malformed HMACs. The spike payload is health-check only; no scene data or actions.
7. **Lifecycle and resource exhaustion:** test duplicate clients, a client that connects but sends nothing, one that sends a partial header/body, blocked reads/writes, cancellation, disconnect during a request, Blender add-on disable, Blender crash, Nexora exit/crash, stale/recreated endpoints, restart, and reconnect. No unbounded worker/thread/task or silent stale `Connected` state is acceptable.
8. **Blender runtime constraints:** test the actual embedded Python versions and Windows builds in the proposed support matrix. Confirm pipe API presence, non-blocking or worker-thread behavior, no UI freeze, deterministic add-on unregister/cleanup, and that any future `bpy` access is marshalled to Blender's permitted main-thread context. The health-only spike should not access or modify a scene.

### Proposed spike envelope (design for a future authorized POC; not implemented)

Use only fixed `hello`, `health_request`, `health_response`, and `close` message types. Each envelope should contain a protocol version, message type, request ID, and bounded payload. Set a small documented maximum (for example, 4 KiB total per frame) and finite deadlines for connect, handshake, read, and write. Validate exact schemas and reject unknown fields/types. Keep the POC's health response to non-sensitive runtime facts needed by the test, such as the Blender version and embedded-Python version. Do not send a scene summary, execute Python/shell, invoke Blender operators, alter/save a scene, contact a network listener/cloud, access credentials, auto-launch Blender, or install an add-on.

The 4 KiB value is a proposed test ceiling, not a measured requirement or production choice. Freeze it in the test harness and report if the implementation cannot enforce it before allocation/read.

## 4. Reproducible Windows + Blender experiment plan

**This is a future test procedure, not a report of tests already performed.** No runnable POC/harness exists in this repository, so there are intentionally no fictional `cargo run` commands or test logs to reproduce. The steps below define what a separately authorized, isolated POC must implement and execute before any production recommendation.

### A. Freeze the test environment and harness

1. Use a disposable Windows VM or physical test machine, standard (non-admin) user, and a snapshot that can be restored. Record Windows edition, exact build/patch, architecture, logon/session context, Nexora build/commit, Rust toolchain, Blender installer/source, exact Blender version/build hash, embedded Python version/architecture, and the POC source revision. Do not report a broad version range from one tested build.
2. Select exact candidate Windows/Blender versions from the product's intended support policy before testing. At minimum, test the oldest intended combination and a current intended combination; add LTS/current Blender combinations only if the product plans to support them. The policy and these matrix entries are currently **undecided**; do not label any version supported until the matrix passes.
3. Build a disposable Rust pipe server and a separately installed/explicitly user-enabled Blender test add-on. Keep both outside production registration and UI. No automatic launch, persistent service, installer, or add-on installation step. Record hashes of both test artifacts.
4. Make the server create a one-instance, non-inheritable local pipe with an explicit DACL, intended user/logon SID policy, remote-client rejection, and first-instance collision detection. Configure one-client-only behavior. Make the test client connect outward. Specify the peer identity checks on both endpoints before interpreting the handshake.
5. Freeze the health-only versioned envelope and limits, HMAC/key-handling policy if used, and connect/read/write/handshake deadlines before running tests. A security claim cannot be deferred to an assumed secret in source or a command-line option.
6. Emit timestamped, redacted test logs containing test case ID, result, protocol version, endpoint outcome, peer PID/session checks, and exception/error category. Do not log a reusable secret, nonce/key material, user names, full private paths, scene/project data, credentials, or message contents not needed for the result.

### B. Positive handshake (must be genuine)

1. Start the Rust test server manually in the test user session. Confirm the endpoint was created with the requested descriptor/options and that no collision occurred.
2. Start the selected Blender build manually, enable only the disposable test add-on through Blender's normal user-controlled UI, and initiate its one-shot health-check client. Do not have Nexora launch Blender.
3. On both sides, record the OS-reported peer PID/session and the result of the specified live process/executable identity comparison. Reject before sending/accepting health data if identity is absent, ambiguous, mismatched, or races process exit.
4. Complete a fresh challenge/version handshake and exchange exactly one valid bounded health request/response. Verify protocol fields, version, size bounds, and finite elapsed time. The response must not read or modify scene state.
5. Disconnect normally, verify both handles/resources close, disable the add-on, and repeat after a clean server/Blender restart. A green result is valid only if the actual two processes exchanged the frame and both peer checks passed; a mock, source test, pipe-open alone, or UI state is not a handshake.

### C. Negative and lifecycle cases

Run each as an independent case, reset the VM/test state between cases where needed, and preserve sanitized logs:

| Case | Required expected outcome |
|---|---|
| Another local user / another logon session attempts connection | DACL/session policy denies it; no health exchange. |
| Remote client attempt | Rejected; no remote session or handshake. |
| Attacker/test process pre-creates the endpoint | Legitimate Nexora fails closed with explicit collision result; it never trusts or reuses the endpoint. |
| Wrong server connects to Blender client | Blender rejects before sending health data. |
| Wrong client connects to Nexora server | Nexora rejects before accepting health data. |
| PID/session mismatch, inaccessible PID, process exits during identity check, or identity race | Fail closed; no `Connected` result. |
| Same-user unintended client races or connects first | Must not be misreported as Blender; show the actual protection/limitation. If it can impersonate the intended peer, security gate fails. |
| Invalid HMAC, stale/replayed challenge, wrong protocol version | Reject and close; never downgrade to unauthenticated mode. |
| Invalid JSON/encoding, duplicate keys/fields, unknown message, truncated header/body, zero/over-limit/oversized frame | Reject within bounded memory/time; never partially execute or accept. |
| Duplicate client / repeated handshake | Reject according to one-client policy; no unbounded instances/queues. |
| Silent client, partial frame, slow reader/writer | Finite timeout; cancel/close cleanly without indefinite worker or UI hang. |
| Client disconnect, Blender crash/disable, server crash/exit | Peer notices within measured bound; no stale session is reported as connected. |
| Restart and reconnect | Old handles/session are invalidated; new session receives a fresh challenge and repeats both identity checks. |
| Cancel during connect/read/write/handshake | Cancellation completes within measured bound; no leaked handle, orphan worker, or stale health. |

Run the duplicate/race cases repeatedly with recorded counts and timings. Add concurrent-client tests only if the eventual design intentionally supports multiple Blender processes; for this one-client spike, every extra client must be rejected deterministically.

### D. Evidence and go/no-go gate

For every matrix entry, publish exact environment versions, artifact hashes, case-by-case result, measured timeouts, sanitized logs, and known limitations. Preserve failures. Separate these evidence classes in the report: source-level, mock/unit, actual Windows named-pipe, Blender runtime, genuine handshake, and security/negative tests. A production recommendation requires all critical actual-runtime and security cases to pass on each declared support matrix entry, plus an explicit owner decision on residual same-user risk and key provisioning. A partial positive result is at most a **CONDITIONAL GO** for further isolated testing, never production.

## 5. Actual outcomes and evidence classification

| Evidence class | Outcome in this investigation | Meaning |
|---|---|---|
| Source-level documentation | **Completed (limited)** | Microsoft and CPython documentation was inspected as summarized above. This establishes documented API options only. |
| Nexora source/configuration inspection | **Completed (limited)** | Existing integration code is candidate-only; no pipe/Blender transport was found. |
| Mock/unit transport test | **Not run / not applicable** | No transport implementation or test harness exists. No mock was created or counted as evidence. |
| Actual Windows pipe / DACL / namespace test | **Blocked** | No Windows host/runtime or Rust toolchain. No pipe was created. |
| Blender embedded-Python API/runtime test | **Blocked** | No Blender executable or embedded Python. |
| Rust ↔ Blender Python interoperability | **Blocked** | Neither implementation nor test harness exists. |
| Genuine health handshake | **Not run** | No actual endpoint pair or protocol exists. No handshake occurred. |
| Peer PID/path/signature/session security tests | **Blocked** | No Windows environment or executable peer to test. Documentation/API availability is not verification. |
| ACL, remote-client, precreation/squatting, race, malformed/oversized, duplicate-client tests | **Blocked** | No Windows pipe implementation or adversarial harness. |
| Timeout/cancel/restart/reconnect/shutdown tests | **Blocked** | No transport implementation, Blender runtime, or lifecycle to exercise. |
| Supported Windows/Blender version matrix | **None established** | Product support policy and exact builds were not supplied/tested. |

There are no actual POC logs, success traces, or sensitive log excerpts to redact. No test outcome is fabricated. The measured host facts above are the only runtime observations.

## 6. Limitations and unresolved questions

- Which Windows and Blender versions Nexora intends to support is undecided; no compatibility claim is justified.
- Blender's precise embedded-Python distribution and whether it supports `multiprocessing.connection.AF_PIPE` must be measured per build. Rust-to-Python framing/authentication compatibility has not been tried.
- How to bind the client PID to the user-approved Blender installation, and the reverse binding of Nexora server PID to the approved Nexora binary, needs a concrete Windows identity policy and race-resistant implementation. PID lookup alone is insufficient.
- An HMAC key requires safe provisioning and rotation. A key placed in add-on source, a CLI argument, or user-readable persistent state may be available to same-user processes; no secure key-delivery solution was tested.
- Same-user code remains a significant residual threat even with a user/logon-session DACL. The product owner must decide whether executable identity + challenge is adequate for the stated threat model or whether a stronger broker/isolation boundary is required.
- Pipe creation flags, DACL contents, token/SID handling, remote rejection, first-instance collision behavior, cancellation, and handle cleanup require real Windows implementation/testing; documentation is not a substitute.
- Python blocking behavior, Blender UI responsiveness, add-on registration/unregistration, and reconnect behavior remain unknown.
- No lockfile/dependency, Rust/Tauri, UI, candidate detection, or production code was changed for this investigation. No Connect/Disconnect controls or Blender add-on were introduced. No Phase 6 work was started.
- The pre-existing worktree contains numerous unrelated modified/untracked files. They have been preserved; this report is the only file created for Phase 5A.

## 7. Sources

Primary documentation reviewed on 2026-10-26. Links below point to Microsoft Learn/CPython documentation; GitHub source copies were consulted for the Microsoft Win32 and SDK API material and CPython multiprocessing documentation.

1. Microsoft, [Named Pipe Security and Access Rights](https://learn.microsoft.com/en-us/windows/win32/ipc/named-pipe-security-and-access-rights) — explicit security descriptor, default DACL, logon SID guidance.
2. Microsoft, [Pipe Names](https://learn.microsoft.com/en-us/windows/win32/ipc/pipe-names) — local pipe naming and namespace constraints.
3. Microsoft, [Named Pipe Instances](https://learn.microsoft.com/en-us/windows/win32/ipc/named-pipe-instances) — instance/concurrency model.
4. Microsoft, [Named Pipe Operations](https://learn.microsoft.com/en-us/windows/win32/ipc/named-pipe-operations) — connect/read/write operations and blocking behavior.
5. Microsoft, [`CreateNamedPipeW`](https://learn.microsoft.com/en-us/windows/win32/api/namedpipeapi/nf-namedpipeapi-createnamedpipew) — security attributes, first-instance and remote-client flags, and instance limits.
6. Microsoft, [`GetNamedPipeClientProcessId`](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-getnamedpipeclientprocessid) and [`GetNamedPipeServerProcessId`](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-getnamedpipeserverprocessid) — peer PID query APIs.
7. Microsoft, [`GetNamedPipeClientSessionId`](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-getnamedpipeclientsessionid) and [`GetNamedPipeServerSessionId`](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-getnamedpipeserversessionid) — peer session query APIs.
8. Microsoft, [`ImpersonateNamedPipeClient`](https://learn.microsoft.com/en-us/windows/win32/api/namedpipeapi/nf-namedpipeapi-impersonatenamedpipeclient) — client-token impersonation semantics and failure warning.
9. Python, [`multiprocessing` — Listeners and Clients](https://docs.python.org/3/library/multiprocessing.html#listeners-and-clients) — Windows `AF_PIPE` and optional HMAC challenge behavior. The exact Python version bundled with a supported Blender release remains unknown.
10. Source copies consulted: [MicrosoftDocs/win32 IPC documentation](https://github.com/MicrosoftDocs/win32/tree/docs/desktop-src/ipc), [MicrosoftDocs/sdk-api named-pipe API documentation](https://github.com/MicrosoftDocs/sdk-api/tree/docs/sdk-api-src/content/namedpipeapi), [MicrosoftDocs/sdk-api Winbase API documentation](https://github.com/MicrosoftDocs/sdk-api/tree/docs/sdk-api-src/content/winbase), and [CPython multiprocessing docs](https://github.com/python/cpython/blob/main/Doc/library/multiprocessing.rst). Repository branch pages can move over time; this source review did not pin commit IDs.

## Decision

**NO-GO for production adoption at this time.** This is an evidence-based hold, not a conclusion that Windows named pipes are inherently unusable. The transport is only a candidate until an isolated POC on actual Windows and supported Blender builds proves Rust/Python interoperability, explicit ACL/session restrictions, fail-closed endpoint ownership, peer identity in both directions, bounded protocol behavior, and clean lifecycle/security failures. Keep Blender candidate-only and non-connectable until that evidence and a separate implementation authorization exist.

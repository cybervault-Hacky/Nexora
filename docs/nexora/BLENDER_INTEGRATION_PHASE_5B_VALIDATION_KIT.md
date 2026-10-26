# Blender Integration Phase 5B — Reproducible Windows Validation Kit

**Status:** Validation-preparation kit created; real transport validation remains blocked.

**Production decision:** **NO-GO FOR PRODUCTION**, unchanged from Phase 5A.

**Date:** 2026-10-26

## Executive result

Prepared an isolated kit under `tools/blender-transport-spike/` with:

- a PowerShell environment preflight for Windows, Rust/Cargo, MSVC linker, Blender and its embedded Python, and required repository/kit files;
- a test matrix for access control, user/session separation, both-direction peer identity, same-user impersonation risk, endpoint precreation, duplicate clients, protocol robustness, lifecycle, and actual Blender compatibility;
- a JSON result template whose tests begin as `NOT_RUN` and whose production decision begins as `NO_GO`;
- a portable static-consistency checker that is explicitly **not** a transport test.

No pipe server, Blender add-on, protocol implementation, mock connection harness, production connector, or UI integration was added. This is deliberate: the current environment has no Windows, Rust/Cargo, Blender, or PowerShell runtime with which to validate an implementation. Writing a transport harness here would require selecting unverified Windows API behavior, framing, authentication, and identity assumptions—the exact questions the kit is intended to help test. The documented Windows pipe/Blender cases therefore remain **BLOCKED — REQUIRES WINDOWS + BLENDER + A SEPARATELY AUTHORIZED, REVIEWED POC**.

Passing the preflight or static checker can never be reported as a pipe connection, peer authentication, Blender compatibility, or genuine handshake. Phase 5A's production **NO-GO** remains in effect.

## 1. Files created

No existing source, configuration, roadmap, test, or Phase 5A file was modified. The following files were added:

1. `tools/blender-transport-spike/README.md` — prerequisites, exact preflight/static-check commands, safety model, result handling, and cleanup.
2. `tools/blender-transport-spike/Invoke-Preflight.ps1` — read-only Windows/tool/repository inventory, bounded Blender version/embedded-Python probe, redacted JSON output, and clear nonzero status for missing required prerequisites.
3. `tools/blender-transport-spike/TEST_PLAN.md` — 18 uniquely identified test cases with procedure, safe expected outcome, evidence, and pass/fail/blocked criteria.
4. `tools/blender-transport-spike/result-template.json` — structured environment, test-result, security-review, redaction, and decision fields; every case defaults to `NOT_RUN`, and the decision defaults to `NO_GO`.
5. `tools/blender-transport-spike/check_static.py` — standard-library-only static consistency check for test IDs, result defaults, and selected non-invasive script contracts. It does not parse or execute PowerShell and performs no IPC.

The kit is not imported by the Tauri app, desktop UI, or installer. It adds no dependencies and changes no production integration status or controls.

## 2. Environment available during preparation

| Requirement | Observed here |
|---|---|
| OS / architecture | Linux x86_64 (`Linux e2b.local`, kernel `6.1.158+`) |
| Python | 3.11.2 |
| PowerShell (`pwsh` / `powershell`) | Not installed |
| `rustc`, Cargo, `rustup` | Not installed / not found |
| Windows runtime / MSVC linker | Not available in this Linux host |
| Blender executable / embedded Python | Not available |
| Actual Windows user/logon sessions | Not available |
| Actual named-pipe endpoint or POC | None created |

Repository inspection included the Phase 5 implementation report, Phase 5A report, proposed connector contract, current candidate-only Tauri/Rust integration files, and existing Windows PowerShell script conventions. Current production source still has no pipe transport or handshake; installation detection remains candidate-only. Existing modified and untracked work was preserved.

## 3. Kit operation on Windows

From the repository root on a normal Windows PowerShell installation, run the preflight in a child process (no elevation and no execution-policy change):

```powershell
powershell.exe -NoProfile -File .\tools\blender-transport-spike\Invoke-Preflight.ps1
```

Optional explicit Blender path:

```powershell
powershell.exe -NoProfile -File .\tools\blender-transport-spike\Invoke-Preflight.ps1 `
  -BlenderPath 'C:\Program Files\Blender Foundation\Blender 4.x\blender.exe'
```

Optional redacted JSON output to a caller-selected existing directory:

```powershell
powershell.exe -NoProfile -File .\tools\blender-transport-spike\Invoke-Preflight.ps1 `
  -OutputPath "$env:TEMP\nexora-blender-transport-preflight.json"
```

The preflight reports Windows product/version/build/architecture; versions and availability for Rust/Cargo and other build tools; presence of the kit, required design documents, Cargo manifest, and current integration source; Blender availability/version; and embedded Blender Python version where the bounded probe succeeds. It does not expose executable paths in its JSON. The Blender probe runs only the selected executable with background/factory-startup/auto-execution-disabled flags and a fixed version-print expression, uses temporary Blender config/script/data locations for the child process, does not open/save a user project, and removes its temporary output/config directories. It never launches Nexora, changes system configuration/security policy, installs software, creates a pipe, or makes a network connection.

A missing required tool/file or failed required version probe yields `BLOCKED_MISSING_PREREQUISITES` and exit code 2. If all prerequisites are detected, the summary is `PREREQUISITES_DETECTED_NOT_TESTED`; all Windows pipe and genuine Blender tests still report `BLOCKED_NO_VALIDATION_HARNESS` because this kit intentionally has no transport implementation. The Windows/Blender support matrix remains undecided.

The portable static kit check is:

```powershell
python .\tools\blender-transport-spike\check_static.py
```

It verifies that the 18 plan IDs match the result template, all template results start `NOT_RUN`, the default decision is `NO_GO`, and selected preflight safety/status markers are present. It does not validate PowerShell syntax or execute any transport test.

For actual evidence, first obtain separate authorization for an isolated Rust server and manually enabled Blender health-only client. Follow [`tools/blender-transport-spike/TEST_PLAN.md`](../../tools/blender-transport-spike/TEST_PLAN.md), then copy `result-template.json` to a new local evidence file and record actual versions, artifact hashes, measured timings, observations, and sanitized evidence references. Do not mark a test passed from prerequisite detection, documentation, mocks, a pipe-open-only result, or a UI state. Preserve failures and blocked cases. Scrub user names, full/private paths, secrets/nonces, credentials, command-line sensitive values, pipe names where identifying, and scene/project content before sharing. Keep raw logs in the test VM.

## 4. Tests actually run

| Check | Result | Evidence boundary |
|---|---|---|
| `python3 tools/blender-transport-spike/check_static.py` | **Passed** after correcting an assertion in the static checker. | 18 case IDs matched; result defaults are `NOT_RUN`/`NO_GO`; selected preflight safety markers passed. This is kit consistency only. |
| JSON parsing and structural checks for `result-template.json` | **Passed as part of the static checker.** | Valid JSON; unique IDs align with plan; all outcomes remain `NOT_RUN`. |
| PowerShell preflight execution/syntax validation | **Not run — PowerShell is unavailable.** | Script was inspected statically only; no Windows execution is claimed. |
| Windows environment inventory / tool detection | **Blocked — requires Windows.** | No Windows host. |
| Rust/Cargo/Windows build validation | **Blocked — requires Rust/Cargo and Windows toolchain.** | None compiled. |
| Blender embedded-Python probe | **Blocked — requires Blender runtime.** | No Blender process was launched. |
| Pipe ACL, user/session, unexpected local client, squatting, client multiplicity, or peer-identity tests | **Blocked — requires Windows plus an authorized POC.** | No endpoint was created; no identity claim was tested. |
| Protocol, timeout, disconnect, restart, and reconnect tests | **Blocked — requires two actual POC peers.** | No protocol or harness exists. |
| Genuine Rust ↔ Blender health-check handshake / scene non-modification | **Not run; blocked — requires Windows + Blender + authorized POC.** | No handshake or scene test occurred. |

The static checker initially failed because its own assertion expected wording not used in the result template. The assertion was corrected and the checker was rerun successfully. This was a checker-text mismatch, not a product/transport test.

The initial whitespace scan found trailing Markdown hard-break spaces in the report header; those were removed and the final scan passed for all six new files. No application test suite, Rust test, Windows test, Blender test, or production build was run for this documentation/preparation phase.

## 5. Test coverage and remaining blocks

The test plan covers all requested areas, including:

- explicit per-user access-control behavior, other user and logon-session denial, and unexpected local-client rejection (no remote-network test is included);
- server-side client identity and client-side server identity, including PID/session observations, executable/file/publisher checks, races, inaccessible or exited processes, and same-user attacker limits;
- pipe precreation/squatting, competing/duplicate clients, and unexpected connection attempts;
- versioned valid health exchange, malformed/duplicate/unsupported messages, size boundaries, partial/truncated frames, finite connect/read/write deadlines, cancellation, disconnect, restart, and reconnect;
- real Blender embedded-Python API availability/interoperability, clean bridge lifecycle, UI responsiveness, and a genuine health-only exchange with no scene reads/writes.

Every plan row specifies the procedure, expected safe behavior, evidence to capture, and `PASS`/`FAIL`/`BLOCKED` criteria. The exact wire harness and endpoint-specific commands are intentionally not fabricated; a future authorized POC must supply those instructions after the transport design is reviewed. Until then, executing the transport cases is blocked, even on a machine where preflight finds all tools.

## 6. Security and scope

- No network listener or named pipe is created by this kit.
- No arbitrary Python/shell payload is executed. The only Python expression in the preflight is a fixed version query passed to Blender with auto-execution disabled.
- No Blender scene is opened, queried, modified, or saved by the preflight.
- No key, credential, challenge, nonce, full executable path, or scene content is written to the preflight JSON.
- Preflight scratch files and isolated Blender user directories are temporary and removed; the caller-selected JSON output is the only optional persistent preflight artifact.
- The result template defaults to `NO_GO`; all tests default to `NOT_RUN`.
- No production connector, named-pipe code, Blender add-on, Connect/Disconnect UI, installer integration, AI task execution, or Phase 6 work was added.

## 7. Remaining security questions and next steps

1. Run preflight on a real standard-user Windows machine with Rust/Cargo/MSVC linker and actual target Blender builds; capture the JSON report and exact builds. Preflight is only inventory.
2. Choose exact Windows/Blender versions from an approved product support policy. No supported range can be claimed until each exact matrix entry is tested.
3. Before transport testing, separately authorize and review an isolated POC's exact API usage, explicit DACL and logon-SID policy, remote rejection, first-instance/precreation behavior, one-client policy, framing limits, finite timeouts, challenge/key provisioning, both-direction process identity, and cleanup behavior.
4. Manually start Blender and the test server; run every positive and negative case in `TEST_PLAN.md`. Capture actual process identities and evidence from both endpoints; do not substitute a mock or another-language transport test.
5. Review same-user threat limitations and the secret/key distribution model. ACL access is not proof of intended-process identity. Resolve all critical security failures and preserve failed evidence.
6. Update the result template with actual statuses and redacted evidence, then perform a separate security/support-matrix review. Keep **NO-GO FOR PRODUCTION** until actual Windows + Blender health communication and acceptable peer-identity/access-control results justify reconsideration.

## Decision

**NO-GO FOR PRODUCTION remains unchanged.** This phase prepared the validation process but did not establish transport feasibility. No Windows pipe or Blender evidence was produced, and preflight/static checks alone cannot justify a transport choice or production implementation.

All pre-existing modified and untracked repository work was preserved. No files were staged or committed; nothing was pushed, merged, tagged, published, or released. Stop here at Phase 5B pending explicit authorization for the next isolated test step; production connector work remains unauthorized.

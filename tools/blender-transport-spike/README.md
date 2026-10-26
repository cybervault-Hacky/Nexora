# Blender transport feasibility validation kit (experimental)

**Not production code. Not a transport implementation. Not evidence of a successful connection.**

This isolated kit prepares a real Windows + Blender validation run for the unresolved Phase 5A feasibility gate. It contains an environment-only PowerShell preflight, test-case specification, and structured result template. It intentionally contains **no pipe server, Blender add-on, protocol implementation, or mocked connection harness**: those would have to choose unverified framing, ACL, authentication, identity, and lifecycle behavior and could create misleading results without a Windows/Rust/Blender runtime to validate them.

The preflight can establish that tools and repository files appear available. It cannot pass a pipe, identity, Blender compatibility, or handshake test. The transport cases remain **BLOCKED — REQUIRES WINDOWS + BLENDER + A SEPARATELY AUTHORIZED, REVIEWED POC** until that genuine test implementation exists.

## Prerequisites

- A real Windows PC or disposable Windows VM, running in a standard user session (do not elevate for this preflight).
- Windows PowerShell 5.1+ or PowerShell 7+.
- Rust/Cargo and the Windows MSVC build tools needed by the separately authorized POC. The script reports what it finds; it installs nothing.
- An existing Blender executable if you want the embedded-Python version probe. The script does not install Blender or discover installations recursively.
- A checkout containing the Phase 5/5A docs and current integration source.

The supported Nexora Windows/Blender version matrix is **not established**. Passing this prerequisite list does not create a support claim.

## Run the environment preflight

From the repository root in a normal PowerShell window, invoke it in a child PowerShell process so its explicit exit code does not close the interactive shell:

```powershell
powershell.exe -NoProfile -File .\tools\blender-transport-spike\Invoke-Preflight.ps1
```

On a PowerShell 7 installation, use `pwsh.exe` in place of `powershell.exe`.

To probe a Blender executable not on `PATH`, supply its exact executable path (the value is not written to the report):

```powershell
powershell.exe -NoProfile -File .\tools\blender-transport-spike\Invoke-Preflight.ps1 `
  -BlenderPath 'C:\Program Files\Blender Foundation\Blender 4.x\blender.exe'
```

To save the redacted JSON preflight report at a path you choose:

```powershell
powershell.exe -NoProfile -File .\tools\blender-transport-spike\Invoke-Preflight.ps1 `
  -OutputPath "$env:TEMP\nexora-blender-transport-preflight.json"
```

The script exits nonzero and explains which required prerequisites are missing. It does not change the execution policy; do not weaken system policy to run it. It does not install or update software, request elevation, alter ACLs, create a named pipe, contact a network service, or launch the Nexora application.

The Blender Python probe starts **only the explicitly selected Blender executable**, in background/factory-startup/auto-execution-disabled mode, with one fixed expression that prints Blender and embedded-Python version numbers. It redirects output to a private temporary directory and removes that directory after the probe. It temporarily points Blender's user config/scripts/datafiles environment variables at temporary locations in the child process only, then restores the calling PowerShell process environment. It never attaches to or terminates an already-running Blender process, opens a user project, or saves a scene. If Blender does not exit within the bounded probe time, only the child process created by this script is stopped and the probe is marked failed.

### Reading the preflight result

- `PREREQUISITES_DETECTED_NOT_TESTED` means only that the required environment was detected. It is **not** a transport test pass.
- `BLOCKED_MISSING_PREREQUISITES` means one or more required tools/files were not detected; the script exits with code 2.
- `BLOCKED_NO_VALIDATION_HARNESS` applies to all actual pipe, identity, protocol, and Blender health-check tests even if every prerequisite is found. This kit deliberately does not contain an unverified transport harness.
- The report avoids full executable paths, user names, pipe names, secret material, and Blender scene/project data. It may still contain machine/software version data; review before sharing.

## Test execution and results

Run the portable static consistency check from the repository root:

```powershell
python .\tools\blender-transport-spike\check_static.py
```

This checks test-ID consistency, the result template's default `NO_GO`/`NOT_RUN` state, and selected non-invasive preflight source contracts. It does not parse or execute PowerShell and proves nothing about Windows, Rust, Blender, pipes, peer identity, or a handshake.

1. Review [`TEST_PLAN.md`](TEST_PLAN.md) and select exact Windows/Blender versions from the product support policy. No policy or matrix is currently approved.
2. Do not attempt to test or claim Rust↔Blender transport using a mock, the OS preflight, a pipe-open check, or a different language's pipe implementation.
3. A real test requires a separately authorized, disposable Rust server + user-enabled Blender-side health-only client, after its wire framing, DACL, peer identity, timeouts, and secret handling have been reviewed. It must remain outside production registration, UI, and installer.
4. Copy [`result-template.json`](result-template.json) to a new, local evidence file. Record each case as `PASS`, `FAIL`, `BLOCKED`, or `NOT_RUN`; attach sanitized evidence references and preserve failures. Never change the default production decision to `GO` based on preflight or source/static checks.
5. Scrub usernames, private/full paths, pipe names if they reveal user identity, challenge/nonces/keys, command-line secrets, scene/project content, credentials, and unrelated logs before sharing evidence. Keep raw logs locally under the test VM's access controls.

## Test modes

| Mode | Included here? | What it can establish |
|---|---:|---|
| Automated environment preflight | Yes | OS/tool/file availability and version facts only. No transport behavior. |
| Static review of plan/result files | Yes | The kit's test IDs and result template are consistent; no runtime/security guarantee. |
| Manual Windows access-control/identity adversarial tests | Specified, not executable without POC | Actual Windows behavior only when driven by the reviewed POC and test accounts/sessions. |
| Real Blender embedded-Python client and health exchange | Specified, not executable without POC | Actual Blender runtime and cross-process interoperability only with the actual two peers. |
| Mocked connector tests | No | Not accepted as evidence of pipe connectivity or peer identity. |

## Cleanup

The preflight removes its own temporary output/config folders. If a future isolated POC is authorized, its runbook must name the exact pipe and processes it owns and close its handles on every exit path. Stop only POC processes started by the tester; do not kill unrelated Blender/Nexora processes. Disable the test add-on through Blender's UI, close the test server normally, verify the owned pipe is gone, and remove only the test artifacts/evidence folder created for that run. Do not delete or modify Nexora production files, existing user Blender settings/projects, or unrelated repository changes.

## References

- [Phase 5A transport feasibility report](../../docs/nexora/BLENDER_INTEGRATION_PHASE_5A_TRANSPORT_SPIKE.md)
- [Phase 5 implementation gate](../../docs/nexora/BLENDER_INTEGRATION_PHASE_5_IMPLEMENTATION.md)
- [Proposed connector contract](../../docs/nexora/APP_CONNECTOR_CONTRACT.md)
- [Phase 5B validation-kit report](../../docs/nexora/BLENDER_INTEGRATION_PHASE_5B_VALIDATION_KIT.md)

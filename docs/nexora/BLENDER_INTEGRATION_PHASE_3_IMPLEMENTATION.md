# Blender Integration — Phase 3 Detection and Connection Foundation

**Implementation status:** Typed lifecycle boundary and honest detector-availability reporting added. A Blender installation detector, connection, and task executor are **not** implemented.
**Date:** 2026-10-26
**Scope authorization:** The requested Phase 3 work package is “Blender detection & connection foundation.” The existing proposed 12-phase outline labels the shared catalogue as Phase 3 and Blender detection as Phase 4, while the delivered Phase 2 implementation already supplied the shared catalogue. This report follows the explicit Phase 3 authorization in the task and records that numbering mismatch without renumbering or rewriting the proposed roadmap.

## What changed

- Added a reusable Rust/Tauri installation-result enum and a narrow `get_blender_detection_status` command in `src-tauri/src/integrations/commands.rs`, registered through the existing explicit Tauri handler. The command reports `unsupported` on both Windows and non-Windows builds, with a platform-specific reason. It performs **no path search, registry query, executable inspection, or process launch**.
- Added a typed TypeScript IPC wrapper that validates native tagged-union results at runtime, avoids invoking native code in browser preview, and converts malformed responses or IPC failures to structured detection errors without exposing raw native details.
- The shared catalogue now represents installation detection as its own tagged state (`unknown`, `checking`, `found`, `not_found`, `unsupported`, `error`, or `access_denied`). The App Integrations page performs a one-time detector-status query when opened, exposes its loading/result via a polite live region, and ignores stale results after unmount. This is a **capability/status query, not an automatic installation scan**; no refresh or Connect control is shown because neither operation is implemented.
- Blender remains **Planned**. Its task readiness is explicitly **Not ready** because there is no validated connector, handshake, or task executor. `connectionStatus` remains null, which the page presents as unavailable rather than as a false `Not Connected` lifecycle observation. Detection cannot change connection state or task readiness. VS Code and Figma remain undetected, planned, and without connection actions.
- Added focused coverage for typed result validation, unsupported platform/browser behavior, malformed and failed IPC, accessible pending-to-unsupported rendering, separation of installation from connection/readiness, and the Rust/TypeScript Tauri command contract.

No Connect button, user-selected executable field, Blender add-on, named pipe, app discovery, process launch, or task/action protocol was added. The generic result type leaves room for future verified `Found`, `NotFound`, `AccessDenied`, and `Error` reports; the current backend emits only `Unsupported`.

## Feasibility review and discovery decisions

The codebase is a Tauri 2 desktop app with an explicit Rust `generate_handler!` command list and thin typed TypeScript IPC wrappers. This supports a native-owned detector boundary, but does not itself provide Windows discovery. The existing `windows-sys` dependency lacks the registry/version-resource APIs needed for a reviewed detector, and this workspace has no Rust compiler or Cargo. Adding uncompiled Windows FFI would create an unsafe, unverifiable detector, so this phase stops at the typed native boundary instead of fabricating results.

Potential **future, bounded Windows discovery sources**—none are implemented or currently supported—are:

1. Per-user and machine uninstall/App Paths registry views, treating `InstallLocation`, `DisplayIcon`, and version strings only as candidate hints, never proof that an executable exists or is Blender.
2. Documented Blender Foundation installation directories under known Windows program/user locations, checking only a short fixed allowlist rather than recursively searching a drive.
3. A user-selected executable obtained through a native file picker in a later authorized phase. Such a path would still require canonicalization, file/PE identity checks, and version validation; merely existing or being named `blender.exe` must not make it trusted.

PATH lookup is not implemented and should not be considered authoritative: it can select an unintended executable and does not establish its identity or compatibility. Portable, store-managed, launcher, symlink, and multiple-install cases need explicit discovery and selection rules before being advertised. No whole-disk scan, arbitrary renderer path, directory recursion, administrator privilege, or automatic install is part of the current code.

### Identity, version, and compatibility

No Blender executable or version was observed in this environment. No supported Blender version range, Windows build range, architecture matrix, minimum Blender release, or compatibility result has been established. Registry metadata can be stale or spoofed. A future detector should inspect a candidate file and Windows PE/version metadata without running it, and define what evidence is sufficient to identify the Blender executable; basename and product strings alone can be forged.

Obtaining authoritative `blender.exe --version` output executes the selected binary. That must remain a separate, explicit, narrowly scoped probe—not something run during catalogue rendering or detection. Any later approved probe needs a fixed argument vector (no shell), explicit user approval for the candidate, sanitized environment, no inherited credentials, bounded output, a short timeout/cancellation policy, safe child cleanup, and clear reporting that probing launches the application executable. No such probe or timeout exists in this phase. No arbitrary user path is accepted by the new command.

## Connection feasibility and boundary

Blender does not have a Nexora-validated connection workflow in this repository. Executable discovery alone would not establish a live connection or task readiness. A reviewed Blender-side add-on or equivalent in-process component is expected to be necessary for a stable, supported API; no such add-on or handshake exists here.

A Nexora-owned, user-scoped Windows named pipe remains a **proposal**, not a verified transport. Windows provides named-pipe primitives, and a Blender add-on could potentially initiate a client connection, but the exact APIs available in Blender’s embedded Python, behavior across candidate Blender builds, thread/main-loop integration, reconnect/cancellation behavior, and shutdown lifecycle have not been tested. Linux frontend tests cannot validate any of these Windows/Blender properties.

Before a later connection phase, a Windows spike should establish:

- A pipe ACL restricted to the logged-in user (and only required system principals), a unique per-session endpoint, first-instance protection, and reliable cleanup on app exit.
- Server-side client PID retrieval and executable-path/file-identity verification, tied to the explicitly selected supported Blender installation. A same-user named-pipe ACL is not a sandbox against malicious code running as that user.
- A per-session challenge/handshake and protocol-version check; no reusable secret in command arguments, logs, or project files.
- Strict framed messages with maximum size, rate, count, and time limits; typed allowlisted operations only; no Python/source-code, shell, or arbitrary Blender operator payloads.
- A small reviewed add-on that queues validated operations onto Blender’s main thread, and explicit user-directed setup, health, reconnect, disconnect, and cleanup behavior.

No network listener, unauthenticated IPC, named-pipe code, process identity check, peer authentication, connection lifecycle, permission grant, Blender control, or AI routing was introduced. In the UI the generic `ConnectionStatus` union still models lifecycle states for future connectors, but Blender has no observed connection state. Task readiness remains blocked independently of installation status.

## Status mapping in this build

| Dimension | Current Blender result | Meaning |
|---|---|---|
| Installation detection | `Unsupported` with a reason | On non-Windows builds, Windows detection is inapplicable. On Windows builds, the detector is not yet implemented/qualified. No search or executable probe was performed. |
| Connection | No observed connection (`null` in the descriptor); no connection controls | No connector or validated handshake exists. This must not be shown as Connected, and installation would not change it. |
| Task readiness | Not ready / blocked | No connector, approved action protocol, permission enforcement, or task executor exists. It must not be inferred from a future Found result. |
| Compatibility | Unverified | No Windows/Blender support matrix has been tested. |

A future real detector may return `Found`, `NotFound`, `AccessDenied`, or `Error`, but this build does not fabricate any such outcome. “Not found” is only valid after the detector actually checks its approved bounded sources; an inability to scan is Unsupported or Error, not NotFound.

## Validation and limitations

- Focused UI/contract run: `corepack pnpm --filter @nexora/desktop exec vitest run ui/src/features/integrations ui/src/pages/AppIntegrationsPage.test.tsx ui/src/lib/rustContract.test.ts` — **4 files, 79 tests passed**.
- TypeScript: `corepack pnpm --filter @nexora/desktop typecheck` — **passed**.
- Full UI suite: `corepack pnpm --filter @nexora/desktop test` — **93 files, 910 tests passed**.
- Production build: `corepack pnpm --filter @nexora/desktop build` — **passed** (TypeScript, Vite with 134 modules, config-security, installer-contract). The build reported existing mixed static/dynamic-import warnings and a 609.76 kB JS chunk above 500 kB. Installer validation found 3 frontend assets / 751,724 bytes and no model/runtime/credential payloads.
- `git diff --check` — **passed**.
- Native verification unavailable: `rustc` and `cargo` are not installed in this Linux environment. No native Tauri build/test, Windows build, Windows registry/path test, named-pipe test, Windows UI/installer test, Blender launch, Blender version probe, or Blender runtime test was run.
- The UI build/browser environment can validate the renderer’s typed status/error behavior and the source-level Rust/TypeScript command registration contract. It cannot validate Rust compilation or Windows security, installation identity, compatibility, connection, or readiness.

## Files changed in Phase 3

- `apps/desktop/src-tauri/src/integrations/mod.rs` — new integration boundary module.
- `apps/desktop/src-tauri/src/integrations/commands.rs` — typed detector result and capability-only Tauri command; always reports Unsupported without scanning or launching.
- `apps/desktop/src-tauri/src/lib.rs` — additive module and command registration only; existing unrelated native modifications are retained.
- `apps/desktop/ui/src/features/integrations/catalogue.ts` — typed installation result states and Blender Not-ready status, separate from connection state.
- `apps/desktop/ui/src/features/integrations/native.ts` — native status IPC wrapper and runtime result validation.
- `apps/desktop/ui/src/features/integrations/native.test.ts` — IPC result, unsupported-browser, and error-boundary tests.
- `apps/desktop/ui/src/features/integrations/catalogue.test.ts` — state-separation assertions.
- `apps/desktop/ui/src/pages/AppIntegrationsPage.tsx` — one-shot status query, accessible states, stale-response guard, and honest Blender labels; no operational controls.
- `apps/desktop/ui/src/pages/AppIntegrationsPage.test.tsx` — accessible loading/unsupported and no-false-ready coverage.
- `apps/desktop/ui/src/lib/rustContract.test.ts` — additive registration and tagged-union source-contract checks; pre-existing media contract coverage is preserved.
- `docs/nexora/BLENDER_INTEGRATION_PHASE_3_IMPLEMENTATION.md` — this implementation and feasibility report.

Existing work in chat, inference, settings, Rust media modules, media UI, Phase 2 catalogue, and evidence was preserved. No dependencies or lockfiles were changed. No files were staged or committed. Phase 3 stops here; Phase 4 and later require explicit authorization.

# Nexora Blender Integration — Phase 4 Implementation Report

**Phase:** 4 — Real Blender Installation Detection  
**Status:** Implemented with qualification limits; Windows/runtime verification pending  
**Scope stop:** Phase 4 only. No Phase 5 work was started.

## Summary

Added a deliberate, Windows-only Blender installation check to the existing App Integrations flow. The user must select **Check Installation** on the Blender catalogue card; opening the page, changing filters, navigating, or rendering does not start a scan. That action calls the registered Tauri command `get_blender_detection_status`, which invokes a bounded Rust detector and returns a tagged, structured result. The renderer validates the returned payload before using it.

A `Found` result is deliberately labelled **candidate only**. It establishes only that a canonical executable path was reached through one of the supported Blender Foundation roots, has the expected `blender.exe` basename, and contains a bounded, structurally plausible Windows PE header with an allowed machine/optional-header pairing. It does **not** establish that the executable is published by Blender Foundation, is authentic Blender, is safe, or is compatible. Version remains `null` / “not verified.” No candidate is launched to strengthen the result.

Installation status remains separate from connection and task readiness. Blender remains Planned, no Connect/Disconnect control was added, and task readiness remains blocked because there is no validated connector, handshake, or executor. VS Code and Figma remain Planned and non-connectable. Existing chat, model selection, settings, media-generation, and catalogue behavior was not intentionally changed.

## Detector scope and bounds

The Windows detector derives at most three candidate roots from the standard `ProgramFiles`, `ProgramFiles(x86)`, and `LOCALAPPDATA` environment entries, appending the corresponding `Blender Foundation` subdirectory (and `Programs` for the per-user root). Roots must be absolute and local; missing, relative, or UNC roots are not scanned and are surfaced as incomplete search information.

For each available root it:

- checks only the root itself and immediate child directories named `Blender` or `Blender <suffix>`;
- considers only a `blender.exe` directly at the root or directly inside one of those immediate child directories;
- limits root enumeration to 128 entries and the reported candidate count to 16;
- canonicalizes candidate paths and requires them to remain beneath a canonical configured root;
- checks the executable basename and reads only a bounded PE header (maximum file-size policy 512 MiB, PE-header offset at most 1 MiB); and
- recognizes the x86, x64, and ARM64 PE machine/optional-header pairings.

This is not a recursive walk. It does not scan drives, consult the registry, accept a renderer-supplied path, require administrator privileges, download/install Blender, or launch a process. Search limits, access denials, unavailable roots, and I/O failures are represented as incomplete/error results rather than silently reported as a confident `NotFound`. If a qualifying candidate is found while some locations are incomplete, the result includes those locations.

These roots are intentionally limited and do not discover every Blender distribution (for example, arbitrary portable/archive locations). Environment-derived locations are not a cryptographic trust source. PE structure, folder name, and executable basename are weak candidate evidence only; they do not prove product or publisher identity.

## Typed IPC and UI behavior

Rust returns one of `found`, `not_found`, `unsupported`, `access_denied`, or `error` through `InstallationDetectionResult`. Found entries include path, nullable version, detection method, candidate-only confidence, and a validation explanation. Incomplete and rejected-candidate metadata is retained where applicable. The TypeScript boundary checks the tagged union and its nested fields and maps malformed/failed IPC to a typed error.

The App Integrations card starts with installation state `unknown`. The explicit action transitions to `checking` and disables itself while a request is pending. A request identifier suppresses a result if the page has unmounted or a newer request supersedes it; the IPC helper coalesces overlapping calls. Browser preview reports that native checking is unsupported without scanning files.

No Connect action or task readiness is inferred from a candidate. Connection and task states are unchanged by an installation result.

## Files changed for Phase 4

- `apps/desktop/src-tauri/src/integrations/detector.rs` — bounded Windows detector, PE-header candidate checks, and focused Rust test source.
- `apps/desktop/src-tauri/src/integrations/commands.rs` and `mod.rs` — structured result types and detector command boundary.
- `apps/desktop/src-tauri/src/lib.rs` — existing explicit command registration retains the Blender detection command.
- `apps/desktop/ui/src/features/integrations/catalogue.ts` — typed installation result shape and accurate user-facing setup note.
- `apps/desktop/ui/src/features/integrations/native.ts` — validated typed IPC and explicit-check helper.
- `apps/desktop/ui/src/pages/AppIntegrationsPage.tsx` and `.test.tsx` — explicit Check Installation control, status rendering, and UI behavior coverage.
- `apps/desktop/ui/src/features/integrations/native.test.ts` — IPC payload validation and explicit-call tests.
- `apps/desktop/ui/src/lib/rustContract.test.ts` — command/status and detector-contract regression checks.
- `apps/desktop/ui/src/styles/app.css` — layout for the installation action.

## Tests and verification

Verified in this Linux sandbox:

- Focused App Integrations, native IPC, and Rust/TypeScript contract tests: **76 passed**.
- Full desktop UI test suite: **93 files, 913 tests passed**.
- Desktop TypeScript typecheck (`corepack pnpm --dir apps/desktop typecheck`): passed.
- Desktop production build plus configuration-security and installer-contract checks (`corepack pnpm --dir apps/desktop build`): passed. Vite emitted existing mixed dynamic/static import and >500 kB chunk-size warnings.

Not verified:

- Rust compilation or Rust unit tests: `rustc`, `cargo`, and `rustup` are not installed in this environment. Rust tests are present in source but have not run.
- Windows compilation, Windows filesystem behavior, Windows Tauri runtime, or detection against a real Blender installation: this sandbox is Linux and has no Windows toolchain/runtime or Blender installation. No such result is claimed.

A Windows build/test and a real Windows smoke test remain required before treating this detector as runtime-qualified. Even then, stronger publisher/product identity validation and a safe version probe would be separate work; absent those, keep results candidate-only with unknown version.

## Phase boundary

Phase 4 stops at installation detection. No connector, handshake, add-on, pipe, Blender launch/version probe, scene/model workflow, VS Code/Figma integration, or Phase 5 implementation is included.

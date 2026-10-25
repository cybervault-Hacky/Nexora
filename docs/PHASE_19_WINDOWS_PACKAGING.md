# Phase 19 — Windows Packaging Audit & Verification

**Result: PARTIALLY VERIFIED — BLOCKED; no Windows installer was produced**

**Review date:** 2026-10-24

**Scope:** Windows Tauri/Rust architecture, NSIS configuration, existing build/smoke scripts, CI packaging path, installer/runtime resources, and supported static/frontend checks. This report does not authorize or perform Phase 20 work.

## Verdict

The existing packaging path has been audited and narrowly corrected. Static/frontend checks pass, and the Windows scripts now make the x64 MSVC target, production settings, WebView2 bootstrapper behavior, current-user paths, secret guards, and uninstall/data-preservation checks explicit.

**Native Windows verification is blocked.** This sandbox is Linux x86_64 and has no Cargo/Rust toolchain, PowerShell, MSVC/Windows SDK, NSIS, or SignTool. No Windows binary or installer was built; no installer contents, signature, installation, upgrade, uninstallation, ACL behavior, GUI launch, WebView2 installation, or Windows inference was executed. The code/script corrections are therefore not Windows-verified, and Phase 19 remains partially verified/blocked.

Phase 18 remains **PARTIALLY VERIFIED — NOT RELEASE-READY**, as recorded in [`PHASE_18_INTEGRATION_VERIFICATION.md`](PHASE_18_INTEGRATION_VERIFICATION.md). Its blockers are carried forward unchanged; this phase did not modify billing, authentication, backend, or payment code.

## Windows architecture and package contract

| Area | Audit result |
|---|---|
| Product identity and bundle | `tauri.conf.json` declares Nexora `0.1.0`, identifier `com.nexora.desktop`, frontend build hook `pnpm build` and `../ui/dist`, and the NSIS target. It configures `currentUser` installation. The Tauri 2.12.5 NSIS template maps this mode to `$LOCALAPPDATA\${PRODUCTNAME}`, so the expected app root is `%LOCALAPPDATA%\Nexora`; it does not require an all-users install. The Tauri `publisher` value is package metadata, not evidence of an Authenticode signature or a verified publisher identity. |
| Supported architecture/toolchain | The repository pins Rust `1.93.0`, pnpm `9.15.4`, and the Tauri 2.12.5 family. The build script now rejects non-Windows and non-x64 hosts, checks Node 22, the pinned pnpm/Rust versions and installed `x86_64-pc-windows-msvc` target, and passes that target explicitly to Tauri. ARM64 and 32-bit packages are not configured. Visual Studio 2022 C++ Build Tools and a Windows SDK remain required; their operation was not tested here. |
| WebView2 | The config now explicitly selects Tauri's silent `downloadBootstrapper` mode. Tauri's pinned NSIS template downloads Microsoft's WebView2 bootstrapper during installation when the Evergreen runtime is absent; therefore a first install can require internet access. It is not an offline-bundled WebView2 runtime. The smoke test now allows a pre-install runtime to be absent and checks for WebView2 **after** the installer has run. Neither bootstrapper download nor runtime installation was executed in this environment. |
| Installation and user data | The app install root is separate from Tauri `app_data_dir()`. Tauri 2.12.5 maps `app_data_dir()` to the platform data directory plus the bundle identifier; on Windows this is `%APPDATA%\com.nexora.desktop`. Conversation history is stored as `conversations.json` there, and the default model directory is `app_data_dir()/models`. The Tauri NSIS template supports an app-data deletion option at uninstall and can remove both roaming and local app-data roots, so preservation must be tested rather than assumed. The revised smoke script refuses non-empty app install, roaming AppData, or local AppData directories; writes disposable sentinels in both data roots; invokes the in-place uninstaller with quoted `_?=$InstallRoot`; and checks both sentinels and the payload afterward. It removes only its own sentinel, the test-installed uninstaller stub when NSIS leaves it in place, and empty directories it created. This is an unexecuted test design, not evidence that Windows uninstall preserves data. |
| App/runtime resources | The inspected bundle config has no `externalBin` or `resources` declarations. A search of the Tauri subtree found no `llama-server.exe`, DLL, GGUF, or other model/runtime binary. The installer does **not** bundle a local inference runtime or model weights. Keep the runtime separately supplied/managed and the Mini/Sonnet artifacts separately acquired through their approved metadata and SHA-256 verification path. Existing `.part` download, digest verification, and final rename behavior is unchanged; both profiles remain `locally_verified: false`. No Linux executable or placeholder runtime was substituted. No Windows PE import table or installer payload was available, so additional native DLL/VC runtime dependencies beyond WebView2 remain undetermined. |
| Icons and metadata | The configured PNG icons have valid PNG signatures and dimensions; the ICO header is structurally valid with six entries. This is a basic file-format check only, not Windows rendering, NSIS inclusion, or installer-resource validation. No icons, signing identity, version, or release metadata were invented. |
| Chat, local inference, and auth | No application behavior was changed. Startup remains chat-first and signed-out local chat/history remain independent of account services. Local inference remains loopback-only. Windows Credential Manager/keyring, process creation/cleanup, file ACLs, and inference were not exercised. |
| CI | `.github/workflows/ci.yml` already has a `windows-2022` job that compiles/tests the desktop crate and calls the Windows build and smoke scripts. The workflow was not changed or triggered; its presence is not a result from this run. No artifact upload/publication step was executed. |
| Signing | Signing remains optional and applies to generated NSIS setup executable(s), not an individual app binary. The build script accepts only a thumbprint for an approved certificate already in the current-user Personal store and verifies Authenticode status when signing is enabled. It rejects legacy PFX path/password environment variables so the password is never passed on the SignTool command line. With no certificate thumbprint configured, a future local build is explicitly unsigned. The PFX variables and thumbprint were unset in this environment. No certificate, private key, signature, installer, or release artifact was created. |

Pinned Tauri source references used for the audit:

- [Tauri 2.12.5 WebView install-mode config](https://github.com/tauri-apps/tauri/blob/tauri-v2.12.5/crates/tauri-utils/src/config.rs) — the pinned default is silent `DownloadBootstrapper`; the project now declares that mode explicitly.
- [Tauri 2.12.5 NSIS current-user install path](https://github.com/tauri-apps/tauri/blob/tauri-v2.12.5/crates/tauri-bundler/src/bundle/windows/nsis/installer.nsi#L534-L547), [WebView2 bootstrapper behavior](https://github.com/tauri-apps/tauri/blob/tauri-v2.12.5/crates/tauri-bundler/src/bundle/windows/nsis/installer.nsi#L591-L650), [in-place uninstall handling](https://github.com/tauri-apps/tauri/blob/tauri-v2.12.5/crates/tauri-bundler/src/bundle/windows/nsis/installer.nsi#L364-L408), and [optional AppData deletion](https://github.com/tauri-apps/tauri/blob/tauri-v2.12.5/crates/tauri-bundler/src/bundle/windows/nsis/installer.nsi#L913-L929).
- [Tauri 2.12.5 Windows data-directory mapping](https://github.com/tauri-apps/tauri/blob/tauri-v2.12.5/crates/tauri/src/path/desktop.rs#L68-L75) and [`app_data_dir()` resolution](https://github.com/tauri-apps/tauri/blob/tauri-v2.12.5/crates/tauri/src/path/desktop.rs#L247-L270).

## Corrections made

1. **Made WebView2 installation explicit** in `apps/desktop/src-tauri/tauri.conf.json`: silent `downloadBootstrapper`. The desktop README now documents that installation may need internet access when Evergreen is absent.
2. **Pinned and hardened the Windows build script** in `scripts/windows/build-installer.ps1`:
   - Restricts builds to native Windows x64 and `x86_64-pc-windows-msvc`; validates Node 22, pnpm 9.15.4, Rust 1.93.0, and the installed Rust target.
   - Passes `--target x86_64-pc-windows-msvc` explicitly and checks the target-specific NSIS output directory.
   - Forces production build settings, rejects loopback HTTP and non-production environment requests, and permits an unconfigured backend API for offline-first packaging and requires HTTPS when an API origin is configured.
   - Adds missing build-environment denylist names for the test database, SMTP password, and alternate Google client-secret name, alongside existing database/payment secret checks.
   - Checks exit codes from external package-manager commands. Signing uses a current-user certificate-store thumbprint; legacy PFX path/password inputs are rejected to avoid command-line credential exposure, and successful signing must report a valid Authenticode signature.
3. **Corrected and made the NSIS smoke test safer** in `scripts/windows/smoke-verify.ps1`:
   - Uses `%LOCALAPPDATA%\Nexora` and the explicit x64 target output path, fixing G-09's stale `Programs\Nexora` assumption.
   - Checks WebView2 after installation, so the test can exercise the configured bootstrapper on a clean machine.
   - Refuses existing install/AppData contents, checks clean-profile roaming and local AppData sentinels across silent uninstall, passes the quoted `_?=$InstallRoot` argument, and verifies the application payload is removed. Because an in-place NSIS uninstaller cannot delete its own executable, the smoke harness removes only that test-installed stub after checking that no other install files remain.
   - Cleanup is limited to the sentinel, the test-installed uninstaller stub, and empty directories created by this smoke test; it does not delete a pre-existing user's AppData or model directory.
4. **Updated packaging documentation** in `apps/desktop/README.md` with the persistent history/model paths, runtime/model separation from the installer, and guarded Windows packaging/smoke commands. The G-09 row in `docs/ARCHITECTURE_DECISION_RECORD.md` now records the code correction while clearly retaining Windows verification as pending.

These are code/config/script changes only. They do not establish that the installer builds or behaves as intended on Windows.

## Checks and results

| Check | Result |
|---|---|
| `corepack pnpm install --frozen-lockfile` | **PASS** earlier in this phase: 165 packages installed from the pinned lockfile. No lockfile change. |
| `corepack pnpm --filter @nexora/desktop typecheck` | **PASS**. |
| `corepack pnpm --filter @nexora/desktop test` | **PASS** — 88 files and 870 tests passed (106.27 seconds reported by Vitest). These are frontend/UI tests, not native Windows tests. |
| `corepack pnpm --filter @nexora/desktop build` | **PASS** — TypeScript check, Vite production build (126 modules), and the config/secret scanner passed. The standalone `node scripts/check-config-security.mjs` also passed. Vite emitted existing non-fatal warnings for `theme-init.js`, mixed static/dynamic imports, and a minified JS chunk above 500 kB. |
| Static Tauri/resource/script assertions | **PASS** — JSON identity, NSIS/current-user/WebView2 settings, absence of `resources`/`externalBin`, icon headers/dimensions, pinned target, production/secret guards, smoke paths, and sentinel checks were inspected with local static checks. No `.exe`, DLL, `.gguf`, or `.bin` runtime/model file was found under `apps/desktop/src-tauri`. This does not parse or execute PowerShell or validate a produced bundle. |
| Tauri CLI help | **PASS — help only.** After frozen dependency installation, `tauri build --help` exposed `--target`; the Linux CLI listed only Linux bundle formats. No Tauri build was attempted. |
| `cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml --target x86_64-pc-windows-msvc` | **BLOCKED**, exit 127: `cargo: command not found`. |
| Windows/PowerShell/NSIS/signing | **BLOCKED / NOT RUN.** Host is Linux x86_64; `pwsh`, Windows PowerShell, the MSVC compiler/linker (`cl.exe`/MSVC `link.exe`), Windows SDK/MSBuild, NSIS, and SignTool are absent. The PowerShell scripts could not be parsed or executed here. |
| Installer build, inspection, signing, installation, upgrade, uninstall, GUI, WebView2 and Windows inference | **NOT RUN.** No Windows installer was generated, no SHA-256 installer manifest exists, and no Windows execution or CI workflow was triggered. |

No database, account, OAuth, payment, or production operation was performed. No package-manager lockfile was changed.

## Preserved Phase 18 blockers and release gate

- **G-01 remains open and release-critical:** a client-selected plan change can mutate an active/trialing subscription without provider verification. No billing/backend fix was made.
- Native Rust and disposable PostgreSQL validation remain unavailable. OAuth/account behavior and payments are unverified; no live provider operation was attempted.
- Razorpay remains blocked and unimplemented. Prices, quotas, trials, and Premium entitlements remain unapproved; do not present them as operational or verified.
- No real Mini/Sonnet model download, digest verification on a target Windows host, load, or inference was run. Preserve approved IDs/metadata and `locally_verified: false`.
- WebView2 bootstrapper behavior, current-user ACLs, Windows Credential Manager, process lifecycle, GUI startup, additional native DLL/VC runtime dependencies, upgrade/uninstall behavior, AppData preservation, and target-runtime compatibility remain unverified until a native Windows run.

**Release decision:** Phase 19 is partially verified/blocked, not release-ready. A later authorized Windows run must build and inspect the NSIS package, classify it as signed or unsigned, and execute the smoke test on a clean Windows profile before any Windows behavior is claimed. No publication, release, deployment, artifact distribution, CI trigger, commit, push, merge, or tag occurred. Stop after Phase 19 and await explicit Phase 20 authorization.

## Worktree preservation

The session remains on `arena/9ea88704-nexora` at `8b2e8415bc67ec31e343218364fa4a6ce3741dab`. The Phase 19 pre-edit snapshot recorded 89 modified tracked paths, 10 untracked paths, and zero staged paths. At Phase 19 end, the worktree has 92 modified tracked paths, 11 untracked paths (the original 10 plus this report), and zero staged paths. Reconstructing that snapshot against the current worktree showed exactly five tracked files changed since the baseline: `apps/desktop/README.md`, `apps/desktop/src-tauri/tauri.conf.json`, `docs/ARCHITECTURE_DECISION_RECORD.md`, `scripts/windows/build-installer.ps1`, and `scripts/windows/smoke-verify.ps1`. The original tracked changes and untracked paths were preserved. No lockfile, model, installer, or other generated package artifact was added; nothing was staged or committed.

## Step 3 installer-contract and acceptance-harness addendum (2026-10-24)

This is a later, additive Step 3 engineering update. It does not erase the earlier Phase 19 findings or change their evidence status. Step 3 is authorized; no Step 4 work is included.

### Changes in this addendum

- Added `scripts/check-installer-contract.mjs` and `scripts/check-installer-contract.test.mjs`. The validator cross-checks the four product versions, Nexora identity/publisher, one visible `main` window, the reviewed main-window capability/permission set, the production frontend entry, NSIS-only/current-user install, silent WebView2 bootstrapper, and required icon file formats/dimensions. It rejects custom NSIS templates/hooks, non-empty resources/sidecars, model/runtime/credential payload extensions, dotenv payloads, and frontend symlinks. Its **15 Node tests** are source/build-output contract checks, not generated-installer inspection.
- Added `pnpm test:installer-contract` to the root `check` command and made the desktop production build run the contract validator after the existing config-security scan.
- Tightened `scripts/windows/build-installer.ps1`: exact Node.js `22.22.3`, pnpm `9.15.4`, Rust/Cargo `1.93.0`, x64 Windows/PowerShell and MSVC tool checks; frozen dependency installation; frontend and contract tests; Rust formatting, Clippy, and tests; explicit `x86_64-pc-windows-msvc` NSIS build; exactly one installer; optional approved certificate-store signing; and SHA-256 manifest generation after signing. The script resolves commands from the repository root and does not clean/remove prior output files.
- Pinned CI Node.js to `22.22.3` and corrected the Windows smoke-step name so it no longer claims GUI launch in its headless GitHub Actions session. The CI workflow was edited but not triggered.
- Expanded `scripts/windows/smoke-verify.ps1` to check installer-manifest SHA-256 and Authenticode status, copy/use Unicode and space-containing test paths, verify the installed AMD64 PE, current-user Add/Remove Programs identity and install location, Start Menu/desktop shortcut targets, WebView2 presence and post-uninstall retention, same-build reinstall bytes, silent uninstall cleanup, and reinstall-after-uninstall. It seeds disposable history/settings/model-storage sentinels and uses cautious cleanup. An interactive local session launches the installed exe outside the checkout and through Start Menu; `GITHUB_ACTIONS`/headless sessions explicitly skip GUI claims. This same-build test is not a previous-version upgrade test and does not test offline network isolation or model inference.
- Updated `apps/desktop/README.md` and `docs/PRODUCTION_DEPLOYMENT_AND_SECURITY.md` for the exact toolchain, startup/data paths, optional uninstall data-delete checkbox, shared WebView2 behavior, commands, and evidence limitations.

Tauri source at pinned tag `tauri-v2.12.5` was read for the current-user `%LOCALAPPDATA%\Nexora` location, `HKCU` uninstall registration, silent-install shortcut creation, conditional AppData deletion (not during update), and WebView2 behavior. The NSIS 3.13 download/tool files are pinned and checked by that Tauri version using upstream SHA-1 values in [the pinned NSIS build source](https://github.com/tauri-apps/tauri/blob/tauri-v2.12.5/crates/tauri-bundler/src/bundle/windows/nsis/mod.rs#L36-L76). These are upstream-source observations only; they do not prove the Nexora-generated installer or Windows runtime behavior.

### Step 3 checks and current limits

| Check | Result |
|---|---|
| `corepack pnpm install --frozen-lockfile` | **PASS** — earlier in this step, 165 workspace packages installed; the lockfile remained unchanged. |
| Bare `corepack pnpm check` | **BLOCKED ON PATH**, exit 1: nested package scripts could not find `pnpm` (`sh: 1: pnpm: not found`). |
| `PATH="/tmp/nexora-pnpm-bin:$PATH" corepack pnpm check` | **PASS** — using a temporary `/tmp` shim that dispatches nested calls through Corepack: TypeScript typecheck passed; Vitest **88 files / 873 tests passed**; installer-contract **15/15** passed; production Vite build, config-security scan, and installer-contract scan passed. Vite emitted the existing `theme-init.js` module, mixed static/dynamic import, and >500 kB chunk warnings. |
| `node scripts/check-installer-contract.mjs` | **PASS** — Nexora `0.1.0`, `com.nexora.desktop`, NSIS; built frontend entry and 4 files / 720,456 bytes found; no disallowed runtime/model/credential payloads found. This is not an installer hash. |
| Tauri CLI `--version`, `info`, and `build --help` | **PASS — diagnostic/help only** — CLI `2.12.3`; host Debian 12 Linux x86_64; Rust/Cargo, WebKit2GTK 4.1, and rsvg2 are unavailable. Linux CLI lists Linux bundle formats. No `tauri build` was run. |
| Native Rust formatting/Clippy/tests and Tauri Windows build | **BLOCKED / NOT RUN** — no `cargo`, `rustc`, Windows SDK/MSVC, or Windows host. |
| PowerShell syntax parse/execution | **BLOCKED / NOT RUN** — neither `pwsh` nor Windows PowerShell is installed on this Linux host. An official portable-release fetch redirected to an unavailable asset endpoint and failed before any binary was downloaded; no parser/tool was installed. An ad hoc lexical delimiter/string/comment balance check passed, but it is not a PowerShell parser. The edited scripts remain unparsed and unexecuted. |
| Windows 10/11 x64 clean install, interactive shortcut launch, same-build reinstall, prior-version upgrade, uninstall/data preservation, WebView2 bootstrapper, signing | **NOT OBSERVED / NOT VERIFIED** — no Windows runner was invoked; no Windows acceptance evidence exists. The smoke script is an authored test, not its result. |

The successful frontend build generated only the ignored `apps/desktop/ui/dist` frontend output. **No NSIS installer, installer SHA-256 manifest, signature, signed/unsigned artifact result, Windows log, or install/uninstall evidence was produced.** No model/runtime binary or model weight was bundled or added. The Node/Rust/package inputs are version-pinned, but the hosted Windows/SDK image and Evergreen WebView2 runtime remain external inputs; no bit-for-bit reproducibility or provenance attestation is claimed.

The source review traced Tauri startup as creating config/data directories and loading settings/history/state without launching a model automatically. Runtime startup remains an explicit user action, requires a separately supplied executable and a validated model, uses `127.0.0.1` with a random port/session key, and sets `CREATE_NO_WINDOW` on Windows. Shutdown paths cancel work and call the supervisor; `Drop` retries/reaps a child rather than intentionally detaching it. Missing/unusable runtime/model failures have typed UI-facing errors. These Rust and Windows observations have not been compiled or executed here. Signed-out local chat/history/profile-selection and account-usage separation were not changed; the frontend suite passed, but mocked/UI tests are not native IPC or Windows runtime acceptance.

**Disposition remains PARTIAL — Step 3 implementation/verification is incomplete until the Windows build, PowerShell parse, and required Windows 10/11 acceptance evidence are produced.** Step 1 remains **IMPLEMENTED — VALIDATION BLOCKED**. Step 2 remains **PARTIAL — IMPLEMENTATION COMPLETE, VERIFICATION BLOCKED**. No Step 4 work, commit, push, merge, tag, publication, or release occurred.

## Step 3 final-clearance follow-up (2026-10-24)

This latest additive entry supersedes the earlier Step 3 test counts and source-script observations above; it does not change the evidence limits or clear Step 1/2.

### Source inspection and fixes

- Rechecked `package.json`, `apps/desktop/package.json`, `rust-toolchain.toml`, `apps/desktop/src-tauri/Cargo.toml`, the production Tauri config/capability, and the Windows workflow. Pins agree: Node `22.22.3` (build script and CI), pnpm `9.15.4` (`packageManager`, each pinned workflow setup step, and build script), Rust/Cargo `1.93.0` (`rust-toolchain.toml`, workflow action, and build-script checks).
- Fixed clean-checkout order in `build-installer.ps1`: after frontend checks and contract tests, run `pnpm build` before Cargo fmt/Clippy/tests because `generate_context!` embeds `apps/desktop/ui/dist`; Tauri's final build hook runs its frontend build again.
- The build script now refuses to reuse or overwrite any existing `.exe` or `SHA256SUMS.txt` in the target NSIS directory, requires a non-empty fresh setup executable, verifies the signing certificate's Code Signing EKU/current validity and resulting signer thumbprint when signing is configured, and publishes the SHA-256 manifest through a create-new temporary file/move. It does not delete earlier artifacts; operators must archive them elsewhere.
- Tightened `smoke-verify.ps1`: requires exactly one EXE and one setup artifact, checks the manifest is not older than the installer, refuses test install paths that overlap the default installation or roaming/local product-data roots, removes only unchanged content-matching sentinels and an unchanged installer copy, and leaves modified/unexpected files for manual recovery. It still labels its reinstall as same-build, not a version upgrade.
- Hardened the source/build-output contract scanner against frontend-directory symlink/junction redirection and added regression cases for redirected frontend paths. The reviewed production config still has one visible main window, the least-privilege capability, NSIS/current-user mode, silent WebView2 bootstrapper, empty bundle resources/sidecars, and no model/runtime payload.
- Updated the existing `windows-2022` CI job to initialize the hosted x64 Visual Studio/Windows SDK environment using the installed `vswhere` and `VsDevCmd.bat`, then verify `cl.exe`, `link.exe`, and `rc.exe` before Rust/native packaging checks. This workflow edit was inspected only; it was not dispatched or run.

### Automated checks and Linux build evidence

| Check | Result |
|---|---|
| `corepack pnpm install --frozen-lockfile` | **PASS**, 165 workspace packages; lockfile unchanged. |
| `PATH="/tmp/nexora-pnpm-bin:$PATH" corepack pnpm check` | **PASS**: TypeScript; Vitest **88 files / 873 tests**; installer contract **17/17**; Vite production build; config-security scan; and production installer-contract scan. |
| `node scripts/check-installer-contract.mjs` after the frontend build | **PASS**: Nexora `0.1.0`, `com.nexora.desktop`, NSIS; **4 files / 720,456 bytes**; no model/runtime/credential payload. This checks frontend/config assets only, not an installer. |
| `node scripts/check-config-security.mjs` | **PASS**, no secret values printed. |
| `git diff --check` | **PASS** at final source review. |
| Vite output | **PASS with non-fatal warnings**: `theme-init.js` lacks `type="module"`; static/dynamic import mixing; 590.21 kB JS chunk exceeds the 500 kB advisory. |

The first check after adding the path-drift regression exposed an incorrect test expectation (the validator rejects the non-reviewed `frontendDist` setting earlier than filesystem containment). The expectation was corrected; the final contract suite passed **17/17**, and the subsequent aggregate check passed. The production build output is ignored `ui/dist`, not a Windows package.

### Windows runner, PowerShell, and acceptance status

- The host remains Linux x86_64; no Windows 10/11 x64 machine is available in this session. PowerShell/Windows PowerShell, Rust/Cargo, MSVC/Windows SDK, NSIS, and SignTool are absent. Neither PowerShell script was parsed or executed after these edits.
- The repository contains a hosted `windows-2022` CI lane, but no run exists for `arena/9ea88704-nexora` (`gh run list --branch arena/9ea88704-nexora` returned no runs). The workflow was configured but not triggered; this worktree is unpushed, and dispatching now would test remote content without these edits. Pushing is explicitly out of scope, so no runner was allocated for this exact worktree.
- **No Windows build or acceptance evidence exists:** no installer, artifact hash manifest, signing result, clean install, GUI launch, offline/error test, WebView2 installation result, valid prior-version upgrade, uninstall/data-retention result, or post-uninstall reinstall was observed. No valid prior supported installer was supplied; same-build reinstall is not an upgrade. No model/runtime binary or weights were added, and no cloud inference fallback was introduced.

**Current Step 3 status: PARTIAL — IMPLEMENTATION COMPLETE, WINDOWS VERIFICATION BLOCKED.** Step 1 remains **IMPLEMENTED — VALIDATION BLOCKED**; Step 2 remains **PARTIAL — IMPLEMENTATION COMPLETE, VERIFICATION BLOCKED**. Public Beta remains NO-GO. Do not start Step 4 or commit, push, merge, tag, publish, or release.

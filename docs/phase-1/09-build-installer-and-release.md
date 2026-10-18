# 09 — Build, Installer, and Release Strategy

**Status:** Proposed for owner approval  
**Related:** [03-application-architecture.md](03-application-architecture.md), [07-security-and-privacy.md](07-security-and-privacy.md), [10-repository-structure.md](10-repository-structure.md)

---

## 1. Goals

- Produce a small, fast, trustworthy Windows installer with no administrator requirement for normal use.
- Make builds reproducible from a tagged commit with pinned toolchains and locked dependencies.
- Sign every distributed binary and update manifest; make tampering detectable.
- Support clean install, upgrade, downgrade, repair, and uninstall without touching user projects.
- Keep models and large GPU runtime packs out of the base installer, downloaded on demand.
- Ship no release without owner approval.

---

## 2. Toolchain

| Tool | Version policy | Notes |
|------|----------------|-------|
| Rust | Stable, pinned in `rust-toolchain.toml`; MSRV documented | `rustfmt`, `clippy`, `cargo-nextest` |
| Node.js | Current LTS, pinned major | Used for UI build and bundled project runtime |
| pnpm | Pinned major | Workspace and lockfile |
| Tauri CLI | Pinned 2.x | Bundler, updater, icons |
| WiX / NSIS | Versions bundled with Tauri | NSIS primary |
| Visual Studio Build Tools | Pinned MSVC toolset | Windows SDK |
| GitHub Actions runners | `windows-latest` + pinned action SHAs | Release on protected environment |
| Code signing | Authenticode OV/EV or Azure Trusted Signing | HSM-backed key |

All versions are recorded in the repository and in the release metadata. Builds fail if the toolchain differs from the pin.

---

## 3. Build profiles

| Profile | Use | Settings |
|---------|-----|----------|
| `dev` | Local development | Debug assertions, fast incremental, no LTO |
| `release` | Distribution | Optimized, LTO thin/fat, `codegen-units=1`, strip symbols (keep a symbol server artifact) |
| `release-debug` | Field diagnostics | Optimized with symbols; internal only |
| `sidecar` | llama-server pack | Built from pinned upstream with recorded flags |

- UI assets are content-hashed and minified; source maps are retained privately for crash triage but not shipped by default.
- The app embeds a build hash, commit SHA, and protocol version, shown in About and diagnostics.
- Sidecars are built in a separate, reproducible job; their hashes are embedded in the app manifest.

---

## 4. Installer

### 4.1 Recommendation: per-user NSIS

| Option | Pros | Cons | Decision |
|--------|------|------|----------|
| **NSIS per-user** | No admin; simple; small; Tauri support; fast install | Per-user only; enterprise deployment less standard | **Primary** |
| MSI/WiX machine-wide | Enterprise GPO/Intune; per-machine | Requires admin; more complex upgrades | Post-1.0 if demanded |
| MSIX | Clean packaging, identity, Store path | Container/updater constraints; sidecar and model storage complexities; signing requirements | Not at 1.0 |
| Portable ZIP | No install; USB use | No updater, no shortcuts, DLL/runtime complexity | Developer-only artifact |

**Layout:**

```
%LOCALAPPDATA%\Programs\Nexora\
  Nexora.exe
  resources\            templates, icons, manifests, licences
  runtime\cpu\          llama-server + DLLs
  runtime\cuda\         downloaded on demand
  runtime\vulkan\       downloaded on demand
  uninstall.exe
%LOCALAPPDATA%\Nexora\
  nexora.db, logs, checkpoints, models, index, diagnostics
```

- Start-menu shortcut; optional desktop shortcut (unchecked by default).
- No Windows service, no scheduled task, no firewall rule, no driver.
- No file associations at 1.0; `.nexora` is not a project format.
- Installer checks Windows version, architecture, free disk, and WebView2; offers the Evergreen bootstrapper if missing.
- Installer verifies its own signature and refuses to run from a network path if policy requires.
- Uninstall removes app + runtime packs; asks separately about models and app data; never touches user project folders.
- Upgrade preserves settings, conversations, models, and checkpoints; a downgrade is allowed with a warning about schema compatibility.

### 4.2 Installer UX

1. Welcome + licence/privacy summary.
2. Install location (default per-user) + disk requirement.
3. Optional desktop shortcut.
4. Progress (extract, verify, register).
5. Finish with "Launch Nexora" checked.
6. On first launch, onboarding handles hardware/model setup.

Silent install flags (`/S`) are supported for test automation but not advertised as an enterprise feature until an MSI exists.

---

## 5. Code signing

- **Authenticode** SHA-256 with RFC 3161 timestamping for `Nexora.exe`, installer, uninstaller, sidecars, and shipped DLLs.
- Certificate options:
  - OV/EV code-signing certificate from a public CA (HSM/token; CI integration).
  - Azure Trusted Signing (identity validation required; good CI integration).
  - Self-signed for development only; never distributed.
- Signing keys never live in the repository or plain CI secrets; use an HSM, Key Vault, or a protected signing service with short-lived credentials.
- Verify every artifact in CI with `signtool verify /pa /all` and fail on an unsigned or invalid file.
- Publish SHA-256 checksums alongside releases.
- SmartScreen reputation is earned over time; EV certificates help but do not guarantee immediate trust. Release notes and a clear publisher name matter.
- Antivirus false positives are expected for a new AI app that spawns processes and downloads models; submit samples to major vendors before launch and document exceptions.

---

## 6. WebView2 dependency

- Windows 11 includes Evergreen WebView2; Windows 10 22H2 may not.
- The installer detects the runtime and, if missing, offers the Microsoft Evergreen bootstrapper (with the user's consent) or links to the official installer.
- The app requires a minimum WebView2 version; it shows a clear message and update link if older.
- No fixed-version WebView2 is bundled at 1.0; the Evergreen runtime receives security updates outside our release cycle.

---

## 7. Update system

### 7.1 Design

- Tauri updater (or an equivalent signed-manifest updater) checks a static HTTPS endpoint.
- Manifest contains: version, channel, minimum OS, notes, publish date, artifact URL, SHA-256, and an Ed25519/minisign signature.
- The app embeds the public key; signature verification happens before download and again after download (hash).
- Authenticode verifies the installer itself; the manifest signature protects against a compromised CDN.
- Updates are downloaded to a temp folder, verified, and launched in passive mode; the app restarts after installation.
- A `pending-update` marker is written before restart; the new version must launch successfully and clear it; a watchdog offers rollback if it does not.

### 7.2 Behaviour

| Setting | Default |
|---------|---------|
| Check automatically | Yes (unless offline mode) |
| Download automatically | No; ask first for major/minor, allow auto for patch if user opts in |
| Install automatically | No; user approves restart |
| Major version | Always explicit; never silent |
| Offline mode | No checks; clear status |
| Metered connection | Warn before download |

### 7.3 Rollback and kill switch

- Keep the previous installer and a rollback entry in Settings → About.
- If a release is bad, update the manifest to point back to the previous version and publish an advisory.
- Stage rollouts by updating the manifest for 5% → 25% → 100% of clients; the app can use a stable machine bucket for deterministic staging.
- Model catalogue and runtime packs have independent kill switches (mark blocked/deprecated) without shipping an app update.
- No remote uninstall, no remote code execution, no hidden commands.

---

## 8. Release channels and versioning

| Channel | Audience | Signing | Update cadence |
|---------|----------|---------|----------------|
| `dev` | Internal | Optional/dev cert | Continuous |
| `beta` | Opt-in testers | Signed | Biweekly or milestone |
| `stable` | All users | Signed | Monthly or security-driven |

- SemVer `MAJOR.MINOR.PATCH`; build metadata includes commit SHA.
- Protocol version is separate and must remain backward-compatible within a major app version.
- Database migrations are forward-only; downgrades that would lose data require explicit warning and a backup.
- Changelog is user-facing: added, changed, fixed, security, known issues, model/catalogue changes.

---

## 9. CI/CD workflows

### 9.1 Pull request

```
fmt → clippy → cargo test → typecheck → eslint → vitest
→ playwright smoke → secret scan → dependency review
→ licence check → CodeQL → unsigned build
```

- No signing keys available to PR workflows.
- Fork PRs never receive secrets.
- Required checks block merge; `main` is protected.

### 9.2 Nightly

- Full E2E suite on Windows runner.
- Agent eval subset on reference hardware.
- Performance smoke and memory check.
- Installer build (unsigned) and clean-VM smoke.
- Accessibility scan.
- Fuzzers (time-boxed).

### 9.3 Release

```
tag vX.Y.Z → protected environment approval
→ clean build from tag → tests/eval gates
→ sign binaries + installer → sign update manifest
→ generate SBOM + checksums + provenance
→ draft GitHub release (private)
→ clean-VM install/upgrade/uninstall tests
→ owner approval
→ publish release + update manifest
```

- The release workflow runs only on tags and requires manual approval in a protected GitHub Environment.
- Artifacts are retained for rollback and audit.
- No release is published automatically, and no repository is made public without explicit approval (Phase 1 constraint).

---

## 10. Reproducibility, SBOM, and provenance

- Locked Cargo and pnpm dependencies; vendored or cached crates where practical.
- Pinned toolchains, build flags, and sidecar commits.
- `cargo-cyclonedx` SBOM + pnpm licence report; shipped as `sbom.json` and `THIRD_PARTY_NOTICES.md`.
- GitHub artifact attestations or SLSA provenance for release artifacts.
- SHA-256 checksums published next to the installer.
- Build metadata includes commit, toolchain, and CI run ID.
- Reproducibility is verified by rebuilding a release candidate and comparing hashes where deterministic; non-determinism is documented (timestamps, signing).

---

## 11. Release checklist

- [ ] Version bumped; changelog written.
- [ ] All Must requirements for the milestone pass.
- [ ] NCE thresholds met for the selected model(s).
- [ ] Zero open P0/P1 bugs; security findings reviewed.
- [ ] External pen test (pre-1.0) closed or accepted.
- [ ] Accessibility audit passed.
- [ ] Performance budgets met on reference hardware.
- [ ] Model catalogue and licences reviewed.
- [ ] Installer tested on clean Windows 11 VMs (and Windows 10 smoke if supported).
- [ ] Upgrade from previous stable, downgrade path, uninstall/data-retention tested.
- [ ] All binaries signed and timestamped; signatures verified.
- [ ] Update manifest signed; staged rollout plan set.
- [ ] SBOM, checksums, notices included.
- [ ] Docs updated (install, models, permissions, troubleshooting, privacy).
- [ ] Owner approval recorded.

No checklist item can be waived for a stable release; beta exceptions require written risk acceptance.

---

## 12. Risks and mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| SmartScreen warning on new publisher | Low adoption | Sign, build reputation, publish hashes, clear publisher info |
| Antivirus false positive | Install/launch blocked | Submit samples pre-launch; document; avoid packers; sign sidecars |
| WebView2 missing/outdated | App cannot render | Bootstrapper + minimum version check |
| CUDA pack size/licence | Download friction/legal | On-demand packs; legal review; Vulkan fallback |
| Signing key compromise | Supply-chain trust loss | HSM, least privilege, rotation plan, advisory |
| Updater manifest compromise | Malicious update | Offline signature key + Authenticode + staged rollout |
| Large installer | Slow download | CPU pack only; on-demand GPU; delta updates later |
| Model download failures | Onboarding drop-off | Resume, mirror fallback, clear errors |
| Long paths | Build failures | Long-path support manifest, path checks, guidance |
| Windows Defender scanning models | Slow load | Exclude model folder only with user consent and clear explanation |
| Driver differences | GPU crashes | Fallback matrix, safe mode, telemetry-free diagnostics |

---

## 13. Decisions

| ID | Decision | Recommendation | Status |
|----|----------|----------------|--------|
| D-06 | Installer | Per-user NSIS, signed | Proposed |
| D-07 | Updater | Signed manifest, user-approved, staged | Proposed |
| D-45 | WebView2 | Evergreen + bootstrapper | Proposed |
| D-46 | GPU packs | On-demand, hashed, signed | Proposed |
| D-47 | Signing | OV/EV or Azure Trusted Signing; HSM-backed | Proposed (budget Q-06) |
| D-48 | Channels | dev/beta/stable | Proposed |
| D-49 | MSI/enterprise | Post-1.0 if demanded | Deferred |
| D-50 | Portable build | Developer-only | Proposed |

Open: Q-06 signing budget/certificate type; Q-30 whether enterprise MSI is needed at launch; Q-31 whether to publish checksums on the website as well as GitHub; Q-32 update host choice.

---

## 14. Sources

- Tauri distribution — https://v2.tauri.app/distribute/
- Tauri updater — https://v2.tauri.app/plugin/updater/
- Tauri Windows installer — https://v2.tauri.app/distribute/windows-installer/
- NSIS — https://nsis.sourceforge.io/
- WiX Toolset — https://wixtoolset.org/
- Microsoft WebView2 distribution — https://learn.microsoft.com/microsoft-edge/webview2/concepts/distribution
- Authenticode — https://learn.microsoft.com/windows/win32/seccrypto/cryptography-tools
- Azure Trusted Signing — https://learn.microsoft.com/azure/trusted-signing/
- SLSA — https://slsa.dev/
- cargo-cyclonedx — https://github.com/cyclonedx/cargo-cyclonedx
- GitHub artifact attestations — https://docs.github.com/actions/using-workflows/using-artifact-attestations

Access date: 2026-10-17.

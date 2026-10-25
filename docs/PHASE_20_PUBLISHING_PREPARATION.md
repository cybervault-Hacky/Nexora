# Phase 20 — Publishing Preparation Audit

- **Result:** AUDIT COMPLETE — BLOCKED / NO RELEASE
- **Review date:** 2026-10-24
- **Audit checkout:** `arena/9ea88704-nexora` at `8b2e8415bc67ec31e343218364fa4a6ce3741dab`

**Scope:** Static audit of product/version identity, workflows, release claims, signing, artifact integrity/provenance, licensing, and support/security reporting. This is preparation only; it is not release approval.

No release, installer, tag, commit, upload, deployment, or workflow run was created or triggered. No build, Windows test, payment, OAuth, account, or production operation was performed. The worktree remains on `arena/9ea88704-nexora`; Phase 18 and 19 statuses are preserved.

## 1. Product identity and version

| Source | Verified repository value | Interpretation |
|---|---|---|
| Root and desktop `package.json` | `0.1.0`; both packages are `private` | No npm publication configured. |
| `apps/desktop/src-tauri/tauri.conf.json` | Product `Nexora`, version `0.1.0`, identifier `com.nexora.desktop`, NSIS target, `currentUser` install mode | App identity is internally consistent; no version approval or release date is recorded. |
| Desktop Rust crate / auth-service crate | `nexora 0.1.0` / `nexora-auth 0.1.0` | Separate crates; their matching current version strings are not release approval. |
| Build pins | Node.js 22, pnpm 9.15.4, Rust 1.93.0, `x86_64-pc-windows-msvc`; Tauri Rust crate 2.12.5 / `tauri-build` 2.7.4, CLI 2.12.3, JS API 2.12.4 | CI/build-script and lockfile/manifest pins. `Cargo.toml`'s Rust MSRV `1.77.2` is a separate minimum, not the pinned build toolchain. |

No local or remote Git tags, GitHub releases, changelog, or release-notes file were found. The bundle's `publisher: "Nexora"` and copyright string are package metadata only; neither establishes a legal publisher, company, verified certificate identity, or ownership. No app version was changed.

## 2. Workflow and publication surface

`.github/workflows/ci.yml` is the only workflow in the checkout. It runs on pushes to `main` and `arena/**`, pull requests to `main`, and manual `workflow_dispatch`. Its Windows job builds the x64 NSIS installer and SHA-256 manifest, then runs the smoke script. The workflow defines no artifact-upload, GitHub Release, tag, deployment, or publication step; its permissions are `contents: read`, actions are SHA-pinned, and checkout disables persisted credentials. The workflow was inspected but not executed or dispatched.

## 3. Artifact, signing, integrity, and provenance

- Tauri is configured for a per-user NSIS package and silent WebView2 bootstrapper. The bundle config declares neither `resources` nor `externalBin`; the installer is not configured to include a local inference runtime or model weights.
- The Windows build script collects `*-setup.exe`, optionally signs it using an approved certificate in the current user's `My` store selected by `SIGNTOOL_CERT_THUMBPRINT`, verifies the Authenticode result, then writes `SHA256SUMS.txt` from the final file bytes. It rejects legacy PFX path/password inputs. The workflow itself does not configure a signing thumbprint.
- The relevant Tauri `target` and frontend `dist` output directories are absent. No installer, model/runtime binary, checksum manifest, SBOM, or build attestation/provenance record exists in this checkout. There is therefore no installer checksum to report and no artifact to classify as signed or unsigned. No certificate or signing operation was used.
- The build pins and lockfiles aid repeatability but are not a provenance attestation. A future authorized release gate must bind the checksum to the exact final artifact (after signing, if any) and record the source commit, product version, target, toolchain, build-run identity, and signing status. It must not claim reproducibility, signature validity, or provenance until that evidence exists.
- The model registry has approved Mini/Sonnet metadata and expected digests; both profiles remain `locally_verified: false`, and neither model file is present. Those model digests are not installer checksums. Keep model acquisition separate from installer/runtime distribution. Mini's license reference currently uses the GGUF repository's mutable `main` path; verify exact-artifact licensing before redistribution without changing approved model metadata in this phase.

## 4. Release-facing claims, legal status, and reporting contacts

- Root `README.md` now labels itself pre-release, describes the Mini/Sonnet path as unverified, preserves signed-out local chat, removes proposed prices/quotas from active-feature wording, and explicitly says Stripe-oriented code is not an active offer, G-01 remains open, and Razorpay is not implemented. The prior forced-authentication description and stale test counts were removed. Root license text no longer asserts proprietary ownership or complete notices.
- `docs/LEGAL_PRIVACY_TERMS.md` remains a pre-launch template requiring legal review and does not define an active commercial offer. No top-level `LICENSE`, complete dependency-license inventory, or distributable third-party `NOTICE` file is present. Existing license summaries and model metadata are not a substitute for legal review of exact bundled/distributed materials.
- No `SECURITY.md`, support/security email, vulnerability-reporting route, or approved support/security URL was found. Do not invent contact details; an owner-approved reporting channel and response process are prerequisites to public distribution.
- The October 22 release-candidate report and the legacy Phase 32 Windows report refer to different historical branches/scopes and contained claims not supported by current Phase 18/19 evidence. Both now carry prominent superseding notices; the Phase 32 “roadmap complete” conclusion was withdrawn as current evidence. Current release status is recorded in the Phase 18/19 reports and this document.

## 5. Documentation corrections and remaining gates

Corrected the confirmed stale Rust version and PFX-based signing instructions in `docs/PRODUCTION_DEPLOYMENT_AND_SECURITY.md`; updated G-10 in `docs/ARCHITECTURE_DECISION_RECORD.md`. Qualified the privacy/export claims in the pre-launch legal template, aligned the root and desktop READMEs with the signed-out/local-first design and unverified integrations, and marked the old release/Phase 32 reports as superseded. These are documentation corrections only: no payment, authentication, inference, packaging, or backend behavior changed.

**Phase 18 remains PARTIALLY VERIFIED — NOT RELEASE-READY.** G-01 (client-selected plan changes can mutate active/trialing subscriptions without provider verification) remains release-critical. Native Rust/PostgreSQL, OAuth/account, payment, Razorpay, commercial terms, and genuine model acquisition/loading/inference remain unverified or blocked.

**Phase 19 remains PARTIALLY VERIFIED / BLOCKED.** No real Windows installer, signature, installer hash manifest, install/upgrade/uninstall, GUI, WebView2, ACL, or Windows inference behavior was verified. Its G-09 packaging correction remains Windows-unverified.

Phase 20 stops here. No Phase 21 implementation or release activity is authorized; await explicit authorization before proceeding.

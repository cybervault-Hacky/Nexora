# Support and diagnostics — pre-release reference

**Status: release-blocking placeholders.** Nexora is not a public beta or supported release. No verified public support contact, support URL, response owner, or support hours are recorded. Do not invent or publish contact details. The current public-beta decision remains **NO-GO**.

## Required public support details

- **Support contact:** `[RELEASE BLOCKER — owner must provide a verified, monitored public support channel.]`
- **Security reporting:** see [`SECURITY.md`](../SECURITY.md); its private-reporting route is also a release-blocking placeholder.
- **Supported operating systems and minimum hardware:** `[RELEASE BLOCKER — Windows build/runtime acceptance and minimum requirements are not verified.]`
- **Supported release version / update policy:** `[RELEASE BLOCKER — no approved release version or update channel is established.]`

The repository targets Windows x64 for packaging, but no Windows installer or native Windows acceptance run has been produced. Windows 10/11 compatibility, ARM64 support, model/runtime compatibility, performance, and minimum hardware are not established by this source-level target.

## What is and is not verified

- The frontend and installer-contract suites can run on the current Linux environment; they are not a substitute for Tauri/Rust, Windows installer, account/database, or genuine model-inference evidence.
- The desktop source uses a separately supplied local inference runtime and user-selected or explicitly acquired model artifacts. Neither runtime nor weights are bundled. Both registry model profiles remain `locally_verified: false`; the registry's expected digests are metadata, not checksums computed for local artifacts.
- The source routes inference to a local loopback runtime and contains no cloud-inference fallback. This is a source review, not a Windows runtime or privacy guarantee.
- Account, OAuth, email, and PostgreSQL behavior are not runtime-verified. Paid features remain disabled; no plan, quota, trial, or commercial entitlement is approved.
- See [`docs/THIRD_PARTY_NOTICES_DRAFT.md`](THIRD_PARTY_NOTICES_DRAFT.md) for the limited frontend runtime-license work. It is not a complete distributable notice.

## Safe diagnostic information

For an authorized internal test, record only the minimum needed to reproduce a failure:

- Nexora version shown by the candidate build (not a support commitment), OS edition/build and CPU architecture, and whether the run was a native Windows build or a Linux frontend test.
- The affected app screen, the visible error code/message, approximate time, and the action immediately before the error.
- Model profile ID, whether the model/runtime was user-supplied or acquired through the app, the runtime's reported version line, and whether the artifact was locally verified. Do not substitute files or bypass integrity checks.
- Whether the user was signed in, without including account identifiers or credential values.

The source's developer-diagnostics command returns platform, architecture, app version, credential-vault label, the configured auth-service base URL, and configuration/status booleans. The auth-service URL can reveal a private deployment; inspect and redact it before sharing. The CSP/offline booleans in that response are source fields, not independent runtime verification. No user should be asked to export or share a prompt, source file, workspace scan, conversation, model weights, password, bearer token, OAuth code, database URL, API key, or private signing material.

The source currently caps retained engine stderr in memory and exposes a short diagnostic tail for runtime errors; this behavior has not been verified on Windows. Do not treat a UI diagnostic, mocked test, or source comment as proof that a full log or data export is safe to share. No remote telemetry or support upload is established by this preparation.

## Internal reproduction precautions

- Use only an authorized disposable test account and isolated test database; PostgreSQL integration tests require a database name ending in `_test`. Never point destructive database tests at production or personal data.
- Use a clean test profile and synthetic workspace files. Do not share real prompts, source code, conversation history, or account data in a bug report.
- Do not disable CSP, change the loopback binding, bypass HTTPS or artifact-integrity checks, replace registry hashes, enable billing, or run an untrusted runtime to make a test pass.
- Do not download or redistribute model weights until the exact artifact, license text, and redistribution permissions have been independently reviewed.
- Windows signing, installer checksums, Authenticode, and account/database runtime evidence must come from genuine authorized tools and artifacts; this environment has none of those results.

This document records prerequisites and safe boundaries only. It does not offer support, declare compatibility, promise response times, or certify privacy or release readiness.

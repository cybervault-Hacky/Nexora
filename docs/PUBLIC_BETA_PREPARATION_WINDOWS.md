# Windows 10/11 x64 Public Beta Preparation — Evidence Report

- **Review date:** 2026-10-24
- **Checkout:** `arena/9ea88704-nexora` at `8b2e8415bc67ec31e343218364fa4a6ce3741dab`
- **Decision:** **NO-GO — PUBLIC BETA NOT READY**
- **Authorization:** Preparation and bounded testing only. No publishing, distribution, upload, signing, deployment, or release is authorized by this report. The final distribution decision remains with Sarthak.

This report records the current source-level G-01 mitigation and the evidence still missing. It does not certify the product, approve commercial terms, or authorize release. The Phase 18–21 reports were reviewed; Phase 21's original NO-GO assessment remains correct for the evidence available at that time. This report supersedes only its G-01 source-status description, not its release decision or the other blockers.

## Decision summary

**Keep the Public Beta candidate unpublished.** The client-selected subscription plan/cancellation database mutations have been removed from the current source path, and checkout activation is more tightly gated. Focused Rust and PostgreSQL regression tests were added but could not be executed because the pinned Rust toolchain and PostgreSQL tooling are absent and no isolated disposable database was configured. The correction is therefore **source-mitigated, not runtime-verified**.

Critical Windows packaging/runtime, genuine local inference, auth/database, artifact integrity/licensing/privacy, and legal/support evidence is still missing. Production billing remains deliberately disabled; no prices, quotas, trials, Premium benefits, or other commercial policy were approved or enabled.

## Baseline and preservation

| Checkpoint | Branch / HEAD | Modified tracked paths | Untracked paths | Staged paths |
|---|---|---:|---:|---:|
| Start of this beta-preparation task | `arena/9ea88704-nexora` / `8b2e8415bc67ec31e343218364fa4a6ce3741dab` | 95 | 13 | 0 |
| End of this preparation | Same branch and HEAD | 95 | 14 | 0 |

The start snapshot files remain under `/tmp/nexora-public-beta-preexisting-*`. Baseline hashes: status snapshot `20e8df30b599d2c0563922675b78717559e6fee3d7177676eed608c3b141fc71`; tracked worktree patch `0b1e063fdd346d65c91ea44057288334a50aed071c75a2b988051c598ee345d4` (818,834 bytes); staged patch is empty (`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`). The end check retained all 108 start-of-task status paths; the only additional path is this report. No pre-existing file was reset, cleaned, stashed, reverted, or overwritten.

No commit, push, merge, tag, release, workflow dispatch, signing, upload, deployment, or production operation occurred. No database or payment provider was contacted.

## G-01 review and source-level correction

### Traced request and mutation path

| Layer | Path | Finding |
|---|---|---|
| Desktop UI / IPC | `apps/desktop/ui/src/lib/auth.ts` → `billing_change_plan` / `billing_cancel_subscription` Tauri commands → `apps/desktop/src-tauri/src/auth/client.rs` | Commands forward the client request to the auth API. The client request contains a plan and interval, or an `immediate` cancellation flag; it contains no provider proof. The current Plans page has no active checkout/plan-change offer. |
| Authenticated API | `POST /v1/billing/change-plan` and `POST /v1/billing/cancel` in `services/nexora-auth/src/app.rs` | Both routes require a bearer session and derive the account from the authenticated user. Authentication alone does not authorize a paid entitlement change. |
| Vulnerable baseline | `change_subscription_plan` and `cancel_subscription` in `services/nexora-auth/src/billing.rs` | At task start, a non-free client-selected plan/version directly updated an active/trialing subscription; the `free` path and cancellation endpoint also mutated subscription state locally without a provider lifecycle operation. This was the confirmed G-01 defect. |
| Other production mutation paths | `activate_order_in_tx`, `verify_checkout_order`, `process_signed_webhook`, and `expire_elapsed_subscriptions` | Subscription activation/reconciliation occurs through checkout verification or signed provider webhooks, with a separate time-based expiry path. Webhooks verify the configured HMAC secret before processing. Production composition still has no webhook secret or configured payment provider. |

### Changes made

- Added `BillingError::ProviderOperationUnsupported` (`503`, code `payment_provider_operation_unsupported`). Unconfigured provider operations return the existing unconfigured-provider error; configured providers without subscription lifecycle methods return the new unsupported-operation error.
- `change_subscription_plan` no longer reads plan versions or updates subscription rows. A `free` request is routed through the same fail-closed cancellation path. `cancel_subscription` no longer mutates subscription state or writes a cancellation audit event. No client-selected plan or status can change paid entitlement through these functions.
- Tightened the existing checkout verification gate: a checkout is eligible only when its provider result describes `mode=subscription`, `status=complete`, `payment_status=paid`, and a non-empty subscription ID. The signed `checkout.session.completed` webhook path applies the same state check.
- `activate_order_in_tx` now acts only on a `pending` payment order and requires a non-empty provider subscription ID. Completed, failed, expired, or refunded orders are not reactivated by a later/replayed completion. Billing history is retained; no migration was changed.
- Kept production provider/quota composition unchanged: `payment_runtime_config` returns `UnconfiguredPaymentProvider` and no webhook secret; production uses `QuotaPolicy::unconfigured()`. Credentials alone still do not enable Stripe. No provider lifecycle operation, checkout, or live payment was enabled.
- Added focused unit and PostgreSQL/HTTP integration regression tests for configured-but-unsupported and unconfigured plan/cancel operations, paid versus unpaid checkout state, trusted provider verification, signed webhook activation, rejected client plan/cancel attempts with unchanged entitlement, and a completion replay after refund. The corresponding test assertions are **not executed** (see status matrix).
- Updated current README/deployment/legal/architecture wording and command comments; Phase 21 now links this subsequent status update. Historical Phase 18–20 findings remain historical evidence.

**Residual billing limitation:** the current provider abstraction still has no verified plan-change or cancellation method, so those operations intentionally remain unavailable even if a provider object reports configured. The existing activation code also has an interval-based fallback when a provider period end is absent; that behavior needs provider and commercial-policy review before any future activation is enabled. Paid checkout and activation remain disabled in production meanwhile.

## Evidence and status matrix

`PASS` means executed and verified for the stated scope. `PASS — STATIC ONLY` is not runtime evidence. `BLOCKED` means the required safe environment/tooling is absent. `NOT VERIFIED` means there is no adequate evidence. `NOT APPLICABLE / NOT AUTHORIZED` means the action was outside the authorized scope.

| Gate | Status | Evidence and remaining limit |
|---|---|---|
| Workspace identity and preservation | **PASS** | Branch and HEAD stayed fixed; all 108 baseline status paths remain; 95 tracked paths are modified, 14 are untracked (13 pre-existing plus this report), and none are staged. |
| Phase 18–21 report review | **PASS** | All four reports were read. Phase 21 recorded NO-GO; this report adds a later G-01 source update but does not change that decision. |
| G-01 direct client mutation | **PASS — STATIC ONLY** | Source assertions confirm the plan/cancel functions no longer contain subscription SQL mutations and return a fail-closed provider error. No Rust compile or database-backed assertion ran. |
| Trusted checkout activation gates | **PASS — STATIC ONLY** | Provider verification and signed webhook paths require paid, completed subscription state; activation requires a pending order and a provider subscription ID. Future provider lifecycle/period semantics still require review. |
| G-01 unit and DB/HTTP regression tests | **BLOCKED / NOT VERIFIED** | Tests were authored but not run: `cargo`, `rustc`, and `rustfmt` are absent; PostgreSQL tools and the isolated test URL are absent. No database was contacted. |
| Production billing and commercial policy | **BLOCKED** | Provider/webhook composition and quota policy remain unconfigured. Pricing, quotas, trials, Premium terms/benefits, and policy periods remain unapproved. No live payment was attempted. |
| Frontend typecheck | **PASS** | `corepack pnpm --filter @nexora/desktop typecheck` completed successfully. |
| Frontend tests | **PASS** | `corepack pnpm --filter @nexora/desktop test`: **88 files / 870 tests passed**. These are frontend tests and do not prove native IPC, Rust, provider, database, or Windows runtime behavior. |
| Frontend production build / config scanner | **PASS WITH NON-FATAL WARNINGS** | `corepack pnpm --filter @nexora/desktop build` completed; the bundled config-security scanner passed without printing secret values. Vite warned about `theme-init.js`, mixed static/dynamic imports, and the 589.58 kB JS chunk exceeding the 500 kB advisory threshold. This produced ignored frontend `dist`, not a Windows installer. |
| Rust formatting, compilation, Clippy, and tests | **BLOCKED / NOT VERIFIED** | The repository pins Rust 1.93.0, but `cargo`, `rustc`, and `rustfmt` are unavailable. The backend and desktop Rust changes/tests have not been compiled. |
| PostgreSQL migrations and integration tests | **BLOCKED / NOT VERIFIED** | `psql`, `pg_isready`, Docker, and Podman are unavailable; `NEXORA_AUTH_TEST_DATABASE_URL` is unset. There is no explicitly confirmed disposable DB. No connection or migration was attempted. |
| Windows 10/11 x64 packaging and runtime | **BLOCKED / NOT VERIFIED** | Host is Linux x86_64 (`Linux 6.1.158+`); PowerShell, MSVC, NSIS, and Windows runtime are absent. No Windows build, installer, install/upgrade/uninstall, WebView2, Authenticode, or GUI test ran. CI was inspected, not triggered. No ARM64 claim is made. |
| Mini/Sonnet artifacts and inference | **NOT VERIFIED / BLOCKED** | Source metadata pins two model files, but no `.gguf` or `llama-server` is present; both profiles remain `locally_verified: false`. Current host has 3.8 GiB RAM and no detected GPU, less than either file's weight size. The model host is outside this session's egress allowlist; no network request or multi-GB download was attempted. No synthetic artifact or substitution was used. |
| Authentication, account, OAuth, and email | **NOT VERIFIED / BLOCKED** | UI tests are mocked. No approved isolated service configuration or disposable DB was available. No live account, email, Google OAuth, production, or credential operation was performed. |
| Installer integrity, signing, provenance, licensing, privacy, and support essentials | **BLOCKED** | No installer, installer checksum, signature, SBOM, provenance/attestation, top-level `LICENSE`, complete `NOTICE` set, `SECURITY.md`, approved support/security channel, or legal approval was found. The legal/privacy document remains a pre-launch template. No contact, guarantee, capability, or legal approval was invented. |
| Publication, distribution, signing, deployment | **NOT APPLICABLE / NOT AUTHORIZED** | No artifact was published, uploaded, signed, deployed, or distributed. Do not proceed without separate explicit authorization. |

## Environment and model evidence

| Check | Observed |
|---|---|
| Host / CPU | Linux x86_64; 2 virtual CPUs; Intel Xeon Processor @ 2.60 GHz. Not a Windows x64 host. |
| Memory / disk | 3.8 GiB total memory (3.6 GiB reported available); 20 GiB disk free. |
| GPU / local runtime | `nvidia-smi`, `/dev/dxg`, and visible DRM devices unavailable; `llama-server`, `llama-cli`, and `gguf-split` absent. |
| Toolchain | Node.js `v22.22.3`; Corepack pnpm `9.15.4`; Rust pin `1.93.0` but Cargo/Rust tools absent. The standalone `pnpm` executable is not on PATH, so frontend commands were run through Corepack. |
| Model network | No request made. Session egress permits GitHub, npm, and PyPI hosts, not the Hugging Face model host referenced by registry metadata. No download was attempted. |

Registry metadata only (not a local digest computation or artifact verification):

| Profile | Pinned artifact metadata | Local status |
|---|---|---|
| Nexora Mini | `Qwen/Qwen2.5-Coder-7B-Instruct-GGUF`, revision `13fb94bfda8c8cf22497dc57b78f391a9acb426a`, `qwen2.5-coder-7b-instruct-q4_k_m.gguf`, 4,683,073,536 bytes (4.36 GiB), expected SHA-256 `509287f78cb4d4cf6b3843734733b914b2c158e43e22a7f4bf5e963800894d3c`. | No file; `locally_verified: false`; no inference. |
| Nexora Sonnet | `Qwen/Qwen2.5-Coder-14B-Instruct-GGUF`, revision `d0a692ef765eefbf2fabb130b3cb2e8917e3d225`, `qwen2.5-coder-14b-instruct-q4_k_m.gguf`, 8,988,110,272 bytes (8.37 GiB), expected SHA-256 `c1e659736d89ac1065fb495330fb824d94001974a4bfa78e7270e43476a8d940`. | No file; `locally_verified: false`; no inference. |

These expected digests are registry metadata, **not verified artifact checksums**. Exact-artifact licensing, the Mini license reference, redistribution rights, Windows compatibility, and real inference remain unresolved. The sandbox's memory is below the model weights themselves; these measurements are not a target-user hardware requirement or a Windows performance claim.

## Checks run

| Command/check | Result |
|---|---|
| `corepack pnpm --filter @nexora/desktop typecheck` | **PASS** — exit 0. |
| `corepack pnpm --filter @nexora/desktop test` | **PASS** — 88 test files / 870 tests. |
| `corepack pnpm --filter @nexora/desktop build` | **PASS WITH WARNINGS** — TypeScript, Vite production build, and bundled configuration-security scan completed. |
| G-01 source assertions | **PASS — STATIC ONLY** — no direct plan/cancel subscription mutation; provider state, pending order state, and production disabled defaults checked. |
| `git diff --check` | **PASS** — no tracked whitespace errors. |
| `cargo fmt`, `cargo check`, `cargo clippy`, `cargo test` | **BLOCKED — NOT RUN** — pinned Rust tooling absent. |
| Auth PostgreSQL integration suite | **BLOCKED — NOT RUN** — no PostgreSQL tooling or isolated test URL; no DB contacted. |
| Windows build/smoke/signing | **BLOCKED — NOT RUN** — no genuine Windows x64 environment; no installer generated. |
| Model download/digest/inference, live OAuth/email/payment | **NOT PERFORMED** — unavailable or unapproved; no live operations. |

## Prioritized evidence required before reconsidering Public Beta

1. **G-01 validation:** Run Rust formatting, compile, Clippy, unit tests, and the focused auth/PostgreSQL HTTP integration suite with the pinned Rust toolchain and an explicitly reviewed, isolated disposable PostgreSQL database. Verify client plan and cancellation requests cannot mutate state, unpaid checkout cannot activate, valid provider verification/webhook can activate a pending order, and terminal/refunded orders cannot reactivate. Keep provider configuration disabled until commercial policy and provider lifecycle methods are approved.
2. **Windows core behavior:** On genuine Windows 10/11 x64, build and test the native app and installer; verify launch, app data/credential storage, authentication behavior, WebView2, install/upgrade/uninstall, and local chat/history. Do not treat Linux frontend tests or Windows CI configuration as runtime evidence.
3. **Real Mini/Sonnet inference:** Obtain an approved route to the exact pinned artifacts, verify file size and SHA-256 through the existing safe acquisition path, review exact-artifact licensing, and demonstrate real inference on supported Windows hardware. Check capacity before any download. Keep `locally_verified: false` until recorded; do not claim fully offline for accounts, acquisition, or remote services.
4. **Auth, database, and privacy:** Test account/session/recovery/OAuth flows only with approved isolated configuration and disposable services. Confirm database integrity and data handling; no production accounts or services.
5. **Distribution essentials:** Obtain owner/legal approval, a release version, an approved publisher/signing identity, complete third-party notices and model redistribution decisions, an approved support/security contact and response process, and artifact-linked checksum/provenance evidence. None may be inferred from this preparation report.

## Final disposition

**NO-GO — keep Nexora unpublished and undistributed.** The G-01 source path now fails closed, but its Rust/database tests are blocked; Windows packaging/runtime, genuine local inference, auth/database security, and distribution essentials remain unverified or unavailable. No installer or beta artifact was produced. Stop here and await Sarthak's separate explicit authorization before any further preparation that expands scope, signing, publication, or distribution.

## Step 1 G-01 follow-up (2026-10-24)

The source audit found additional gaps beyond the earlier client plan/cancellation fix. Checkout activation now binds the provider response/webhook to the stored Stripe order, account, session, provider, paid/completed state, matching server-stored amount/currency, active subscription, unexpired order, and provider-reported period end. Subscription ID collisions no longer upsert another account's record; activation uses only provider-confirmed period data, and existing rows without the new provider event watermark fail closed for entitlement. Order-sensitive signed lifecycle updates require a newer event timestamp, map unpaid/paused/unknown provider states to inactive local status, cannot revive terminal states, and cannot move plan IDs from webhook metadata; signed payment-intent/refund and subscription-event identities are retained so an earlier refund or terminal event cannot be bypassed by a later-delivered activation. A full signed refund linked through a payment order/provider event may revoke a legacy row with no watermark. `trialing` rows no longer grant effective-plan access.

A new additive forward-only migration, `20261024000009_subscription_provider_event_ordering.sql`, adds a nullable subscription event watermark and non-destructive webhook ordering/refund evidence columns and indexes; it was not applied to any database and does not rewrite/delete billing records. Focused unit and isolated-PostgreSQL HTTP regression tests were updated/added for identity mismatches, wrong owner/provider, forged status/payment assertions, unpaid/pending orders, missing refund identity, amount/currency mismatches, non-active provider states, duplicate delivery, future/stale/out-of-order events, terminal refunds, and one provider-confirmed activation path. **Rust and PostgreSQL execution remain BLOCKED**: Cargo/Rust/PostgreSQL tools are absent and `NEXORA_AUTH_TEST_DATABASE_URL` is unset. No database or payment provider was contacted. Production still composes an unconfigured payment provider, no webhook secret, and unconfigured quota policy; no frontend/local-chat/model-routing source was changed in Step 1. No release, installer, publication, or Step 2 work was started.

**Step 1 status: IMPLEMENTED — VALIDATION BLOCKED.** G-01 is not runtime-verified and the original NO-GO decision remains unchanged.

## Step 2 full-resolution follow-up (2026-10-24)

### Decision

**Step 2 remains BLOCKED — NOT COMPLETED. The Public Beta remains NO-GO.** This follow-up records bounded source fixes and frontend checks only; it does not certify acquisition, model integrity, Windows behavior, or inference. G-01 remains separately **IMPLEMENTED — VALIDATION BLOCKED**. No Step 3, release, installer, signing, publishing, or distribution activity was authorized or performed.

### Preservation and scope

| Checkpoint | Branch / HEAD | Modified tracked paths | Untracked paths | Staged paths |
|---|---|---:|---:|---:|
| Step 2 full-resolution pre-edit checkpoint | `arena/9ea88704-nexora` / `8b2e8415bc67ec31e343218364fa4a6ce3741dab` | 95 | 15 | 0 |
| After this follow-up | Same branch and HEAD | 104 | 15 | 0 |

Compared with `/tmp/nexora-step2-preflight-status.txt`, all baseline paths remain present; nine additional tracked paths are modified: the acquisition, readiness, and settings Rust modules; model setup/selector components and their tests; the download IPC wrapper; and the model manager page. Three files already modified at the pre-edit checkpoint (`commands.rs`, `inference.test.ts`, and `rustContract.test.ts`) also received Step 2 edits. The support-module path is unchanged; only Git's untracked-directory display differs between status modes. `git diff --cached --name-only` is empty. Dependency/build outputs are ignored. No file was reset, cleaned, stashed, reverted, or overwritten; no commit or remote operation occurred.

### Bounded source fixes and regressions authored

- Removed the caller-controlled `targetDir` from `start_model_download` IPC and its UI wrapper. Production now derives the destination from Tauri `app_data_dir()/models`; the UI sends only the selected profile and progress channel. The unrelated disk-space query still accepts its own path solely for measurement.
- Hardened the acquisition path: the private `.part` file is created exclusively (`create_new`), cleanup removes only a partial file this invocation owns, cancellation is polled while awaiting response headers, and publication uses an atomic same-directory hard link that cannot replace an existing file. Filesystems that do not support that no-replace operation fail closed; there is no overwrite-prone rename fallback.
- Registered-model loading now treats the path-keyed session verification record only as a hint. If the selected file was previously verified, loading recomputes its size and SHA-256 before GGUF inspection or process spawn. A mismatch clears the cached success, is recorded as a verification failure, and blocks loading. Beginning a recheck also clears the earlier success so a failed/unreadable recheck cannot leave stale readiness evidence.
- Authored regressions for same-path model mutation after verification, clearing cached success during a recheck, cancellation before response headers without a partial file, and concurrent no-replace publication. Updated UI/Tauri IPC contract tests for the path-free download command. These Rust tests are **authored but not executed**; the UI tests below do not substitute for native tests.

### Evidence and exact checks

| Gate | Status | Evidence / remaining limit |
|---|---|---|
| Workspace identity and preservation | **PASS** | Branch/HEAD unchanged; nine new tracked status paths compared with the Step 2 preflight; all baseline paths retained; 0 staged. |
| Pinned JavaScript tooling and dependency install | **PASS** | `corepack pnpm --version` returned `9.15.4`; `corepack pnpm install --frozen-lockfile` completed successfully (165 packages; lockfile unchanged). |
| Frontend typecheck, tests, production build, config scanner | **PASS** | `PATH="/tmp/nexora-pnpm-bin:$PATH" corepack pnpm check` completed all three scripts: TypeScript typecheck passed; Vitest **88 files / 870 tests passed**; Vite production build and config-security scan passed. Vite emitted non-fatal warnings for `theme-init.js`, mixed static/dynamic imports, and the approximately 589.55 kB JavaScript chunk. These frontend checks do not execute Tauri/Rust code or prove local inference. |
| Whitespace and Rust source parsing | **PASS — LIMITED STATIC CHECK** | `git diff --check` passed. The installed `@scalar/rust-fmt` WASM formatter accepted the four touched Rust modules as Rust 2021 source; this is not the pinned native `rustfmt`, a compiler, or a test run. |
| Rust formatting, compilation, Clippy, and regression tests | **BLOCKED / NOT VERIFIED** | `cargo`, `rustc`, native `rustfmt`, and `clippy-driver` are absent. No Rust test, including the new regressions or G-01 tests, was executed. |
| Authoritative artifact identity, integrity metadata, and licensing | **BLOCKED / NOT VERIFIED** | The registry entries and expected values in the preceding table remain source metadata only. Hugging Face is outside this sandbox's allowed egress hosts, so no authoritative request was made. Repository/revision file existence, exact size and digest, GGUF architecture, context claims, and license text at each pinned revision are not independently confirmed. **Neither artifact was acquired**; no artifact was hashed or substituted. |
| Real Windows 10/11 x64 acquisition and local inference | **BLOCKED / NOT VERIFIED** | This host is Debian 12 Linux x86_64, not Windows; PowerShell, `llama-server`, both GGUF files, and adequate model memory are unavailable. Host RAM is about 3.84 GiB with no swap, below Mini's 4.36 GiB weights before runtime/headroom and well below Sonnet's 8.37 GiB. No supported Windows device/runtime was available. Nexora-managed acquisition, independent artifact hashing, managed-runtime loading, and a real response through the desktop chat route were **not performed for either profile**. |
| Local-only/profile routing and account-usage separation | **NOT VERIFIED AT RUNTIME** | Frontend route/selector tests passed, and the inspected source continues to route chat to the managed local process; there was no runtime launch, inference, or cloud-fallback test. No prompt was sent to an account-usage service. |
| G-01 | **IMPLEMENTED — VALIDATION BLOCKED** | Unchanged from Step 1. Its Rust/database tests and migration validation remain outstanding; Step 2 results do not clear G-01. |

The first direct `corepack pnpm check` invocation stopped because the shell could not find a `pnpm` executable for nested scripts. A temporary shim outside the repository at `/tmp/nexora-pnpm-bin/pnpm` routed nested calls through the pinned Corepack package manager; the exact successful command is recorded above. This did not alter the repository or dependency lockfile.

No Windows test, model-host request, model download, independent SHA-256 calculation, runtime launch, or inference occurred. The source fixes and authored tests remain unverified by native Rust execution. Do not treat the registry's current `locally_verified: false` values as changed or as evidence of a completed run.

### Remaining acceptance work before Step 2 can clear

1. On an approved network path, independently confirm each profile's pinned repository/revision/file, exact size, SHA-256, format/architecture, context metadata, and license text at that revision. Do not acquire a file if any authoritative value disagrees or remains unconfirmed.
2. With the pinned Rust 1.93.0 toolchain, format, compile, lint, and run acquisition/readiness/loading/profile-routing tests, including the newly authored regressions; separately complete G-01 Rust/database and migration validation.
3. On supported Windows 10/11 x64 hardware, acquire Mini and Sonnet independently through Nexora's managed HTTPS path, independently hash each completed file, load through the managed runtime, and generate real responses through the ordinary desktop chat route. Record machine/runtime details and failure/recovery behavior without using mocks, CLI-only inference, substitutions, or cloud fallback.
4. Keep local signed-out chat/history/model selection intact; if account-usage requests are enabled, verify separately that they contain no prompt content and are not an inference dependency.

**Step 2 disposition at the time of the preceding update: BLOCKED — NOT COMPLETED.** The beta remains NO-GO. Stop after Step 2; await explicit authorization before any Step 3 work.

## Step 2 final implementation and verification checklist (2026-10-24)

This additive update supersedes only the preceding Step 2 disposition. The scoped source implementation is now complete to the extent feasible in this environment; the required native, authoritative-source, and Windows verification is not. **Current status: PARTIAL — IMPLEMENTATION COMPLETE, VERIFICATION BLOCKED. Public Beta: NO-GO.** G-01 remains separate and **IMPLEMENTED — VALIDATION BLOCKED**; this Step 2 update does not clear it. No Step 3, commit, push, merge, tag, package, publication, or release activity occurred.

### Implementation / defect / fix checklist

| Area | Defect or audit finding | Implemented fix / regression | Status and evidence limit |
|---|---|---|---|
| Registry and license-reference hygiene | Mini's license URL previously used mutable `blob/main`, unlike the artifact's immutable revision. Registry literals for both models remain claims, not independent evidence. | Mini's license pointer now targets its pinned GGUF repository revision; a registry regression asserts that the license URL contains the artifact revision. Sonnet already had this same-revision consistency check. | **Source fix complete; authoritative license text is NOT VERIFIED.** The URL consistency assertion is an internal contract check, not evidence that the file exists or that the license permits the intended distribution. |
| Acquisition destination / partial files | The download IPC previously accepted a caller-selected target directory; partial-file ownership, header-wait cancellation, and publication replacement risks were audited. | Destination is derived from Tauri `app_data_dir()/models`; partial files are exclusively created and only invocation-owned partials are removed; cancellation is polled before headers; publication uses same-directory no-replace hard-link semantics. | **Source fix complete; native acquisition tests NOT RUN.** Unsupported filesystem publication fails closed; no overwrite-prone rename fallback was added. |
| Artifact integrity / stale readiness | A path-keyed prior verification could become stale after same-path mutation; network digest checks alone would not prove the published bytes. | Re-hash the staged bytes before publication; clear cached success when a recheck begins; re-hash a registered model before load; inspect and pass the same locked file handle to the child so the checked file cannot be replaced between validation and use. | **Source fix complete; native tests NOT RUN.** No real Mini/Sonnet artifact exists here; no SHA-256 was computed for either. |
| Readiness, memory, context, and disk estimates | Estimates and context limits must not turn registry claims into proof of machine capability. | Readiness accounts for artifact weight size, per-profile KV-cache geometry/context, runtime allowance, and installation headroom. Context settings clamp to 2,048–131,072 tokens; a load rejects a context above the selected profile's registered documented maximum. | **Static implementation reviewed; native tests and Windows measurements NOT VERIFIED.** No advertised estimate is a measured Windows load result. |
| Loading and process recovery | Profile/load/generation races and unconfirmed child shutdown could permit a replacement process or release a model guard while a runtime might still be alive. Stderr-reader thread creation could also fail after spawn. | Fail-closed profile gates and loaded-model snapshots serialize selection/load/generation; RAII claims release operations; a failed stop retains child, key, and locked-model guard. Stderr-reader startup failure attempts bounded stop. `Supervisor::drop` now retains the child in a shared ownership slot; if reaper-thread creation fails it falls back to synchronous reaping instead of dropping the guard. Regression coverage was added for unconfirmed-stop retention. | **Source fix complete; native lifecycle tests NOT RUN.** Thread-start, Windows handle/share-mode behavior, and kill/recovery are not runtime-verified. |
| Profile routing / ordinary desktop chat | Generation must not silently run with no approved/loaded profile or continue across a profile switch. UI callers must fail closed rather than relying only on backend rejection. | Rust checks request/selected/loaded profile consistency; desktop chat, conversation, patch-generation, and repair paths refuse inference without an approved profile. Signed-out chat/history/model selection remain available. | **Frontend regressions PASS; Rust checks NOT RUN; no real desktop inference.** Mock/UI outcomes are not treated as model or Windows evidence. |
| Signed-out local-only behavior / account usage | Account status must remain separate from inference and must not send prompt content or become an inference dependency. | Local inference remains on loopback with proxy use and redirects disabled. Account-usage reservation/release carries model/request identifiers, not prompts; accounting failures are warning-only and do not substitute cloud inference. No cloud inference fallback was observed in the audited path. | **Static source review and frontend tests only.** No live signed-out desktop session or account-service request was exercised. |
| Frontend test-contract regressions | Fail-closed profile checks exposed missing catalogue fixtures in several UI tests; a Rust-contract assertion also expected the prior conversion syntax. | Added a reusable Mini catalogue fixture and wired/reset catalogue mocks in interaction, welcome, chat, and Windows-verification tests; changed the typed `InferenceError` conversion to `.into()`. | **Retested successfully:** the first post-guard full run exposed 21 failing frontend tests (852 passed) because test mocks lacked the new catalogue call and a Rust-contract assertion expected the old conversion spelling. After those fixes, the targeted suite passed 5/5 files, 93/93 tests; the final full frontend check passed 88/88 files, 873/873 tests. |

### Registry values recorded for independent verification

Every value below is transcribed from the current registry. **None is independently confirmed in this run.** The artifact repository/revision/file/size/digest, GGUF format and architecture, context claims, and license text/rights must be checked against authoritative upstream records before either artifact is acquired or Step 2 can pass.

| Profile | Registry-pinned artifact repository / revision / file | Registry size / expected SHA-256 | Registry format / architecture / context claims | Registry license reference / base revision | Independent result |
|---|---|---|---|---|---|
| Nexora Mini | `Qwen/Qwen2.5-Coder-7B-Instruct-GGUF` / `13fb94bfda8c8cf22497dc57b78f391a9acb426a` / `qwen2.5-coder-7b-instruct-q4_k_m.gguf` | `4,683,073,536` bytes / `509287f78cb4d4cf6b3843734733b914b2c158e43e22a7f4bf5e963800894d3c` | `GGUF`, `Q4_K_M`; registry KV geometry claims Qwen2-family, 28 layers, 4 KV heads, 128 head dimensions; recommended/documented max `32,768 / 32,768` tokens | `Apache-2.0`; current source pointer is `https://huggingface.co/Qwen/Qwen2.5-Coder-7B-Instruct-GGUF/blob/13fb94bfda8c8cf22497dc57b78f391a9acb426a/LICENSE`; base-model revision recorded as `c03e6d358207e414f1eca0bb1891e29f1db0e242` | **BLOCKED.** No upstream request; artifact identity, object digest, config/GGUF architecture, context, and exact license file/text unconfirmed. |
| Nexora Sonnet | `Qwen/Qwen2.5-Coder-14B-Instruct-GGUF` / `d0a692ef765eefbf2fabb130b3cb2e8917e3d225` / `qwen2.5-coder-14b-instruct-q4_k_m.gguf` | `8,988,110,272` bytes / `c1e659736d89ac1065fb495330fb824d94001974a4bfa78e7270e43476a8d940` | `GGUF`, `Q4_K_M`; registry KV geometry claims Qwen2-family, 48 layers, 8 KV heads, 128 head dimensions; recommended/documented max `8,192 / 32,768` tokens | `Apache-2.0`; source pointer is `https://huggingface.co/Qwen/Qwen2.5-Coder-14B-Instruct-GGUF/blob/d0a692ef765eefbf2fabb130b3cb2e8917e3d225/LICENSE`; base-model provenance revision recorded as `aedcc2d42b622764e023cf882b6652e646b95671` (registry notes this is not proof of the quantizer's conversion input) | **BLOCKED.** No upstream request; artifact identity, object digest, config/GGUF architecture, context, and exact license file/text unconfirmed. |

The Mini URL correction prevents a mutable `main` pointer from drifting, but does **not** independently establish the license. Hugging Face is not in this sandbox's permitted egress hosts. No host request, download, artifact substitution, independent hash, license claim, or local-verification state change was made. Both registry entries remain `locally_verified: false`.

### Exact commands and observed results

| Command / environment | Observed result | What it does not establish |
|---|---|---|
| `PATH="/tmp/nexora-pnpm-bin:$PATH" corepack pnpm check` (repository root) | **PASS.** TypeScript `tsc --noEmit`; Vitest **88/88 files, 873/873 tests**; production Vite build; configuration-security scan passed without printing secrets. Build warnings: `theme-init.js` lacks `type="module"`; mixed static/dynamic imports for Tauri `window.js` and `lib/patch.ts`; minified JS chunk `590.21 kB` exceeds Vite's advisory `500 kB` threshold. | Does not execute Tauri/Rust, prove acquisition, or prove native inference. |
| `./node_modules/.bin/vitest run ui/src/App.interaction.test.tsx ui/src/App.welcome.test.tsx ui/src/App.phase25Chat.test.tsx ui/src/App.phase32WindowsVerification.test.tsx ui/src/lib/rustContract.test.ts` (`apps/desktop`) | **PASS.** 5/5 files, 93/93 tests. | UI mocks/contracts are not Rust/native or Windows evidence. |
| WASM parser command below (repository root) | **PASS — LIMITED STATIC CHECK.** Accepted all **39 modified tracked Rust files** as Rust 2021 source. | Not pinned native `rustfmt`, Rust typechecking, compilation, Clippy, or tests. |
| `git diff --check` | **PASS.** | Whitespace check only. |
| Tool probe: `for tool in cargo rustc rustfmt clippy-driver cmake llama-server llama-cli pwsh powershell; do command -v "$tool"; done` | Every listed tool unavailable. Host: Debian 12 Linux x86_64; RAM `3.8 GiB`, no swap. No `.gguf`/`.gguf.part` under `apps/desktop`. | Cannot validate native Rust or any real Windows 10/11 x64 load/chat run. |
| Tool probe: `for tool in psql pg_isready docker podman; do command -v "$tool"; done`; presence-only test of `NEXORA_AUTH_TEST_DATABASE_URL` | PostgreSQL/container tools unavailable; test database URL unset. No database contacted. | G-01 Rust/database tests and migration validation remain blocked. |

The exact WASM parse command was:

```sh
node --input-type=module <<'JS'
import { readFile } from 'node:fs/promises';
import { execFileSync } from 'node:child_process';
import { format } from './apps/desktop/node_modules/@scalar/rust-fmt/dist/index.js';
const files = execFileSync('git', ['diff', '--name-only', '--', '*.rs'], {encoding: 'utf8'}).split('\n').filter(Boolean);
for (const file of files) await format(await readFile(file, 'utf8'), { edition: '2021' });
console.log(`PASS: ${files.length} modified tracked Rust files parsed as Rust 2021 (WASM formatter; not rustc).`);
JS
```

### Final acceptance checklist and blockers

- [x] Audit and implement the safe in-scope source corrections listed above; add regressions and retest the frontend.
- [x] Preserve the existing worktree: branch `arena/9ea88704-nexora`, HEAD `8b2e8415bc67ec31e343218364fa4a6ce3741dab`; final status **110 modified tracked, 16 untracked, 0 staged**. All paths in the Step 2 preflight status remain present; Git now displays the existing untracked `services/nexora-auth/tests/support/mod.rs` under its parent directory entry. Final snapshot: `/tmp/nexora-step2-final-status.txt`.
- [ ] Run pinned native Rust formatting, compilation, Clippy, and relevant Rust acquisition/readiness/process/profile tests. **Blocked:** `cargo`, `rustc`, native `rustfmt`, and Clippy are absent.
- [ ] Independently verify both artifacts' pinned repository, immutable revision, filename, byte size, SHA-256, GGUF format/architecture, context and exact license text/rights. **Blocked:** authoritative model host is outside permitted egress; neither artifact was acquired or hashed.
- [ ] On real supported Windows 10/11 x64, acquire Mini and Sonnet only after authoritative verification, independently hash them, exercise managed-runtime loading and normal desktop-chat generations for both, and test failure/recovery without mocks, CLI-only inference, substitutions, or cloud fallback. **Blocked:** no Windows device/runtime, model artifacts, `llama-server`, or adequate model-host environment is available.
- [ ] Complete G-01's own native Rust/database/migration verification. **Still separate and blocked; G-01 remains IMPLEMENTED — VALIDATION BLOCKED.**

**Current Step 2 disposition: PARTIAL — IMPLEMENTATION COMPLETE, VERIFICATION BLOCKED. Beta remains NO-GO. Do not start Step 3.**

## Step 3 Windows Installer status update (2026-10-24)

This additive update records the newly authorized Step 3 work only. Earlier Step 2 paragraphs that said Step 3 was not yet authorized remain historical to their checkpoint; this update does not clear Step 1 or Step 2 and does not begin Step 4.

**Current decision: NO-GO — Step 3 remains PARTIAL because native Windows build, PowerShell parsing, and acceptance evidence are blocked.** No installer, signature, or Windows test result is claimed.

### Worktree preservation and scope

| Checkpoint | Branch / HEAD | Modified tracked paths | Untracked paths | Staged paths |
|---|---|---:|---:|---:|
| Step 3 pre-edit snapshot | `arena/9ea88704-nexora` / `8b2e8415bc67ec31e343218364fa4a6ce3741dab` | 110 | 16 | 0 |
| Current after Step 3 edits | Same branch and HEAD | 111 | 18 | 0 |

All original status paths remain present. The tracked change-count increase is `package.json`; two new untracked paths are the installer-contract checker and its tests. Git's all-path listing expands the previously untracked `services/nexora-auth/tests/support/` directory to `support/mod.rs`. No file was reset, cleaned, stashed, reverted, or removed. `apps/desktop/ui/dist` was generated by the successful frontend build and remains ignored. Nothing was staged. No commit, push, merge, tag, CI trigger, upload, signing, publication, or release occurred.

### Files changed and purpose

| Path | Purpose |
|---|---|
| `scripts/check-installer-contract.mjs` | Validates package/Tauri/Rust version agreement; Nexora identity; one visible `main` window and reviewed capability permissions; production frontend entry; NSIS/current-user/WebView2 configuration; icons; and absence of custom NSIS hooks, model/runtime/credential payloads, disallowed resources/sidecars, dotenv payloads, and frontend symlinks. It is a source/build-output contract, not inspection of a generated installer. |
| `scripts/check-installer-contract.test.mjs` | **15** Node tests, including negative fixtures for hidden window, broadened permissions, identity drift, custom hooks, malformed/missing icons, forbidden payloads, bad install mode, and WebView2/resource drift. |
| `package.json`, `apps/desktop/package.json` | Adds the contract-test script to root `check` and runs the contract validator after the desktop production security scan. |
| `scripts/windows/build-installer.ps1` | Enforces Node `22.22.3`, pnpm `9.15.4`, Rust/Cargo `1.93.0`, native x64 PowerShell/MSVC prerequisites, frozen install, frontend/contract checks, Rust format/Clippy/tests, explicit x64 NSIS build, optional certificate-store signing, exactly one setup executable, and post-signing SHA-256 manifest generation. |
| `scripts/windows/smoke-verify.ps1` | Authored Windows-only install/reinstall/uninstall test described below; not yet parsed or run. |
| `.github/workflows/ci.yml` | Pins Node `22.22.3`; corrects the headless Windows smoke job description. Workflow was not dispatched or triggered. |
| `apps/desktop/README.md`, `docs/PRODUCTION_DEPLOYMENT_AND_SECURITY.md`, `docs/PHASE_19_WINDOWS_PACKAGING.md` | Corrects persisted-history documentation; records toolchain, packaging commands, data paths, upstream uninstall/WebView2 source behavior, test scope, and current blockers. |

### Packaging/runtime source review and smoke-test design

The checked-in config remains Nexora `0.1.0`, identifier `com.nexora.desktop`, NSIS-only, `currentUser`, a visible main window, production frontend at `../ui/dist`, and Tauri's silent WebView2 Evergreen bootstrapper. The bundle has no configured `resources` or `externalBin`; no model weights or runtime were added. The new checker requires these constraints and rejects custom NSIS templates/hooks that could bypass the reviewed lifecycle.

Pinned Tauri 2.12.5 upstream source inspection—not a Nexora installer test—shows the per-user default `%LOCALAPPDATA%\Nexora`, `HKCU` Add/Remove Programs key `...\Uninstall\Nexora`, Start Menu and silent-install desktop shortcuts, an interactive app-data delete checkbox, and conditional data deletion that is skipped during updates. `app_data_dir()`/`app_config_dir()` resolve through the Windows roaming-data path for `com.nexora.desktop`; model acquisition defaults to `app_data_dir()/models`; Tauri's local-data directory is separate under `%LOCALAPPDATA%`. WebView2 is a shared Evergreen runtime, not bundled model/runtime content.

The revised smoke script verifies the setup-file SHA-256 manifest and Authenticode status, OS/PowerShell architecture, installed x64 PE, per-user identity/version/install location, shortcut targets, WebView2 before/after install and after uninstall, and file-hash stability across a **same-build** reinstall. It copies the installer into a Unicode/space path, installs below `%LOCALAPPDATA%`, and seeds only disposable history/settings/model-storage sentinels in otherwise empty app-data folders. It verifies silent uninstall, then a fresh install after uninstall and a final uninstall, while removing only test-owned files and empty directories. On an interactive local session it launches the installed exe from outside the checkout and launches again through Start Menu; GitHub Actions/headless mode explicitly skips GUI claims. It does not claim a previous-version upgrade, offline network isolation, model inference, or code-signing beyond the observed Authenticode status.

Source inspection found that Tauri startup creates app config/data directories and loads settings/history without automatically launching a model. Model/runtime startup is an explicit user action; the supervisor uses `127.0.0.1`, a random port and per-session key, Windows `CREATE_NO_WINDOW`, and shutdown/reaper paths. Missing/unusable model/runtime cases map to typed UI-facing failures. `capabilities/main.json` remains scoped to the `main` window with Tauri core/window permissions; no Step 3 permission broadening was made, and capability enforcement was not tested here. The frontend continues to allow signed-out local chat/history/model selection and keeps account usage separate; these behaviors were not changed by Step 3. These are source findings, not native runtime evidence.

### Exact checks and outcomes

| Check | Result |
|---|---|
| `corepack pnpm install --frozen-lockfile` | **PASS** — 165 workspace packages; lockfile unchanged. |
| Bare `corepack pnpm check` | **FAIL — environment PATH only** — nested scripts could not find `pnpm` (`sh: 1: pnpm: not found`). |
| `PATH="/tmp/nexora-pnpm-bin:$PATH" corepack pnpm check` | **PASS** — temporary shim outside Git routed nested `pnpm` calls through Corepack. TypeScript passed; Vitest **88 files / 873 tests passed**; installer-contract **15/15 passed**; Vite production build, config-security scan, and installer-contract scan passed. Vite warned about `theme-init.js` lacking `type="module"`, mixed static/dynamic imports, and a **590.21 kB** minified JS chunk over the 500 kB advisory. |
| `corepack pnpm test:installer-contract` | **PASS** — 15 passed, 0 failed. Intermediate test revisions exposed four over-escaped fixture newlines; the malformed JSON was corrected and all cases rerun successfully. |
| `node scripts/check-installer-contract.mjs` | **PASS** — product `Nexora 0.1.0`, identifier `com.nexora.desktop`, NSIS; production frontend **4 files / 720,456 bytes**; no forbidden model/runtime/credential payload detected. This is not an installer artifact/hash. |
| Tauri CLI version/info/help | **PASS — diagnostic/help only** — CLI `2.12.3`; Debian 12 Linux x86_64; Rust/Cargo, WebKit2GTK 4.1, and rsvg2 unavailable. The Linux CLI listed only Linux bundle formats. No Tauri build was attempted. |
| PowerShell parser and Windows scripts | **BLOCKED / NOT RUN** — `pwsh`, Windows PowerShell, and .NET parser tooling are not installed here. An official portable-release fetch redirected to an unavailable asset endpoint and failed before any binary was downloaded; no parser/tool was installed. An ad hoc lexical delimiter/string/comment balance check passed, but it is not a PowerShell parser. The edited `.ps1` scripts have **not** been syntax-checked or executed. |
| Native Rust check/Clippy/tests and Windows NSIS build | **BLOCKED / NOT RUN** — no Rust/Cargo toolchain, MSVC/Windows SDK, or Windows host. |
| Windows 10/11 x64 clean install, GUI shortcut launch, upgrade, uninstall/reinstall, WebView2 bootstrapper, Authenticode, offline/error behavior | **NOT OBSERVED / NOT VERIFIED** — no Windows runner/device was invoked. CI was inspected but not triggered. |

The successful build produced only the ignored frontend `dist` output. **No Windows installer or installer SHA-256 manifest was generated; there is no installer hash to report. Signing was not attempted; no Authenticode result or certificate status is claimed.** No model/runtime binary or model weights were downloaded or bundled. Tauri 2.12.5 pins NSIS 3.13 and checks upstream toolchain/package files using SHA-1; Windows SDK/host image and the Evergreen runtime remain external inputs, so no bit-for-bit reproducibility or provenance attestation is claimed.

### Required remaining acceptance work

1. On native x64 Windows 10 and Windows 11, parse both PowerShell scripts and run the frozen build/test/package flow. Save actual build logs, exact installer name/size/SHA-256, and Authenticode status; report failure output without manufacturing artifacts.
2. On clean Windows profiles, run the interactive smoke test and record OS/build, install path, shortcut launch, process exit, data preservation, WebView2 before/after/uninstall, uninstall cleanup, and reinstall-after-uninstall. A headless CI result can validate package automation but does not prove visible shortcut launch.
3. Test a real prior-version upgrade only when a supported prior installer is available; the script's same-build reinstall is explicitly not an upgrade. Verify user data survives that upgrade and the supported uninstall choices.
4. Exercise offline startup and missing-model/runtime recovery on Windows without cloud inference fallback. Do not acquire or bundle model weights until their existing approved revision/size/SHA-256/licensing path is independently verified; `locally_verified` stays false.
5. Keep code signing and publisher identity separate: do not claim signed status without a real artifact, approved certificate, and successful Authenticode verification.

**Step 3 disposition: PARTIAL — implementation/checks advanced, Windows build and acceptance blocked.** Step 1 remains **IMPLEMENTED — VALIDATION BLOCKED**; Step 2 remains **PARTIAL — IMPLEMENTATION COMPLETE, VERIFICATION BLOCKED**. Public Beta remains NO-GO. Do not start Step 4 or commit/push/merge/tag/publish/release.

## Step 3 final-clearance update (2026-10-24)

This latest additive report supersedes the preceding Step 3 test count and implementation notes only. It does not clear Step 1 or Step 2, authorize a release, or start Step 4.

### Source inspection and implementation corrections

- Reconciled the toolchain pins across repository config: Node `22.22.3`, pnpm `9.15.4`, Rust/Cargo `1.93.0`. The root package, each CI pnpm setup step, Rust toolchain file, build script, and Windows job use matching pins.
- Fixed the clean-checkout build ordering: Windows packaging runs frontend typecheck/tests and installer-contract tests, then `pnpm build` before Cargo fmt/Clippy/tests because the native context macro embeds `ui/dist`; the final Tauri command runs its configured production frontend build again.
- Added non-destructive stale-output safeguards: a build refuses any existing EXE or `SHA256SUMS.txt` in the target NSIS directory, never deletes prior artifacts, requires exactly one non-empty fresh setup EXE, and creates the hash manifest without overwriting an existing file. The smoke harness checks exactly one bundle EXE and that the manifest is not older than the installer.
- Hardened optional signing checks: the selected current-user certificate must have a private key, Code Signing EKU, current validity, and the final valid Authenticode signer must match the configured thumbprint. No certificate was configured or used.
- Hardened smoke cleanup: the test install destination cannot overlap the default install or Nexora data roots; only unchanged known sentinel files and an installer copy with the expected hash are removed. Changed/unexpected files and directories are retained for manual review; there is no recursive cleanup of the package-copy directory.
- Hardened the installer contract validator against a frontend `dist` symlink/junction, with new regression coverage. The reviewed production config still has one visible main window and the reviewed capability, NSIS/current-user setup, WebView2 strategy, and no model/runtime/secret bundle resources.
- Updated the existing `windows-2022` job to initialize the hosted x64 Visual Studio/Windows SDK environment with its installed `vswhere` and `VsDevCmd.bat`, then verify MSVC/SDK tools. This is workflow source configuration only; it was not run.

### Automated and source/build evidence (Linux)

| Gate | Result | Limitation |
|---|---|---|
| Frozen dependency install | **PASS** — `corepack pnpm install --frozen-lockfile`, 165 packages, lockfile unchanged. | No Windows packages produced. |
| Aggregate frontend/security/contract check | **PASS** — `PATH="/tmp/nexora-pnpm-bin:$PATH" corepack pnpm check`: TypeScript; Vitest **88 files / 873 tests**; installer-contract **17/17**; production Vite build; config-security scan; production contract scan. | Linux/frontend/asset checks only; no native Rust execution. |
| Production contract scan | **PASS** — `node scripts/check-installer-contract.mjs`: Nexora `0.1.0`, `com.nexora.desktop`, NSIS; **4 assets / 720,456 bytes**, no model/runtime/credential payload. | Checks source/config/frontend output, not an NSIS executable. |
| Security and whitespace checks | **PASS** — `node scripts/check-config-security.mjs`; `git diff --check`. | No PowerShell/native packaging semantics established. |
| Vite build warnings | Non-fatal: `theme-init.js` is not bundled without `type="module"`; static/dynamic import mixing; 590.21 kB JS chunk exceeds the 500 kB advisory. | Not a packaging acceptance failure, but retained for tracking. |

### Windows availability and exact blocker

The execution host is Linux x86_64. `pwsh`, Windows PowerShell/.NET parsing, Cargo/Rust, MSVC/Windows SDK, NSIS, and SignTool are unavailable. The two PowerShell scripts were **not parsed or executed**. The existing hosted `windows-2022` workflow is configured but no run exists for `arena/9ea88704-nexora` (`gh run list --branch arena/9ea88704-nexora` returned none). This local worktree is unpushed; dispatching now would test remote content without these edits, and pushing is explicitly out of scope. No runner was allocated for this exact worktree. No authorized Windows 10/11 x64 machine was otherwise available.

No installer, artifact SHA-256 manifest, signing outcome, Windows install, GUI launch, WebView2 bootstrap, offline/missing-model error, uninstall/data-preservation, or reinstall result exists. No valid previous supported installer was supplied; same-build reinstall is not an upgrade. The authored harness does not prove local inference or cloud-fallback behavior on Windows. Do not promote source inspection, Linux checks, mocked UI tests, or a configured CI lane into Windows acceptance claims.

**Current Step 3 status: PARTIAL — IMPLEMENTATION COMPLETE, WINDOWS VERIFICATION BLOCKED.** Step 1 remains **IMPLEMENTED — VALIDATION BLOCKED**. Step 2 remains **PARTIAL — IMPLEMENTATION COMPLETE, VERIFICATION BLOCKED**. Public Beta remains **NO-GO**. No commit, push, merge, tag, release, publication, or Step 4 work occurred.

## Step 4 release-candidate/security review update (2026-10-25)

This additive update supersedes only the preceding statement that Step 4 had not begun. It preserves prior phase evidence and does not convert source review, authored tests, or frontend checks into native/database/Windows acceptance. Step 1 remains **IMPLEMENTED — VALIDATION BLOCKED**; Step 2 remains **PARTIAL — IMPLEMENTATION COMPLETE, VERIFICATION BLOCKED**; Step 3 remains **PARTIAL — IMPLEMENTATION COMPLETE, WINDOWS VERIFICATION BLOCKED**. **Step 4 is PARTIAL — available source hardening and documentation are prepared; runtime, legal, and release gates remain blocked. Public Beta is NO-GO.** Step 5 was not started.

### Findings and changes

1. **JavaScript dependency advisories.** The initial production audit flagged `diff@5.2.0` (low, GHSA-73rr-hh4g-fpgx / CVE-2026-24001); the reviewed desktop source uses `diffLines`, not the affected `parsePatch`/`applyPatch` APIs, but the package is now pinned to `5.2.2`. A subsequent full audit found five development-graph advisories (three moderate in the then-locked Vitest 3.2.7 / `@vitest/mocker` graph and two critical in `tinypool@1.1.1`). Vitest is now pinned to `4.1.12`; the current full graph has no reported advisories. Both the production-only and full `pnpm audit` now exit 0 with **0 low, moderate, high, or critical advisories**. The current result is limited to the npm/pnpm graph and registry advisory snapshot; it says nothing about Rust/native packages. CI and the Windows build script now run `pnpm audit` after frozen installation.
2. **Account deletion atomicity.** Source inspection found the `SELECT ... FOR UPDATE` was executed through `PgPool`, so its implicit statement transaction did not keep the row lock through password confirmation and deletion; the prior best-effort audit write could also precede a failed delete. `services/nexora-auth/src/account.rs` now holds the lock, validates the confirmation/credentials, records a sanitized audit row, deletes the user, and commits in one SQLx transaction. Audit-write failure aborts deletion; success logging occurs only after commit. `services/nexora-auth/src/audit.rs` adds the caller-transaction helper and propagates storage failures. The existing cascades and audit retention schema are unchanged; no migration was edited. The existing ignored `account_profile_db` integration test was extended for anonymous rejection, mismatched confirmation/no success audit, successful deletion/retained audit, and session invalidation. It is **authored but not run**.
3. **Authentication, authorization, and desktop boundary.** Source/config review revisited the protected account routes, bearer-session identity, Tauri command registration, CSP/capabilities, explicit model acquisition, and inference routing. The desktop currently registers 86 Tauri application IPC handlers; the README now records the broad trusted-webview command surface instead of claiming only ten narrow commands. No generic shell-execution IPC handler was found. Browser-side production CSP restrictions and the defined Rust HTTPS account/model-download requests are source/configuration evidence only. Existing signed-out local chat and no-cloud-fallback boundaries were not changed; UI tests exercise mocked/local contracts, not an actual model runtime. Billing/subscription activation and deferred commercial features remain out of scope and inactive.
4. **Migrations/database safety.** The repository's nine auth migration files, including Step 1's additive migration, were left intact. No PostgreSQL connection, migration, database test, or destructive query was run. The new integration test checks that the connected database name ends in `_test` before migrating or writing fixtures; its required `NEXORA_AUTH_TEST_DATABASE_URL` is not configured. Deletion behavior, migration compatibility, retention semantics, and the new transaction remain unverified against PostgreSQL. The detailed follow-up is in [`docs/database-migrations-audit.md`](database-migrations-audit.md).
5. **Third-party notices and legal blockers.** [`docs/THIRD_PARTY_NOTICES_DRAFT.md`](THIRD_PARTY_NOTICES_DRAFT.md) records verified metadata and license texts for only five frontend runtime packages (`@tauri-apps/api`, `diff`, `react`, `react-dom`, `scheduler`). The Tauri package metadata and matching upstream license texts were checked; differing supplied copyright/supplier records are reported without drawing an ownership conclusion. This is not a full SBOM or distributable notice. The 460 desktop and 249 auth Rust lockfile packages, system/native dependencies, icon provenance, exact `llama-server` runtime rights, model license files, and quantized-model redistribution rights remain unresolved. No approved Nexora root license or legal publisher identity was found. [`docs/LEGAL_PRIVACY_TERMS.md`](LEGAL_PRIVACY_TERMS.md) now withdraws unverified runtime/model-license compatibility assertions. Legal review and owner-approved redistribution evidence are still required.
6. **Support/security reporting.** Added [`SECURITY.md`](../SECURITY.md) and [`docs/SUPPORT_AND_DIAGNOSTICS.md`](SUPPORT_AND_DIAGNOSTICS.md), and linked them from the root README. They deliberately contain release-blocking placeholders because no verified public support address or private security-reporting route was available. They are not operational contacts; none was invented.
7. **Reproducibility controls.** A frozen pnpm install succeeded without lockfile changes. CI Cargo format, check, Clippy, and test invocations now use `--locked`; the Windows build script likewise locks Cargo checks/tests and forwards `--locked` to Tauri's Cargo build. CI and the Windows script were not executed after these edits. Rust/Cargo and Cargo advisory tooling are absent, so no native compilation, formatting, Clippy, Rust tests, or Rust dependency audit ran. No local YAML parser/actionlint is installed, so the workflow file was not independently syntax-parsed. These source changes are not proof that the workflow or PowerShell forwarding syntax works on its target runner.

### Checks executed in this environment

| Check | Result | Evidence boundary |
|---|---|---|
| `PATH="/tmp/nexora-pnpm-bin:$PATH" corepack pnpm install --frozen-lockfile` | **PASS** — lockfile up to date; workspace already installed. | Node workspace only. |
| `PATH="/tmp/nexora-pnpm-bin:$PATH" corepack pnpm audit --prod --json` | **PASS** — exit 0; all severity counts 0. | Production npm/pnpm dependency graph only. |
| `PATH="/tmp/nexora-pnpm-bin:$PATH" corepack pnpm audit --json` | **PASS** — exit 0; all severity counts 0. | Complete npm/pnpm graph only; not Cargo/native or model artifacts. |
| `PATH="/tmp/nexora-pnpm-bin:$PATH" corepack pnpm check` | **PASS** — TypeScript; Vitest **88 files / 873 tests**; installer-contract **17/17**; Vite production build; config-security scan; installer-contract scan. | Linux/frontend tests and source/config contracts; no Rust, PostgreSQL, Windows, or genuine inference. |
| Production build/contract output | **PASS** — 4 frontend assets / 720,456 bytes; no model/runtime/credential payload detected. | Not a Windows installer or a signed/release artifact. |
| Tauri CLI `build --help` | **PASS** — confirms `[ARGS]...` are passed to the runner and `--` marks their start. | CLI help only; the Windows PowerShell command and native build were not run. |
| Rust WASM formatter parse of `account.rs`, `audit.rs`, and `account_profile_db.rs` | **PASS** — accepted as Rust 2021 syntax. | Auxiliary parser only; not pinned `rustfmt`, compilation, Clippy, or test evidence. |
| Whitespace checks | **PASS** — `git diff --check` and a focused trailing-whitespace scan of new/updated untracked docs/tests. | Does not validate semantics. |

The Vite build retained non-fatal warnings: `theme-init.js` lacks `type="module"` for bundling; static/dynamic imports are mixed for Tauri window and patch modules; the 590.21 kB main JavaScript chunk exceeds Vite's 500 kB advisory. The earlier dependency install also reported deprecation notices for `@types/diff@8.0.0` and `whatwg-encoding@3.1.1`; both remain in the lockfile as hygiene follow-ups, not current audit findings. These warnings are recorded, not presented as release validation.

### Blocked evidence and release matrix

| Gate | Status | Missing acceptance evidence |
|---|---|---|
| Step 1 billing/auth-service security | **BLOCKED** | Rust/Cargo/PostgreSQL integration and migration checks; legal/business policy approvals. No billing operation was activated. |
| Step 2 genuine inference | **BLOCKED** | Exact approved model/runtime artifacts, verified acquisition/integrity, real local inference and error-path evidence. No weights/runtime were downloaded; no cloud fallback was enabled. |
| Step 3 Windows packaging | **BLOCKED** | PowerShell parsing/execution, native MSVC build, NSIS artifact, Windows 10/11 install/upgrade/uninstall, WebView2, and UI smoke evidence. |
| Step 4 database/auth correction | **BLOCKED at runtime** | Run the focused ignored test and migration suite against a reviewed disposable `_test` database; Cargo/PostgreSQL are unavailable here. |
| Step 4 dependency/security review | **PARTIAL** | pnpm audits pass; Cargo advisory/license inventory and native dependency review remain outstanding. |
| Step 4 third-party/legal readiness | **BLOCKED** | Complete shipped-artifact SBOM/notices, verified Rust/native/assets/model/runtime rights, approved Nexora license/publisher, and legal sign-off. |
| Support/security operations | **BLOCKED** | Owner-verified public support contact and private security-reporting process/route. Placeholders are release-blocking. |
| Signing, checksum, provenance | **NOT PRODUCED** | No Windows installer exists here; no hash manifest, Authenticode result, signing certificate validation, or provenance attestation is claimed. |

### Required prerequisites before reconsidering Public Beta

- Keep Step 1–3 statuses blocked until their own genuine acceptance criteria pass; do not substitute this report or frontend tests for their evidence.
- On an authorized, disposable database whose actual name ends in `_test`, run Rust formatting/Clippy and the auth migration/integration tests including `account_profile_db`; save exact command output and review deletion/audit/retention semantics. Do not run against production or user data.
- Obtain legal review and owner-approved redistribution evidence for the exact model revisions/quantized artifacts, runtime, Rust/native dependencies, icons/assets, and final shipped artifact set; generate complete notices/SBOM from that exact set.
- Supply and verify actual support and private vulnerability-reporting contacts and the operating process behind them. Replace placeholders only with owner-authorized details.
- Run the locked workflow and build script on their genuine Linux/Windows runners. On authorized x64 Windows 10/11, produce a fresh installer, record the actual artifact name/size/SHA-256, verify Authenticode only if a valid approved signing certificate is used, and exercise install/upgrade/uninstall/reinstall and offline recovery. No checksum or signature can be inferred before that run.
- Verify actual local inference using an approved, integrity-checked artifact without adding a cloud fallback. Keep billing and deferred features disabled.

### Preserved worktree and repository state

The pre-edit snapshot recorded branch `arena/9ea88704-nexora`, HEAD `8b2e8415bc67ec31e343218364fa4a6ce3741dab`, **0 staged / 111 modified tracked / 18 untracked** paths. The final state is the same branch and HEAD, **0 staged / 114 modified tracked / 21 untracked** paths. Every preflight tracked and untracked path remains present. The three additional modified tracked paths are `pnpm-lock.yaml`, `services/nexora-auth/src/account.rs`, and `services/nexora-auth/src/audit.rs`; the three additional untracked files are `SECURITY.md`, `docs/SUPPORT_AND_DIAGNOSTICS.md`, and `docs/THIRD_PARTY_NOTICES_DRAFT.md`. Other pre-existing modified/untracked work was preserved and was not reset, cleaned, stashed, or blanket-reverted. No commit was created.

**Step 4 disposition at the close of Step 4 (historical): PARTIAL — source/dependency corrections and draft documentation prepared; Rust/database, Windows/signing, inference, full licensing, legal, and support evidence blocked. Public Beta: NO-GO.** No database was contacted; no installer/checksum/signature or inference evidence was produced; no commit, push, merge, tag, publish, release, billing activation, or Step 5 work occurred at that time.

## Step 5 final-gate update (2026-10-25)

This additive update performs the requested final gate; it does **not** authorize publication or distribution. It supersedes only the preceding historical statement that Step 5 had not started. The allowed final-gate dispositions below are used consistently: `PASS — VERIFIED`, `PARTIAL — EVIDENCE INCOMPLETE`, and `BLOCKED — NOT VERIFIED`. A `PASS` for a local check does not clear a separate native, database, model, Windows, legal, or release-artifact requirement.

### Final Step 1–4 dispositions

| Phase | Final-gate disposition | Current assessment |
|---|---|---|
| Step 1 — billing/security invariants | **BLOCKED — NOT VERIFIED** | Source hardening and regression coverage exist, but Cargo and database tests were not run. Payment provider and quota policy remain explicitly unconfigured. |
| Step 2 — Mini/Sonnet integrity and inference | **BLOCKED — NOT VERIFIED** | Historical publisher metadata is recorded, but no model bytes were downloaded or independently hashed and no genuine local inference was run. Both profiles remain `locally_verified: false`. |
| Step 3 — Windows installer acceptance | **BLOCKED — NOT VERIFIED** | No Windows PowerShell run, native build, installer, prior-version upgrade, installation lifecycle result, checksum, or signature exists. |
| Step 4 — release-candidate readiness | **PARTIAL — EVIDENCE INCOMPLETE** | The pnpm audit and frontend suite pass; notices cover only five frontend runtime packages. Full native/model/runtime/assets licensing, owner/legal approvals, verified support/security contacts, and artifact-linked release evidence are missing. |
| Step 5 — final approval gate | **BLOCKED — NOT VERIFIED** | Multiple mandatory gates remain unverified; Public Beta is **NO-GO**. |

These final-gate labels do not erase the historical implementation labels: Step 1 remains **IMPLEMENTED — VALIDATION BLOCKED**, Step 2 **PARTIAL — IMPLEMENTATION COMPLETE, VERIFICATION BLOCKED**, Step 3 **PARTIAL — IMPLEMENTATION COMPLETE, WINDOWS VERIFICATION BLOCKED**, and Step 4 **PARTIAL**.

### Step 5 source hardening performed

- **Webhook timestamp overflow:** The signature verifier previously used signed subtraction and `.abs()` on a parsed webhook timestamp. Extreme `i64` values could overflow before signature comparison. It now uses unsigned `i64::abs_diff` and rejects negative tolerances. Added a Rust unit regression for `i64::MIN`, `i64::MAX`, and negative tolerance.
- **Conflicting duplicate event content:** The service stored each webhook payload SHA-256 but treated every duplicate `(provider, provider_event_id)` as an ordinary deduplicated delivery without comparing the payload. It now returns the normal duplicate acknowledgement only when the signed raw payload digest matches the stored digest; a different signed body for the same ID rolls back and returns a conflict without a state transition. Added a focused database regression alongside the exact-payload duplicate test. The shared test fixture fixes the provider `created` timestamp so repeated helper calls really send the identical body rather than receiving different generated timestamps.
- No migration was edited. These are uncompiled source fixes and authored tests, not runtime-verified behavior. Production composition still supplies `UnconfiguredPaymentProvider` and `QuotaPolicy::unconfigured`; billing, subscriptions, and paid entitlements remain inactive.

### Definitive release-gate matrix

| Criterion | Disposition | Required evidence | Evidence actually available / tests executed | Remaining defect or external prerequisite | Exact action to clear | Release-blocking? |
|---|---|---|---|---|---|---|
| **Step 1 — server-side entitlement and authorization** | **PARTIAL — EVIDENCE INCOMPLETE** | Native route/database tests must show identity comes from authenticated sessions, client plan fields cannot grant entitlements, activation requires server/provider evidence, and unconfigured provider/quota paths fail closed. | Source review: account/billing routes authenticate bearer sessions; `main.rs` always selects `UnconfiguredPaymentProvider` and `QuotaPolicy::unconfigured`; lifecycle changes fail closed; effective-plan resolution reads active, current subscriptions. Existing billing/auth tests are present but **were not executed**. Frontend tests are not backend evidence. | No native Rust compile/test or PostgreSQL result; no approved commercial/entitlement policy. | With pinned Rust, run the auth unit and integration suites against an authorized disposable `_test` database; review exact outcomes and keep provider/quota disabled pending owner approval. | **Yes** |
| **Step 1 — webhook authentication, replay, duplicates, refunds, event ordering** | **PARTIAL — EVIDENCE INCOMPLETE** | Execute HMAC freshness/tampering, duplicate and conflicting-ID, idempotency, refund, cancellation, expiry, stale/out-of-order and concurrent-transition regressions. | Static review found HMAC-SHA256 verification, freshness tolerance, constant-time signature comparison, provider/event uniqueness, transition locks, ordering watermarks, fail-closed activation evidence, and full-refund handling. Timestamp overflow and duplicate-payload comparison were hardened; new tests were authored. Rust WASM parser accepted the touched Rust files, but **no Rust test ran**. | No native compile, no test execution, and no live provider validation (provider intentionally disabled). | Run billing unit tests and `plans_billing_quota_db` with the pinned toolchain and a disposable `_test` database; review all test outputs. Do not enable Stripe/Razorpay or credentials as a test shortcut. | **Yes** |
| **Step 1 — migrations, account deletion, database safety** | **BLOCKED — NOT VERIFIED** | Apply all nine migrations to a clean disposable database; execute migration/auth/account-deletion tests; verify deletion audit, cascades, retention, rollback and session invalidation. | Nine migration files were inspected and left unchanged. Test helpers verify the actual database name ends in `_test`; `plans_billing_quota_db` truncates fixture tables only after that check. Account deletion is now transaction-scoped and a PostgreSQL regression is authored. **No database was contacted, no migration applied, and no test run.** | Cargo, PostgreSQL tools, Docker/Podman, and `NEXORA_AUTH_TEST_DATABASE_URL` are unavailable. The integration test suite is destructive to its test database. | First provide an authorized, reviewed disposable database whose actual name ends `_test`; then run `cargo --locked test --manifest-path services/nexora-auth/Cargo.toml -- --include-ignored --test-threads=1`, plus fmt/Clippy. Confirm the connection target before execution; never use production or user data. | **Yes** |
| **Step 2 — Mini pinned metadata and bytes** | **PARTIAL — EVIDENCE INCOMPLETE** | Independently confirm the exact repository, immutable revision, filename, byte length, content SHA-256 and license at the pinned revision; download only through Nexora's approved workflow; hash the actual bytes independently. | Registry pins `Qwen/Qwen2.5-Coder-7B-Instruct-GGUF`, revision `13fb94bfda8c8cf22497dc57b78f391a9acb426a`, `qwen2.5-coder-7b-instruct-q4_k_m.gguf`, 4,683,073,536 bytes, expected SHA-256 `509287f78cb4d4cf6b3843734733b914b2c158e43e22a7f4bf5e963800894d3c`. Historical host metadata is recorded in `docs/nexora-mini/PHASE_4_MODEL_REGISTRY.md`; it is not a checksum of local bytes. No Mini file is present. Sandbox measurement only (not target-device evidence): Linux x86_64, 2 logical CPUs, 4,034,452 kB total / 3,729,852 kB available RAM, and 20,509,691,904 bytes free filesystem space; no GPU measurement. Available RAM is below the expected weight-file size; no model load was attempted. | Hugging Face is outside this sandbox's permitted egress hosts; no Step 5 request or download was made. Exact-artifact redistribution rights remain subject to legal review. | On an authorized network path, re-fetch the pinned model API/tree and exact license; compare all values with the registry. Only then, if storage/hardware and authorization permit, acquire through the in-app flow and independently hash the file. Keep `locally_verified: false` until real inference is recorded. | **Yes** |
| **Step 2 — Sonnet pinned metadata, bytes and license** | **PARTIAL — EVIDENCE INCOMPLETE** | Independently reconfirm pinned repository/revision/file/size/SHA-256/license and redistribution rights; hash actual downloaded bytes. | Registry pins `Qwen/Qwen2.5-Coder-14B-Instruct-GGUF`, revision `d0a692ef765eefbf2fabb130b3cb2e8917e3d225`, `qwen2.5-coder-14b-instruct-q4_k_m.gguf`, 8,988,110,272 bytes, expected SHA-256 `c1e659736d89ac1065fb495330fb824d94001974a4bfa78e7270e43476a8d940`. Historical host metadata and license-source observations are recorded in `docs/nexora-sonnet/ARTIFACT_EVIDENCE.md`; the bytes were not fetched or locally hashed. No Sonnet file is present. | No Step 5 host re-fetch or local artifact verification; historical metadata is not a current checksum or legal approval. The same Linux sandbox measurement (2 logical CPUs; 3,729,852 kB available RAM; 20,509,691,904 bytes free filesystem space; no GPU measurement) is not target-device evidence; available RAM is below the expected weight-file size. Exact converted-artifact redistribution terms remain unresolved. | Revalidate the publisher API/tree and license at the pinned revision through an approved route; acquire only via Nexora's approved flow if authorized, verify bytes independently, and obtain legal review before redistribution or bundling. | **Yes** |
| **Step 2 — genuine local inference, hardware and no-cloud fallback** | **BLOCKED — NOT VERIFIED** | On supported hardware, load both verified artifacts through the installed Nexora app and managed local runtime; record OS/hardware/runtime, memory, errors and real output; verify no silent cloud fallback. | Source routes generation to the supervised loopback runtime and no cloud fallback was added. Frontend tests use mocks/local contracts. `find` found no `.gguf`, `llama-server`, or Windows executable in the desktop tree; both model registry flags remain `false`. **No model/runtime was downloaded, no inference attempted, and no performance or error result exists.** | Exact artifacts, adequate authorized hardware, and genuine Windows app/runtime are unavailable; no artifact license clearance. | On authorized target hardware, first verify source licensing/approval and capacity; acquire through the app, independently hash each file, run load/chat/cancel/error/offline tests, record actual environment and results, and only then consider changing a verification flag. | **Yes** |
| **Step 3 — PowerShell parsing and native Windows build** | **BLOCKED — NOT VERIFIED** | Parse and execute both PowerShell scripts on authorized Windows 10/11 x64 with pinned Node/pnpm/Rust/MSVC/SDK, then produce a real native build. | The Linux frontend test and Tauri CLI help are not Windows evidence. `pwsh`, Windows PowerShell/.NET, Cargo/Rust, MSVC/SDK, NSIS and SignTool are absent. Scripts were **not parsed or executed**; no Windows workflow run exists for this unpushed worktree. | No authorized Windows 10/11 runner/device or genuine runner logs. | On an authorized Windows x64 environment, run PowerShell parser checks, record actual tool versions/exit codes, run the locked build and relevant Windows Rust tests, and retain logs. | **Yes** |
| **Step 3 — install, launch, upgrade, offline behavior, uninstall/reinstall** | **BLOCKED — NOT VERIFIED** | Exercise a clean install and GUI launch, missing-model/offline recovery, data/credential behavior, genuine prior-version upgrade, uninstall and reinstall on Windows. | No installer or Windows test was produced. No supported prior-version installer was supplied; same-build reinstall is not an upgrade. | Windows host, real installer and genuine previous supported version are unavailable. | With real artifacts, test clean profiles on Windows 10 and 11; separately upgrade from a genuine prior supported version; record OS/build, install path, launch, WebView2, data preservation, uninstall and reinstall observations. | **Yes** |
| **Step 3 — payload, SHA-256, Authenticode and provenance** | **BLOCKED — NOT VERIFIED** | Validate the final installer payload, independently recalculate SHA-256, verify Authenticode signer if signed, and create artifact-linked provenance/evidence. | NSIS target output directory is absent; no `.exe`, `SHA256SUMS.txt`, certificate thumbprint, SignTool, signature or build attestation exists. The Linux frontend contract scan found no model/runtime/credential payload in four built frontend assets; that is not an installer scan. | No genuine installer, authorized signing identity/tool, or build provenance. | After an authorized Windows build, inspect the actual installer/payload; independently hash it twice or with a separate verifier; verify signer identity/status if applicable; record toolchain and logs. Do not create a fictional manifest. | **Yes** |
| **Step 4 — pnpm dependency and available frontend checks** | **PASS — VERIFIED** | Frozen install, production/full dependency audits, TypeScript, all frontend tests, production build, config-security and installer-contract tests. | `pnpm install --frozen-lockfile` passed; the lockfile was up to date and 157 packages were installed from it. Production and full `pnpm audit --json` both exited 0 with zero advisories. `pnpm check` passed: TypeScript, Vitest **88 files / 873 tests**, installer-contract **17/17**, Vite build and both scans. | This verifies only the current pnpm/frontend graph and Linux checks, not Cargo/native/Windows/model packages. Existing Vite bundle/import warnings and two deprecation notices remain. | Retain these checks in candidate CI and rerun for the exact candidate; separately clear native and platform gates below. | **Yes — this subset is required; broader release gates remain open** |
| **Step 4 — Cargo/native dependency vulnerability and license audit; reproducibility** | **BLOCKED — NOT VERIFIED** | Run locked Rust formatting/build/Clippy/tests, inventory exact native dependencies and licenses, run an applicable advisory audit, validate CI/workflow syntax. | CI/script source uses `--locked`; Cargo locks contain 460 desktop and 249 auth packages. An auxiliary WASM parser accepted five changed Rust files; this is not compilation or rustfmt. No Cargo audit ran. PyYAML, Ruby YAML, `actionlint`, `yamllint`, Cargo and Rust tools are absent; modified workflow was not independently parsed or executed. | No native toolchain/audit tooling; no CI run for this worktree. | On the pinned Rust CI toolchain, run fmt/check/Clippy/tests with `--locked`; add/run an authorized crate/license audit; validate `.github/workflows/ci.yml` using a real workflow linter and execute the workflow on the exact revision. | **Yes** |
| **Step 4 — complete third-party notices/SBOM and redistribution review** | **PARTIAL — EVIDENCE INCOMPLETE** | Inventory exact final shipped frontend, Rust/native, OS/system, runtime, model, icon/font/asset dependencies; include required license texts and attribution; independently clear redistribution rights; produce complete artifact-linked SBOM/notices. | `docs/THIRD_PARTY_NOTICES_DRAFT.md` contains verified metadata/license texts for five frontend runtime packages only. It is explicitly incomplete and not for distribution. No installer exists to inventory. Rust/native packages, system components, runtime, icons/assets, model rights and exact quantized-artifact permissions remain unresolved. | No exact release payload, full Cargo license metadata, verified icon/runtime provenance, owner-approved model permissions, or legal review. No approved root `LICENSE`/publisher identity. | Generate a complete inventory from the final authorized artifact and lockfiles; verify every license/notice and exact model/runtime rights; obtain project-owner and legal approval before distribution. | **Yes** |
| **Step 4 — support/security contacts and operating process** | **BLOCKED — NOT VERIFIED** | Verified, monitored public support route; verified private vulnerability-reporting route; named owner, triage/response process and disclosure policy. | `SECURITY.md` and `docs/SUPPORT_AND_DIAGNOSTICS.md` clearly retain release-blocking placeholders. No contacts were invented and no operational channel was tested. | Owner-authorized real contacts and operating process are absent. | Project owner must provide and verify the actual channels and responsible response owners; test both routes before release. | **Yes** |
| **Step 4 — privacy, retention, legal and release documentation** | **PARTIAL — EVIDENCE INCOMPLETE** | Approved privacy/terms and retention disclosures; reviewed diagnostics and support text; documented target requirements/known limitations; authorized legal approval. | The legal template withdraws unverified model/runtime license conclusions and states account deletion is unrun. Support docs warn about private auth-service URL disclosure and prohibit sharing secrets/prompts. No legal/runtime approval or actual OS verification exists. | No authorized legal reviewer/approval, verified support data flow, or native account/deletion retention evidence. | Obtain legal review against actual deployed data flows; verify retention/deletion on disposable DB and privacy behavior on real OS before publishing any guarantees. | **Yes** |

### Re-run checks, failures and exact results

- `PATH="/tmp/nexora-pnpm-bin:$PATH" corepack pnpm install --frozen-lockfile` — **PASS**, lockfile up to date; 157 packages installed from the lockfile.
- `PATH="/tmp/nexora-pnpm-bin:$PATH" corepack pnpm audit --prod --json` — **PASS**, exit 0; zero info/low/moderate/high/critical advisories.
- `PATH="/tmp/nexora-pnpm-bin:$PATH" corepack pnpm audit --json` — **PASS**, exit 0; zero info/low/moderate/high/critical advisories.
- `PATH="/tmp/nexora-pnpm-bin:$PATH" corepack pnpm check` — **PASS after environment-wrapper repair**: TypeScript; **88/88 files and 873/873 tests passed**; installer-contract **17/17**; production build; config-security and production contract scans. Built frontend output is 4 assets / 720,456 bytes, with no model/runtime/credential payload detected.
- Failure handled: the first `pnpm check` attempt stopped immediately because the fresh tool session lacked `/tmp/nexora-pnpm-bin/pnpm`, so nested scripts could not resolve `pnpm`. This was not a code/test failure. Recreated a wrapper outside the repository that invokes the pinned Corepack pnpm 9.15.4 and reran the exact check successfully; no repository file was changed to make the wrapper work.
- An auxiliary `@scalar/rust-fmt` WASM parser accepted `account.rs`, `audit.rs`, `billing.rs`, `account_profile_db.rs`, and `plans_billing_quota_db.rs` as Rust 2021 syntax. **No `cargo fmt`, compile, Clippy, Rust unit test, or Rust/PostgreSQL integration test ran.**
- `git diff --check` and `git diff --cached --check` — **PASS**. A focused scan of all **21** untracked UTF-8 text files found no trailing spaces or tabs.

Vite continues to report non-fatal `theme-init.js`/`type="module"`, mixed static/dynamic import and >500 kB chunk warnings. `@types/diff@8.0.0` and `whatwg-encoding@3.1.1` remain deprecation hygiene notices, not current pnpm advisories. The pnpm audit does not cover Cargo/native crates.

### Windows/model/database evidence and artifact checklist

- **Windows:** None obtained. No Windows tool, runner result, parsed script, native build, installer lifecycle, prior-version upgrade, or WebView2 result. Do not substitute Linux checks.
- **Model:** None obtained in Step 5. Registry entries and historical publisher metadata are not local artifact evidence. No `.gguf` or runtime file was found in the desktop tree; no file was downloaded/hashed/loaded, and no real output, inference-time memory measurement, error, or tokens-per-second result exists. The available host-memory/disk measurements above describe only this Linux sandbox, not a supported model target. Preserve both `locally_verified: false` flags and no-cloud-fallback behavior.
- **Database:** None obtained. No database was contacted, no migration was applied, no test was run, and the account-deletion and webhook regressions remain authored/unrun. The integration suite must only run against an explicitly reviewed disposable database whose actual name ends `_test`; billing DB tests perform `TRUNCATE` after the guard.
- **Release artifact:** There is no valid Windows installer. No filename/size/hash, independent checksum comparison, signature, signer identity, payload inspection, provenance record, or attestation exists. **No release manifest was created.**

Evidence checklist for a future authorized candidate (not a manifest):

- [ ] Build a fresh x64 Windows NSIS installer on an authorized runner; record exact file name, size, command, runner/OS, tool versions, exit codes and raw logs.
- [ ] Independently calculate SHA-256 from the produced installer and compare with the generated checksum file; retain both results.
- [ ] Verify actual Authenticode status and signer identity if signed; otherwise record unsigned status without implying approval.
- [ ] Inspect the exact final payload for secrets, development-only files, unexpected runtime/model assets and required notices.
- [ ] Produce provenance/SBOM bound to that exact artifact; record all required test outputs and limitations.
- [ ] Complete Windows clean install/launch/offline/missing-model/upgrade-from-real-prior-version/uninstall/data-preservation/reinstall acceptance.
- [ ] Complete Mini and Sonnet exact-artifact integrity, legal and genuine local-inference acceptance on supported hardware.
- [ ] Complete disposable-DB migrations/auth/billing/account-deletion/webhook tests, full native Rust checks, legal review, support/security contact verification and owner approvals.

### Final blockers and required actions

1. **Step 1:** Provide a reviewed disposable `_test` database and pinned Rust toolchain; run migration/auth/billing/account-deletion/webhook suites and fmt/Clippy; inspect results. Keep payment provider/quota unconfigured and obtain commercial-policy approval.
2. **Step 2:** Revalidate each exact upstream model revision/file/size/digest/license via an authorized network route; obtain owner-approved redistribution rights; acquire only through Nexora's workflow; independently hash bytes and run genuine local inference without a cloud fallback. Leave verification flags false until evidence exists.
3. **Step 3:** Use authorized Windows 10/11 x64 runner/device, prior supported installer, PowerShell parser, pinned toolchain/MSVC/SDK and (if approved) signing certificate; execute build, install/upgrade/uninstall/reinstall and offline checks; record actual checksum/signature.
4. **Step 4:** Complete exact shipped-artifact SBOM/notices and Rust/native/runtime/icon/model license review; obtain approved Nexora license/publisher and legal sign-off; replace support/security placeholders only with owner-verified monitored channels and tested processes.
5. **Step 5:** Re-evaluate only after all blocking evidence is attached to the exact candidate. Do not lower acceptance criteria or publish as part of this work.

### Final decision

**Step 5 gate result: BLOCKED — NOT VERIFIED. Public Beta decision: NO-GO — BLOCKERS REMAIN.** At least Steps 1–3 have no required runtime evidence; Step 4 still lacks complete licensing, legal approval, operational contacts and artifact-linked release evidence. There is no qualified release candidate and no approval package can truthfully include installer or signature details. Stop at the approval boundary and do not publish/distribute.

### Repository state at the Step 5 final gate

Pre-Step-5 snapshot: branch `arena/9ea88704-nexora`, HEAD `8b2e8415bc67ec31e343218364fa4a6ce3741dab`, **0 staged / 114 modified tracked / 21 untracked** paths. Final verification confirms the same branch and HEAD, still **0 staged / 114 modified tracked / 21 untracked**; all 114 baseline modified tracked paths and all 21 baseline untracked paths remain present. `git diff --check` and the staged-diff check passed; all 21 untracked UTF-8 text files were scanned with no trailing whitespace. Step 5 modified already-dirty `services/nexora-auth/src/billing.rs`, `services/nexora-auth/tests/plans_billing_quota_db.rs`, `README.md`, `docs/PRODUCTION_DEPLOYMENT_AND_SECURITY.md`, and existing untracked reports, without replacing unrelated work. No commit, push, merge, tag, publish, release, or billing activation occurred at that Step 5 checkpoint.

## Post-PR continuous-integration update (2026-10-25)

This additive update records the first GitHub Actions run triggered by the requested draft PR #9: [`Public beta readiness and Windows packaging hardening`](https://github.com/cybervault-Hacky/Nexora/pull/9). It supersedes only the earlier Step 5 statement that no CI run existed for this worktree. It does not clear any release gate or change the NO-GO decision.

Run `40177719356` for commit `f731ce9008d435c84a46be0105badecef2f17289` completed with an overall **failure**:

| CI job | Result | Observed boundary |
|---|---|---|
| Frontend — typecheck, tests, production build | **PASS — VERIFIED** | Frozen install, pnpm audit, TypeScript, unit/contract tests, and production build all passed on the exact PR commit. |
| Rust Desktop Core — fmt, check, Clippy, tests (Linux) | **BLOCKED — NOT VERIFIED** | `cargo --locked fmt --all -- --check` failed on formatting differences in `apps/desktop/src-tauri/src/auth/client.rs` and `apps/desktop/src-tauri/build.rs`; typecheck, Rust tests, and Clippy were skipped. |
| Auth, Plans, Billing & Quota Service (PostgreSQL 16) | **BLOCKED — NOT VERIFIED** | `cargo --locked fmt --all -- --check` failed on formatting differences in `services/nexora-auth/src/account.rs`, `services/nexora-auth/src/app.rs`, and `services/nexora-auth/src/billing.rs`; Clippy and unit/HTTP/property/PostgreSQL tests were skipped. The PostgreSQL service container started, but test/migration steps did not run. |
| Windows — compile, Rust tests & NSIS installer verification | **BLOCKED — NOT VERIFIED** | The hosted Windows runner reached the toolchain setup step but failed `Configure the hosted x64 MSVC and Windows SDK environment`; Windows typecheck/tests, NSIS build, checksum creation, and install smoke test were skipped. No installer was produced. |

GitHub's check annotations establish the failing steps and affected formatting files. Retrieval of the detailed Actions log failed because its log-download host was outside the available network route; the underlying Windows bootstrap error is therefore not asserted. These CI results do not replace native/database/Windows acceptance: the relevant Rust tests were skipped, and the Windows job did not reach compilation or packaging.

**Follow-up:** apply the pinned Rust formatter to the affected files and rerun both Rust CI jobs; inspect the Windows bootstrap log through an authorized GitHub Actions view, correct the specific environment failure, and rerun the Windows job. Keep the PR draft and Public Beta **NO-GO — BLOCKERS REMAIN** until all release criteria have their required evidence. No release, billing activation, or database migration was performed by this CI run.

# Nexora Privacy Policy, Terms of Service & Third-Party Licensing Status (Draft)

_Last updated: 2026-10-25 — Pre-launch template with release-candidate, licensing, and support-status corrections. Requires legal review before use; it does not describe an active commercial offer._

---

## 1. Privacy Policy

### 1.1 Intended Local Processing (Not Runtime-Verified)
- **Prompts, source code, workspace files, symbol indexes, diff proposals, and conversation transcripts** are designed to remain local and use the loopback-only `llama.cpp` inference path in the source. No genuine Mini/Sonnet inference or Windows runtime has been verified, and no deployed service was audited; do not publish this draft as a confirmed privacy guarantee until actual behavior and data flows are reviewed.

### 1.2 What the Nexora Account Service Stores
The current schema and service code are designed to store account data such as:
- **Account profile**: Account UUID, display name, email address, email verification status, Argon2id password hash (never plaintext passwords), and linked OAuth subject/email identifiers.
- **Sessions**: SHA-256 hashes of opaque session tokens and expiry timestamps; raw tokens are held locally in the operating system's credential manager.
- **Billing records**: Subscription and payment-order fields plus provider references and deduplicated Stripe webhook event IDs exist in the schema. Existing Stripe-oriented code has not been runtime-validated. Client-selected plan changes and cancellation now fail closed because the provider abstraction has no verified lifecycle operation; focused Rust/PostgreSQL regression tests are not yet executed. These rows must not be interpreted as verified paid/Premium or quota entitlement. No Razorpay implementation is present. Payment card details are not stored in Nexora's PostgreSQL schema; do not infer that an active Stripe offer is available.
- **Daily usage accounting**: UTC per-model counters and reservation records may be stored. Production quota policy defaults to unconfigured, so no daily allowance is active or reported; usage rows do not include prompts or source content.
- **Security audit events**: Redacted security lifecycle records (e.g., sign-in, password reset, plan change, account export, account deletion).

This inventory describes repository schema/code, not a claim that each integration is deployed or active. Confirm production data practices and retention with counsel and the actual deployment before publication.

### 1.3 Data Export & Deletion (Not Runtime-Verified)
- Source includes account-export and account-deletion endpoints and matching UI actions. Step 4 moved the account-deletion row lock, success audit, and cascade into one database transaction and added a focused PostgreSQL regression, but the Rust/database test remains unexecuted. Neither PostgreSQL behavior nor end-to-end retention/deletion semantics are verified. Do not advertise completeness, deletion timing, or retention guarantees until an isolated database run and legal review confirm them.
- Local files and separately acquired model artifacts are outside the account-service deletion path in the source design; actual storage and deletion behavior remains subject to target-platform verification.

---

## 2. Terms of Service & Subscription Terms

1. **Software & Local Execution**: Nexora is designed as desktop coding software for running approved open-weight model artifacts through a local runtime. Genuine Mini/Sonnet inference and Windows runtime behavior have not been verified; this draft is not a performance or compatibility claim. Hardware capability (RAM, CPU/GPU speed, free disk space) will affect any eventual local model performance.
2. **Plans & Daily Quotas**: No Free/Premium plan terms, prices, daily model limits, token allowances, trials, feature-access rules, or reset/billing periods are approved by this work. The production account quota policy defaults to unconfigured, and the user-facing plan page has no active checkout action. Stripe-oriented server handlers and draft plan defaults remain in source; do not enable provider configuration or present those draft values as an offer. A missing quota is neither unlimited paid access nor zero Free access. Local chat remains available signed out and does not imply a subscription or quota entitlement.
3. **Billing, Cancellation & Refunds**: This file is a pre-launch template, not an approved billing offer. Stripe-oriented source code exists but is not runtime-validated; client-selected plan changes and cancellation currently fail closed because the provider lifecycle operations are unsupported. The focused Rust/PostgreSQL regression tests are not yet executed. Keep paid flows disabled until provider lifecycle, entitlement, and legal evidence are approved and verified. Subscription rows are not reliable paid/Premium/quota evidence. Razorpay was blocked and is not implemented. Do not rely on renewal, cancellation, refund, or paid-access statements here until the provider flow and legal terms are approved and verified.

---

## 3. Third-Party Software, Model, Runtime & Asset Licensing — Incomplete

**Distribution is blocked.** The assertions in the earlier draft about the `llama.cpp` MIT license and the Qwen model Apache-2.0 licenses are withdrawn as verified licensing conclusions. They were not established from complete metadata and exact redistributed artifacts. The source-tree license labels below are not legal approval or evidence of redistribution rights.

- **Frontend runtime dependencies:** A partial metadata/notice inventory was prepared for the locked production JavaScript graph (`@tauri-apps/api`, `diff`, `react`, `react-dom`, and `scheduler`), using the installed package metadata and license files. See [`THIRD_PARTY_NOTICES_DRAFT.md`](THIRD_PARTY_NOTICES_DRAFT.md). That document is explicitly incomplete and is not a distribution-ready notice.
- **Native desktop and auth-service Rust dependencies:** `apps/desktop/src-tauri/Cargo.lock` contains 460 locked packages and `services/nexora-auth/Cargo.lock` contains 249. Lockfiles identify versions/checksums but do not provide license expressions or license texts. The complete native/backend license inventory and legal compatibility review remain outstanding.
- **Tauri JavaScript API:** The installed `@tauri-apps/api@2.12.4` package declares `Apache-2.0 OR MIT` and includes SPDX metadata. Its package/version/license metadata is recorded in the draft notice; this does not cover the separate Tauri Rust dependency tree.
- **`llama-server` / `llama.cpp`:** The runtime is not bundled and is supplied separately by the user. The exact runtime binary, source revision, build provenance, license text, and required notice delivery have not been verified here. No MIT license claim is made for the separately supplied runtime.
- **Model profiles and weights:** The model registry currently records `Apache-2.0` and points at upstream license URLs for two Qwen GGUF profiles. Those fields and URLs have not been independently checked against the exact quantized files and license texts in this review. Neither model is bundled, and neither the base model's public source license nor a registry label establishes rights to redistribute the converted/quantized artifact. Exact-artifact licensing and redistribution permissions remain unresolved; keep both `locally_verified: false` and do not bundle or redistribute either model.
- **Icons and other assets:** The Windows PNG/ICO icon files are present, but their author, source, and redistribution license were not verified. No font assets were found in the searched desktop UI paths; this is not a comprehensive packaged-binary inventory.
- **Project license and ownership:** No approved top-level Nexora `LICENSE`, complete root `THIRD_PARTY_NOTICES`, verified legal publisher identity, or legal approval was found. Do not assert proprietary ownership or authorize distribution from the current metadata.

The notice draft is a preparation artifact only. Complete the exact shipped-binary/SBOM review, verify every required license and attribution, establish asset provenance, resolve model/runtime rights, and obtain legal review before any public release.

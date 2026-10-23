# Nexora Privacy Policy, Terms of Service & Open-Source Attribution

_Last updated: 2026-10-22 — Template prepared for legal review prior to commercial launch._

---

## 1. Privacy Policy

### 1.1 What Stays Exclusively on Your Device
- **Prompts, source code, workspace files, symbol indexes, diff proposals, and conversation transcripts** remain stored on your local computer and are processed exclusively by your local `llama.cpp` runtime (`127.0.0.1`). Nexora does **not** upload source code or chat transcripts to Nexora servers or third-party AI APIs.

### 1.2 What the Nexora Account Service Stores
When you create an account or sign in, `services/nexora-auth` stores only:
- **Account profile**: Account UUID, display name, email address, email verification status, Argon2id password hash (never plaintext passwords), and linked OAuth subject/email identifiers.
- **Sessions**: SHA-256 hashes of opaque session tokens and expiry timestamps (stored locally on your device in the operating system's credential manager).
- **Billing & entitlements**: Active subscription tier, billing period dates, provider customer/subscription references, payment order status, and deduplicated Stripe webhook event IDs. Payment card details are collected directly by **Stripe** and never touch Nexora servers.
- **Daily model quota counters**: Per-day (`UTC`), per-model (`nexora-mini`, `nexora-sonnet`) count of completed/reserved generations used solely to enforce your plan's daily allowance (zero prompt content).
- **Security audit events**: Redacted security lifecycle records (e.g., sign-in, password reset, plan change, account export, account deletion).

### 1.3 Data Export & Right to Deletion
- **Portable Data Export (`GET /v1/account/export` — Account -> Export JSON)**: Download a complete machine-readable JSON archive of all server-stored account, subscription, order, and quota records at any time.
- **Permanent Account Deletion (`POST /v1/account/delete` — Account -> Delete account)**: Permanently deletes your user record, credentials, sessions, OAuth links, subscriptions, orders, and quota counters via database `ON DELETE CASCADE`, recording only a non-identifying audit timestamp. Local files and GGUF models on your PC remain under your control.

---

## 2. Terms of Service & Subscription Terms

1. **Software & Local Execution**: Nexora provides desktop coding software that executes approved open-weight models locally on your hardware. Hardware capability (RAM, CPU/GPU speed, free disk space) determines local model performance.
2. **Plans & Daily Quotas**: Plans (`Free`, `Pro`, `Team`) govern daily per-model generation allowances reset at `00:00 UTC`. Prices and allowances displayed at checkout are enforced authoritatively by the server.
3. **Billing, Cancellation & Refunds**: Paid subscriptions renew automatically on the selected billing interval (`monthly` or `yearly`) via Stripe until cancelled. You may schedule cancellation at the end of the current billing period or cancel immediately at any time from **Plans & Billing** inside the app. Refunded charges automatically revoke paid plan entitlements back to `Free`.

---

## 3. Third-Party Open-Source Software & Model Attribution

- **Local Inference Engine**: `llama.cpp` (`llama-server`) — Licensed under the **MIT License** (`https://github.com/ggml-org/llama.cpp`).
- **Approved Local Model Profiles**:
  - **Nexora Mini**: `Qwen/Qwen2.5-Coder-7B-Instruct` (`Q4_K_M` GGUF) — Licensed under **Apache License 2.0** by the Qwen Team, Alibaba Group.
  - **Nexora Sonnet**: `Qwen/Qwen2.5-Coder-14B-Instruct` (`Q4_K_M` GGUF) — Licensed under **Apache License 2.0** by the Qwen Team, Alibaba Group.
- **Desktop Framework & Libraries**: Tauri 2 (`Apache-2.0 OR MIT`), React 19 (`MIT`), Axum / Tokio / SQLx (`MIT`), Argon2 RustCrypto (`Apache-2.0 OR MIT`).

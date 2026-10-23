# Nexora — Windows Desktop AI Coding Assistant

Nexora is an integrated Windows desktop AI coding assistant combining **100% local on-device GGUF model execution** (`Nexora Mini` 7B and `Nexora Sonnet` 14B via `llama.cpp`) with a **server-authoritative authentication, subscription plans, billing, and daily quota backend** (`services/nexora-auth` on Rust + Axum + SQLx + PostgreSQL).

All source files, workspace scans, AST symbols, diff previews, conversation transcripts, and prompt contexts stay strictly on your local Windows machine and never traverse external cloud AI APIs.

---

## 1. Architecture Overview

```text
┌──────────────────────────────────────────────────────────────────────────────┐
│                         Nexora Windows Desktop Window                        │
│                                                                              │
│  ┌────────────────────────────────────────────────────────────────────────┐  │
│  │        React 19 + TypeScript Desktop UI (Light & Dark Neutral UI)      │  │
│  │  - Direct-to-Chat launch (no welcome screen, no separate auth window)  │  │
│  │  - Floating AuthModal over Chat preserving unsent prompt across login  │  │
│  │  - Claude-style Model Selector, Streaming Code Blocks, Plans & Account │  │
│  └───────────────────────────────────┬────────────────────────────────────┘  │
│                                      │ Typed Tauri IPC (`invoke`)            │
│                                      ▼                                       │
│  ┌────────────────────────────────────────────────────────────────────────┐  │
│  │                     Tauri 2 Native Core (`src-tauri`)                  │  │
│  │  - Workspace Scanner, AST Symbol Indexer, Atomic Patch Engine          │  │
│  │  - Model Verifier (SHA-256 + GGUF v3), Hardware Fit & Readiness Gate   │  │
│  │  - Process Supervisor (`llama-server` on 127.0.0.1 + 32-byte keyfile)  │  │
│  │  - OS Credential Vault (`keyring` / Windows Credential Manager)        │  │
│  │  - Typed Auth, Account, Plans, Billing & Server Quota Client           │  │
│  └───────────────┬───────────────────────────────────────┬────────────────┘  │
│                  │ Loopback SSE (127.0.0.1:ephemeral)    │ HTTPS JSON REST   │
│                  ▼                                       ▼                   │
│  ┌───────────────────────────────────┐   ┌────────────────────────────────┐  │
│  │   Local `llama-server` Sidecar    │   │     `services/nexora-auth`     │  │
│  │  - Local Qwen2.5-Coder 7B / 14B   │   │  - Axum + SQLx + PostgreSQL 16 │  │
│  │  - Zero cloud prompt transmission │   │  - Email/Password + Google PKCE│  │
│  └───────────────────────────────────┘   │  - Plans, Stripe Billing &     │  │
│                                          │    Signed Webhook Idempotency  │  │
│                                          │  - UTC Daily Quota Reservations│  │
│                                          └────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────────────────────┘
```

---

## 2. Core Product Capabilities

### 2.1 Direct-to-Chat Desktop Experience (`#/chat`)
- Opens directly into the primary desktop Chat workspace on every launch — no marketing splash screen, no separate sign-in page, and no popup auth window.
- Clean, calm neutral desktop palette in both **Light** and **Dark** themes with WCAG AA contrast (`>= 4.5:1`), restrained solid borders, readable typography, and zero glassmorphism, neon glow, or gratuitous motion.
- When an unauthenticated user submits a prompt, a centred **floating authentication modal** (`Sign in`, `Create account`, `Continue with Google`, inline email verification, and password recovery) opens directly over the conversation, preserves the exact unsent prompt text, and seamlessly continues in the same chat immediately after authentication.

### 2.2 Local On-Device AI Models (`#/models` & Header Model Selector)
- **Nexora Mini** (`qwen2.5-coder-7b-instruct-q4_k_m.gguf`) and **Nexora Sonnet** (`qwen2.5-coder-14b-instruct-q4_k_m.gguf`) with pinned SHA-256 verification, GGUF v3 header checks, live available-RAM + free-disk checks, download progress/cancellation, explicit load/unload controls, and **strict no-silent-fallback attribution**.
- Streaming Server-Sent Events (SSE) token delivery, fenced code blocks with copy controls, immediate generation cancellation (`Stop`), truncation/crash detection (never saving partial crashed streams as complete), and persistent local conversation history.

### 2.3 Server-Side Authentication, Plans, Billing & Daily Usage Limits (`services/nexora-auth`, `#/plans`, `#/account`)
- **Authentication & Security**: Argon2id password hashing, opaque rotating session tokens stored in OS keyring, email verification, password reset, Google OAuth 2.0 Authorization Code + S256 PKCE with loopback callback (`127.0.0.1:<port>/callback`), explicit Google account linking, structured log secret scrubbing, and endpoint rate limiting.
- **Configurable Plans & Stripe Billing**: Server-authoritative plan catalogue (`free`, `pro`, `team` with env-configurable pricing — `$0`, `$19/mo` or `$190/yr`, `$39/mo` or `$390/yr` — and per-model daily quotas `25/5`, `300/100`, `1000/300`; all proposed defaults documented for review in `.env.example`), idempotent Stripe Checkout session creation (`POST /v1/billing/checkout`), server-side order verification (`POST /v1/billing/verify`), RFC-compliant `Stripe-Signature` HMAC-SHA256 webhook verification with replay window & event-id deduplication (`POST /v1/billing/webhooks/stripe`), plan changes (`POST /v1/billing/change-plan`), subscription cancellation (`POST /v1/billing/cancel`), and Billing Portal sessions (`POST /v1/billing/portal`).
- **Backend-Authoritative Daily Quotas**: Atomic PostgreSQL row-locked (`SELECT ... FOR UPDATE`) per-user, per-model (`nexora-mini` / `nexora-sonnet`), UTC-day (`00:00:00Z`) quota reservation (`POST /v1/usage/reserve`), completion commit (`POST /v1/usage/commit`), failed-generation release (`POST /v1/usage/release`), and live remaining/reset summary (`GET /v1/usage`). Local offline work (`workspace scan`, `symbol search`, `offline local usage ledger`) never breaks when offline, and server-authoritative quota states are surfaced honestly without fabrication.
- **Account Self-Service & GDPR Controls (`#/account`, `#/settings`)**: Display name updates (`PATCH /v1/account/profile`), password rotation (`POST /v1/auth/password-change`), Google identity linking (`POST /v1/auth/google/link`), full JSON account data export (`GET /v1/account/export`), and explicit confirmation-gated account deletion (`POST /v1/account/delete`).

---

## 3. Environment Configuration & Secret Hygiene

1. Copy `.env.example` to `.env` for local development (`.gitignore` excludes `.env` and all secret/key files):
   ```bash
   cp .env.example .env
   ```
2. Review every section in [`.env.example`](.env.example):
   - **Never put server secrets, OAuth client secrets, database URLs, or Stripe keys into `VITE_*` variables or frontend files.**
   - `services/nexora-auth` validates configuration at startup and **fails fast in `NEXORA_ENV=production`** if `NEXORA_AUTH_DATABASE_URL` is missing, if configured secrets/URLs still contain placeholder values (`REPLACE_WITH`, `CHANGE_ME`, `nexora.local`), or if `STRIPE_SECRET_KEY` uses `sk_test_*`.
3. Full deployment, TLS reverse-proxy, webhook, signing, and rollback instructions are in [`docs/PRODUCTION_DEPLOYMENT_AND_SECURITY.md`](docs/PRODUCTION_DEPLOYMENT_AND_SECURITY.md), and legal/privacy documentation is in [`docs/LEGAL_PRIVACY_TERMS.md`](docs/LEGAL_PRIVACY_TERMS.md).

---

## 4. Development, Build & Verification Commands

### 4.1 Frontend & Cross-Language Contract Suite
```bash
pnpm install --frozen-lockfile
pnpm typecheck          # TypeScript strict typecheck
pnpm test               # 85 Vitest suites (792 unit, UI, IPC contract & build config tests)
pnpm build              # Production Vite frontend bundle -> apps/desktop/ui/dist
```

### 4.2 Desktop Rust Core (`apps/desktop/src-tauri`)
```bash
cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml
cargo clippy --manifest-path apps/desktop/src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml    # 366 tests
```

### 4.3 Auth, Billing & Quota Service (`services/nexora-auth`)
```bash
cargo clippy --manifest-path services/nexora-auth/Cargo.toml --all-targets -- -D warnings
NEXORA_AUTH_TEST_DATABASE_URL="postgres://postgres@127.0.0.1:5432/postgres" \
  cargo test --manifest-path services/nexora-auth/Cargo.toml -- --include-ignored --test-threads=1   # 184 tests
```

### 4.4 Windows 10/11 NSIS Installer Build & Smoke Verification
Run on a Windows 10 (22H2+) or Windows 11 x64 host with Rust MSVC, Node.js 22 LTS, pnpm 9, and Visual Studio 2022 C++ Build Tools:
```powershell
# Reproducible release build + secret scan + optional Authenticode signing + SHA256SUMS.txt
.\scripts\windows\build-installer.ps1

# Automated per-user silent install, binary launch sanity check, and silent uninstall smoke test
.\scripts\windows\smoke-verify.ps1
```

See [`docs/VERIFICATION_REPORT_2026_10_22.md`](docs/VERIFICATION_REPORT_2026_10_22.md) for the complete executed `PASS` / `FAIL` / `BLOCKED` verification matrix.

---

## 5. License

Proprietary / All rights reserved by repository owner unless otherwise specified; third-party Apache-2.0 and MIT runtime/model notices are documented in [`docs/LEGAL_PRIVACY_TERMS.md`](docs/LEGAL_PRIVACY_TERMS.md).

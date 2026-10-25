# Nexora — Windows Desktop AI Coding Assistant

> **Pre-release status (2026-10-25): NO-GO.** Step 1 remains **IMPLEMENTED — VALIDATION BLOCKED**; Step 2 remains **PARTIAL — IMPLEMENTATION COMPLETE, VERIFICATION BLOCKED**; Step 3 remains **PARTIAL — IMPLEMENTATION COMPLETE, WINDOWS VERIFICATION BLOCKED**; Step 4 remains **PARTIAL — EVIDENCE INCOMPLETE**. Step 5 final gate is **BLOCKED — NOT VERIFIED** and is not authorization to publish. The current pnpm checks and audits pass, and Step 5 added fail-closed webhook timestamp and duplicate-payload checks, but native Rust/PostgreSQL validation, Windows packaging/signing, genuine model inference, complete licensing/legal review, and verified support/security operations remain blocked. This README describes repository design and source features, not a released or fully validated product. See [Phase 18](docs/PHASE_18_INTEGRATION_VERIFICATION.md), [Phase 19](docs/PHASE_19_WINDOWS_PACKAGING.md), [Phase 20](docs/PHASE_20_PUBLISHING_PREPARATION.md), the [Phase 21 final assessment](docs/PHASE_21_FINAL_APPROVAL_AND_RELEASE.md), the [current Windows Public Beta preparation report](docs/PUBLIC_BETA_PREPARATION_WINDOWS.md), [security reporting status](SECURITY.md), [support/diagnostics prerequisites](docs/SUPPORT_AND_DIAGNOSTICS.md), and the [incomplete third-party notices](docs/THIRD_PARTY_NOTICES_DRAFT.md).

Nexora is a Windows-first Tauri desktop application designed around signed-out local chat and loopback-only inference using a separately supplied `llama.cpp` runtime. The Mini and Sonnet profiles have pinned artifact metadata, but neither genuine model has been locally verified (`locally_verified: false`); neither runtime nor model weights are bundled. Optional account-service code is separate and has not been validated as a live service. Payment-provider composition and account quota policy remain unconfigured.

The architecture is designed to keep prompts, source files, workspace scans, and conversation transcripts out of hosted AI APIs by using local inference. This is a source/design statement, not evidence of native Windows runtime behavior. Account operations are separate and may exchange account metadata with a configured service.

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
│  │  - OS credential-vault integration (Windows behavior unverified)        │  │
│  │  - Optional account-service code        │  │
│  └───────────────┬───────────────────────────────────────┬────────────────┘  │
│                  │ Loopback SSE (127.0.0.1:ephemeral)    │ HTTPS JSON REST   │
│                  ▼                                       ▼                   │
│  ┌───────────────────────────────────┐   ┌────────────────────────────────┐  │
│  │   Local `llama-server` Sidecar    │   │     `services/nexora-auth`     │  │
│  │  - Model profile metadata only   │   │  - Axum + SQLx + PostgreSQL 16 │  │
│  │  - Inference path unverified │   │  - Auth/OAuth source unverified│  │
│  └───────────────────────────────────┘   │  - Stripe-oriented source only │  │
│                                          │    (provider unconfigured)     │  │
│                                          │  - Quota policy unconfigured  │  │
│                                          └────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────────────────────┘
```

---

## 2. Core Application Design (Not Release-Verified)

### 2.1 Direct-to-Chat Desktop Experience (`#/chat`)
- Opens directly into the primary desktop Chat workspace on every launch — no marketing splash screen, no separate sign-in page, and no popup auth window.
- Light and Dark themes use a neutral palette, restrained borders, and readable typography. WCAG AA contrast has not been independently verified.
- Signed-out local chat and conversation history do not require an account. Sign-in and account screens are separate optional flows; prompts are not gated on authentication. Live account/OAuth integration remains unverified.

### 2.2 Local On-Device AI Models (`#/models` & Header Model Selector)
- Source defines **Nexora Mini** (`qwen2.5-coder-7b-instruct-q4_k_m.gguf`) and **Nexora Sonnet** (`qwen2.5-coder-14b-instruct-q4_k_m.gguf`) profiles, pinned SHA-256 metadata, GGUF header checks, readiness checks, and explicit user-initiated acquisition. These mechanisms have not been verified against a genuine downloaded artifact or a Windows inference run; both profiles remain `locally_verified: false`.
- The source includes streaming, cancellation, crash/truncation handling, and local conversation-history paths. Frontend tests are mocked; no genuine Mini/Sonnet inference has been recorded.

### 2.3 Optional Account-Service Code (Not Release-Verified)
- **Authentication/account source** includes email/password and Google PKCE flows, account operations, and OS credential-storage integration. Backend/provider interoperability and native credential-vault behavior have not been verified; signed-out local chat remains independent.
- **Plans and payments are not an active offer.** The service has Stripe-oriented source, but production composition uses an unconfigured provider. G-01 source paths now fail closed for client-selected plan changes and unsupported cancellation; the Rust/PostgreSQL regression tests have not run in this environment. Do not treat a client plan request as entitlement evidence. Razorpay is not implemented. Pricing, quotas, trials, Premium terms, and paid benefits are unapproved; no draft defaults in `.env.example` should be presented as offers.
- **Quota policy is unconfigured.** No Free/Premium limit, token allowance, reset period, or paid entitlement is approved or active. Do not treat subscription rows or provider/backend failures as entitlement evidence.
- These integrations are not evidence of a deployed service, verified account behavior, or production readiness. See the Phase 18 verification report and the pre-launch legal template for current limitations.

---

## 3. Environment Configuration & Secret Hygiene

1. Copy `.env.example` to `.env` for local development (`.gitignore` excludes `.env` and all secret/key files):
   ```bash
   cp .env.example .env
   ```
2. Review every section in [`.env.example`](.env.example):
   - **Never put server secrets, OAuth client secrets, database URLs, or Stripe keys into `VITE_*` variables or frontend files.**
   - `services/nexora-auth` validates configuration at startup and **fails fast in `NEXORA_ENV=production`** if `NEXORA_AUTH_DATABASE_URL` is missing, if configured secrets/URLs still contain placeholder values (`REPLACE_WITH`, `CHANGE_ME`, `nexora.local`), or if `STRIPE_SECRET_KEY` uses `sk_test_*`.
3. Deployment, TLS reverse-proxy, webhook, signing, and rollback guidance is in [`docs/PRODUCTION_DEPLOYMENT_AND_SECURITY.md`](docs/PRODUCTION_DEPLOYMENT_AND_SECURITY.md); it is reference material, not evidence of a deployed service or release approval. The pre-launch legal/privacy template is [`docs/LEGAL_PRIVACY_TERMS.md`](docs/LEGAL_PRIVACY_TERMS.md).

---

## 4. Development, Build & Verification Commands

### 4.1 Frontend & Cross-Language Contract Suite
```bash
pnpm install --frozen-lockfile
pnpm typecheck          # TypeScript strict typecheck
pnpm test               # Vitest UI, unit, and contract tests
pnpm build              # Production Vite frontend bundle -> apps/desktop/ui/dist
```

### 4.2 Desktop Rust Core (`apps/desktop/src-tauri`)
```bash
cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml
cargo clippy --manifest-path apps/desktop/src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml    # Rust unit tests (not run in this sandbox)
```

### 4.3 Auth, Billing & Quota Service (`services/nexora-auth`)
```bash
cargo clippy --manifest-path services/nexora-auth/Cargo.toml --all-targets -- -D warnings
# CAUTION: isolated throwaway test DB only; integration tests include destructive cleanup.
# Review the URL and confirm the target is disposable before running anything.
NEXORA_AUTH_TEST_DATABASE_URL="postgres://postgres@127.0.0.1:5432/nexora_test?sslmode=disable" \
  cargo test --manifest-path services/nexora-auth/Cargo.toml -- --include-ignored --test-threads=1
```

### 4.4 Windows 10/11 NSIS Installer Build & Smoke Verification
Run on a Windows 10 (22H2+) or Windows 11 x64 host with the pinned Rust 1.93.0 MSVC toolchain, Node.js 22 LTS, pnpm 9.15.4, and Visual Studio 2022 C++ Build Tools:
```powershell
# Guarded native Windows build + secret scan + optional certificate-store signing + SHA256SUMS.txt
.\scripts\windows\build-installer.ps1

# Automated per-user silent install, binary launch sanity check, and silent uninstall smoke test
.\scripts\windows\smoke-verify.ps1
```

Current verification evidence is recorded in [Phase 18](docs/PHASE_18_INTEGRATION_VERIFICATION.md), [Phase 19](docs/PHASE_19_WINDOWS_PACKAGING.md), and [Phase 20](docs/PHASE_20_PUBLISHING_PREPARATION.md). The older [2026-10-22 report](docs/VERIFICATION_REPORT_2026_10_22.md) is historical and superseded; do not use it as current release evidence.

---

## 5. License, Support & Security Contact Status

No approved top-level `LICENSE` or complete distributable third-party notices file is present. [`docs/THIRD_PARTY_NOTICES_DRAFT.md`](docs/THIRD_PARTY_NOTICES_DRAFT.md) covers only a subset of frontend runtime dependencies and is explicitly not distribution-ready. The legal/privacy document is a pre-launch template; exact native Rust, model/runtime, icon, and redistribution-license review remains incomplete. No legal publisher, signing identity, or owner-approved model redistribution rights are established here.

No verified support contact or private security-reporting route is recorded. [`SECURITY.md`](SECURITY.md) and [`docs/SUPPORT_AND_DIAGNOSTICS.md`](docs/SUPPORT_AND_DIAGNOSTICS.md) contain explicit release-blocking placeholders rather than invented contact details. Do not treat them as operational channels or distribute Nexora until the owner supplies and verifies the required contacts and processes.

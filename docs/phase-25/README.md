# Phase 25: Developer Account & Founder Control Centre

## 1. Overview & Objectives

Phase 25 implements the **Developer Account & Founder Control Centre** for Nexora. It expands the desktop authentication system introduced in Phase 24 with dedicated developer identity management, password security updates, session revocation, and a real-time observability console for core contributors and founders.

Importantly, Phase 25 preserves Nexora's core invariant: **Complete Offline-First Workspace Sovereignty**. Local GGUF models, inference engines, context budget planning, code intelligence, symbol graphs, and project files remain 100% functional without an account or network connection.

---

## 2. Architecture & Components

```
+-----------------------------------------------------------------------------------+
|                               Nexora WebView UI                                   |
|                                                                                   |
|  +--------------------+  +--------------------+  +----------------------------+  |
|  |    AccountPage     |  |    FounderPage     |  |        SettingsPage        |  |
|  | - Developer Profile|  | - Engine Lifecycle |  | - Interface Preferences    |  |
|  | - Password Change  |  | - IPC Telemetry    |  | - Account Quick Link       |  |
|  | - Email Verify Req |  | - OS Vault Audit   |  | - Founder Mode Toggle      |  |
|  | - OS Session Exit  |  | - Auth Probe       |  |                            |  |
|  +---------+----------+  +---------+----------+  +--------------+-------------+  |
|            |                       |                            |                 |
|            +-----------------------+----------------------------+                 |
|                                    |                                              |
|                   useAuth Hook & lib/telemetry.ts                                 |
|                  (In-Memory Traffic Logger - Privacy Safe)                        |
|                                    |                                              |
+------------------------------------+----------------------------------------------+
                                     | Tauri IPC (`invoke`)
+------------------------------------+----------------------------------------------+
|                              Desktop Core (Rust)                                  |
|                                                                                   |
|  +-----------------------------------------------------------------------------+  |
|  | Tauri Command Handlers (`apps/desktop/src-tauri/src/auth/commands.rs`)      |  |
|  | - `auth_change_password` (validates session, updates remote credential)     |  |
|  | - `get_developer_diagnostics` (inspects arch, vault provider, CSP, config)  |  |
|  +-------------------------------------+---------------------------------------+  |
|                                        |                                          |
|            +---------------------------+---------------------------+              |
|            |                                                       |              |
|  +---------v----------+                                  +---------v-----------+  |
|  |  Credential Vault  |                                  |   Remote Auth API   |  |
|  | Windows Credential |                                  |  (nexora-auth /     |  |
|  |   Manager (DPAPI)  |                                  |   127.0.0.1:8787)   |  |
|  +--------------------+                                  +---------------------+  |
+-----------------------------------------------------------------------------------+
```

---

## 3. Tauri IPC Commands & Contract Types

### New IPC Commands Registered in `lib.rs`

1. **`auth_change_password(request: ChangePasswordPayload) -> Result<AuthStatusResponse, AuthError>`**
   - Validates that an active OS session exists.
   - Forwards request with the encrypted access token to `POST /api/v1/auth/password/change`.
   - Immediately purges plaintext password strings from memory upon completion.
   - Returns typed confirmation status or mapped domain errors (`unauthorized`, `invalid_credentials`, `weak_password`).

2. **`get_developer_diagnostics() -> Result<DeveloperDiagnostics, AuthError>`**
   - Provides internal operational diagnostics without exposing tokens, keys, or passwords.
   - Inspects target platform (`std::env::consts::OS`), architecture (`std::env::consts::ARCH`), credential vault provider, CSP enforcement status, and auth service target URL.

### TypeScript Contract DTOs (`apps/desktop/ui/src/lib/auth.ts`)

```typescript
export interface ChangePasswordPayload {
  currentPassword: string;
  newPassword: string;
}

export interface DeveloperDiagnostics {
  platform: string;
  arch: string;
  appVersion: string;
  credentialVault: string;
  authServiceUrl: string;
  cspEnforced: boolean;
  offlineCapable: boolean;
}
```

---

## 4. Security & Privacy Guarantees

1. **Zero Credential Exposure in Logs or WebView Storage**
   - Passwords and auth tokens are strictly forbidden from localStorage, sessionStorage, IndexedDB, or browser cookies.
   - Password input fields in `AccountPage` clear immediately after submission.
2. **Privacy-Preserving Telemetry**
   - The in-memory IPC telemetry recorder (`telemetry.ts`) logs only command names, durations, and high-level status flags.
   - Error summaries are capped and scrubbed; passwords, tokens, source code, and model prompts are never captured.
   - Telemetry is held strictly in volatile memory (capped at 100 recent entries) and is never persisted to disk or sent across the network.
3. **CSP Lockdown**
   - Content Security Policy remains strictly enforced (`connect-src ipc: http://ipc.localhost`).
   - Arbitrary network fetches from WebView scripts remain impossible; all backend communication is routed through Rust IPC command boundaries.

---

## 5. Offline-First Workspace Sovereignty

Nexora guarantees that developers can work in completely air-gapped, offline environments:
- **No Sign-In Wall**: The app launches directly into Welcome/Workspace modes without requiring authentication.
- **Local Guest Mode**: When unauthenticated, the `AccountPage` displays a clear status notice: *"You are currently working in Local Guest Mode. All AI chat, project exploration, local inference, code intelligence, and settings remain 100% available without an account."*
- **Offline AI Independence**: GGUF local model execution, AST parsing, symbol graph indexing, retrieval-augmented patch generation, and project exploration require zero remote services.

---

## 6. Verification Results

### Linux Host Verification (Sandbox Environment)
- **TypeScript Typecheck**: `pnpm typecheck` passed with 0 errors.
- **Production Build**: `pnpm build` passed with 0 errors (115 modules transformed into `dist/`).
- **Comprehensive Test Suite**: `pnpm test` executed with **57 test files and 609 tests passing (0 failures)**.
  - Dedicated Phase 25 integration tests: `App.phase25Integration.test.tsx` (4/4 passed).
  - Account profile & password tests: `AccountPage.test.tsx` (4/4 passed).
  - Founder console & telemetry tests: `FounderPage.test.tsx` (3/3 passed).
  - In-memory telemetry unit tests: `telemetry.test.ts` (4/4 passed).
  - IPC contract tests: `auth.test.ts` (13/13 passed).
  - Core Rust/UI parity contract tests: `rustContract.test.ts` (22/22 passed).
  - Offline AI regression tests: `App.offlineRegression.test.tsx` (5/5 passed).

### Remaining Windows Verification Steps

*Note: Per standing engineering constraints, Linux test passage does NOT constitute proof of Windows runtime verification.*

To verify on a native Windows x64 machine:

1. **Prerequisites**:
   - Windows 10/11 x64 with WebView2 Evergreen Runtime.
   - Rust toolchain `stable-x86_64-pc-windows-msvc`.
   - Node.js 20+ and pnpm.
2. **Compilation**:
   ```powershell
   pnpm install
   pnpm build
   pnpm --filter @nexora/desktop tauri build --debug
   ```
3. **Runtime Credential & Profile Audit**:
   - Launch `target\debug\nexora.exe`.
   - Sign in with developer credentials.
   - Navigate to `#/account` and verify active session identity and verified badge.
   - Execute password change in `#/account` and verify status message.
   - Open Windows Credential Manager (`control keymgr.dll`) under **Windows Credentials** -> inspect `nexora-desktop/session` entry.
   - Click **Sign out** on `#/account` -> verify credential entry is immediately removed from Windows Credential Manager.
4. **Founder Control Centre Inspection**:
   - Navigate to `#/settings` -> toggle **Founder Control Centre**.
   - Click **Open Console** or click **Founder Control** in the sidebar.
   - Verify architecture displays `windows (x86_64)` and credential storage displays `Windows Credential Manager (DPAPI)`.
   - Trigger local model or workspace operations and verify live IPC dispatch events in the telemetry monitor.

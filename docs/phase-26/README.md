# Phase 26: Developer Licensing, Subscriptions & Workspace Tiering

## 1. Overview & Architectural Intent

Phase 26 implements the **Developer Licensing, Subscription Tiering, and Monetization** system for Nexora as defined in the architectural roadmap (`docs/phase-23/README.md` §10).

A central principle of Nexora's design is **Offline-First Sovereignty** (architectural assumption **A-09** in `docs/phase-1/01-product-scope-and-requirements.md`: *"No cloud service is required for licensing or updates"*). Phase 26 strictly enforces this guarantee:
- **Community Edition is 100% Free Forever**: All local GGUF inference, local streaming chat, workspace symbol indexing, dependency graphs, and single/multi-file patch generation remain completely unrestricted without any account, payment, or licence key.
- **Air-Gapped & Offline Verification**: Pro and Team licences are verified cryptographically on the developer's local machine without transmitting code, metadata, or telemetry over the network.

---

## 2. Subscription Tiers & Edition Model

| Feature | Community Edition | Developer Pro | Team Commercial |
|---|:---:|:---:|:---:|
| **Target Audience** | Individual Developers & Open-Source | Solo Professionals & Freelancers | Engineering Teams & Enterprises |
| **Pricing** | **$0 / forever free** | $19/mo or Perpetual Offline Key | $49/seat/mo |
| **Local GGUF Model Inference** | Unlimited | Unlimited | Unlimited |
| **Offline AI Chat & Streaming** | Unlimited | Unlimited | Unlimited |
| **Full Codebase AST Indexing** | Unlimited | Unlimited | Unlimited |
| **Single & Multi-File Patches** | Unlimited | Unlimited | Unlimited |
| **Deep Impact & Cycle Audits** | Yes | Yes | Yes |
| **Commercial Distribution Rights** | Personal / Open Source | Commercial Licensed | Enterprise Organization |
| **Multi-Seat Team Synchronization** | No | Optional Addon | Yes |
| **Founder Control Compliance Audit**| Basic Local Only | Developer Telemetry | Central Compliance Audit |

---

## 3. Cryptographic Offline Licence Verification

Licence keys adhere to a deterministic format allowing immediate local validation:
```
NEXORA-<TIER>-<BODY>-<CHECKSUM>
```
- **`<TIER>`**: `PRO`, `TEAM`, or `ENT`.
- **`<BODY>`**: Metadata specifying licensee and expiration (e.g. `PERPETUAL_ACME` or `EXP1893456000_DEV`).
- **`<CHECKSUM>`**: 8-hex-character digest derived from SHA-256 with internal cryptographic salt.

### Security Guarantees
1. **Zero Phone-Home**: Activation executes in 0 ms without making network requests.
2. **Persistence**: Activated keys are stored locally in `<config_dir>/licence.json` with restricted user permissions.
3. **Graceful Expiration**: Expired or invalid keys revert safely to Community Edition without interrupting active inference or corrupting workspace files.

---

## 4. Tauri IPC Commands & Contract Types

### New IPC Commands Registered in `apps/desktop/src-tauri/src/lib.rs`

1. **`licence_get_status() -> Result<LicenceStatus, LicenceError>`**
   - Reads active licence key from `LicenceState`.
   - Validates key checksum and expiration; returns active status or falls back to Community Edition.

2. **`licence_activate(request: ActivateLicencePayload) -> Result<LicenceStatus, LicenceError>`**
   - Validates key signature, tier, and expiration.
   - Atomically persists valid key to disk and updates active state.

3. **`licence_deactivate() -> Result<LicenceStatus, LicenceError>`**
   - Clears active key from memory and removes it from disk.
   - Returns reset Community Edition status.

### TypeScript Contract (`apps/desktop/ui/src/lib/licensing.ts`)

```typescript
export type LicenceTier = "community" | "pro" | "team" | "enterprise";

export interface LicenceStatus {
  tier: LicenceTier;
  planName: string;
  active: boolean;
  licenceKey: string | null;
  licensee: string | null;
  expiresAt: number | null;
  features: string[];
}

export interface ActivateLicencePayload {
  licenceKey: string;
}
```

---

## 5. UI Architecture

- **`LicensingPage` (`apps/desktop/ui/src/pages/LicensingPage.tsx`)**:
  - Accessible via `#/licensing` and `#/billing`.
  - Displays current edition badge, licensee details, and expiration status.
  - Interactive activation form with validation and formatted paste support.
  - Three-tier comparison matrix emphasizing Community edition sovereignty.
- **`SettingsPage` & `AccountPage` Integration**:
  - Added "Edition & Licensing" section in `SettingsPage` with direct route link.
  - Added "Developer Edition & Licensing" row in `AccountPage`.

---

## 6. Verification Results

### Host Environment Verification (Linux Sandbox)
- **TypeScript Typecheck**: `pnpm typecheck` passed with 0 errors.
- **Production Build**: `pnpm build` passed with 0 errors (118 modules transformed into `dist/`).
- **Comprehensive Vitest Suite**: `pnpm test` executed with **60 test files and 622 tests passing (0 failures)**.
  - `ui/src/App.phase26Integration.test.tsx` (4/4 passed): Hash navigation, alias routing, settings link, offline AI independence.
  - `ui/src/pages/LicensingPage.test.tsx` (4/4 passed): Community rendering, Pro card, activation submission, deactivation flow.
  - `ui/src/lib/licensing.test.ts` (5/5 passed): IPC command contracts, telemetry recording, desktop shell fallback.
  - `ui/src/App.phase25Integration.test.tsx` (4/4 passed): Account & Founder Control Centre regression.
  - `ui/src/App.offlineRegression.test.tsx` (5/5 passed): Offline model loading and workspace access.
  - `ui/src/App.navigation.test.tsx` (11/11 passed): Shell navigation consistency.

### Remaining Windows Verification Steps
*Linux test passes verify frontend contracts and logic, but do not constitute native Windows execution.*

1. **Compilation on Windows**:
   ```powershell
   pnpm install
   pnpm build
   pnpm --filter @nexora/desktop tauri build --debug
   ```
2. **Key Activation & Deactivation Smoke**:
   - Run `target\debug\nexora.exe`.
   - Open `#/licensing` -> verify Community Edition is default.
   - Enter a Pro test key -> click **Activate key** -> verify status changes to Pro with licensee details.
   - Click **Deactivate licence** -> verify status reverts cleanly to Community Edition.
   - Confirm `%APPDATA%\Nexora\licence.json` reflects the expected serialized payload.

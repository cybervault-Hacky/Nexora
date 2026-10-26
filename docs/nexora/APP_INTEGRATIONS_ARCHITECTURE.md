# App Integrations — Shared Architecture Proposal

**Status:** Proposed architecture and requirements only; not implemented or approved for execution.
**Scope:** A reusable integrations surface inside the existing Nexora desktop experience, with Blender as the first candidate connector.
**Related:** [APP_CONNECTOR_CONTRACT.md](APP_CONNECTOR_CONTRACT.md), [BLENDER_INTEGRATION_REQUIREMENTS.md](BLENDER_INTEGRATION_REQUIREMENTS.md), [BLENDER_INTEGRATION_12_PHASE_PLAN.md](BLENDER_INTEGRATION_12_PHASE_PLAN.md), `docs/phase-1/03-application-architecture.md`, `docs/phase-1/04-ai-runtime-and-model-strategy.md`, `docs/phase-1/07-security-and-privacy.md`.

> This document describes a future design. It does not claim that Nexora currently contains an App Integrations page, connector registry, Blender support, or a general external-app execution service.

## 1. Recommendation

Add one **App Integrations** destination to Nexora's existing application shell. It provides a searchable catalogue and common connection/configuration experience for all supported apps. Each card is rendered from a typed connector descriptor and live native status. App-specific configuration appears as an inline expandable section, side panel, or modal on that page—not as a new page per application.

The app page is for discovery, setup, connection health, and permissions. Users perform AI work through the existing **AI Chat**. Chat asks the shared integration service for currently available capabilities, constructs a provider-compatible structured plan, obtains the required consent, and sends validated actions through the selected connector. There is no separate Blender Studio or Blender task page.

The connector boundary belongs in Nexora's privileged Rust/Tauri core. The React renderer displays typed state and submits user intent; it does not communicate directly with Blender or decide permissions. The existing `llama.cpp` inference path remains the local provider. An integration connector is not itself an AI provider and must not change model routing.

## 2. What exists today (confirmed by repository inspection)

- The desktop shell is Tauri 2, React, and TypeScript, and its hash router launches directly into AI Chat. Current primary navigation includes Home, AI Chat, Projects, Code Intelligence, Local Models, Plans, and Settings. There is no integrations route, page, or app catalog.
- The Tauri host registers explicit commands, and the main-window capability file grants a small set of core window permissions. Native functionality is divided into subsystem modules. There is no generic `AppConnector` contract, integration registry, or Blender module.
- Local inference is managed by the existing Rust inference supervisor and the `llama.cpp` sidecar path; the renderer calls typed Tauri commands/channels. Current settings describe local inference and no external AI inference endpoints. No user-configurable external AI provider abstraction/API-key setup was found in the current source.
- Existing media managers and chat status paths are separate, domain-specific systems. They are not an app-integration registry and should not be repurposed as one.
- The repository's Phase 1 architecture/security docs are proposals; source code and current route/command registration are the evidence for existing capabilities. In particular, existing architecture-document pseudocode for provider abstractions or crate boundaries must not be mistaken for implemented code.

## 3. Shared experience and information model

### 3.1 App Integrations page

The proposed page should provide:

- A search field and optional category/status filters over the supported-app catalogue.
- Cards with a locally packaged icon or standard fallback, app name, concise purpose, declared capabilities, supported platform/version range, installation facts, connection state, execution-readiness state, prerequisites, and required permission summary.
- Contextual **Connect**, **Disconnect**, **Reconnect**, and **Configure** actions only when meaningful for that connector and current state. Reconnect targets the last user-approved installation/session; it never silently picks another executable.
- Accessible state labels and explanations. Color is supplementary. Error and unsupported states say what was checked, what was not checked, and the next safe action.
- Inline settings or a shared configuration panel. App-specific forms are declared as typed configuration data or purpose-built controls within that shared shell; an app does not receive a separate management page by default.
- An explicit statement when an application is installed but not connected, connected but not ready, or merely listed as supported.

The page does not run AI tasks or embed another application's full UI. The user returns to AI Chat to request scene work.

### 3.2 Independent state dimensions

Do not collapse installation, connection, and task readiness into one green “ready” badge.

```text
Installation: Installed(path, detectedVersion) | NotFound | Unsupported | Unknown | AccessDenied
Connection:   NotConnected | Connecting | Connected(session) | NeedsSetup
              | Disconnected(reason) | Unsupported(reason) | Error(code, retryable)
Task readiness: Ready(capabilities) | Blocked(reasons) | Unknown(reasons)
```

The UI must expose the requested user-facing states—**Not Connected, Connecting, Connected, Needs Setup, Unsupported, Error, Disconnected**—while retaining the independent facts above. “Not Connected” means no connection has yet been established; “Disconnected” means a previously connected session was intentionally closed or lost. `Connected` reports a live, protocol-valid session; it does not assert that the current app version, scene, provider, permissions, or resources are ready for every task.

Unsupported app versions/platforms are not silently coerced into a supported state. A stale health check is `Unknown` until refreshed, not `Connected` or `Ready` by assumption. A connector may report connected while task readiness is blocked.

## 4. Proposed component architecture

```text
React shell
  ├─ App Integrations page: search/catalog cards/configuration/state
  └─ AI Chat: user intent, consent/plan UI, progress and result summary
           │ typed Tauri commands and bounded event channels
           ▼
Rust IntegrationService (trusted enforcement point)
  ├─ ConnectorRegistry: reviewed IDs, versions, descriptors, handlers
  ├─ Permission/consent gate: user-scoped grants and action risk checks
  ├─ Integration state store: installation facts, user config, connection health
  ├─ Task router: validated capability -> optional connector task executor
  ├─ Progress/cancellation broker: task IDs, bounded events, shutdown cleanup
  └─ Connector adapters (e.g., future Blender connector)
           │ per-connector local protocol; no renderer-to-app socket
           ▼
External application (future Blender + reviewed add-on)
```

- **UI:** Renders authoritative Rust DTOs and submits narrow requests. It cannot load connector code, scan arbitrary paths, open sockets to apps, or grant permissions.
- **IntegrationService:** The single host-side entry point. It validates connector IDs and protocol versions, checks connection state and declared capability, applies permission policy, constrains inputs, associates tasks with cancellation and progress, and writes redacted audit events.
- **ConnectorRegistry:** Maps stable connector IDs to reviewed implementations and descriptors. IDs are stable, namespaced strings (for example, `com.blender.blender`). Connector/version compatibility is checked before a handler is made available.
- **Connector implementation:** Owns app-specific discovery, handshake, health, command mapping, and cleanup. The generic registry/page does not contain Blender-specific UI logic.
- **AI orchestration:** Uses provider/model capabilities to produce a schema-constrained plan; the connector receives only validated action objects, never free-form assistant text or executable code.

### 4.1 Extensibility without per-app pages

A new integration should normally add a descriptor, reviewed adapter, permissions/prerequisites, compatibility rules, and generic configuration schema. The common catalogue and card renderer consume those declarations; they do not need a new route or bespoke lifecycle UI for each app.

Initial implementations should be reviewed adapters shipped with Nexora, not arbitrary dynamically loaded plugins. A future signed connector-package protocol could permit separately versioned connectors, but it is a separate security/supply-chain decision: signature verification, publisher trust, sandboxing, IPC compatibility, revocation, and uninstall must be specified before enabling executable third-party extensions. A manifest is data, not authority to load code. No dynamic plugin loader is proposed for the first connector.

## 5. Chat integration and provider independence

1. The user asks for an operation in the existing AI Chat.
2. Nexora obtains current connector health, the active application/scene summary (only with the relevant read grant), and provider/model capabilities.
3. A planner produces a user-readable plan and typed action list. Invalid or unsupported actions are rejected before reaching the connector.
4. The user reviews the plan and any required write/destructive/network consent. Only then can the task executor begin.
5. Typed progress/results flow back to the same conversation. The connector reports facts and verification evidence; the model cannot declare success without those results.

The planner is provider-neutral. The current local `llama.cpp` path may be used for tasks whose prompt, modality, context, and structured-output requirements it actually supports. Future provider adapters can declare text/image input, structured output or tool-call reliability, context limits, and network/billing characteristics. When a provider lacks a required capability, Nexora must explain the limitation or offer a separate user choice; it must not silently route to a paid cloud model.

No cloud key is required for supported local workflows. Future paid-provider use requires an explicit per-task confirmation that identifies the provider/model, data to be sent, and potential charge. API-key storage or provider configuration is not part of this architecture phase and must not alter current inference behavior.

## 6. Security and lifecycle principles

- Rust owns discovery, process supervision, local IPC, path checks, validation, permissions, cancellation, audit, and recovery. The renderer and model are untrusted.
- Network listeners exposed beyond a tightly scoped local boundary are prohibited. Blender-specific transport and ACL requirements are in `BLENDER_INTEGRATION_REQUIREMENTS.md`; unauthenticated LAN listeners are never acceptable.
- Capability declarations are allowlists, not grants. Permissions are denied by default and checked for each operation. Data reads, scene mutation, file import/export, overwrite, delete, and paid network use have distinct risk levels.
- No arbitrary AI-generated shell commands, Python, JavaScript, or Blender console text is executed. A connector maps validated action enums to fixed reviewed API calls.
- Connector inputs, app metadata, scene names, file names, reference images, `.blend` projects, and app output are untrusted. Apply size, count, time, path, and schema limits.
- Mutating tasks require a user-visible action plan and recovery point. Destructive actions, overwrites, and changes with broad impact require specific confirmation. Cancellation is task-scoped and reports whether an action stopped, completed, or could not be interrupted safely.
- Errors are typed, redacted, correlated, and actionable. Credentials never appear in action payloads, event logs, crash diagnostics, or generated reports.
- Disconnect closes the connector session and cancels/settles connector-owned tasks safely; it does not terminate an application process unless Nexora launched that process and the user explicitly chose an exit action.

## 7. Technical choices and unresolved decisions

| Topic | Proposal | Unresolved / validation gate |
|---|---|---|
| UI placement | One “App Integrations” destination in existing sidebar/shell; shared cards and inline configuration | Exact navigation position, naming, and accessibility review |
| Contract types | Rust `serde` DTOs and enums at the Tauri boundary; mirrored TypeScript types and contract tests consistent with current repo practice | Whether a schema/type generator is justified; do not add a dependency solely for this proposal |
| Connector process | Host-side adapter registered by stable connector ID; app-side protocol client only where required | In-process adapter vs reviewed helper process for future connectors; per-connector threat review |
| Catalog extension | Generic renderer and typed descriptor; first-party adapters reviewed and released through normal Nexora update | Signed external connector packages are deferred and require a separate plugin security design |
| Connection transport | Connector-specific local boundary, never renderer access; Blender proposal uses Windows named pipes | Verify Blender's embedded Python support, pipe ACLs, lifecycle, protocol behavior, and supported Blender builds in a spike |
| Permission persistence | Per-user, per-app, per-capability grants with narrow scope and revocation; task-specific confirmation for high risk | Exact grant expiry and Windows storage schema |
| Provider selection | Existing local path first when capable; explicit opt-in for any future external provider | Future provider/API-key architecture is not present today and is outside this phase |
| Health freshness | Timestamped typed health with explicit stale/unknown behavior | Polling cadence and reconnect backoff per adapter |

## 8. Dependencies and non-goals

**Dependencies for later implementation:** owner-approved UX; typed Rust/UI contract; user-grant and audit design; Windows lifecycle/ACL tests; app-specific version matrix; a tested connector transport; Blender add-on distribution and update policy; provider structured-output evaluation; cancellation/recovery tests; compatibility and hardware test matrix.

**Non-goals here:** implementing this page or registry; changing Nexora's approved core or media roadmaps; adding Blender code/add-ons; changing model/provider routing or billing; storing API keys; downloading/installing software, models, or runtimes; executing Blender; generating 3D assets; or claiming Windows readiness.

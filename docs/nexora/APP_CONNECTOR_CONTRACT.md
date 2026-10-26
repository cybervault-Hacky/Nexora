# App Connector Contract — Proposed Typed Boundary

**Status:** Proposed contract; no connector implementation or new dependency is authorized by this document.
**Related:** [APP_INTEGRATIONS_ARCHITECTURE.md](APP_INTEGRATIONS_ARCHITECTURE.md), [BLENDER_INTEGRATION_REQUIREMENTS.md](BLENDER_INTEGRATION_REQUIREMENTS.md).

> The definitions below are design sketches, not code that exists in Nexora. Rust owns the authoritative native DTOs and enforcement. The React UI receives only serialized, validated state through explicit Tauri commands and bounded event channels.

## 1. Contract principles

- A stable connector identity and contract version are separate from the connected application's identity/version.
- Discovery, installation, connection, health, capability readiness, permission grants, and task success are distinct facts.
- A descriptor advertises what a connector can do; only the host permission gate authorizes a particular operation.
- Typed enums and bounded values cross the boundary. Free-form AI text is never an executable request.
- Connector-specific payloads are validated against a versioned schema, converted to concrete typed actions, and revalidated in the connector adapter.
- `Unknown`, missing, stale, unsupported, false, and zero are never collapsed.
- Progress/cancellation are scoped to a task ID and connector session. Every event carries a correlation ID and monotonically increasing sequence.

## 2. Rust-like DTO sketch

```rust
struct ConnectorDescriptor {
    id: ConnectorId,                 // stable reverse-DNS identifier
    display_name: String,
    connector_version: SemVer,
    contract_version: u32,
    description: String,
    icon_key: LocalIconKey,          // packaged resource or trusted generic glyph
    supported_os: Vec<OsConstraint>,
    app_identity: AppIdentity,
    app_version_range: VersionRange,
    capabilities: Vec<CapabilityDescriptor>,
    prerequisites: Vec<PrerequisiteDescriptor>,
    configuration_schema: Option<ConfigSchema>,
}

struct InstallationReport {
    state: InstallationState,        // Installed | NotFound | Unsupported | Unknown | AccessDenied
    executable_path: Option<ApprovedPath>,
    app_version: Option<AppVersion>,
    observed_at: Timestamp,
    evidence: Vec<DetectionEvidence>,
}

enum ConnectionState {
    NotConnected,
    Connecting,
    Connected { session_id: SessionId, since: Timestamp },
    NeedsSetup { requirements: Vec<SetupRequirement> },
    Unsupported { reason: ErrorCode },
    Error { error: ConnectorError, retryable: bool },
    Disconnected { reason: DisconnectReason, was_connected: bool },
}

struct HealthReport {
    connection: ConnectionState,
    protocol_version: Option<u32>,
    app_version: Option<AppVersion>,
    checked_at: Timestamp,
    freshness: Freshness,            // Fresh | Stale | Unknown
    task_readiness: Vec<CapabilityReadiness>,
}

struct CapabilityDescriptor {
    id: CapabilityId,
    label: String,
    description: String,
    input_schema_version: u32,
    permission_requirements: Vec<PermissionRequirement>,
    prerequisites: Vec<PrerequisiteDescriptor>,
    app_version_range: VersionRange,
    resource_class: ResourceClass,
    risk: RiskClass,
}
```

`InstallationState` and `ConnectionState` are deliberately independent. `Connected` is not equivalent to a capability being ready; each `CapabilityReadiness` should be `Ready`, `Blocked(reasons)`, `Unsupported(reason)`, or `Unknown(reason)` with a timestamp/evidence reference.

Stable identifiers, paths, and limits are validated in Rust. The UI should not infer a version match by comparing strings or decide that a path exists merely because a path is configured.

## 3. Connector service contract

Conceptual trait operations:

```rust
trait AppConnector: Send + Sync {
    fn descriptor(&self) -> &ConnectorDescriptor;
    fn detect(&self, request: DetectionRequest) -> DetectionFuture<InstallationReport>;
    fn connect(&self, request: ConnectRequest) -> ConnectorFuture<ConnectionInfo>;
    fn disconnect(&self, session: SessionId, reason: DisconnectReason)
        -> ConnectorFuture<DisconnectReport>;
    fn health(&self, session: Option<SessionId>) -> ConnectorFuture<HealthReport>;
    fn configure(&self, request: ValidatedConfiguration)
        -> ConnectorFuture<ConfigurationReport>;
}

trait ConnectorTaskHandler: AppConnector {
    fn execute(&self, context: TaskContext, request: ValidatedTaskRequest,
               events: TaskEventSink) -> ConnectorFuture<TaskOutcome>;
    fn cancel(&self, session: SessionId, task: TaskId)
        -> ConnectorFuture<CancelOutcome>;
}
```

`ConnectorTaskHandler` is registered only for connectors that declare executable capabilities. Status-only connectors can implement lifecycle/health without exposing a fake `execute` endpoint. Initial first-party registrations can use explicit enum/static dispatch. If dynamic trait objects are later justified, use boxed futures or an already-approved mechanism rather than adding an async-trait dependency by default; this is a design choice for an implementation spike, not a Phase 1 dependency.

The `IntegrationService` is the only caller-facing entry point. It must:

1. Resolve a registered connector ID and verify connector/contract versions.
2. Refresh or validate the session and app version; reject stale/unsupported state for execution.
3. Check capability membership, payload schema, resource bounds, exact permission grants, and required user confirmations.
4. Bind a `TaskContext` to connector/session/task IDs, requesting chat conversation, base scene revision when relevant, and approved file roots.
5. Start execution, stream bounded progress, enforce time/output limits, and route cancellation.
6. Return typed success/failure plus validation evidence. It never converts an error or timeout into success.

## 4. Task, progress, cancellation, and error shapes

```rust
struct ValidatedTaskRequest {
    connector_id: ConnectorId,
    capability_id: CapabilityId,
    schema_version: u32,
    payload: ConnectorPayload,       // concrete enum after parsing/validation
    base_revision: Option<RevisionId>,
    consent_receipts: Vec<ConsentReceiptId>,
}

struct TaskEvent {
    task_id: TaskId,
    sequence: u64,
    stage: TaskStage,
    completed: Option<u64>,
    total: Option<u64>,
    message_code: MessageCode,        // renderer selects localized safe copy
    safe_details: Map<String, SafeValue>,
    cancellable: bool,
}

enum CancelOutcome {
    Cancelled,
    AlreadyFinished,
    TooLate { safe_point: Option<String> },
    UnknownTask,
}

struct ConnectorError {
    code: ErrorCode,
    user_message_key: MessageCode,
    retryability: Retryability,
    correlation_id: CorrelationId,
    safe_details: Map<String, SafeValue>,
}
```

Progress messages are informational, not proof of a completed operation. `TaskOutcome` should report the exact completed actions, resulting app/scene revision, output paths/format, and validation checks. If completion is uncertain after a transport loss, the service reports `Unknown/Needs reconciliation` and queries health/state before offering a retry; it does not repeat non-idempotent changes blindly.

Cancellation is cooperative at documented action boundaries. A connector must say when an operation cannot be interrupted immediately and must stop before starting the next action. A disconnect or app crash cancels its active task, records the final known state, and preserves recovery data. The host does not terminate an app it did not launch.

## 5. Permission and prerequisite declarations

Each capability declares narrow permission IDs, scope, and risk. For a Blender connector, example future permissions include:

- `blender.scene.inspect` — read selected scene/object metadata for this task.
- `blender.scene.modify` — perform a specifically previewed action set.
- `blender.scene.delete` — remove named objects; confirm each high-impact set.
- `blender.reference.read` — read a user-selected reference image from an approved root.
- `blender.project.save` — save to a user-approved `.blend` target.
- `blender.export.<format>` — write an explicitly supported export format to an approved directory.
- `blender.external_ai.send` — separately consented transfer to a named external provider; not implied by normal connect permission.

Prerequisites (e.g. supported Windows build, supported Blender version, enabled reviewed add-on, open scene, active capability, adequate resource evidence) are data shown to users and rechecked by the host before execution. Installing or enabling an add-on requires explicit user confirmation and package-integrity checks; connection does not imply blanket file or scene access.

Grants are per user, connector, capability, scope, and expiry; they are revocable and not expanded by model output. Destructive changes, overwrite, external data transfer, and high-resource AI inference use separate confirmation receipts tied to the exact request.

## 6. Versioning and compatibility rules

- Connector IDs never change when display copy changes. Connector implementation version and wire-contract version are separate fields.
- Every request/event includes schema version and correlation/task IDs. Unknown major versions fail closed with an actionable compatibility error; compatible minor extensions must ignore unknown optional fields safely.
- App versions are parsed into numeric components and compared against a reviewed support range, not lexically.
- Health includes the detected app executable identity/version and a freshness timestamp. Version support must be based on executed compatibility tests; upstream API documentation alone is not evidence.
- Configuration schema migration is explicit and reversible. A failed migration leaves the last valid config intact.
- Connector-specific action enums and schemas are versioned independently from generic lifecycle DTOs. Adding a connector should not force unrelated connectors to change.

## 7. Tauri and TypeScript boundary

Future Tauri commands should be explicit and narrow, e.g. `integration_catalog`, `integration_status`, `integration_connect`, `integration_disconnect`, `integration_reconnect`, `integration_configure`, `integration_execute`, and `integration_cancel`. Exact names are not committed until implementation. A renderer-supplied connector ID or payload is untrusted and never maps directly to arbitrary command names or filesystem paths.

Rust `serde` DTOs should be the authority. TypeScript should mirror the tagged unions and use the repository's existing contract-test approach. Consider schema generation only if the project adopts and maintains it across current IPC, not as a one-off new dependency. Tauri events/channels carry capped task events; they do not expose raw named-pipe handles or credentials to the renderer.

## 8. Error codes and observability

Stable error categories should include `not_found`, `unsupported_app_version`, `needs_setup`, `permission_denied`, `protocol_mismatch`, `authentication_failed`, `invalid_action`, `stale_scene`, `resource_blocked`, `file_access_denied`, `cancelled`, `app_exited`, `timeout`, `recovery_required`, and `internal`. Exact code names are implementation-time API decisions.

Logs may include connector/task IDs, app version, safe action type/count, timing, and result classification. Logs must exclude API keys, pipe authentication material, prompts containing private scene/project data, full file contents, and sensitive paths where they are unnecessary. User-facing errors explain what happened and how to reconnect, retry safely, or recover; raw Python tracebacks and secret-bearing process arguments are not surfaced.

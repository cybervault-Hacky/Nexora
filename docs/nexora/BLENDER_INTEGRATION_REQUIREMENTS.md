# Blender Integration — Product, Technical, and Security Requirements

**Status:** Proposed requirements only. Blender support is not implemented, installed, tested, or declared Windows-ready.
**Related:** [APP_INTEGRATIONS_ARCHITECTURE.md](APP_INTEGRATIONS_ARCHITECTURE.md), [APP_CONNECTOR_CONTRACT.md](APP_CONNECTOR_CONTRACT.md), [BLENDER_INTEGRATION_12_PHASE_PLAN.md](BLENDER_INTEGRATION_12_PHASE_PLAN.md), Nexora `docs/phase-1/03-application-architecture.md`, `04-ai-runtime-and-model-strategy.md`, and `07-security-and-privacy.md`.

## 1. Product boundary and current-state evidence

Blender is a candidate connector in the shared App Integrations system. Its catalogue card, setup and health belong to the one shared App Integrations page. The user ultimately creates, inspects, revises, previews, and saves work through their existing AI Chat; no Blender Studio page or parallel conversational workspace is proposed.

**Confirmed now:** repository inspection found no Blender reference, Blender add-on, Blender process detector, connector registry, integrations page/route, or Blender command. The desktop shell is Tauri 2 + React/TypeScript; its hash router includes AI Chat, Projects, Code Intelligence, Local Models, Settings, and other existing views. Native app functions are explicitly registered Tauri commands. The current code has a local `llama.cpp` inference path and dedicated media managers, but no general external-app connector or user-configurable third-party AI inference provider. The existing media/resource modules do not provide Blender integration and are not assumed to do so.

**Proposed:** a reviewed first-party Blender connector, optionally paired with a small reviewed Blender Python add-on, communicating through Nexora's Rust/Tauri core. All work is opt-in per user/task and subject to supported-version, permission, resource, and provider checks. Nothing in this document establishes Blender compatibility or a real capability.

## 2. Supported installation discovery and lifecycle

### 2.1 Windows discovery

A future detector should be read-only and bounded. Discovery sources may include Windows uninstall/App Paths registry entries (per-user and machine, 32/64-bit views as applicable), documented Blender installation locations, and an explicitly selected executable. It must not recursively scan drives, inspect unrelated user data, download Blender, change PATH, or launch arbitrary executables.

For each candidate, retain evidence: normalized executable path, file identity, architecture, version, source of discovery, access result, and observation time. Validate the actual executable, not only a registry/string claim. Querying `blender.exe --version` must occur only after selecting/approving the candidate, in a bounded child process with sanitized environment, fixed arguments, timeout, output cap, and cancellation. Detection is not compatibility proof.

The product must publish an exact tested Windows and Blender version matrix before advertising support. This Phase 1 does **not** select a minimum Blender release; the supported range is an unresolved decision for the feasibility/compatibility phase. Unsupported, beta, portable, store, and multiple-install cases need explicit, honest results. Never silently switch to a different Blender executable when a chosen installation changes.

### 2.2 Setup and connection states

Proposed connection sequence:

1. User selects **Connect** on the Blender card. Nexora reports detected installation/version and prerequisites.
2. If required, the user explicitly installs/enables a reviewed Nexora add-on in the chosen Blender installation. Do not silently mutate Blender preferences or install into an unapproved location. Package hash/signature and compatible add-on version must be verified.
3. The add-on initiates a local connection to the Nexora-owned endpoint. Nexora verifies the pipe ACL, peer process identity/path, protocol handshake, supported app version, and session freshness.
4. The connector records health and capability readiness separately. A successful connection does not imply a compatible version, open scene, granted file access, available model, or task readiness.
5. **Disconnect** revokes the session and grants as specified; it does not kill a user-owned Blender process. **Reconnect** reuses only the last user-approved executable/add-on and asks again if identity or version changed.

Required visible states: Not Connected, Connecting, Connected, Needs Setup, Unsupported, Error, and Disconnected. Include the reason and next action. The app may be installed when there is no connection, connected while task readiness is blocked, or unknown when detection is inconclusive.

## 3. Local communication and Blender-side execution

### 3.1 Proposed transport

For Windows, prefer a **Nexora-owned, per-user named pipe** over an HTTP/TCP command listener. The reviewed Blender add-on connects outward to that pipe; the renderer never opens a pipe or socket. Apply an explicit DACL restricted to the current Windows user, use a per-session challenge/nonce and versioned handshake, verify the connected process ID and approved Blender executable, and close stale endpoints on shutdown. Enforce message size, action count, rate, and request-time limits. Avoid reusable secrets in command-line arguments, logs, project files, or add-on source.

Do not bind an unauthenticated server to `0.0.0.0`, LAN, or an unrestricted localhost port. Loopback-only TCP is not the preferred first boundary; if named-pipe feasibility fails, any alternative needs a fresh threat review (authentication, browser-origin/CSRF risks, binding/firewall behavior, PID binding, and port lifecycle) before adoption.

Named-pipe access is not a sandbox against malicious code already running as the same Windows user. The host still validates each request and restricts it to explicit capabilities. Whether Blender's bundled Python provides the required named-pipe client API and whether the host can reliably verify client PID/path across supported Blender builds are **Phase 2 spike questions**, not assumed facts.

### 3.2 Versioned, declarative command protocol

The protocol should exchange a versioned hello (connector/app/Blender/add-on/protocol versions), health, capability list, scene revision, task ID, action request, bounded progress, completion/evidence, and cancellation. Use typed JSON/serde-compatible envelopes, strict schemas, correlation IDs, sequence numbers, and explicit rejection for unknown major versions. The command set is an allowlisted declarative vocabulary; the add-on maps each action to fixed `bpy` API calls on Blender's main thread, for example through a bounded main-thread timer/queue.

Initial action families could include:

- **Inspect:** scene summary; active scene/collection; objects by stable Nexora ID; object type, transforms, bounds, material references, cameras, lights, render settings, and supported animation metadata. Avoid dumping arbitrary custom properties or all file contents by default.
- **Create:** reviewed primitives/mesh templates; collections; camera and light; bounded transforms; approved modifiers; materials from a restricted node allowlist; and generated procedural furniture/environment layouts with caps on geometry and complexity.
- **Modify:** transform or material changes to explicitly identified objects; bounded parameters; create/update only within approved scene/collection scope.
- **Save/export:** explicit target paths and supported format IDs; do not accept a destination path directly from the model.
- **Animation:** only separately declared action types and supported object/rig types (e.g., validated transform keyframes); no arbitrary drivers, expressions, or executable embedded code.

AI-authored Python, Python console strings, `eval`, arbitrary operator names, shell commands, and unvalidated path strings are not protocol actions. The connector validates a typed action once before sending and the add-on validates it again before `bpy` execution.

Every mutation should be associated with a task and expected scene revision. If the user edits the scene between inspect and execute, detect a stale revision and ask to re-plan rather than applying stale changes. Use stable connector-managed object IDs where possible; display names alone are mutable and not safe identifiers.

## 4. AI chat planning and task experience

The AI path should be a staged, auditable workflow:

1. **Interpret:** resolve the user's request and ask clarifying questions when target object, scale, style, output, or destructive intent is ambiguous.
2. **Inspect (if approved):** collect only the minimum scene facts required; imported scene data and text are untrusted prompt content.
3. **Plan:** produce a human-readable summary plus typed action list, assumptions, affected objects, resource estimate, file writes, and risk classification.
4. **Review/consent:** show the exact planned changes and request task-scoped permission/confirmation before mutations, overwrites, deletes, external transfer, or high-resource work.
5. **Execute:** send validated actions with task ID; stream connector-originated progress and allow stop/cancel.
6. **Verify/recover:** ask Blender for the resulting state, verify expected changes and output files, report partial/unknown results honestly, and provide recovery/undo options.

The chat must never report an action as complete merely because a model proposed it, a pipe accepted a request, or a progress event fired. Completion is based on connector results and applicable validation. Manual changes in Blender remain authoritative; user-requested revisions should inspect current state again and generate a new plan.

## 5. Capability boundaries

### 5.1 Achievable through Blender scripting APIs (subject to supported API/version tests)

Blender's Python API can support deterministic, procedural scene operations such as creating/manipulating objects, meshes, collections, transforms, cameras and lights; constructing materials using supported node types; applying bounded modifiers; configuring scenes; inspecting supported properties; keyframing supported properties; saving native `.blend` projects; and invoking validated exporters available in the tested build.

This can produce useful stylized/procedural furniture, props, environments, and scenes from structured parameters. It does not by itself provide robust semantic image understanding, artistic judgment, photorealistic assets, accurate topology, or arbitrary production-quality rigs. Version-specific operators/add-ons/exporters must be probed and tested.

### 5.2 Requires additional AI/runtime or domain systems

| Capability | Additional requirement / honest boundary |
|---|---|
| Reference-image analysis | A model/provider that accepts images plus an explicit image privacy decision. The current local inference request path does not establish a vision-capable model or image-input contract. Without that, Blender may import/display the reference locally, but Nexora cannot claim it analyzed the image. |
| Image-to-3D | A separately reviewed compatible reconstruction/generation model or explicit external provider, complete asset/runtime/licence review, measured memory/compute profile, and post-processing/validation. A single image is ambiguous about hidden geometry, scale, materials, and topology; output is an approximation, not an exact fully detailed reconstruction guarantee. |
| Detailed organic/sculpted assets | Dedicated 3D generative models, sculpting/retopology/UV/baking tools, or user-authored assets; each adds dependencies and quality gates. |
| Rigging and animation beyond simple keyframes | Rig libraries, validated rigs, retargeting/mocap/animation systems, domain constraints, and explicit tested model coverage. Procedural transform keyframes do not equal character rigging. |
| Cloud generation or vision | A supported external provider, explicit per-task consent describing data and possible charges, secure user credential storage designed in a separate phase, internet connectivity, provider/model capability check, and no silent fallback. |

Provider/model capability must include at least modality, structured-output/tool-call support and observed reliability, maximum context/input limits, local/offline status, resource profile, and external-data/billing behavior. A text-only model may plan simple tasks from user text but must not be presented as an image analyzer or image-to-3D model.

## 6. Files, preview, saving, and export

- File access is limited to roots explicitly approved by the user through the native file picker or a scoped grant. Canonicalize paths and reject traversal, junction/symlink escapes, UNC/device paths, alternate streams, and unexpected extensions.
- Treat `.blend`, imported images, textures, fonts, linked libraries, and archives as untrusted. Enforce file-size, image-dimension, archive-depth, scene-object, mesh, texture, and render limits. Keep Blender's automatic execution of embedded Python scripts disabled; never ask users to enable it to make AI actions work.
- Import reference images only through an approved file selection and explicit operation; import does not imply analysis or upload. Validate file signatures/content and store no extra copy unless the user approves.
- Preview should use the connected Blender viewport or a requested bounded render. Any preview artifact is saved to a user-approved area, shown with its actual state, and is not a claim that the `.blend` project is saved.
- `.blend` is the required editable project format. glTF/GLB and FBX are candidate exports only after verification in each supported Blender build and export configuration; any additional format is separately declared and tested. Export never overwrites by default.
- Save to a user-confirmed `.blend` target. Use safe temp/backup/checkpoint and replace strategies, validate the result, and preserve the original on failure. Confirm overwrite/delete explicitly. An existing project must not be silently replaced. Recovery should identify the last verified checkpoint and any uncommitted scene changes.

## 7. Windows, hardware, offline, and resource requirements

- Supported host OS, architecture, Windows builds, Blender versions, Blender add-on versions, GPU APIs, and driver minimums must be published from tested Windows results. No Linux source/build result establishes Windows support.
- Discovery and named-pipe access use current-user permissions; no service, administrator elevation, or machine-wide installation is required. Add-on files/configuration and Nexora state must have restrictive per-user ACLs. Do not inherit secrets through process environment.
- Blender procedural actions and AI inference have different resource profiles. A simple mesh build can work on CPU/low-memory systems while an image-to-3D model or high-resolution render cannot. Keep these as separate readiness/admission checks.
- Detect CPU/RAM/GPU/VRAM/backend only with confidence/freshness metadata. Detection is not runtime compatibility, and checkpoint weight size is not peak memory. When required measurements are unknown, block or constrain the expensive operation and say why.
- Define per-task caps: action count, mesh/object/vertex/face budgets, texture bytes/dimensions, render resolution/frames/samples, task duration, temporary disk, concurrent tasks, and cancellation/cleanup behavior. Introduce a Blender workload profile into shared resource coordination only after reviewing the existing media-specific coordinator; do not assume it already accounts for Blender, other apps, or persistent model loads.
- Offline: app detection, named-pipe control, supported procedural actions, local saves, and local export should work without cloud access once Nexora, Blender, and any approved local models are present. Installing Blender/add-ons or acquiring model/runtime assets is never automatic. External provider/image-to-3D workflows require internet and separate consent. If local provider capability is unavailable, the task must remain unavailable rather than silently switching online.

## 8. Security, permissions, and user control

| Operation | Proposed minimum control |
|---|---|
| Detect installation/version | Read-only, bounded locations; disclose candidate path; no launch until selected/approved |
| Connect/reconnect | Explicit user action; verify current-user pipe ACL, peer process/path, protocol and version; no broad network listener |
| Read scene | Capability-specific, task-scoped, minimum data; show what scene data is sent to a model/provider |
| Modify scene | Show exact structured plan and target objects; user approval before execution; confirm destructive/high-impact actions; create recovery point |
| Import/open file | Explicit picker/scope, type/size validation, untrusted-input handling; block embedded script execution |
| Save/export/overwrite | Approved directory and format allowlist; preview destination; no overwrite without explicit confirmation |
| Cloud/provider task | Named provider/model, files/prompts to be sent, network status and potential charge shown; explicit affirmative consent each task; never silent fallback |
| Cancel/disconnect | Task-scoped cancel, clear outcome (cancelled/finished/too late/unknown); do not kill user-owned Blender |

Scene content, imported file metadata, object names, material names, image pixels, and provider responses are untrusted and may contain prompt injection. Treat them as data. Policy code—not model instructions—enforces permissions and schemas. Do not expose credentials to Blender unless a separately reviewed connector feature strictly requires it; do not log credentials, prompts, image contents, or private file payloads.

## 9. Dependencies, risks, and unresolved decisions

### Dependencies before implementation

1. Owner-approved generic App Integrations UX and connector contract.
2. Windows spike proving pipe ACL, connection discovery, peer PID/path verification, Blender embedded Python support, cancellation and reconnect behavior.
3. Supported Blender/Windows/add-on matrix based on real builds and tests.
4. Reviewed add-on packaging, update, integrity, uninstall, and recovery process.
5. Typed action DSL, plan/consent UI, Tauri IPC DTOs, and permission/audit enforcement.
6. Resource measurements for each supported operation and hardware tier.
7. Local image-vision/image-to-3D provider capability decisions, licensing and model provenance before any model acquisition.
8. Windows end-to-end security, compatibility, accessibility, performance, and file-recovery tests.

### Major risks / decisions

- **Embedded Python IPC support:** Blender's Python distribution may vary by release; prove named-pipe support and lifecycle before committing to it.
- **Connector vs add-on trust:** an add-on runs inside a powerful application. Keep it small, reviewed, signed/hashed, capability-limited, and free of arbitrary remote code.
- **Stale scene and partial mutation:** manual user actions or app crashes can invalidate the plan midway. Scene revision detection and recovery/undo semantics need a prototype and fault tests.
- **Blender API/version drift:** operators, exporter availability, nodes, and file formats vary. Choose a narrow tested matrix and fail clearly outside it.
- **Provider honesty/privacy:** existing local path has no established image-input support. Image analysis and image-to-3D must remain unavailable unless a capable local model or explicitly consented provider is configured.
- **High resource variance:** render/geometry/model memory changes drastically with assets, settings, and GPU. No claim can be based solely on device detection or model weights.
- **File trust:** `.blend` and linked resources can contain scripts or paths; disable auto-run and validate all imports/exports.
- **Dynamic extension supply chain:** external connector packages multiply code trust. Defer dynamic loading until a signature, sandbox, revocation, and review model is approved.
- **Support floor and deliverable formats:** exact Windows builds, Blender versions, export formats, plug-in policy, and minimum hardware are unresolved; do not advertise them before validation.

These requirements do not change subscription/billing/entitlements, existing local provider behavior, the approved core roadmap, or the separate media-generation roadmap.

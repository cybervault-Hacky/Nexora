# 01 — Product Scope and Requirements

**Status:** Proposed for owner approval  
**Depends on:** None  
**Feeds into:** UX specification, architecture, agent design, roadmap

---

## 1. Product definition

**Nexora is a Windows desktop AI coding studio.** A user describes a website, application, animation, 3D experience, or code change in natural language. Nexora uses a locally running coding model to plan the work, inspect the project, create and edit multiple files, run controlled commands, preview the result, run checks, attempt bounded repairs, and report what was actually verified.

Nexora is designed for people who want professional results without sending their source code to a hosted AI service. It is local-first and offline-capable after the initial model download. It is not a thin chat wrapper: the durable value is the combination of a premium conversational workspace, a controlled agent runtime, a project-aware toolchain, and evidence-based verification.

### Positioning statement

> For Windows developers, designers, and technical founders who need to build real multi-file web projects with AI, Nexora is a local-first AI coding studio that combines a premium conversational interface with a bounded, auditable coding agent. Unlike browser-based AI builders, Nexora keeps code and inference on the user's machine, works offline after model download, and only claims success when local checks support it.

### Product principles

1. **Local first.** Core generation and editing run on the user's hardware. Network access is explicit, visible, and limited.
2. **Evidence over assertion.** The agent distinguishes generated, built, tested, previewed, and unverified work.
3. **Bounded autonomy.** Every autonomous action has a permission, a budget, a checkpoint, and a stop condition.
4. **Reversible by default.** File changes and task milestones are checkpointed; destructive actions require confirmation.
5. **Premium calm.** The interface is quiet, precise, and fast; glass and motion support hierarchy rather than decorate it.
6. **Honest failure.** A blocked, partial, or unverified result is a valid product outcome and must be reported clearly.
7. **Windows-native quality.** Correct DPI, window management, file paths, permissions, process handling, and installer behaviour.
8. **No lock-in.** Projects are ordinary folders and ordinary stacks; Nexora does not require proprietary project formats.

---

## 2. Target users

| Persona | Context | Primary need | Success looks like | Constraints |
|---------|---------|--------------|--------------------|-------------|
| **P1 — Solo developer** | Builds client sites and side projects; comfortable with code | Generate a strong first draft and iterate quickly across many files | From brief to runnable, responsive site with a reviewable diff in one session | 16–32 GB RAM, mid-range GPU, values speed and control |
| **P2 — Designer / technical founder** | Strong visual taste, limited backend depth | Turn a design idea into a responsive, animated, possibly 3D site | A polished preview they can show, with clear notes on what is not production-ready | May not know terminal commands; needs safe defaults and plain-language status |
| **P3 — Professional developer on confidential code** | Works under NDA or internal policy | AI assistance without uploading source or prompts | Local inference, auditable network behaviour, permission controls, no hidden telemetry | Corporate laptop, restricted install rights, may have no GPU |
| **P4 — Student / hobbyist** | Learns by building; budget-sensitive | A capable coding assistant without subscriptions or API bills | Runs acceptably on modest hardware; explains failures without overclaiming | 8–16 GB RAM, integrated or entry GPU, may be offline |

### Anti-personas (not the design target)

- Users who want a hosted multi-user collaboration platform.
- Users who want a no-code drag-and-drop editor with no code exposure.
- Users who expect frontier-model quality on 8 GB of RAM.
- Enterprise IT administrators buying a fleet management product (not at 1.0).

---

## 3. Core user journeys

### J1 — Idea to multi-page website

1. User creates a project and writes a brief: purpose, pages, tone, brand colours, required sections, whether 3D or animation is needed.
2. Nexora asks at most 3–5 high-value clarifying questions, then proposes a short plan and stack.
3. User approves the plan.
4. Agent scaffolds the project, writes pages/components/styles, installs dependencies, builds, starts a local preview, captures screenshots at three widths, fixes bounded failures, and reports evidence.
5. User reviews the diff, preview, and verification report; requests changes conversationally.
6. User can roll back any checkpoint or open the folder in another editor.

### J2 — Fix a failing project

1. User opens an existing folder and reports the failure, or selects "Diagnose build".
2. Agent inspects the repo, reproduces the failure with the project's own command, classifies the error, and proposes a minimal fix.
3. Agent edits, re-runs the failing check, and stops after a bounded number of attempts.
4. If unresolved, it reports the exact command, error signature, attempts made, and the most promising next step. It does not claim success.

### J3 — Add a 3D or animated experience

1. User asks for an interactive 3D hero, scroll animation, or product configurator.
2. Nexora selects an appropriate stack (for example React Three Fiber + Three.js, or vanilla Three.js), plans scene structure, assets, performance budget, and fallback.
3. Agent implements, verifies WebGL context creation, checks console errors, captures screenshots, and reports frame-budget risks.
4. If GPU or headless capture is unavailable, the visual result is explicitly marked unverified.

### J4 — Work offline

1. User has previously downloaded a supported model.
2. Nexora starts without network, loads the model, and supports chat, editing, build, test, and preview using locally cached dependencies.
3. Any action that needs network (new dependency, update check, model download) is blocked with a clear explanation and an explicit retry option. Nothing silently falls back to a cloud service.

### J5 — Review and roll back

1. The task timeline shows each step, command, file change, and check.
2. User opens a checkpoint diff, restores one file or the whole checkpoint, and confirms.
3. Nexora records the rollback in the audit log and re-verifies the restored state.

---

## 4. Scope

### 4.1 In scope for 1.0

- Windows 11 x64 desktop application, per-user install, no administrator requirement for normal use.
- Custom title bar, sidebar, conversation workspace, composer, project explorer, editor, terminal/output panel, preview workspace, task timeline, settings.
- Local model manager: browse approved models, check hardware fit, download with verification and resume, select, update, remove, relocate storage.
- Local inference with GPU acceleration where supported and CPU fallback.
- Streaming responses, cancellation, tool calling, structured outputs, context management.
- Project creation from templates or empty folders; multi-file read/write; file search; symbol-aware context.
- Controlled command execution: install, build, lint, test, dev server, limited scripts.
- Local preview with responsive device presets, console capture, screenshots.
- Verification pipeline: parse/typecheck, build, tests, runtime smoke, visual snapshots, accessibility scan where practical.
- Bounded automatic repair with explicit limits and no-progress detection.
- Checkpoints and rollback at file, step, and task level.
- Permission controls and audit log for filesystem, process, network, and secrets.
- Project/website generation for static sites, React/Vite apps, Next.js full-stack apps, and Three.js/R3F experiences.
- Conversation history, project persistence, export, and deletion.
- Diagnostics bundle exported locally by the user.
- Accessibility: keyboard-first operation, reduced motion, screen-reader labels, 200% scaling.
- Signed installer and signed updates with manual approval and rollback.

### 4.2 Out of scope for 1.0

| Excluded | Reason | Revisit |
|----------|--------|---------|
| Android, iOS, web-hosted Nexora | Product is Windows desktop only | Never for core product |
| Linux, macOS | Focus and quality | Post-1.0 only with a new decision |
| Team collaboration, shared workspaces, cloud sync | Local-first single-user product | Post-1.0 |
| Cloud AI as a required dependency | Core must work without paid external API | Optional BYO endpoint may be added later |
| Autonomous production deployment | Security and liability; verification cannot guarantee production readiness | Post-1.0, separate design |
| Model training or fine-tuning | Out of product scope | Not planned |
| Image, video, or music generation | Different product surface and hardware profile | Not planned as core |
| General desktop automation (email, spreadsheets, OS settings) | Expands threat surface beyond coding | Not planned |
| Plugin marketplace / third-party extensions | Supply-chain and support burden | Post-1.0 |
| Mobile app previews for native Android/iOS | Requires platform SDKs and licenses | Not planned |
| Windows ARM64 at launch | Smaller audience; runtime and installer work | Post-1.0 based on demand |
| 32-bit Windows | Model memory requirements make it impractical | Never |
| Light theme at 1.0 | Focus; dark theme is the brand | Post-1.0 |
| Multi-user or shared PC mode | Security model assumes one user per Windows session | Post-1.0 |
| Built-in Git commit/push automation | Risk of unwanted history changes | Read-only Git at 1.0; write actions deferred |
| Guaranteed one-click deployment to hosting providers | Deployment is not verification | Post-1.0 |

### 4.3 Product boundaries and non-promises

Nexora **MUST NOT** claim or imply:

- zero bugs, zero errors, or guaranteed working code;
- superiority over any commercial AI system;
- that generated code is secure, accessible, performant, or production-ready without checks;
- that local inference makes the entire application offline or private in all cases;
- that a project is deployed when it has only been generated or previewed locally;
- that a model's vendor benchmark score predicts the user's experience on their hardware.

---

## 5. Capability map

```
Nexora
├─ Workspace
│  ├─ Projects, recent folders, templates
│  ├─ Conversation history and search
│  ├─ File explorer, tabs, editor, diffs
│  └─ Terminal, output, problems
├─ Intelligence
│  ├─ Model manager and hardware fit
│  ├─ Local inference and streaming
│  ├─ Context builder and repo index
│  └─ Coding agent and bounded repair
├─ Build
│  ├─ Stack selection and scaffolding
│  ├─ Dependency and asset management
│  ├─ Build/lint/test workflows
│  └─ Preview and visual checks
├─ Trust
│  ├─ Permissions and confirmations
│  ├─ Checkpoints and rollback
│  ├─ Secret detection and redaction
│  └─ Audit log and network disclosure
└─ Platform
   ├─ Installer, updates, diagnostics
   ├─ Settings, storage, accessibility
   └─ Crash recovery and export
```

---

## 6. Functional requirements

Priority: **M** = Must for 1.0, **S** = Should, **C** = Could (post-1.0), **W** = Won't.  
Release: `A` = alpha (internal), `B` = beta, `1.0`, `Post` = after 1.0.  
Every requirement has an observable verification method.

### 6.1 Onboarding and hardware readiness

| ID | Requirement | Pri | Rel | Verification |
|----|-------------|-----|-----|--------------|
| FR-ONB-01 | First run presents a short welcome explaining local inference, network use, and privacy in plain language. | M | A | UI test; copy review |
| FR-ONB-02 | Nexora detects CPU, RAM, GPU vendor/model, VRAM, driver version, free disk, and WebView2 version. | M | A | Fixture tests on 3 PCs |
| FR-ONB-03 | Hardware report explains which model tiers are usable and why, without promising performance. | M | A | Snapshot tests per tier |
| FR-ONB-04 | User can skip model download, continue to the app, and be shown a clear "no model" state. | M | A | E2E test |
| FR-ONB-05 | Onboarding can be replayed from Settings → About → Run setup again. | S | B | UI test |
| FR-ONB-06 | Onboarding discloses every network destination used during setup (model host, update check, if enabled). | M | A | Manual + network trace test |
| FR-ONB-07 | User can choose a custom model storage folder with a disk-space check. | M | B | Integration test |

### 6.2 Model management

| ID | Requirement | Pri | Rel | Verification |
|----|-------------|-----|-----|--------------|
| FR-MDL-01 | Model catalogue lists only Nexora-approved models with size, quantization, licence, context, hardware floor, and capability tags. | M | A | Catalogue schema tests |
| FR-MDL-02 | Each model file is downloaded from a pinned revision and verified against SHA-256 before use. | M | A | Corrupt-fixture test |
| FR-MDL-03 | Downloads support pause, resume, cancel, retry, and progress with bytes and percentage. | M | A | Network fault tests |
| FR-MDL-04 | User must see and accept the model licence before download. | M | A | UI test |
| FR-MDL-05 | Nexora refuses to load a model that fails hash, size, header, or runtime compatibility checks. | M | A | Corrupt model tests |
| FR-MDL-06 | User can select one active model per task size class (default, fast, high-quality) and switch at any time between turns. | M | B | Integration test |
| FR-MDL-07 | Model updates are explicit, never silent; old version remains until the new one verifies. | M | B | Update test |
| FR-MDL-08 | User can delete a model and reclaim space, with confirmation and no effect on projects. | M | B | E2E test |
| FR-MDL-09 | Nexora detects insufficient VRAM/RAM and automatically chooses a safe context size, GPU layer count, or CPU fallback, explaining the choice. | M | A | Matrix tests |
| FR-MDL-10 | Model loading shows staged progress (verify → load → warm-up) and can be cancelled. | M | A | UI test |
| FR-MDL-11 | Nexora reports runtime crashes and offers a safe reload or smaller configuration. | M | A | Fault injection |
| FR-MDL-12 | Users can import a compatible local GGUF only through an advanced flow with warnings and hash display. | C | Post | Manual test |

### 6.3 Conversation and composer

| ID | Requirement | Pri | Rel | Verification |
|----|-------------|-----|-----|--------------|
| FR-CHT-01 | User can create, rename, archive, search, and delete conversations. | M | A | E2E test |
| FR-CHT-02 | Responses stream token-by-token with a visible generating state and elapsed time. | M | A | Timing test |
| FR-CHT-03 | Stop button cancels generation promptly and preserves partial output as clearly marked incomplete. | M | A | Cancellation test |
| FR-CHT-04 | Composer supports multiline input, Enter to send, Shift+Enter newline, Esc to stop, and IME composition without premature send. | M | A | Keyboard/IME tests |
| FR-CHT-05 | Composer exposes model selector, project-context toggle, attachment control, and send/stop control. | M | A | UI test |
| FR-CHT-06 | Composer shows ready, generating, stopped, error, disabled, offline, and model-loading states. | M | A | State matrix snapshots |
| FR-CHT-07 | User can attach text files and drag-and-drop supported files into the conversation with size/type validation. | S | B | E2E test |
| FR-CHT-08 | Messages render Markdown, code blocks with copy, file links, diff cards, tool-call cards, and status chips safely. | M | A | XSS/rendering tests |
| FR-CHT-09 | User can copy, regenerate, edit-and-resend, and branch a conversation from a message. | S | B | E2E test |
| FR-CHT-10 | Long conversations are compacted with visible summary markers and retained task state. | M | B | Long-run test |
| FR-CHT-11 | Conversation data is stored locally and can be exported as Markdown/JSON and deleted permanently. | M | B | Export/delete test |
| FR-CHT-12 | Input is preserved across panel switches, app restarts, and crashes. | M | A | Recovery test |

### 6.4 Project workspace

| ID | Requirement | Pri | Rel | Verification |
|----|-------------|-----|-----|--------------|
| FR-WSP-01 | User can create a project from a bundled template, open an existing folder, or start empty. | M | A | E2E test |
| FR-WSP-02 | Project explorer shows a lazy-loaded tree with icons, filtering, and ignored-file handling. | M | A | 10k-file fixture |
| FR-WSP-03 | Editor supports syntax highlighting, search, multi-tab editing, save, undo/redo, and large-file guardrails. | M | B | Editor tests |
| FR-WSP-04 | Diffs show staged agent changes with per-file accept/reject and clear add/remove highlighting. | M | B | Diff tests |
| FR-WSP-05 | External file changes are detected and surfaced as conflicts, never silently overwritten. | M | B | Watcher tests |
| FR-WSP-06 | Project root is explicit; all default file tools are confined to it. | M | A | Path-escape tests |
| FR-WSP-07 | User can reveal the project in Explorer, open it in an external editor, and copy its path. | S | B | UI test |
| FR-WSP-08 | Nexora records project-specific settings in a visible, documented location. | M | B | Schema test |
| FR-WSP-09 | Project removal from Nexora never deletes the folder from disk without a separate explicit confirmation. | M | A | E2E test |
| FR-WSP-10 | Supported project sizes: up to 50k files indexed, with graceful degradation and clear limits beyond that. | S | B | Scale test |

### 6.5 Coding agent

| ID | Requirement | Pri | Rel | Verification |
|----|-------------|-----|-----|--------------|
| FR-AGT-01 | Agent performs task analysis and asks at most five high-value clarifying questions when ambiguity blocks correctness. | M | A | Eval tasks |
| FR-AGT-02 | Agent produces a visible plan with ordered steps before making non-trivial changes. | M | A | Eval + UI |
| FR-AGT-03 | Agent inspects relevant project files before editing and cites them in its reasoning summary. | M | A | Eval |
| FR-AGT-04 | Agent can create, edit, move, and delete files through validated tools with atomic batch application. | M | A | Tool tests |
| FR-AGT-05 | Agent can run allowlisted commands through the policy engine with timeout, output cap, and job-object kill. | M | A | Security tests |
| FR-AGT-06 | Agent can install dependencies only with network permission and a lockfile-preserving workflow. | M | B | Install tests |
| FR-AGT-07 | Agent can start/stop a local preview and inspect HTTP status, console output, and screenshots. | M | B | Preview tests |
| FR-AGT-08 | Agent classifies failures and attempts bounded repair with a maximum of 3 attempts per failure signature and 8 total cycles per task. | M | B | Loop tests |
| FR-AGT-09 | Agent stops and escalates when progress is not being made, when a budget is exhausted, or when a permission is denied. | M | B | Fault injection |
| FR-AGT-10 | Agent cannot weaken, delete, or skip tests, lint rules, or type checks to make a failure disappear without explicit user approval. | M | B | Adversarial evals |
| FR-AGT-11 | Agent can resume a long-running task after app restart from a persisted task journal. | S | 1.0 | Recovery test |
| FR-AGT-12 | Agent supports cancellation at any stage with a checkpoint left behind. | M | A | Cancellation test |
| FR-AGT-13 | Only one mutating agent run per project at a time; read-only queries may run concurrently. | M | B | Concurrency test |
| FR-AGT-14 | Agent reports completed, failed, blocked, and unverified work separately in a final report. | M | A | Report schema test |
| FR-AGT-15 | Agent may not claim a project works unless the verification gates in FR-VER pass. | M | A | Honesty evals |

### 6.6 Tools, permissions, and audit

| ID | Requirement | Pri | Rel | Verification |
|----|-------------|-----|-----|--------------|
| FR-TOOL-01 | All privileged operations pass through a single policy engine; no UI or model path bypasses it. | M | A | Architecture test |
| FR-TOOL-02 | Permissions are scoped per project for read, write, execute, network, secrets, outside-workspace, and Git write. | M | A | Permission matrix tests |
| FR-TOOL-03 | First write, first command, first install, and any outside-workspace access require explicit consent or a remembered scoped grant. | M | A | E2E tests |
| FR-TOOL-04 | Destructive operations (delete, overwrite large file, reset) show a preview and require confirmation unless explicitly pre-approved for the task. | M | A | Adversarial tests |
| FR-TOOL-05 | Commands run as argument arrays without a shell by default; shell metacharacters are never interpreted. | M | A | Parser tests |
| FR-TOOL-06 | Commands have timeouts, output caps, working-directory enforcement, and process-tree termination. | M | A | Fault tests |
| FR-TOOL-07 | Every tool call is recorded in an append-only local audit log with timestamp, actor, action, target, result, and duration. | M | B | Log tests |
| FR-TOOL-08 | User can review and revoke remembered grants per project. | M | B | UI test |
| FR-TOOL-09 | A panic switch stops generation and kills all agent child processes. | M | A | Kill test |
| FR-TOOL-10 | Git access is read-only at 1.0 (status/diff/log); commit/push are not available to the agent. | M | B | Capability test |

### 6.7 Verification and reporting

| ID | Requirement | Pri | Rel | Verification |
|----|-------------|-----|-----|--------------|
| FR-VER-01 | Verification levels are defined: static, build, tests, runtime, visual/accessibility, and manual/unverified. | M | B | Spec review |
| FR-VER-02 | Nexora records the exact command, exit code, duration, and truncated output for every check. | M | B | Persistence test |
| FR-VER-03 | UI status chips distinguish Generated, Built, Tests passed, Preview loads, Visually checked, and Unverified. | M | B | Snapshot tests |
| FR-VER-04 | Final reports list changed files, checks run, failures, unverified items, and risks. | M | B | Schema test |
| FR-VER-05 | A failed or skipped check can never be rendered as passed. | M | B | Adversarial test |
| FR-VER-06 | Verification results are tied to a checkpoint hash so stale results are invalidated after edits. | M | B | Integration test |
| FR-VER-07 | User can re-run any check manually from the task timeline. | S | B | UI test |

### 6.8 Website builder

| ID | Requirement | Pri | Rel | Verification |
|----|-------------|-----|-----|--------------|
| FR-WEB-01 | User can specify pages, content, style, responsive behaviour, animation, 3D, and backend needs in a structured brief or free text. | M | B | UI test |
| FR-WEB-02 | Nexora recommends a stack based on the brief and explains the choice; it does not default every project to one framework. | M | B | Decision-table tests |
| FR-WEB-03 | Bundled templates scaffold offline without network access. | M | B | Offline test |
| FR-WEB-04 | Generated sites are responsive at 360, 768, 1024, and 1440 px with no horizontal overflow in checked routes. | M | B | Playwright checks |
| FR-WEB-05 | Generated sites use accessible semantic structure, focus states, labels, and reduced-motion handling. | M | B | axe-core + manual |
| FR-WEB-06 | Nexora supports code-based 3D scenes with WebGL detection, performance guardrails, and fallback content. | S | 1.0 | Demo fixtures |
| FR-WEB-07 | Generated sites include a self-hosted or system font strategy and do not hotlink unlicensed assets. | M | B | Template audit |
| FR-WEB-08 | Build outputs are verified locally; deployment is explicitly out of scope and never claimed. | M | B | Report test |
| FR-WEB-09 | User can export a project as a normal folder with no Nexora runtime dependency. | M | B | Export test |

### 6.9 Preview and testing workspace

| ID | Requirement | Pri | Rel | Verification |
|----|-------------|-----|-----|--------------|
| FR-PRV-01 | Preview runs a project dev server on loopback only and displays it in an isolated frame. | M | B | Network test |
| FR-PRV-02 | Preview supports reload, route navigation, viewport presets, and opening in the system browser. | M | B | UI test |
| FR-PRV-03 | Console errors, failed requests, and server logs are captured in a console drawer. | M | B | Fixture test |
| FR-PRV-04 | Nexora can capture screenshots for verification and store them with the task report. | M | B | Screenshot test |
| FR-PRV-05 | Port conflicts, server crashes, and slow starts are handled with clear states and retry. | M | B | Fault tests |
| FR-PRV-06 | Preview cannot access Nexora IPC or privileged APIs. | M | B | Security test |
| FR-PRV-07 | User can stop the preview and all child processes from one control. | M | B | Kill test |

### 6.10 Checkpoints and rollback

| ID | Requirement | Pri | Rel | Verification |
|----|-------------|-----|-----|--------------|
| FR-CKP-01 | A checkpoint is created before the first write of a task, before dependency installation, and before any destructive operation. | M | B | Integration test |
| FR-CKP-02 | User can restore a single file, a step, or an entire checkpoint. | M | B | Restore tests |
| FR-CKP-03 | Restore verifies file hashes and refuses to overwrite files changed outside Nexora without confirmation. | M | B | Conflict test |
| FR-CKP-04 | Checkpoints are stored outside the user's project by default and never pollute the repository. | M | B | Layout test |
| FR-CKP-05 | Retention policy is configurable; pinned checkpoints are never auto-deleted. | S | 1.0 | Retention test |
| FR-CKP-06 | Rollback is recorded in the audit log and reflected in the task timeline. | M | B | Log test |

### 6.11 Settings and configuration

| ID | Requirement | Pri | Rel | Verification |
|----|-------------|-----|-----|--------------|
| FR-CFG-01 | Settings cover general, appearance, models, inference, permissions, privacy, advanced, and about. | M | B | UI test |
| FR-CFG-02 | Inference settings include context size, GPU layers, KV cache type, threads, and backend, with safe defaults and warnings. | M | B | Validation test |
| FR-CFG-03 | Invalid or unsafe settings are rejected with an explanation, not silently clamped. | M | B | Validation test |
| FR-CFG-04 | Settings can be reset per section or entirely. | S | B | UI test |
| FR-CFG-05 | Advanced settings are hidden behind an explicit "advanced" toggle. | S | B | UI test |
| FR-CFG-06 | Language is English at 1.0; user-facing strings are externalized for future localization. | M | 1.0 | String audit |

### 6.12 Privacy, diagnostics, and updates

| ID | Requirement | Pri | Rel | Verification |
|----|-------------|-----|-----|--------------|
| FR-DIAG-01 | No telemetry is sent by default. | M | A | Network trace |
| FR-DIAG-02 | Crash reporting is opt-in and never includes prompts, file contents, or secrets. | M | B | Payload review |
| FR-DIAG-03 | User can export a diagnostics bundle containing logs, hardware report, settings, and recent task metadata after reviewing its contents. | M | B | Export test |
| FR-DIAG-04 | Update checks are disclosed; offline mode disables them cleanly. | M | B | Network test |
| FR-DIAG-05 | Updates are signed, verified, and require user approval; the previous version remains available for rollback. | M | 1.0 | Update/rollback test |
| FR-DIAG-06 | A visible network activity log lists destinations contacted by Nexora itself (not by user project dependencies). | S | 1.0 | Network test |
| FR-DIAG-07 | User can delete all Nexora data (conversations, checkpoints, logs, models) with a clear scope selector. | M | B | Deletion test |

### 6.13 Accessibility and keyboard

| ID | Requirement | Pri | Rel | Verification |
|----|-------------|-----|-----|--------------|
| FR-ACC-01 | Every primary action is reachable by keyboard with visible focus. | M | B | Keyboard audit |
| FR-ACC-02 | The app exposes meaningful names, roles, and states to screen readers. | M | B | NVDA audit |
| FR-ACC-03 | Reduced-motion preference removes non-essential motion. | M | B | Emulation test |
| FR-ACC-04 | UI remains usable at 100–200% Windows scaling without clipped controls. | M | B | Visual matrix |
| FR-ACC-05 | Text and interactive states meet WCAG 2.2 AA contrast. | M | B | Contrast audit |
| FR-ACC-06 | A command palette exposes navigation and core actions. | S | B | UI test |

---

## 7. Non-functional requirements

Targets are measured on the reference hardware defined in [11-hardware-benchmarking.md](11-hardware-benchmarking.md). "Cold" excludes model load unless stated.

### 7.1 Performance

| ID | Requirement | Target | Measurement |
|----|-------------|--------|-------------|
| NFR-P-01 | Cold start to interactive shell | ≤ 2.5 s p95 | Automated startup trace |
| NFR-P-02 | Warm start to interactive shell | ≤ 1.2 s p95 | Automated trace |
| NFR-P-03 | Composer input latency | ≤ 50 ms p95 keystroke-to-paint | UI profiling |
| NFR-P-04 | Panel transition frame rate | 60 fps target, no long tasks > 100 ms | Performance trace |
| NFR-P-05 | Idle memory, shell + UI, no model | ≤ 250 MB | Process metrics |
| NFR-P-06 | Time to first streamed token | Runtime-dependent; UI shows progress within 150 ms of send | Event trace |
| NFR-P-07 | Streaming UI update cadence | 20–30 updates/s max, batched; no dropped characters | Instrumented stream |
| NFR-P-08 | Cancellation acknowledged in UI | ≤ 300 ms; runtime stops ≤ 2 s or is force-restarted | Fault test |
| NFR-P-09 | Open project tree, 10k files | ≤ 3 s to first screen; lazy expansion | Fixture test |
| NFR-P-10 | File search, 50k files, warm index | ≤ 500 ms p95 for typical query | Benchmark |
| NFR-P-11 | Model load | Visible staged progress; no UI freeze > 100 ms | Trace |
| NFR-P-12 | Large file open | Files > 2 MB open read-only with warning; > 10 MB not loaded in editor | E2E test |

### 7.2 Reliability and data integrity

| ID | Requirement | Target |
|----|-------------|--------|
| NFR-R-01 | Crash-free sessions | ≥ 99.5% during beta; ≥ 99.8% at 1.0 |
| NFR-R-02 | No committed user file loss from app crash | Zero; writes are atomic temp+rename with backups |
| NFR-R-03 | Conversation/task survives app restart | Full recovery of last committed state |
| NFR-R-04 | Inference crash recovery | Detect, report, offer reload; no app crash |
| NFR-R-05 | Disk-full handling | Detect before write/download; fail safely with guidance |
| NFR-R-06 | Corrupt database | Detect, attempt backup restore, never silently discard |
| NFR-R-07 | Update failure | Roll back to previous version automatically |
| NFR-R-08 | Orphaned processes | Zero child processes survive app exit or panic switch |

### 7.3 Security and privacy

| ID | Requirement | Target |
|----|-------------|--------|
| NFR-S-01 | Privileged operations through policy engine | 100% |
| NFR-S-02 | Secrets in logs or diagnostics bundles | Zero known leaks; automated scanner gate |
| NFR-S-03 | Release artifacts signed | 100% of stable/beta artifacts |
| NFR-S-04 | Remote content in privileged webview | None; preview isolated |
| NFR-S-05 | Model and runtime downloads verified | SHA-256 mandatory; signature for manifests |
| NFR-S-06 | Default telemetry | None |
| NFR-S-07 | Permission bypass findings | Zero P0/P1 at release |
| NFR-S-08 | Dependency vulnerabilities | No known critical/high reachable vulnerabilities at release |

### 7.4 Usability and accessibility

| ID | Requirement | Target |
|----|-------------|--------|
| NFR-U-01 | A new user can create and preview a site | ≤ 15 minutes on recommended hardware, excluding model download |
| NFR-U-02 | Primary workflows keyboard-complete | 100% of core actions |
| NFR-U-03 | WCAG | 2.2 AA for app UI |
| NFR-U-04 | Error messages | Plain language, cause, next action, no raw stack trace by default |
| NFR-U-05 | Window sizes | Usable at 960×640 logical px (compact mode); optimized at 1280×800 and above |
| NFR-U-06 | DPI | 100%, 125%, 150%, 200% without clipping |
| NFR-U-07 | Onboarding clarity | ≥ 90% comprehension in 5-user unmoderated test |

### 7.5 Compatibility and platform

| ID | Requirement | Target |
|----|-------------|--------|
| NFR-C-01 | OS | Windows 11 23H2+ x64; Windows 10 22H2 ESU best-effort, not release-gating |
| NFR-C-02 | WebView2 | Evergreen runtime; bootstrapper in installer; minimum tested version documented |
| NFR-C-03 | GPU | NVIDIA CUDA, AMD/Intel Vulkan, CPU fallback; exact matrix in doc 11 |
| NFR-C-04 | RAM | 16 GB minimum; 32 GB recommended |
| NFR-C-05 | Disk | 10 GB app + 20 GB models minimum; configurable |
| NFR-C-06 | Install | Per-user, no admin for normal use |
| NFR-C-07 | Uninstall | Removes app and runtimes; preserves user projects and optionally models |
| NFR-C-08 | ARM64 | Not supported at 1.0; detected and explained |

### 7.6 Maintainability and operability

| ID | Requirement | Target |
|----|-------------|--------|
| NFR-M-01 | Module boundaries | Enforced by crate/package dependency rules in CI |
| NFR-M-02 | Automated tests | ≥ 70% line coverage in core crates; critical policy paths ≥ 90% |
| NFR-M-03 | Lint/format | `cargo clippy -D warnings`, `rustfmt`, ESLint, Prettier, TypeScript strict |
| NFR-M-04 | ADRs | Every irreversible or cross-cutting decision recorded |
| NFR-M-05 | Build reproducibility | Pinned toolchains; locked dependencies; SBOM per release |
| NFR-M-06 | Local observability | Structured rotating logs with redaction; no cloud dependency |
| NFR-M-07 | Localization readiness | Strings externalized; no concatenated sentences |
| NFR-M-08 | Documentation | Architecture, security, user guide, troubleshooting, model licences |

### 7.7 Efficiency and resource use

| ID | Requirement | Target |
|----|-------------|--------|
| NFR-E-01 | Glass effects | Maximum 3 simultaneous blurred layers; no blur behind body text; GPU cost measured |
| NFR-E-02 | Background work | Indexing and downloads throttled and pausable; no inference while idle |
| NFR-E-03 | Model unload | Optional idle unload after configurable period to free VRAM |
| NFR-E-04 | Installer size | Target ≤ 250 MB without models or GPU runtime packs |
| NFR-E-05 | Update size | Target ≤ 80 MB typical delta/full update |

---

## 8. Assumptions and constraints

| ID | Assumption / constraint | Impact if false |
|----|-------------------------|-----------------|
| A-01 | Most target users have Windows 11 with WebView2 available. | Installer and support burden increase. |
| A-02 | A 20–30B quantized model can provide useful agentic coding on 16–24 GB GPUs. | Product must target smaller models or accept lower quality. |
| A-03 | Users accept a 10–20 GB model download for good local quality. | Product needs a smaller default and clearer trade-offs. |
| A-04 | Local dev servers and package managers work acceptably inside process containment. | Sandbox must be relaxed for builds, reducing isolation. |
| A-05 | Edge is present for headless visual checks. | Bundle Chromium (~150 MB) or lose screenshot checks. |
| A-06 | Users can provide a project folder and, for installs, network access. | Offline project generation limited to zero-dependency templates. |
| A-07 | The team can maintain a Rust + TypeScript codebase. | Stack should shift toward Electron/TypeScript. |
| A-08 | Model licences permit commercial desktop distribution of a client that downloads them separately. | Model shortlist and distribution model must change. |
| A-09 | No cloud service is required for licensing or updates. | Adds backend and privacy surface. |
| A-10 | Local models will remain worse than frontier hosted models on some tasks. | Product must set expectations and design for verification, not hype. |

---

## 9. Success metrics

### Product outcomes

- **Activation:** ≥ 60% of users who complete onboarding create or open a project within the first session.
- **First useful result:** median time from brief to local preview ≤ 15 minutes on recommended hardware (excluding model download).
- **Task completion:** ≥ 70% of benchmarked website tasks reach "build + preview" without human code edits; ≥ 50% reach "tests pass".
- **Trust:** zero false "verified/works" claims in the release evaluation suite.
- **Retention:** ≥ 35% weekly return rate among activated users in beta.
- **Support load:** ≤ 5% of sessions produce a diagnostics export; top issues documented.

### Engineering outcomes

- Crash-free sessions ≥ 99.5% beta, ≥ 99.8% 1.0.
- Zero P0 security findings at release.
- Startup and input-latency budgets met on reference hardware.
- Agent repair loops never exceed configured bounds in fault-injection tests.
- All release artifacts signed and reproducible from a tagged commit.

Metrics that will **not** be used to claim quality: raw token throughput alone, vendor benchmark scores alone, number of files generated, or lines of code produced.

---

## 10. Requirement traceability

Each requirement must map to at least one design section, test, and phase deliverable. The traceability matrix is maintained in [08-testing-and-quality.md](08-testing-and-quality.md) Section 8 and updated at every phase gate. Requirements marked Must and not covered by a later-phase deliverable block 1.0.

---

## 11. Open product questions

The following require owner input and are repeated with options in [12-roadmap-risks-and-decisions.md](12-roadmap-risks-and-decisions.md) Section 5:

- Q-01 Windows 11-only baseline vs Windows 10 best-effort.
- Q-02 Minimum hardware tier and whether CPU-only is officially supported.
- Q-03 Maximum acceptable default model download size.
- Q-04 Free/paid licensing and whether the app may contact a licence server.
- Q-05 Whether an optional bring-your-own cloud endpoint is permitted at 1.0.
- Q-06 Code-signing budget and certificate type.
- Q-07 Telemetry and crash-report policy.
- Q-08 Git write operations at 1.0.
- Q-09 Deployment/publishing roadmap.
- Q-10 English-only at 1.0.
- Q-11 Windows ARM64 timing.
- Q-12 Brand/trademark clearance for the name and icon.

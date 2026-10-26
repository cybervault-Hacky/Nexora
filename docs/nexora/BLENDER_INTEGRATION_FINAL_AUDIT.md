# Blender Integration — Final Master Audit

**Audit date:** 2026-10-26
**Branch / base:** `arena/11fb4a83-nexora` / `517b5c9aa9960e413aa4991acd3ab8212476f949`
**Overall readiness:** **NOT PRODUCTION READY**
**Phase 5 transport gate:** **NO-GO FOR PRODUCTION**
**Release decision:** None made. This report is an evidence audit, not owner approval or a release authorization.

## 1. Executive result

The existing Blender work is a collection of candidate-only installation detection, UI catalogue behavior, strict transport-independent action/scene/animation/workflow/preview contracts, and image-reference preparation. It is **not** a connected or executable Blender integration. No Blender process, transport, add-on, live scene source, task executor, native integration command for planning/execution, or end-to-end consent/task lifecycle is present. The only registered Blender Tauri command checks installation status; it does not connect to Blender.

The available frontend, installer-contract, source-kit, and isolated Vite build checks passed in this audit. They do not compile or run the Rust core, validate Windows behavior, exercise a Blender runtime, prove peer identity/transport security, or evidence a scene operation. The host lacks the pinned Rust tools, Windows/PowerShell, and Blender. Thus the Rust tests authored for Phases 6–12 remain **NOT RUN**, and the Windows/Blender/transport requirements remain **BLOCKED**.

No reproducible in-scope implementation defect was confirmed in the source-reviewable paths, so no application-code fix was justified. This is not a clean bill of health: uncompiled Rust and untested platform/runtime boundaries prevent a full stabilization claim. Existing cleanup limitations, unavailable transport/cancellation, provenance inconsistencies, and absent release evidence are recorded below. Phase 5 remains NO-GO; production remains NOT READY.

## 2. Scope, authority, and preservation

This audit covers App Integrations and installation detection; Phase 5/5A/5B transport evidence and security; Phase 6 action parsing, permissions, exact confirmation and disabled execution; image-reference boundaries and cleanup; scene/animation/workflow planning; Phase 11 preview/resource admission; and Phase 12 Windows/security/package/docs evidence. It inspected both source paths and tests, then reran all feasible checks after completing the review. It did not add Phase 13, change a roadmap, alter historical phase reports, or make a release decision.

**Authority/provenance limitation:** `BLENDER_INTEGRATION_12_PHASE_PLAN.md` identifies itself as a *Proposed 12-Phase Implementation Outline*, says Phases 2 onward were not started at the time of its creation, and requires owner acceptance/authorization. `BLENDER_INTEGRATION_PHASE_12_FINAL_VERIFICATION.md` describes the plan as approved. The current task authorized this final verification, but that is not independent evidence of the plan's earlier owner approval. This audit does not infer or invent that approval. The proposal/approval inconsistency is documented, not resolved here.

**Phase-label inconsistency:** `BLENDER_INTEGRATION_PHASE_5_IMPLEMENTATION.md` scopes Phase 5 as secure local communication, while the source outline labels Phase 5 read-only scene inspection and Phase 4 the connection lifecycle. The Phase 5 report already records this discrepancy. No report or phase number was rewritten.

The worktree was already dirty and contained substantial tracked modifications, untracked project directories, and ignored dependency/build outputs before this audit. They were preserved. No reset, clean, stash, restore, stage, commit, push, merge, tag, publish, or release was performed. The existing ignored `apps/desktop/ui/dist` was not overwritten: the isolated Vite build used `/tmp/nexora-blender-final-audit-dist`.

## 3. Evidence rules and environment

Statuses below apply to the stated claim and evidence class—not to a general notion of “phase complete.” **PASS** means the specified source/UI/static check passed at its stated boundary. It does not silently imply native compilation or runtime validation. **FAIL** identifies a demonstrated requirement/document mismatch. **BLOCKED** means evidence requires unavailable platform/runtime/authorization. **NOT RUN** means no test was executed. **NOT APPLICABLE** means a capability is deliberately absent from this candidate-only build; it is not a production waiver.

Observed environment: Linux x86_64; Node.js `v22.22.3`; Corepack pnpm `9.15.4`; root `rust-toolchain.toml` pins Rust `1.93.0` with `rustfmt` and `clippy`. `cargo`, `rustc`, `rustfmt`, `rustup`, `blender`, `powershell`, and `pwsh` are unavailable. No Windows host/MSVC runtime or Blender session was available. The package manager, scripts, toolchain pin, and `.github/workflows/ci.yml` were inspected. CI defines frontend checks and Linux/Windows Rust build/test jobs, but this audit did not inspect a remote CI run; the workflow does not establish a real Blender transport/scene qualification result.

Evidence sources inspected include the proposed plan, architecture, connector contract, product/security requirements, Phase 5/5A/5B reports and kit, Phase 6–12 reports, current Rust/UI/Tauri source and tests, CI/scripts/manifests, and the pre-existing verification evidence index. The Phase 5B result template remains at `NO_GO` with all 18 transport cases `NOT_RUN`. The static checker explicitly says it does not execute PowerShell or test Windows pipes, Rust, Blender, peer identity, or a handshake. No actual POC logs, transport results, Blender scene, render, Windows qualification evidence, or release artifacts were found in the inspected transport evidence set.

## 4. Requirements-to-evidence checklist

| ID | Requirement / claim | Status | Evidence and boundary |
|---|---|---|---|
| A1 | App Integrations is one searchable catalogue; lifecycle facts are not collapsed into a green “ready” state. | **PASS** | `AppIntegrationsPage.tsx`, catalogue definitions and tests cover search/category/empty state, explicit detection, and distinct installation/connection/readiness display. This is a UI behavior result only. |
| A2 | Blender detection is user-initiated; browser preview does not scan; candidate presence does not imply connection/readiness. | **PASS** | Frontend `native.ts`, page, Tauri command registration and tests show explicit IPC only; browser reports unsupported without native invocation; no Connect/Disconnect controls; candidate copy is explicit. |
| A3 | Native discovery is bounded, read-only, path-confined and does not launch a candidate. | **PASS (source review)** | `integrations/detector.rs` checks three standard Windows roots, at most 128 entries per root, at most 16 reportable candidates, basename/canonical-root confinement and bounded PE structure; no process launch/registry/recursive scan. Windows execution and Rust unit tests are **NOT RUN**. |
| A4 | Installation evidence proves Blender publisher/product/version and compatibility. | **NOT APPLICABLE to current detector; BLOCKED for production** | Detector intentionally reports `candidate_only`; publisher/product identity, version and compatibility are not verified. No trusted/selected installation is recorded. |
| A5 | Real Windows transport, explicit user-only ACL, endpoint ownership, Blender client support, bidirectional peer identity, challenge/handshake and lifecycle evidence. | **BLOCKED — NO-GO FOR PRODUCTION** | No pipe/TCP implementation, bridge/add-on, framing, authentication, peer checks, connection manager or handshake exists. Phase 5A/5B explicitly require an authorized real Windows + Blender POC; the 18-case result template is still `NOT_RUN`/`NO_GO`. |
| A6 | Transport adversarial cases: other user/session, remote client, squatting, identity races, malformed/oversized data, duplicate clients, timeouts, cancellation, restart/reconnect. | **NOT RUN / BLOCKED** | No endpoint or harness exists. Static kit consistency is not a test of these properties; no mock was counted as evidence. |
| A7 | Action parsing is strict, bounded, typed and allowlisted; arbitrary Python/shell/eval/operator strings are excluded. | **PASS (source review)** | `blender_actions.rs` implements byte/action/string/vector/enum/numeric bounds, duplicate-key rejection on the JSON entry point, strict fields, typed `ValidatedBatch`, and a fixed action set. Rust tests are authored but **NOT RUN**. |
| A8 | Permission classes and exact user confirmation bind to the validated mutation payload, not just a reusable request ID. | **PASS (contract source); BLOCKED end-to-end** | `authorize_batch` checks connector approval, every action permission, and exact `ValidatedBatch` equality. Workflow binding compares the full preview and candidate batches. No consent UI, durable grant/receipt store, Tauri planning/authorization command, or executed host confirmation flow exists. |
| A9 | Stale revision, scene mismatch, missing/ambiguous target and incomplete name resolution fail closed. | **PASS (planner source); BLOCKED live-scene evidence** | Scene, animation, and workflow planners validate supplied snapshot scope/revision and targets. A supplied schema-valid snapshot is not genuine Blender provenance; no live snapshot source or revision provider exists. Rust tests were not run. |
| A10 | Executor cannot execute or report success without the approved path. | **PASS (source review)** | `UnavailableBlenderExecutor` always returns `connector_unavailable`; planners retain candidates/previews, not execution receipts. No execution command or Apply/Run UI exists. Rust tests confirming this remain **NOT RUN**. |
| A11 | Image references are validated locally, path-free in transcript metadata, size/dimension-bounded and separated from image understanding. | **PASS (frontend/source); BLOCKED native runtime** | UI preflight caps selected files at 8 MiB; existing Rust PNG importer is reused; metadata carries an opaque ID and bounded dimensions/size; task asserts `analysisAvailable: false`. No image-analysis capability is claimed. Rust importer/decoder tests were not compiled or run here. |
| A12 | Image-reference request/metadata/bytes do not enter coding-model turns or a provider request. | **PASS (source/test boundary)** | Local-only message handling and `toChatTurns` filtering are covered by frontend tests; image bytes go to local Tauri import, not a model call in this flow. This is code-path inspection and frontend coverage, not a network trace or native IPC runtime test. |
| A13 | Image reference cleanup on remove, replace, switch, unmount, submit and conversation deletion is visible and safe. | **PASS for covered normal paths; LIMITATION remains** | UI revokes object URLs and requests deletion by opaque ID; deletion failures are surfaced. Phase 7 documents that process termination can leave an orphan and there is no startup orphan sweep; deletion cannot be guaranteed when the managed-source delete fails. Native storage tests were not run. |
| A14 | Scene edit plans are bounded and do not fabricate objects or claim applied work. | **PASS (planner source); BLOCKED operationally** | Scene planner accepts supplied validated data, resolves exact IDs/names, reports unavailable/stale/ambiguous cases, and returns candidates/previews. No genuine scene reader, Tauri planning route, execution, or Blender verification exists. |
| A15 | Animation is bounded to explicit keyframes/object types and exact scene revision; no rig/general animation claim. | **PASS (planner source); BLOCKED runtime** | Phase 9/11 planners constrain frame/value/keyframe ranges and target types and require mutation confirmation metadata. No Blender keyframe or rig operation has been performed; Rust tests are unrun. |
| A16 | Workflow validation, dependency handling, permission/confirmation scope, image association and status accurately distinguish plan from execution. | **PASS (source review); BLOCKED native tests** | Workflow validates schema/size/dependencies, propagates failed/unavailable dependencies, scopes scene/revision, compares preview plus exact candidates, and reports no execution. Phase 12 source includes the preview/workflow scope equality check and regression test. Those Rust tests are not compiled/run. Proposed outputs are not composed into later inputs. |
| A17 | Phase 11 preview requests are bounded and resource admission never treats unknown evidence as admitted. | **PASS (source review); BLOCKED measurement/runtime** | Preview planner bounds frame/resolution/samples, checks camera/revision, reports `Unknown` or `Deferred`, and writes no output. Shared coordinator does not reserve for Blender and keeps unknown peak system/GPU/disk evidence unadmitted. No Blender measurements, preview or concurrent render test exists. |
| A18 | Progress, cancellation, task errors, shutdown and cleanup are tied to a real connector task/session. | **BLOCKED / NOT APPLICABLE to current planner-only code** | No Blender task/session, progress channel, cancel command, transport, or shutdown lifecycle exists. Detection errors are converted to safe status results, but this is not task cancellation evidence. |
| A19 | Windows qualification, PowerShell preflight, MSVC/Tauri build, Windows tests and clean install/update/uninstall. | **BLOCKED / NOT RUN** | Linux-only host; PowerShell/Windows/MSVC unavailable. Phase 5B script was inspected, not parsed or executed. CI configuration is not a run result and cannot substitute for the named-pipe/Blender tests. |
| A20 | Signed add-on/app payload provenance, supported Windows/Blender matrix, license and rollback/security review. | **BLOCKED / NOT RUN** | No Blender add-on or support matrix exists; no signed transport artifact, measured supported build matrix, accountable threat-model approval, license decision or rollback rehearsal was evidenced. Current installer-contract tests do not qualify an add-on. |
| A21 | Frontend↔backend serialization boundary and schema parity for Blender operations. | **PARTIAL / BLOCKED** | Installation status has a registered Tauri command and runtime-shape validation in the UI. Action/scene/animation/workflow/preview DTOs are Rust contracts with TypeScript summary/preview mirrors but no registered Blender planning/execution IPC. Rust serialization and Tauri runtime behavior are unverified. |
| A22 | Documentation and authority/status statements are consistent and supported by evidence. | **FAIL — provenance/phase-label discrepancies remain; no owner approval inferred** | Plan says proposed/owner review; Phase 12 calls it approved. Phase 5 report's scope label differs from the plan. Reports otherwise preserve NO-GO, candidate-only, unrun Rust/Windows/Blender caveats. The inconsistencies are retained rather than silently resolved. |
| A23 | Release readiness has genuine evidence for all advertised capabilities and separate owner authorization. | **BLOCKED — NOT PRODUCTION READY** | No real transport, Blender operation, Windows support matrix, license/security sign-off, or owner release authorization. Frontend/static checks do not clear these gates. |

## 5. Cross-cutting implementation review

### App Integrations and detection

The current catalogue is static metadata plus a user-triggered Blender detector. Blender remains marked planned; `connectionStatus` is `null`, action list is empty, and readiness is blocked. `null` is not misrepresented as a measured `Disconnected` state. Detection returns a candidate-only result with no version or publisher verification; the UI preserves that distinction and does not select, launch, trust, or connect to the candidate.

The Windows source uses standard `ProgramFiles`, `ProgramFiles(x86)`, and `LOCALAPPDATA` roots, direct Blender-named child directories, canonical path containment and structural PE checks. Non-Windows returns unsupported without scanning. Source inspection supports the stated bounded candidate-detector contract; Windows behavior, path races, permissions, and the Rust tests remain unverified.

### Phase 5 transport evidence and security

No Blender transport implementation exists. There is no named pipe, TCP listener, pipe DACL, authentication material, challenge, peer process binding, protocol frame, add-on, health session, or Tauri connection lifecycle. The Phase 5A comparison correctly treats named pipes as an unadopted spike candidate and loopback TCP as rejected for this phase; the Phase 5B static kit is preparation only. The NO-GO gate remains controlling. No transport readiness or candidate-installation trust is inferred.

### Phase 6 action/permission boundary

The action parser is a contract layer, not an AI planner or command endpoint. It uses an allowlist and typed validated actions; mutations are distinct from read-only requests. Authorization requires approved connector state and host-owned permission/confirmation facts. Exact batch equality prevents same-ID/different-payload confirmation reuse. This policy foundation is not an implemented user-consent flow, permission store, or live connector. The executor is disabled and fails with `connector_unavailable`; no arbitrary code path was found in reviewed integration code.

### Image references and cleanup

The in-chat image path keeps the selected file separate from normal model turns, validates a bounded PNG through the existing local importer, and stores only opaque-ID metadata/task text in local-only conversation records. The UI's object URL and managed-source cleanup paths were reviewed. Known limitations remain: a hard process exit can leave an unreferenced managed source; no startup orphan sweep/management screen exists; deletion failure cannot guarantee removal; PNG ancillary metadata is preserved rather than comprehensively stripped; persisted references do not restore a thumbnail. No bytes were analyzed by vision or sent to a provider in the reviewed reference-preparation path.

### Scene, animation, workflow and preview/resource planning

These Rust modules validate supplied data and create typed, reviewable candidates/previews. They do not discover a live Blender scene, provide genuine object IDs/revisions, apply actions, create keyframes, render, or write artifacts. Exact target/revision checks and workflow binding are present in source. Workflow dependencies order review and propagate blocked/unavailable states; they do not simulate outputs or compose earlier proposals into later inputs. Phase 11 preview/resource code remains conservative: request bounds are explicit; unknown Blender measurements stay unknown; active shared reservations defer; no Blender work is reserved or admitted. Rust compilation and unit/regression tests remain unverified.

### Frontend/backend boundaries, cancellation, errors and status

The detector is the only registered Blender-specific Tauri command. It reports bounded status; the UI validates the tagged result, coalesces overlapping requests, renders errors as safe status copy, and suppresses stale UI updates after unmount. The broader typed Rust planners are not wired through a serialized Tauri command; TypeScript helpers are not an integration transport. No connector task exists to cancel, stream progress for, reconcile after disconnect, or clean up at a task safe point. This absence is an implementation blocker—not successful cancellation behavior.

### Packaging and documentation

Current installer-contract checks confirm only the existing frontend/package contract and absence of model/runtime/credential payloads in that checked frontend bundle. They do not establish signed Blender add-on provenance, Windows installation behavior, supported app versions or release readiness. The proposed connector/requirements docs remain proposals. Phase reports are historical records and were left untouched; the Phase 12 report was not edited because this audit found no new reproducible code defect warranting a historical update.

## 6. Findings and stabilization decision

1. **No reproducible in-scope code defect confirmed.** Source-reviewable paths and their frontend/static tests behaved consistently with their stated candidate-only/data-only boundaries. No application-code fix was made. Absence of a compiler/runtime means this is not a claim that all Rust/native paths are defect-free.
2. **Transport/security qualification is a release blocker, not a passing test.** Phase 5A/5B remains NO-GO. The missing Windows peer/ACL/handshake evidence cannot be synthesized by this host or inferred from source/static tests.
3. **The native Rust surface is unverified.** The repo pins Rust `1.93.0`; no `cargo`, `rustc`, `rustfmt`, or `rustup` is installed. All Rust compilation, formatting, Clippy, and unit-test outcomes below are blocked/not run.
4. **No actual Blender/Windows evidence.** There are no real scenes, IDs, keyframes, renders, measurements, Windows installation records, signed add-on artifacts, or genuine peer handshakes in the evidence examined.
5. **Documentation authority discrepancy remains.** The roadmap's proposed status and the Phase 12 report's approved wording are inconsistent; the current verification authorization does not establish past owner approval. Phase 5 numbering/scope also differs from the outline. No document was changed to make an unsupported authority claim.
6. **Image storage cleanup is best-effort, not complete retention management.** Normal paths have explicit cleanup/error handling, but the documented process-crash orphan gap and absence of an orphan sweep remain.

**Stabilization outcome:** no source fix was safe or justified from reproduced evidence. Preserve the current fail-closed behavior and collect the missing platform/runtime evidence in a separately authorized, reviewed Windows + Blender environment. Do not enable a connector, mark task readiness, or change Phase 5's NO-GO based on this audit.

## 7. Commands run and exact results

Commands were run from `/home/user/Nexora` on the pre-existing dirty worktree. No dependency reinstall was performed during this audit, avoiding mutation of ignored dependency directories. The existing ignored `apps/desktop/ui/dist` was left untouched.

| Command | Result |
|---|---|
| `corepack pnpm --dir apps/desktop typecheck` | **PASS**, exit 0; `tsc --noEmit`. |
| `corepack pnpm --dir apps/desktop test -- --reporter=dot` | **PASS**, exit 0; **98 test files, 930 tests passed**. Frontend/UI/source-contract coverage only; no Rust or Blender runtime. |
| `corepack pnpm test:installer-contract` | **PASS**, exit 0; **17/17 tests**. |
| `node scripts/check-config-security.mjs && node scripts/check-installer-contract.mjs` | **PASS**, exit 0; config security passed; installer contract reported 3 frontend assets / 763,529 bytes and no model/runtime/credential payloads. |
| `corepack pnpm --dir apps/desktop exec vite build --outDir /tmp/nexora-blender-final-audit-dist` | **PASS**, exit 0; Vite transformed 135 modules. Non-fatal dynamic/static import chunk warnings and a minified chunk of 619.55 kB (>500 kB warning). Output was directed outside the repository to preserve the existing ignored build output. |
| `python3 tools/blender-transport-spike/check_static.py` | **PASS**, exit 0; 18 test IDs align; template remains `NO_GO`/`NOT_RUN`; preflight source markers pass. Output explicitly limits the check from PowerShell/pipe/Rust/Blender/identity/handshake evidence. |
| `git diff --check` | **PASS**, exit 0; no whitespace errors in the pre-existing tracked diff at audit time. |
| Environment inventory: `uname -srm`; `node --version`; `corepack pnpm --version`; `command -v` for `cargo rustc rustfmt rustup blender powershell pwsh`; inspect `rust-toolchain.toml` | Linux `6.1.158+ x86_64`; Node `v22.22.3`; pnpm `9.15.4`; all seven platform/native tools unavailable; Rust pin `1.93.0`. |
| Native Rust checks (`cargo --locked fmt --all -- --check`, `cargo --locked check --all-targets`, `cargo --locked clippy --all-targets -- -D warnings`, `cargo --locked test`) | **BLOCKED / NOT RUN**: `cargo` and Rust tools unavailable. No successful native Rust check is claimed. |
| Windows/PowerShell/Blender/transport checks | **BLOCKED / NOT RUN**: no Windows, PowerShell, Blender runtime, POC transport, peer, or handshake. No actual check was attempted or claimed. |

No application-code changes were made, so there is no before/after regression test or defect-fix claim. Frontend checks above were rerun against the final reviewed source state. The isolated Vite output under `/tmp` is not an application artifact or release build.

## 8. Readiness by capability area

| Area | Readiness |
|---|---|
| Architecture/contracts and candidate-only UI | **SOURCE-LEVEL FOUNDATION; frontend tests PASS** |
| Installation detector | **Candidate-only, Windows runtime unverified** |
| Frontend/backend detector boundary | **PASS at source/frontend-test level; Tauri runtime unverified** |
| Blender transport, peer identity, handshake, health/reconnect | **BLOCKED; Phase 5 NO-GO FOR PRODUCTION** |
| Action parser/policy/executor | **Contract-level source present; Rust checks NOT RUN; executor intentionally disabled** |
| Image references | **Local data-preparation path; Rust/runtime unverified; cleanup limitations remain** |
| Scene planning / animation / workflows | **Supplied-data planning only; no live snapshot or execution evidence** |
| Preview / resource admission | **Conservative planning only; measurements absent; Blender preview not admitted/executed** |
| Windows qualification / Blender runtime / security adversarial tests | **BLOCKED / NOT RUN** |
| Packaging, signatures, license, support matrix, release authorization | **BLOCKED / NOT ESTABLISHED** |
| **Overall** | **NOT PRODUCTION READY** |

## 9. Closure

Keep Blender candidate-only and non-connectable; keep the executor disabled; keep Phase 5 **NO-GO FOR PRODUCTION**. Required next evidence is a separately authorized/reviewed Windows + supported Blender POC and subsequent actual compile/test/security/package qualification—not a roadmap expansion or a source-only status change. This audit ends here. No Phase 13, roadmap change, commit, release decision, or publication was made.

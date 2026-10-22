# Phases 5–7 — Chat selector, routing, and existing workflows

## 1. One interface, one control

There is no Sonnet page, no per-model dashboard, and no second chat. The only new UI is a
compact control above the existing composer:

`ui/src/components/ModelSelector.tsx`

- a labelled `<select>` (`label` + `id`, keyboard operable, `aria-describedby` for its status);
- options rendered as `"{displayName} — {readiness label}"`, where the label is the human
  name for the core's readiness state (`Ready to download`, `Verification required`,
  `Ready to load`, `Loaded`, `Not enough memory`, …). The set of states is documented in
  [READINESS.md](./READINESS.md); the UI maps names, and never invents a state;
- one status line beneath it, `aria-live="polite"`;
- disabled while a reply streams, with a title explaining why;
- renders nothing at all when there is no desktop core (browser preview).

The component holds no availability logic. Every status string comes from
`get_model_catalog`, so the UI cannot disagree with the core about whether a model is ready.
Since the readiness phase the status line shows the core's own sentence for the selected row
(`entry.readiness.detail`) rather than a generic one, and the `data-state` attribute carries
the raw state name for styling.

## 2. Switching models

`useModelCatalog` fetches the catalogue, exposes `select(id)`, and re-reads the catalogue after
a selection instead of assuming the outcome. `select_model` persists only the id — it never
reads, writes, or validates a model file, so switching cannot silently repoint the configured
path.

**A switch is refused while the engine is busy** (see §7). `select_model` and `save_settings`
both call `ensure_switch_allowed`, which asks `registry::check_switch` with the real generation
and load state. A refused switch returns `busy` with a sentence naming the target profile, the
hook stores that sentence in `switchError`, and the selector shows it instead of the readiness
line. The selection genuinely did not move, so the control and the core still agree.

**Conversation state is untouched by a switch.** The conversation lives in
`usePersistentConversation` at the shell level and neither the hook nor the selector touches it,
so the transcript survives; only the engine binding changes.

## 3. Never the wrong model

Three independent layers:

1. **Core guard.** `start_generation` calls `check_selection` before reserving a usage unit.
   A Sonnet request cannot be answered by Mini, and an unavailable profile is refused with
   `artifact_pending`.
2. **Catalogue signal.** `get_model_catalog` returns `selectionError`, produced by the same
   function, and the composer disables sending with that exact message.
3. **Truthful availability.** A profile reads `Loaded` only when its exact file is on disk,
   its digest was confirmed in this session, the engine is running, and it is the model that
   was loaded. `unverified`, `downloaded_verified` and `ready_to_load` are distinct states, and
   `unavailable` can never be described as installed. The earlier four-value status
   (`ready`/`installed`/`not_installed`/`unavailable`) was replaced by the fourteen readiness
   states in [READINESS.md](./READINESS.md).

`load_model` records what actually started (`LoadedModel { profile_id, path, context_size }`),
and clears it on unload and on every failure path.

A fourth layer was added in Phase 6: **the request names the profile it was composed under.**
`GenerateRequest.profile_id` is optional; when present, `registry::check_request_profile`
requires it to be the selected profile and refuses otherwise. This is not how the model is
chosen — the core still reads its own settings — it is how a *stale* caller is caught. A chat
pane rendered before a switch would otherwise be answered by a different model than its
composer names. An unknown id is refused rather than ignored, and omitting the field keeps an
older client working against the settings-derived selection. The attribution is by file name, so it is a statement
about identity, not about byte integrity — integrity is `verify_model`'s job.

## 4. Reuse of existing workflows

| Workflow | Reused unchanged? | Notes |
| --- | --- | --- |
| Streaming, stop, timeouts | Yes | `client::stream_chat` — first-token 600 s, idle 120 s, `Notify` cancellation |
| Engine supervision | Yes | `Supervisor::start(runtime, model, ctx, dir)`, bounded `stop()`, `Drop` |
| Usage allowance | Yes | 50 generations/day, UTC midnight reset, reserved before the engine is asked to work; unchanged by which model is selected |
| Export (single file, ZIP, Download Mode) | Yes | Model-independent; no Sonnet-specific paths |
| Context budget | Yes | Uses the engine's reported context size; Sonnet's recommended 8,192 is shown in setup but never applied silently |
| History / persistence | Yes | No schema change |

## 5. Model setup page

`ModelSetup` follows the selection rather than assuming Mini:

- refetches when the selected profile changes;
- shows "Selected model" with the profile's real name;
- disables **Download** and **Verify my file** when the artefact is `pending`, with a title and
  an inline explanation of why;
- passes `profileId` to both commands;
- persists `modelId` alongside the paths so saving settings cannot clear the selection.

## 6. Files touched in these phases

| File | Change |
| --- | --- |
| `ui/src/components/ModelSelector.tsx` | new |
| `ui/src/components/ModelSelector.test.tsx` | new, 6 tests |
| `ui/src/features/models/useModelCatalog.ts` | new |
| `ui/src/pages/ChatPage.tsx` | renders the selector, blocks on `selectionError` |
| `ui/src/App.tsx` | owns the catalogue, refreshes it on engine transitions |
| `ui/src/pages/ModelManagerPage.tsx` | passes the selected id through |
| `ui/src/components/ModelSetup.tsx` | selection-aware download/verify/save |
| `ui/src/lib/ipc.ts`, `ui/src/lib/inference.ts` | `ModelArtifact`, `ModelCatalog`, `selectModel`, profile-aware calls |
| `ui/src/styles/app.css` | styles for `.model-select*` |
| `ui/src/lib/rustContract.test.ts` | contract fixtures extended for the new surface |

---

## 7. Phase 6 — integration completed and re-verified

Phase 6 traced the whole path — composer → `generate()` → `start_generation` → `stream_chat`
→ Tauri channel → transcript — and closed the gaps it found. It added no page, no second chat,
no engine, and no new capability.

### 7.1 What was missing, and what changed

| Gap found | Change |
| --- | --- |
| A message carried no record of the profile it was composed under, so a pane rendered before a switch could be answered by another model | `GenerateRequest.profileId` (optional) + `registry::check_request_profile`; all four `generate()` call sites name the profile at send time (`usePersistentConversation`, `useConversation`, `usePatchGeneration`, `usePatchRepair`) |
| Switching was only disabled in the chat UI; the settings form and any other caller could switch while the engine was answering or loading | `registry::check_switch` + `ensure_switch_allowed` in both `select_model` and `save_settings`; the refusal is surfaced in the selector as `switchError` |
| The engine panel named `settings.model_path` even while a different file was loaded | `status()` reports the path of the engine that is recorded as loaded; `model_path` in the same payload stays configuration, for the setup form |
| A request budget was checked against the *configured* context, which can differ from the context the running engine was started with | `LoadedModel.context_size` is recorded at load; `start_generation` budgets against the running engine, before reserving usage |
| `usePatchRepair` and `usePatchGeneration` sent no profile | both name the selected profile on every request they issue |

No silent fallback exists anywhere on this path: `artifact_pending`, `model_unavailable` and
`model_mismatch` are surfaced verbatim, and nothing is substituted.

### 7.2 Usage accounting (unchanged rules, now actually exercised)

The rules are untouched: one generation per accepted request, reserved before the engine is
asked to work, committed only after a 2xx that begins streaming, released on anything earlier,
and reset at 00:00 UTC. What changed is that the `usage` module's 25 tests can now run in this
environment at all.

They could not before: the tests pin `TODAY = 20_746` (2026-10-20) while `UsageState::load`
read the wall clock, so from 2026-10-21 the stored day looked *ahead* of the test's clock and
every test saw a full 50/50 day. `UsageState::load_at(config_dir, now)` now takes the instant,
`load()` is `load_at(.., SystemTime::now())`, and the tests load at the instant they reason
about. Production behaviour is identical; the tests are no longer date-dependent.

### 7.3 Tests run for this phase

| Check | Command | Result |
| --- | --- | --- |
| Typecheck | `tsc --noEmit` (`apps/desktop`) | 0 errors |
| Frontend tests | `vitest run` | **849 passed / 849**, 84 files |
| Frontend build | `vite build` | built, `index-N2i249U6.js` 550.55 kB, CSS 102.37 kB |
| Rust formatting | `rustfmt --check` on all 44 files under `src-tauri/src` | clean |
| Rust unit tests | standalone `rustc --test` harness | **151 passed / 151** |

The harness compiles the modules that have no Tauri or network dependency — `budget` 9,
`loading` 13, `readiness` 41, `registry` 39, `models` 19, `hardware` 5, `usage` 25 — against a
stub `serde` that supplies trait shapes only. **It is not `cargo test`.** `commands.rs`,
`mod.rs`, `client.rs` and the Tauri surface cannot be compiled here.

New and extended tests:

| Area | Tests |
| --- | --- |
| Profile routing (backend) | a request prepared for the selected profile is accepted; no expectation uses the stored selection; a stale request cannot be answered by another profile (both directions); an unknown requested profile is refused; switching is allowed only while idle; refused during a generation; refused during a load |
| Profile routing (UI) | every request names the profile selected *at send time*; no profile is named when there is none; conversation order and states survive a switch; a switch never re-sends; patch generation carries the profile; the catalogue shows the core's refusal and keeps the old selection; the selector renders that refusal |
| Stream integrity | a delta for another request id is dropped; a broken stream becomes `error`, not `complete`, and is not resent as an assistant turn; a stopped reply stays `stopped` with its partial text through save/restore |
| Usage | the 25 existing accounting tests, now clock-independent (reserve/commit/release, one charge per generation, the last unit, concurrent reservations, UTC rollover, clock rollback, corrupt and foreign records, fail-closed writes) |

### 7.4 What these tests do not prove

- **No inference ran.** Every streaming assertion is against a mocked `generate()`; the Rust
  streaming client, the HTTP contract and the runtime handshake are covered by source review
  and unit tests only. Genuine Sonnet inference: **not demonstrated.**
- `commands.rs` (the guard order in `start_generation`, `load_model`'s lifecycle) is
  hand-reviewed and formatted, not compiled.
- Export is workspace-only (files, ZIP). Chat text is never exported, so there is no path that
  could mark an incomplete reply as complete; that is a property of the current design, not a
  new guarantee.
- Windows/MSVC/WebView2 remain unverified.

---

## 8. Phase 7 — integration cleanup (2026-10-22)

Phase 7 traced the send and load paths for races between a reply and an engine replacement.
One concrete gap was found and closed; the rest of the routing rules are unchanged.

### 8.1 The gap

Found by reading the source; it has not been reproduced against a running engine.

`load_model` starts a new engine process, and `Supervisor::start` stops the running child before
spawning. Nothing checked whether a reply was streaming. The Start-engine control in the model
setup page is enabled by local busy state only, and the chat stream keeps running when the user
navigates away. A load started during a reply therefore cut that reply off. The reply failed
with a network error, and its usage unit was already committed because the engine had accepted
it. Nothing was answered by the wrong model, but the reply was lost without an explanation.

### 8.2 The rule now

| Action | Refused with `busy` when | Decision function |
| --- | --- | --- |
| `load_model` | a reply holds the active slot | `registry::check_load_allowed` |
| `start_generation` | a load is in flight | `registry::check_generation_allowed` |

Ordering, so the two sides cannot interleave:

1. `load_model` claims the loading marker (`begin_loading`) first, then reads the active slot.
   An unreadable slot is treated as busy.
2. `start_generation` reads the loading marker while holding the active-slot lock, before it
   reserves the slot and any usage unit.

So either the reply reserved the slot before the load checked it (the load is refused), or the load
marked itself before the reply checked (the reply is refused). Nothing is cancelled on the user's
behalf, and no usage is charged for a refused request.

### 8.3 Verification of §8

| Check | Result |
| --- | --- |
| Standalone harness, `registry.rs` + `error.rs` (real source, serde derives stripped) | **41 / 41** (new: `a_model_cannot_be_loaded_while_a_reply_streams`, `a_message_cannot_be_sent_while_a_model_loads`) |
| `rustfmt --check` on `src-tauri/src` | clean |
| `commands.rs` `load_model` / `start_generation` | source review only; **not compiled** |

Not proven: that a live reply is actually refused by a running engine, and that the UI shows the
`busy` message readably. Both need the engine run described in
[PHASE_9_VERIFICATION.md](./PHASE_9_VERIFICATION.md) §11.

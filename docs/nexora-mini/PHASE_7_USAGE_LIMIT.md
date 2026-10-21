# Phase 7 — Daily Free Usage Limit & Reset System

Branch: `arena/18030bbb-nexora` · Base commit: `9bd4f7ad36ab012b50e48333c27fdc9b8244d5ff`
Scope: Phase 7 only. Phase 8 (comprehensive QA, Windows verification) was not started.

Nexora Mini now enforces a **daily free allowance of 50 local generations**, counted and
stored on the user's machine, reset at **00:00 UTC**, and enforced in the Rust command that is
the only possible entry point to the model. There is no account, no network check, no
telemetry and no payment path involved.

---

## 1. What existed before this phase

Audit of the tree as it actually is (not as earlier reports describe it):

| Area | Finding |
| --- | --- |
| Chat entry point | `ui/src/lib/inference.ts::generate` → one Tauri command, `start_generation` |
| Other generation entry points | `usePersistentConversation`, `useConversation`, `usePatchGeneration`, `usePatchRepair` (×2) all call the same `generate`; nothing else reaches the engine |
| Engine client | `inference/client.rs::stream_chat` posts to `127.0.0.1:<port>/v1/chat/completions`; it is called from exactly one place |
| Cancellation | `cancel_generation` sets an `AtomicBool` + `Notify`; `stream_chat` returns `{cancelled:true}` |
| Errors | `InferenceError { code, message }`; pre-stream failures (not running, not ready, HTTP rejection) return `Err` before any output |
| Token usage | **Not available.** The streamed response is parsed only for `delta.content` and `finish_reason`; no `usage`, `timings` or token counts are read or stored |
| Existing counters | **None.** No quota, usage, daily-limit or counter type existed anywhere in `src-tauri` or `ui` |
| Persistence pattern | `workspace/settings.rs` uses `tempfile::NamedTempFile` + `persist` for atomic writes; `inference/settings.rs` uses a plain `fs::write` |
| Licensing | `PLAN_TIERS` in `ui/src/lib/licensing.ts`, `LicenceStatus` in `src-tauri/src/licensing/mod.rs`, three tiers with prices |
| Conflicting claim | **Present:** `"Unlimited local GGUF inference"` in all three plan tiers; feature id `unlimited_local_inference` in Rust and TypeScript; plus `docs/phase-26/README.md` |

Nothing in the repository tracked or limited usage before this phase, so there was no second
tracking system to avoid — the new one is the only one.

---

## 2. The metric, and why

**Unit: one generation.** Every chat reply, patch proposal and patch repair is one generation.

Why not tokens:

- The runtime is queried with `stream: true` and Nexora never asks for or parses a usage
  block. `GenerationOutcome` carries only `finish_reason` and `cancelled`.
- Token counts could be *estimated* from characters, but labelling an estimate as tokens
  would be false, and the phase rules forbid exactly that.
- The runtime's OpenAI-compatible endpoint *may* report usage in a final stream chunk
  (`stream_options.include_usage`), but that behaviour has not been verified against the
  pinned `b11714` build on this machine (no runtime, no model, no Windows host). Building a
  quota on an unverified field would make the limit silently wrong if the field is absent.

A generation is exactly observable at the command boundary: either the engine accepted the
request and began streaming, or it did not. That makes the policy deterministic, testable
without a model, and impossible to double-count.

**Trade-off, stated plainly:** a one-word reply and a 4,000-token patch both cost one unit.
The limit is therefore about how often the model is used, not how much it produces. If a
verified token signal becomes available in Phase 8, the metric can move to tokens — the
policy constant and record version exist precisely so that change is explicit.

### Allowance

```rust
// src-tauri/src/usage/mod.rs
pub const DAILY_ALLOWANCE: u32 = 50;
pub const ALLOWANCE_IS_PROVISIONAL: bool = true;
```

- **50 generations per UTC day.**
- **Provisional.** The user has not approved a number; Phase 7 required a documented default
  rather than an unbounded or hidden quota. 50 is a generous single-developer day (more than
  a typical chat session) while still being a real limit; it sits in one constant, and the UI
  says "The allowance is provisional" so nobody is told a final number was agreed.
- The value is duplicated once, in `PUBLISHED_DAILY_ALLOWANCE` (`ui/src/lib/usage.ts`), for
  plan copy. `rustContract.test.ts` compares the two and fails on drift.

---

## 3. Reset policy

**Boundary: 00:00 UTC. Timezone: UTC. No daylight-saving transitions, no system time-zone
lookup.**

The Rust side does not link a time-zone database and the OS APIs that expose one are
platform-specific (`windows-sys` would need another feature; Linux/macOS need TZif parsing or
`libc`). Adding an unverifiable dependency or an `unsafe` FFI call in a phase that cannot
compile the crate was judged worse than a simple, honest, timezone-stable policy.

The UI converts the boundary instant into the viewer's own clock
("Resets in 6h 12m · 05:30 your time") and always states "Resets at 00:00 UTC each day", so
the rule is never hidden.

Derivation (no stored countdown):

```
day   = floor(unix_seconds / 86_400)      // UTC calendar day
reset = (day + 1) * 86_400_000            // next UTC midnight, as Unix ms
```

`remaining` time is recomputed from the clock on every 30 s tick. A reset is *claimed* only
after `Date.now() >= resetAtMs`, at which point the record is re-read from the core.

### Edge cases

| Situation | Behaviour |
| --- | --- |
| Restart on the same day | `used` is preserved; the record is read at startup |
| Restart on a later day | `used` is 0 for the new day; the old record is replaced on the next charge |
| App left running across midnight | the next status/reservation rolls to the new day; no timer is trusted |
| Clock moved **backwards** | the stored record is dated *after* today, so today counts as fully used. Moving the clock back never buys generations |
| Record dated tomorrow (small skew) | same rule: today is treated as used up, the record is left alone |
| Record dated far in the future (> 1 day) | treated as damage and repaired to *today, fully used* — a repair must not become a free reset |
| Record dated before today | a finished period; today starts empty |
| Missing file | a fresh day, no notice |
| Corrupt / unparsable / unknown version / other policy | repaired to a fresh day **with a visible notice** |
| Counter above the allowance | clamped to the allowance (exhausted), never reset |
| Negative or absurd counter | record rejected as invalid → fresh day + notice |
| Reset instant in the past at render time | the UI shows "any moment" and re-reads within one tick |

---

## 4. Persistence and privacy

File: `<app_config_dir>/daily-usage.txt` — the same directory that already holds
`inference-settings.json` and `workspace-settings.json`.

```
version=1
policy=nexora-mini-free-generations-v1
day=20746
used=12
updated=1760000000000
```

| Property | Decision |
| --- | --- |
| Fields | version, policy id, UTC day index, used count, informational timestamp |
| Not stored | prompts, replies, file contents, file names, workspace paths, model paths, account data, device identifiers |
| Format | plain text, five fixed keys; unknown, missing or duplicated keys make the record unreadable |
| Validation | version, policy, day plausibility (0 … today + 36,600), `used ≤ 1,000,000`; then clamped |
| Atomicity | sibling `.tmp` file → `write_all` → `sync_all` → `fs::rename` → parent `sync_all` (Unix) |
| Failure handling | a failed write keeps the charge **in memory**, records the error, and blocks further generations until a write succeeds |
| Transport | none. The file is never read by anything except the Rust core and never leaves the machine |

`tempfile` is available, but a hand-written temp+rename keeps `usage/` free of dependencies so
its tests can actually be executed here (see §8).

**Tamper resistance is explicitly not claimed.** A local-only limit is a product policy. A
user who can edit or delete `daily-usage.txt`, patch the binary, or run their own
`llama-server` can bypass it. Phase 7 deliberately adds no obfuscation, no signing, no
anti-debugging and no remote attestation.

---

## 5. Enforcement boundary

```
React (5 call sites) → lib/inference.ts::generate
                     → invoke("start_generation")
                     → inference::commands::start_generation      ← enforcement
                     → client::stream_chat                        ← only caller
                     → POST 127.0.0.1:<port>/v1/chat/completions
```

There is exactly one command that can make the model generate, and the allowance is taken
inside it. A frontend that skipped the UI entirely and invoked `start_generation` directly
would still be refused, because the check is in Rust. `rustContract.test.ts` pins this
structurally: `stream_chat` is defined once and called once, from that command.

### Accounting lifecycle

1. `validate` the request.
2. Confirm the engine is running and healthy (`not_running` / `not_ready` → no charge).
3. Take the single active-generation slot. If another generation is running → `busy`, no charge.
4. **Reserve** one unit under the same lock: `usage.reserve(now)`.
   - Exhausted → `quota_exhausted` with the reset instant; the slot is not taken.
   - Write recovery failed → `usage_unavailable`; the slot is not taken.
5. Call `stream_chat`. It sets `accepted = true` immediately after the engine returns 200 and
   the stream starts.
6. After the stream: `if accepted { charge.commit() } else { drop(charge) }`.
   Dropping releases the reservation, so a request that never reached the model costs nothing.
7. `finish_generation` clears the slot.

| Outcome | Charged? |
| --- | --- |
| Completed normally | yes |
| Stopped by the user after streaming began | yes (the model ran) |
| Timed out / connection dropped after streaming began | yes |
| Empty completion (model produced no text) | yes — streaming started |
| Model not loaded, model loading, engine dead, health check failed | no |
| `busy`, invalid request, exhausted allowance | no |
| Engine rejected the request (400/401/503) | no — `accepted` is still false |
| Click that never produced an IPC call | no |
| Double click while generating | second call is `busy`; no charge |

**Exactly once.** A `Charge` is consumed by `commit()`; it cannot be committed twice, because
`commit` takes `self` and `Drop` only releases un-settled charges. The reservation counts
against `remaining` while the request is in flight, so two threads cannot both spend the last
unit (tested with 200 concurrent threads against 50 units).

**Model loading never charges.** `load_model` and `unload_model` do not touch usage.

**Runtime exit.** If the engine dies mid-stream, the outcome is an error *after* `accepted`, so
the unit stays charged; the charge is persisted before the command returns.

---

## 6. UI

A single strip above the composer on the Chat page (`components/UsageMeter.tsx`):

```
◆ Daily free usage                            38 of 50 left
▬▬▬▬▬▬▬▬▬▬▬▬▬▬▬▭▭▭▭▭▭▭▭▭▭▭▭▭▭▭▭▭▭▭▭▭▭▭▭▭▭▭
Resets in 5h 12m · 05:30 your time. The allowance is provisional.
Resets at 00:00 UTC each day.
```

- **Exhausted:** "50 of 50 used", "Daily free allowance reached. It resets in …", the meter
  switches to a muted violet state, the composer is disabled and its placeholder says
  "Daily free allowance reached — it resets at 00:00 UTC".
- **Unreadable:** "Daily usage is unavailable: …" in an alert; no numbers are invented.
- **Repaired record:** the core's notice is shown ("…today's allowance was restarted").
- **Failed write:** "The usage record could not be updated: …".
- **Loading:** "Checking daily usage…".
- **Browser preview:** no meter at all — there is no record and no engine, so there is nothing
  truthful to show.
- **Accessibility:** `role="progressbar"` with `aria-valuemin/max/now/valuetext`,
  `aria-label="Daily free usage"`, plain-text `role="status"`/`role="alert"` messages, no
  colour-only state, reduced-motion respected.
- **No monetisation:** no upgrade button, no "restore usage", no price, no link to licensing.
  A test asserts that no such wording appears.

The view is refreshed from the authoritative command: on mount, after every settled
conversation, on explicit reload, and once the reset instant passes.

---

## 7. The "unlimited" claim

It existed, in four places that matter:

| Location | Before | After |
| --- | --- | --- |
| `ui/src/lib/licensing.ts` (3 tiers) | `"Unlimited local GGUF inference"` | `` `${PUBLISHED_DAILY_ALLOWANCE} local generations per day` `` |
| `ui/src/lib/licensing.ts` default features | `unlimited_local_inference` | `daily_local_inference` |
| `src-tauri/src/licensing/mod.rs` default features | `unlimited_local_inference` | `daily_local_inference` |
| `src-tauri/src/licensing/validator.rs` activated features | `unlimited_local_inference` | `daily_local_inference` |
| `ui/src/pages/LicensingPage.tsx` header + guarantee | "completely free forever", "No feature gating" | states the daily free allowance and that it is enforced locally |
| `ui/src/App.phase26Integration.test.tsx` fixture | `unlimited_local_inference` | `daily_local_inference` |

The allowance applies to **every** edition, because no tier-based exemption is implemented and
inventing one would be a monetisation change. Prices, tiers, the activation form and the
licence validator are otherwise untouched: no tier was invented or repriced.

`docs/phase-26/README.md` still contains an "Unlimited" comparison table. It is a historical
Phase 26 document, not current product copy, and Phase 7 does not rewrite earlier reports; it
is listed here as stale rather than silently edited.

---

## 8. Tests actually executed

### Rust — executed with `rustc --test` (24 tests, all passing)

The crate cannot be compiled here: crates.io is unreachable (`[35] SSL connect error` on
`index.crates.io`), so `cargo check`/`cargo test` fail during dependency resolution before any
Rust is compiled. The usage core is deliberately dependency-free apart from `serde` derives
(the wire type), so it was compiled with `rustc --edition 2021 --test` against the installed
1.88.0 standard library, with the derive stripped. **This is the real module source**, not a
re-implementation.

```
COMPILE=0
test result: ok. 24 passed; 0 failed
```

`usage/mod.rs` — 18 tests:

| Test | What it proves |
| --- | --- |
| `a_new_install_has_the_full_allowance` | initial state, stability, notice/error empty |
| `a_committed_generation_is_counted_once` | one reserve+commit → exactly one unit |
| `a_released_reservation_costs_nothing` | drop before start → no charge |
| `the_last_unit_is_granted_and_the_next_is_refused` | exact boundary at 50, `quota_exhausted` message |
| `concurrent_reservations_cannot_exceed_the_allowance` | 200 threads, 50 grants |
| `an_uncommitted_reservation_holds_a_unit_until_it_settles` | in-flight unit is not double-spent |
| `usage_survives_a_same_day_restart` | persistence across load |
| `a_new_utc_day_starts_a_fresh_allowance` | reset on day change, first charge lands in the new day |
| `a_charge_stays_in_the_period_it_was_reserved_in` | midnight crossing |
| `moving_the_clock_back_does_not_hand_out_usage` | rollback defence |
| `a_record_that_claims_tomorrow_is_kept_rather_than_reset` | no free reset from skew |
| `an_implausible_future_record_is_repaired_and_reported` | repair is exhausted, not free |
| `a_counter_above_the_allowance_is_clamped_not_reset` | malformed counter cannot grant |
| `a_corrupt_file_is_repaired_and_reported` | self-heal + notice, repair persists |
| `a_record_from_another_policy_is_not_trusted` | policy migration guard |
| `an_unwritable_record_fails_closed_without_losing_the_charge` | write failure → charge kept, error reported, next request refused |
| `utc_day_labels_are_correct_across_month_and_leap_boundaries` | date arithmetic (incl. 2024-02-29, pre-epoch) |
| `the_reset_instant_is_the_next_utc_midnight` | boundary computation |

`usage/store.rs` — 6 tests: round-trip, missing file, atomic replace without a leftover temp
file, garbage rejected, negative/oversized/unknown-key records rejected, and the encoded file
containing no paths or content.

### Frontend — executed with the repository's Vitest setup

| Command | Exit | Result |
| --- | --- | --- |
| `pnpm typecheck` | **0** | no errors |
| `pnpm test` | **0** | **80 files, 804 tests passed** (was 771; +33) |
| `pnpm build` | **0** | built in 2.24 s; existing >500 kB chunk warning left visible |
| `git diff --check` | **0** | no whitespace errors |
| `cargo fmt --all -- --check` | **0** | rustfmt 1.8.0 (1.88 toolchain) |

New frontend tests:

| File | Tests | Coverage |
| --- | --- | --- |
| `lib/usage.test.ts` | 10 | IPC read, browser-preview `null`, protocol rejection, error normalisation, shape validation, published allowance, countdown math, local reset clock, percent clamping |
| `components/UsageMeter.test.tsx` | 7 | remaining display, progressbar ARIA, exhaustion without upsell, loading, unreadable record, repaired record + write error, provisional note |
| `features/usage/useUsage.test.tsx` | 6 | initial load, no-status, read failure, refresh key, reset-boundary refetch (fake clock), explicit reload |
| `components/Composer.test.tsx` | +1 | allowance wording replaces the "no model" wording |
| `pages/ChatPage.test.tsx` | +1 | no meter in the browser preview |
| `App.phase25Chat.test.tsx` | +2 | exhausted: send disabled, Enter does not generate, no purchase wording; backend `quota_exhausted` is reported honestly |
| `lib/licensing.test.ts` | +1 | no tier promises "unlimited"; the allowance line is present |
| `lib/rustContract.test.ts` | +5 | `UsageStatus` parity, argless `get_usage_status`, allowance constant cross-check, provisional flag, single `stream_chat` caller |

Also updated: 14 App-level suites now answer `get_usage_status` so they exercise the real
component path, and `App.offlineRegression` mocks the usage module (it mounts the shell with a
stubbed Tauri object).

### Not executed

- `cargo check --all-targets`, `cargo test` (whole crate), `cargo clippy`, `cargo build`,
  Tauri bundling: **blocked** by crates.io TLS.
- Any Windows build, installer or click-through: **no Windows host**.
- A real generation through llama.cpp, therefore the `accepted` signal in a live stream:
  **not demonstrated**. The rule is exercised only through the pure state machine.

---

## 9. Known limitations

1. **Not tamper-proof.** Editing or deleting `daily-usage.txt` resets the allowance. Stated in
   the UI? No — deliberately not advertised, but documented here and accepted as inherent to
   an offline, account-free product.
2. **UTC, not local midnight.** A user in IST sees the reset at 05:30 local. Defensible and
   tested, but a local-day policy would need a reviewed time-zone dependency or a verified OS
   API (Phase 8 decision).
3. **Coarse unit.** One generation is one unit regardless of length or model.
4. **Write failure blocks generation.** If the config directory is read-only, the app refuses
   to generate rather than silently running unmetered. The error message says why.
5. **A failed write loses accounting across a restart.** The charge is kept in memory and
   blocks further use, but if the app is restarted before the disk recovers, that one unit is
   lost. It is at most one unit, and it fails in the user's favour rather than against them.
6. **Clock tampering.** Moving the clock forward starts a new day (as it must, for real
   midnight to work); moving it back does not grant anything. There is no remote time check by
   design.
7. **Provisional allowance.** 50 is a documented default, not an approved product decision.

---

## 10. Remaining work for Phase 8

- Compile the crate and run the full Rust suite; confirm the `State<UsageState>` injection,
  the `Charge` lifetime across `await`, and the `From<UsageError>` conversion in a real build.
- Run one genuine generation and confirm the unit is charged exactly once end-to-end, and that
  a cancelled and a timed-out generation behave as documented.
- Decide the allowance number with the product owner, and the metric (generations vs. verified
  token usage from `stream_options.include_usage`).
- Decide whether the reset should be local midnight, and if so pick a reviewed time-zone
  source.
- Windows test: path handling for `%APPDATA%`, file locking during rename, and the read-only
  directory failure path.
- Review the stale `docs/phase-26/README.md` "Unlimited" table.

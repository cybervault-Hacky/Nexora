# Multi-File Patch Proposal — Roadmap Continuation

**Status:** Implemented in the local working tree; uncommitted. **This is not a new numbered product phase** — it is the already-planned roadmap item *"Multi-file patch proposal"* (feature-status table, `README.md` §11: "Planned (future phase beyond 18 in old roadmap)"), first deferred in `docs/phase-9/README.md` §7 ("multi-file editing, patch application, and agent tools (a later coding phase)") and specified by `docs/phase-1/05-coding-agent-workflow.md` §14 (multi-file editing protocol: plan first, read before write with recorded hashes, **atomic batch — fully or not at all**, conflict/stale handling, size limits).

**Why now:** the Phase 23B verification gate is BLOCKED (no Rust toolchain/crates.io/PostgreSQL — status preserved, see `docs/phase-23b/README.md`). Phase 23C (registration/email verification) is explicitly gated behind that verification and was NOT started. Multi-file patch proposals are the next planned roadmap task and have **zero dependency** on the authentication backend.

## What shipped

A patch proposal can now span **1–20 files** and applies **atomically — all files or none** (Phase-1 §14). Additive everywhere: the entire single-file contract (v1) is untouched and all pre-existing tests pass unchanged.

### Contract v2 (`patchContract.ts` ⇄ `patch_contract.rs`, kept in sync)

```json
{ "version": 2, "explanation": "...",
  "files": [ { "targetPath": "src/a.ts", "proposedContent": "<full content>",
               "explanation": "..." }, ... ] }
```

- Strict parsing (unknown fields rejected), `1..=MAX_BATCH_FILES (20)` entries, **case-insensitive unique** target paths.
- Per-file rules are exactly the v1 rules: traversal/absolute/backslash rejection, sensitive-file exclusion (`.env*`, `*.pem`, `*.key`, …), 256 KiB / 200 000-char / NUL bounds, no-change detection against known originals.
- A v1 single-file object parses as a 1-file batch (`parseAndValidatePatchBatch`); the v1 parser (`parseAndValidatePatchResponse`) is **unchanged**, including its exact error messages.

### Rust batch layer (`workspace/patch.rs`)

`PatchBatch`, `BatchEdit`, `BatchValidation` (per-file entries + path-qualified errors/warnings), `BatchApplyResult`; `create_patch_batch` (each file goes through the full single-file create checks — read-before-write, hashes recorded), `validate_patch_batch`, `apply_patch_batch`.

**Atomicity:** the whole batch is validated (including stale detection) before the first write; files are replaced with the existing tempfile+persist pattern; if any replace fails, every file already replaced by this batch is **restored from its recorded original content** (atomic write) and the apply reports an error. Documented residual: a process crash mid-apply is not journaled — each individual file remains well-formed and the next validate/apply reports stale hashes.

### IPC (`workspace/commands.rs`, registered in `src-tauri/src/lib.rs`)

`create_patch_batch { root, edits }`, `validate_patch_batch { root, batch }`, `apply_patch_batch { root, batch }` — additive beside the single-file commands (those are untouched).

### Frontend

- `lib/patch.ts`: `PatchBatch`/`BatchEdit`/`BatchValidation`/`BatchApplyResult` types + `createPatchBatch`/`validatePatchBatch`/`applyPatchBatch` wrappers (same desktop-only guards as the single-file API).
- `usePatchGeneration`: model output that is **contract v2** becomes `previewBatch` → `appliedBatch` states (additive). v1 output keeps the exact single-file path. The generation prompt now admits both schemas (it still prefers single-file; multi-file proposals for files whose content the model never saw are reviewed as full diffs before anything can apply).
- `PatchPreview`: optional `files` prop renders one diff section per file with an **Apply N files** action (single-file rendering byte-identical when `files` is absent).
- `PatchGenerateBar`: `previewBatch`/`appliedBatch` branches; note states "All files apply together or not at all."

## Tests and actual results

| Suite | Result |
|---|---|
| `pnpm typecheck` | **PASS** |
| `pnpm test` | **PASS — 50 files / 569 tests** (baseline 558; **+11**: 7 contract-v2 (`patchContract.test.ts`), 4 batch generation (`usePatchGeneration.test.tsx`); **no test removed or weakened**) |
| `git diff --check` | **PASS** |
| Rust tests (`patch.rs` 6 + `patch_contract.rs` 6 = **12 written**) | **NOT RUN — no cargo/crates.io in this environment** (same blocker as Phase 23B; not claimed as passing) |
| `pnpm build` | **NOT RUN** — preserves `dist/` as in the verification gate |

Frontend coverage: valid multi-file batch; v1-wrapped-as-batch; case-insensitive duplicates; empty/oversized batches; per-file traversal/sensitive/no-change/NUL rules; wrong versions/unknown fields; unseen files deferred to disk checks; generation v2 → `previewBatch` (create+validate wiring); atomic `apply_patch_batch` call; batch validation failure → honest error; **v1 output stays on the unchanged single path**.

## Limitations / follow-ups (existing roadmap, not invented work)

- Manual single-file editing (Phase 12 contract) is unchanged; multi-file **editing UI** (adding files to an edit session) and P15 per-file detailed `ValidationPanel` aggregation for batches belong with the deferred **"agent tools"** work (`docs/phase-9/README.md` §7).
- `usePatchRepair` (Phase 15) remains single-file.
- Multi-file **generation prompting** (feeding several file contents into the model, per Phase-1 §14 "read before write") follows with agent tools; today the model may return v2 files but has only seen the selected file's content — the review gate (full per-file diffs, explicit Apply) is the safety boundary.
- Rust sources remain **compiler-unverified** in this environment; run `cargo fmt --check`, `cargo check --all-targets`, and `cargo test` in `apps/desktop/src-tauri` before relying on the batch layer on Windows.

## Security posture (unchanged invariants)

Traversal/sensitive/binary/size checks per file; stale hashes refuse the whole batch **before any write**; no secrets; no network; local-first AI untouched; no authentication claims of any kind (the Phase 23B BLOCKED status stands).

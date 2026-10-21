# Phase 6 — Single-File Export, ZIP Export and Download Mode

Branch: `arena/18030bbb-nexora` · Base commit: `9bd4f7ad36ab012b50e48333c27fdc9b8244d5ff`
Scope: Phase 6 only. Phase 7 (quotas/reset) was not started.

This phase adds a real desktop export path: a Rust export core that plans, validates and
writes files, and a Download Mode panel that lets a user review what will be written before
anything is saved. It does **not** add cloud upload, publishing, deployment or any
network transfer of project contents.

---

## 1. What was built

| Layer | File | Lines | Purpose |
| --- | --- | --- | --- |
| Rust | `src-tauri/src/export/mod.rs` | 706 | Wire types, `ExportError`, path/name validation, destination rules, temp-file publish, unique suggestions |
| Rust | `src-tauri/src/export/plan.rs` | 469 | Reviewable plan for a whole project or a selection; exclusion accounting; re-resolution before writing |
| Rust | `src-tauri/src/export/single.rs` | 422 | Streamed single-file copy (256 KiB chunks) with cancellation |
| Rust | `src-tauri/src/export/zip.rs` | 579 | Dependency-free STORED ZIP writer, CRC-32, DOS timestamps, two-pass verify |
| Rust | `src-tauri/src/export/commands.rs` | 313 | Five Tauri commands, `spawn_blocking`, one-slot busy guard, progress channel |
| UI | `ui/src/lib/export.ts` | 145 | Typed IPC wrappers, `ExportError` normalisation, desktop-only refusal |
| UI | `ui/src/lib/ipc.ts` | +117 | Export DTOs, shared with the Rust contract test |
| UI | `ui/src/features/export/useExport.ts` | 382 | Scope/status/plan/destination/conflict controller |
| UI | `ui/src/components/ExportPanel.tsx` | 293 | Download Mode panel |
| UI | `ui/src/components/ExportPanel.test.tsx` | 291 | 10 component tests |
| UI | `ui/src/lib/export.test.ts` | 240 | 14 IPC tests |
| UI | `ui/src/components/Icon.tsx` | +8 | `download` glyph |
| UI | `ui/src/pages/ProjectsPage.tsx` | +7 | Mounts the panel above the evidence panel |
| UI | `ui/src/styles/app.css` | +246 | Panel styling in the existing graphite/violet system |

No existing feature was duplicated: the workspace scanner, `is_sensitive_file`, the
`WorkspaceState` root and the `Channel` streaming pattern are reused as-is.

`src-tauri/src/lib.rs` registers `mod export;`, `ExportState` and five handlers.

---

## 2. Single-file export

`export_single_file` copies one file that the workspace already lists.

- **Source** is a workspace-relative path from the previewed file. The Rust side rejects a
  missing file (`not_found`), a directory, a symbolic link (`symlink_excluded`), a
  secret-bearing name (`secret_excluded`) and a path that canonicalises outside the project
  root (`forbidden`).
- **Contents are byte-exact.** The copy is a plain `Read`/`Write` loop in 256 KiB chunks; no
  newline, encoding or line-ending conversion happens. A 0-byte file is copied as 0 bytes.
- **Name and extension** are preserved by construction: the suggested destination is the
  original file name beside the project folder (`suggest_export_path("file", path)`), and the
  destination validation rejects Windows-reserved names, trailing dots/spaces, drive letters
  and UNC shapes. The user may rename it; the extension is not forced.
- **No silent overwrite.** `publish` refuses an existing destination with `already_exists`
  unless the request carries `overwrite: true`, which the UI only sets after an explicit
  "Replace existing file" confirmation.
- **Cancellation** returns `{ cancelled: true }`, discards the temporary file and leaves no
  partial output. The command never returns success for a cancelled copy.
- **Errors** carry stable codes (`not_found`, `forbidden`, `secret_excluded`,
  `symlink_excluded`, `already_exists`, `storage`, `busy`, `changed`, `invalid_request`).
- **Atomicity.** Bytes go to a sibling temporary file in the destination folder, are
  `flush`ed and `sync_all`ed, and only then renamed into place. A failure at any point leaves
  the destination untouched.
- **Large files** are streamed from a `spawn_blocking` thread, so the webview stays
  responsive. Copying a file onto itself is refused (`invalid_request`) so the source cannot
  be truncated.

A single-file export is allowed **into** the project folder (for example "save a copy as
`notes.backup.md`") because it cannot include itself in a wider archive and never deletes
anything. Writing onto an existing project file still requires the explicit overwrite flag.

---

## 3. ZIP export

`export_zip` writes a standard ZIP of the reviewed plan.

### Structure

- Entries are **workspace-relative** (`src/main.rs`, `README.md`) with `/` separators. There
  is no synthetic top-level folder; the archive reflects the project as it is shown in the
  explorer.
- Entries are written **in plan order**, which is the scanner's deterministic order, and a
  `HashSet` guarantees no duplicate names.
- **STORED only** (no deflate). Source bytes are copied unchanged, so the archive is exact
  and the writer needs no compression dependency. STORED archives open in Windows Explorer,
  `unzip`, 7-Zip, macOS Archive Utility and Python `zipfile`.
- UTF-8 names set the general-purpose flag bit 11, so `naïve café.txt` survives a round trip.
- DOS date/time is derived from the source file's modification time, clamped to the format's
  range (1980–2107).
- Zip64 is **not** written. Each file and the whole plan must stay below `u32::MAX`
  (4 GiB − 1); the plan refuses anything larger with `too_large` rather than emitting a
  corrupt archive. Entries are capped at 60,000, below the 65,535 format limit.
- The writer does two passes: it records local headers, then re-stats every source and
  verifies size and CRC before writing the central directory. A file that changed while the
  archive was being written fails with `changed` instead of producing a wrong archive.

### Integrity and finalisation

The archive is written to a sibling temporary file. On success it is flushed, `sync_all`ed,
and renamed. On cancellation or any error the temporary file is removed (`discard`), so a
failed export never leaves a plausible-looking `.zip` behind.

### Destination rules

- The destination must be **absolute** and have a usable file name; the suggested name is
  `<project>.zip` beside the project folder, de-duplicated (`project-2.zip`, …).
- A destination **inside** the project root is refused: the archive would otherwise try to
  include itself.
- An existing file is refused unless `overwrite: true` (explicit confirmation).
- The destination's parent must exist and be a directory; the export does not create
  directories implicitly.

### Progress and cancellation

`Channel<ExportEvent>` carries `preparing` (entry count), `progress` (done/total,
bytesDone/bytesTotal, current name) and `finalizing`. Sending is best-effort; the write
itself does not depend on the UI receiving events. Cancellation is checked between chunks
and between entries; a cancelled archive returns `{ cancelled: true }` with no file.

### Empty and inaccessible projects

- Empty project → `no_files` with a message that names the exclusion policy.
- Project above the scanner's ceiling (8,000 files or 12 levels) → `too_large`; the ZIP is
  refused rather than silently truncated.
- Files that disappear mid-plan → `not_found`; files that become links → `symlink_excluded`.

---

## 4. Exclusion policy

Planning reuses `workspace::scanner`, so the archive sees exactly what the Projects page
shows, plus two export-only filters.

| Class | Rule | Counted in the plan |
| --- | --- | --- |
| VCS internals | `.git`, `.hg`, `.svn` | omitted by the scanner |
| Dependencies | `node_modules`, `vendor`, `.venv`, `venv`, `.tox` | omitted by the scanner |
| Build output | `target`, `dist`, `build`, `out` | omitted by the scanner |
| Caches | `__pycache__`, `.mypy_cache`, `.pytest_cache`, `.ruff_cache`, `.cache`, `.parcel-cache`, `.next`, `.nuxt`, `.svelte-kit`, `.vite`, `.turbo`, `.output`, `coverage`, `.nyc_output` | omitted by the scanner |
| Editor/IDE | `.idea`, `.arena` | omitted by the scanner |
| `.gitignore` matches | simple pattern matching from the project's `.gitignore` | omitted by the scanner |
| Secrets | `.env`, `.env.*`, `credentials`, `.netrc`, `.git-credentials`, `secrets.json/yaml/yml`, `id_rsa`, `id_ed25519`, `id_dsa`, `*.pem`, `*.key`, `*.p12`, `*.pfx`, `*.kdbx` | `excludedSensitive` |
| Symbolic links | never followed; skipped in a project plan, refused when named directly | `excludedSymlink` |
| Windows-unsafe names | reserved devices (`CON`, `PRN`, `AUX`, `NUL`, `COM1`–`COM9`, `LPT1`–`LPT9`, `CONIN$`, `CONOUT$`), trailing dot/space, control characters, over-long components | `excludedUnsafe` |
| Path escapes | `..`, absolute paths, drive letters, UNC prefixes, backslashes | refused (`forbidden` / `invalid_request`) |

`plan.excludedSensitive`, `excludedSymlink` and `excludedUnsafe` drive the review text
("2 secret files withheld, 1 symbolic link skipped"). Nothing is silently included: if the
user selects a withheld file explicitly, `plan_selection` returns `secret_excluded` or
`symlink_excluded` with the file name, and the UI shows that message instead of exporting.

No archive is ever written outside the approved workspace root, and no entry name can escape
the archive directory when extracted.

---

## 5. Download Mode

**Placement.** Download Mode lives in the Projects page, directly above the existing evidence
panel. It is part of the export workflow, not of chat or generation: it exports files that
already exist on disk and never invents or generates one. The copy makes this explicit:
"Generated files stay in your project folder. Download Mode writes copies to a location you
choose; it never changes or deletes the project."

**Scopes.**

1. *Whole project* — everything the scanner would list.
2. *Selected files (n)* — only the explorer selection; disabled until something is selected.
3. *Single file* — only the previewed file; disabled until a file is previewed.

**States.** `idle → preparing → ready → saving → completed | cancelled | failed`.

| State | What the user sees |
| --- | --- |
| `preparing` | "Preparing the file list…" |
| `ready` | File count, total GiB, first eight entries with byte sizes, "and N more", withheld summary, exclusion note |
| `saving` | "Writing X of Y · path" from real progress events; Cancel is available |
| `completed` | "Saved N files (X GiB) to `<path>`." plus "The previous file was replaced." when applicable |
| `cancelled` | "Export cancelled. Nothing was written." |
| `failed` | The Rust error message in an alert, with a retryable Export button |
| `idle` | Nothing is planned (no project, or no file previewed for single-file mode) |

**Destination.** A Rust-generated, non-colliding suggestion is filled in as soon as a plan is
ready, unless the user has already typed or picked one. The field is editable and labelled
"Save archive to" / "Save file to". `Choose…` dynamically imports
`@tauri-apps/plugin-dialog`; because that plugin is not installed in this build, the attempt
fails and the UI explains that the path must be typed. The typed path is still validated in
Rust, so the fallback does not weaken anything. (Adding the plugin would require a new
dependency, a capability entry and a re-review of the permission surface, so it was left out
of this phase rather than broadening permissions.)

**Conflicts.** An existing destination produces an inline alert with "Replace existing file"
and "Choose another name". Only the first button sends `overwrite: true`, and it re-sends the
exact pending request; the second focuses the destination input and clears the conflict.
Nothing is replaced as a side effect of retrying.

**Concurrency.** `useExport` holds a `busy` ref for the duration of a call and the Rust side
holds a single `ExportState` slot. A second export started from the UI while the first is
saving is ignored; a second command from anywhere else returns `busy`.

**Cancellation.** The button calls `cancel_export`, which flips an `AtomicBool` owned by the
running task. The button shows "Cancelling…" until the promise settles. Cancellation never
produces a success message, and the destination is left untouched.

**Workspace preservation.** Export never mutates the project, never changes the open project,
never refreshes or closes the workspace, and never writes into the evidence or patch state.
Switching projects resets scope, plan, destination, progress and outcome.

**Browser preview.** Without `__TAURI_INTERNALS__`, every wrapper throws
`{ code: "desktop_only" }` before calling `invoke`, so there is no fake download and no
misleading success.

---

## 6. Security model

- All boundary checks live in Rust: destination absoluteness and name safety, project-root
  containment (canonicalised), source containment (canonicalised), symlink refusal,
  secret-name refusal, entry-name validation and size limits. The frontend only chooses
  values; it cannot widen them.
- The frontend gets **no** filesystem API. `capabilities/main.json` still contains only
  `core:default` and the four window commands. No `fs:*` or `dialog:*` permission was added.
- No broad paths are accepted: sources come from the remembered workspace root, and
  destinations are explicit absolute paths chosen by the user.
- Exported bytes never leave the machine. There is no upload, no cloud call, no telemetry
  and no content logging. Errors carry messages and codes, never file contents.
- Temp files live next to the destination (same filesystem, so `rename` is atomic) and are
  removed on failure.
- Archive entries cannot traverse: `..`, absolute paths, drive letters, UNC prefixes,
  backslashes, control characters and reserved device names are rejected on **every**
  platform, so an archive created on Linux cannot break on Windows.
- Zip is limited to < 4 GiB total and < 60,000 entries to avoid integer overflow and
  non-Zip64 readers.

---

## 7. Commands and wire contract

| Command | Arguments | Returns |
| --- | --- | --- |
| `plan_workspace_export` | `selection?: string[]` | `ExportPlan` |
| `suggest_export_path` | `kind: "zip" \| "file"`, `relativePath?` | absolute path string |
| `export_single_file` | `request: SingleExportRequest`, `onEvent: Channel<ExportEvent>` | `ExportOutcome` |
| `export_zip` | `request: ZipExportRequest`, `onEvent: Channel<ExportEvent>` | `ExportOutcome` |
| `cancel_export` | — | `boolean` (whether one was active) |

`ExportPlan { rootName, files[{path,size}], excludedSensitive, excludedSymlink,
excludedUnsafe, totalBytes, truncated }`.

`ExportEvent = {type:"preparing",total} | {type:"progress",done,total,bytesDone,bytesTotal,
current} | {type:"finalizing",total}`.

`ExportOutcome { path, files, bytes, cancelled, replaced }`; on cancellation `path` is empty
and `files`/`bytes` are 0.

`rustContract.test.ts` now scans `lib/export.ts` in addition to `lib/inference.ts`, pins the
five command names and their argument names, and compares every export struct field,
including optionality, against `export/mod.rs`.

---

## 8. Tests actually run

### Rust (executed through a standalone `rustc --test` harness)

The crates.io registry is unreachable in this sandbox (`[35] SSL connect error` on
`index.crates.io`), so `cargo test` cannot resolve dependencies. The export core is
dependency-free apart from `std`, `serde` derives and the workspace scanner, so its modules
were compiled with `rustc` against a minimal scanner stub (`/tmp/exp`, outside the repo) and
every test was executed.

```
COMPILE=0
test result: ok. 27 passed; 0 failed
```

| Module | Tests | Covers |
| --- | --- | --- |
| `export::mod` | 8 | unsafe components, entry-name validation (drive/UNC/`..`), destination rules, unique suggestions, temp create/publish/discard |
| `export::plan` | 6 | secret/unsafe/traversal refusal, missing files, directories, duplicate paths, symlinks, source re-resolution, missing root |
| `export::single` | 6 | byte-exact copy, empty file, missing/secret/symlink refusal, overwrite refusal and confirmed replace, self-copy refusal, cancellation leaves nothing |
| `export::zip` | 7 | structure and ordering, nested paths, UTF-8 flag, empty archive, CRC, DOS timestamps, size limits |

`plan_whole_project`'s scanner-driven exclusions are not covered by these 27 tests (the
harness stubs the scanner); they compose `scan_workspace` (15 existing scanner tests, not run
here) with the filters exercised in `plan_selection`.

### ZIP validated with real tools

A test archive produced by the actual `write_archive` was checked independently:

- `unzip -t` → **"No errors detected"**, exit 0.
- Python `zipfile.testzip()` → `None`.
- `unzip -l` listed the expected entries with correct sizes and dates.
- `naïve café.txt` round-tripped; general-purpose flag `0x0800` (UTF-8) present.

### Frontend

| Command | Result |
| --- | --- |
| `pnpm typecheck` | exit 0 |
| `pnpm test` | exit 0 — 77 files, 771 tests passed |
| `pnpm build` | exit 0 — `index-*.js` 537.23 kB (gzip 150.20 kB), `index-*.css` 96.77 kB |
| `git diff --check` | exit 0 |

New frontend tests: `lib/export.test.ts` (14) and `components/ExportPanel.test.tsx` (10).
They cover error normalisation, the desktop-only refusal (asserting `invoke` is never
called), planning argument shape, event forwarding, ZIP and single-file request shape,
`already_exists` → confirm → `overwrite: true` retry, cancellation shown as cancelled,
duplicate-submission prevention, and an honest empty-project note.

`rustContract.test.ts` — 31 tests, including seven export struct fixtures, the five command
signatures and the argless-command list.

One pre-existing integration test used `findByText("src/main.ts")`, which now matches both
the explorer and the review list. It was changed to `findAllByText(...)`; the assertion is
not weaker, it just no longer assumes a single occurrence.

`cargo fmt --all -- --check` → exit 0 (rustfmt 1.8.0-stable, 1.88 toolchain).

---

## 9. Checks not run

| Check | Status | Reason |
| --- | --- | --- |
| `cargo check --all-targets` | **not run** | Dependency resolution fails before compilation; crates.io and mirrors are TLS-blocked |
| `cargo test` (full suite) | **not run** | Same |
| `cargo clippy` | **not run** | Binary not installed and cannot be fetched |
| `cargo build` / Tauri bundle | **not run** | Same |
| Windows build / installer | **not run** | No Windows host; only the Linux std target is installed |
| Real desktop click-through | **not run** | No display, no compiled shell; UI verified through jsdom with mocked IPC |
| Native save dialog | **not available** | Plugin not installed; typed-path fallback verified in tests |

`commands.rs` (Tauri `State`, `Channel`, `spawn_blocking`) has only been parse-checked, not
type-checked against the real `tauri` crate. That is the highest-value thing to compile on a
networked machine.

---

## 10. Known limitations

- **No compression.** Archives are larger than a deflate ZIP. This is a deliberate trade for
  a dependency-free, verifiable writer.
- **No Zip64.** A project or single file ≥ 4 GiB cannot be archived; single-file export still
  works for such a file.
- **No folder creation.** The destination's parent must already exist.
- **No native picker.** `Choose…` falls back to typed paths until the dialog plugin is added
  with an explicit capability review.
- **`.npmrc`, `.aws/credentials` only indirectly covered.** `credentials` is matched by exact
  name; `.npmrc` is allowed because the scanner treats it as previewable. If it can contain a
  token in this product's target projects, the sensitive list should be revisited.
- **Whole-project plan is limited by the scanner ceiling** (8,000 files / 12 levels). Above
  that the export refuses instead of truncating.
- **Plan and write are separated by a re-resolution step**, but a file can still change
  between the final stat and the read; the writer detects a size mismatch and fails with
  `changed` rather than writing bad bytes.
- **Cancellation is cooperative**: it lands between chunks/entries, so a very large single
  copy may take a moment to stop.
- **Progress is best-effort**; if the window closes, the write still completes in the backend.

---

## 11. Phase 1–5 preservation

- `PHASE_1_AUDIT.md` … `PHASE_5_LOCAL_INFERENCE.md` are untouched.
- No authentication, licensing, telemetry, settings, workspace scanning or inference code
  was modified in this phase.
- No dependency was added: the ZIP writer, CRC-32 and DOS time are hand-written.
- `capabilities/main.json`, `tauri.conf.json` and the CSP are unchanged.
- The existing UI shell, typography, spacing and violet accents are reused; the panel adds
  no second notification system and no global state.

---

## 12. What Phase 7 has not inherited

Quotas, daily resets, usage counters and enforcement were not touched. No counters, timers or
persisted usage fields were introduced by this phase.

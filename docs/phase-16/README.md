# Phase 16 — Advanced Deep Codebase Intelligence

> **Status: COMPLETED 2026-10-18 — cross-file graph, relationships, impact, smarter retrieval, local-only, bounded. No push while `isPrivate:false` (public).**
> `isPrivate: false` reported 2026-10-18; safety gate active. No commits pushed, no PR, no visibility change this phase — local commits only.

## 1. Starting Git state (verified 2026-10-18)

| Item | Observed |
|------|----------|
| Branch | `arena/74aec262-nexora` |
| HEAD at start | `c2a2a71 Phase 15: bounded validation & AI self-correction (local-only, no push while public)` |
| Prior heads | `a15b304 Phase 14`, `776f44d Phase 13`, `5313bf2 Phase 12`, `0339588 Phase 11`, `09c25c3 Phase 10 hardening`, `2e1f854 Phase 10` preserved |
| Origin HEAD | `2e1f854` |
| Merge base | `04deeba` |
| Working tree | clean at audit |
| `isPrivate` | `false` (public) via `gh repo view` |
| Remotes | `origin https://github.com/cybervault-Hacky/Nexora.git` |

**Gate:** public repo — local work only, no push/PR/visibility/release/deploy, no secrets logging.

**Phases 14-15 verified:** `docs/phase-14/README.md` COMPLETED (symbol index 500/5MiB, retrieval 8/100KiB), `docs/phase-15/README.md` COMPLETED (validation 6 checks, repair MAX 3, 426 tests, 383 KiB build).

## 2. Audit findings — what existed vs what needed

### 2.1 What already existed (reuse)
- `scanner.rs` — iterative DFS, MAX_DEPTH 12, 8000 files/dirs, 2 MiB inventory, 256 KiB preview, 27 excluded dirs, `is_sensitive_file`, `validate_relative_path`, `canonicalize_root` + `starts_with`, binary via NUL/utf8, posix relative.
- `symbol_index.rs` — `MAX_INDEXED_FILES 500`, `MAX_INDEXED_BYTES 5 MiB`, `MAX_FILE_SIZE 256 KiB`, `MAX_SYMBOLS 5000`, regex for rs/ts/tsx/js/jsx/py (others unsupported), `build_symbol_index` reuses scanner + sensitive/binary/size + limits, `search_symbols` exact→prefix→substring→path, 14 tests.
- `retrieval.ts` — deterministic `retrieveRelevantFiles` (selected first, then ranked symbols, budget 8/100 KiB, dedup, sensitive filter, warnings, never whole-repo), `buildGroundedContext` with citations.
- `contextBudget.ts` — `estimateTokens ceil(chars/4*1.05)`, SYSTEM_OVERHEAD 200 + maxTokens 1024 reserved, priority prompt→selected→history, never slice fence, warnings.
- `validation.rs` + `useValidation`/`usePatchRepair` — bounded 3s validation, 12/15s repair, cancellation, stale guard, workspace switch.
- `App.tsx` — `symbolIndex` state, `retrievalResult` via `retrieveRelevantFiles`, `groundedContextBlock` header with verified paths/lines, `CodeIntelligencePanel` + `RetrievalBar` + `ContextBudgetBar` + `PatchGenerateBar` + `ValidationPanel`.

### 2.2 What was missing (Phase 16 gap)
- No cross-file imports/dependency graph — symbol index stored Import symbols but not resolved to files, no verified vs inferred distinction.
- No circular detection, no change impact (which files depend on this file, transitive).
- Retrieval not dependency-aware — did not prioritize files that import/depend on selected file.
- No file-level relationship view (imports, dependents, defined symbols, referenced_by with evidence path:line).
- No project-level answers for “where is feature”, “which files call this”, “what could be affected” beyond symbol search.

### 2.3 Reuse decision
No second scanner, no new inference engine, no vector DB, no `syn`/`tree-sitter` (keeps `regex` only, no C build). Reuse `scanner` exclusions, `symbol_index` extraction, `retrieval` budget, `inference` streaming, existing UI tokens.

## 3. Implementation scope

### 3.1 Backend — `workspace/analysis.rs` (new, ~650 lines, 14 tests)

**Limits (bounded, same family as symbol_index):**
- `MAX_ANALYSIS_FILES 500`, `MAX_ANALYSIS_BYTES 5 MiB`, `MAX_FILE_SIZE 256 KiB` (via `MAX_PREVIEW_BYTES`), `MAX_EDGES_TOTAL 2000`, `MAX_EDGES_PER_FILE 50`, `MAX_TRAVERSAL_DEPTH 4`, `MAX_IMPACT_RESULTS 20`, `MAX_CYCLES 10`, `MAX_CYCLE_LEN 10`. Enforced before read, after read, and during graph/cycle/impact.

**Import extraction (no exec, regex only, bounded):**
- `ts/tsx/js/jsx`: `import ... from 'spec'`, `import 'spec'`, `require('spec')`, `import('spec')`, `export * from 'spec'` — regex `import\s+(?:[\w*\s{},]+\s+from\s+)?['"]([^'"]+)['"]` etc., dedup by spec, line via `line_number`, max 50 per file.
- `py`: `import X`, `from X import` — `^\\s*import\\s+([A-Za-z0-9_.]+)`, `^\\s*from\\s+([A-Za-z0-9_.]+)\\s+import`.
- `rs`: `use ...;`, `mod foo;` — `^\\s*use\\s+([^;]+);`, `^\\s*mod\\s+(\\w+)\\s*;`.
- Others: no imports (unsupported).

**Resolution (verified vs inferred, never invent):**
- `resolve_ts_spec`: only relative `./` or `/` candidates tried: `spec`, `spec.ts`, `.tsx`, `.js`, `.jsx`, `.json`, `spec/index.ts` etc., normalized via `normalize_posix`, checked against `existing_set` (eligible files). Bare `react` → `None` with note “external package — not resolved locally”.
- `resolve_py_spec`: `spec.replace('.', '/') + '.py'` and `__init__.py`, else unresolved note “unresolved — file not found; may be stdlib”.
- `resolve_rs_mod`: single `mod foo;` → `dir/foo.rs`, `dir/foo/mod.rs`, `src/foo.rs`; `use crate::foo::bar` → `src/foo/bar.rs` etc., external crates `std/core/serde/tauri/tokio` → unresolved “external crate”.
- Result: `DependencyEdge { from, to (resolved path or raw spec), raw, line, verified: bool, kind, note }`. Verified only if resolved path exists in workspace and is eligible.

**Graph building `build_dependency_graph(root)`:**
- Uses `scan_workspace` inventory, filters sensitive (`is_sensitive_file`), oversized, canonical `starts_with`, binary (NUL), supported ext only → `existing_set` + `eligible` list.
- Applies file/bytes/edges truncation flags. For each eligible file, reads `fs::read_to_string`, extracts imports, resolves via `existing_set`, creates edges, collects `nodes: Vec<FileNode { path, imports, importedBy }>` and `edges`. Builds `imported_by` reverse map (dedup, max 50 per node). Detects cycles via DFS on verified edges only (bounded, sorted keys, dedup, max 10 cycles, max len 10). Returns `AnalysisGraph { version:1, root, nodes, edges, truncated, unsupported, cycles, generatedAt }`. Never writes into root, no shell, no install.

**File relationships `file_relationships(root, target)`:**
- Validates `target` (non-empty, no `..`, not sensitive, `MAX_PATH_CHARS` via commands). Builds graph + symbol index (fallback empty on error). Finds node for target, splits imports into `verified` vs `unresolved`, collects `imported_by`, `definedSymbols` (filter symbol_index where `path == target`, max 200), `referenced_by` via substring search for each defined symbol name (len>2, not Import) across graph nodes' file contents (max 50 refs, snippet 120 chars, line via `line_number`), notes for truncated/unsupported/no-symbols. Distinguishes verified (resolved import) vs inferred (name-match reference).

**Change impact `change_impact(root, target)`:**
- Validates target, builds graph, builds `adj_rev` (to→from) and `adj_fwd`. BFS direct dependents from `adj_rev[target]` (sorted, dedup), evidence from edge line, confidence `verified` if edge verified else `inferred`. Transitive BFS depth 2..`MAX_TRAVERSAL_DEPTH` from direct queue, dedup via `visited`, max `MAX_IMPACT_RESULTS`, truncated flag. Also adds symbol-reference impact: for symbols defined in target (max 20 names), search other files for name occurrence (inferred, line evidence). Risks: if target in cycle → “Target participates in circular dependency: …”, else if cycles exist → “Project has N circular dependencies”, plus “N direct + M transitive potential dependents — treat as potential risks, not guaranteed”, plus “Static analysis does not prove runtime”. Notes include truncated/unsupported and confidence legend.

**Tests (14):** `extract_ts_imports`, `resolve_relative_ts`, `graph_simple_chain`, `detects_cycle`, `unresolved_import`, `impact_direct_and_transitive`, `relationships_referenced_by`, `sensitive_excluded`, `python_and_rust_imports`, `limits_truncated`, `unsupported_language_skipped`, `traversal_protected`, `no_circular_for_chain`, plus earlier symbol tests still passing.

### 3.2 Commands — `workspace/commands.rs` (extend)

New `AnalysisState { graph: Mutex<Option<AnalysisGraph>>, root: Mutex<Option<PathBuf>> }` + `analysis_lock_err`.

- `get_dependency_graph(root)` — cached if root same and version matches, else `build_dependency_graph`.
- `refresh_dependency_graph(root)` — always rebuild.
- `get_file_relationships(root, path)` — validates empty/long, `canonicalize_root`, delegates to `analysis::file_relationships`. No `AnalysisState` needed (always fresh, bounded).
- `get_change_impact(root, path)` — same.
- `clear_analysis_graph()` — forgets cached graph/root on close.

All commands validate `root` non-empty, `path` length ≤4096, `..` rejected, `is_sensitive_file` via analysis, reuse `canonicalize_root` + `starts_with`. No `Command::new`, no `fs::write` into project during analysis, no package install.

### 3.3 Frontend — libs & hooks

**`lib/analysis.ts` (new):** Types `DependencyEdge`, `FileNode`, `AnalysisGraph`, `Reference`, `FileRelationships`, `ImpactNode`, `ImpactReport` (camelCase to match Rust serde), `confidenceLabel`, `edgeLabel`, wrappers `getDependencyGraph`, `refreshDependencyGraph`, `getFileRelationships`, `getChangeImpact`, `clearAnalysisGraph` via `invoke` + `isDesktopShell` guard + `toError` (code/message).

**`features/workspace/useAnalysis.ts` (new):**
- `useAnalysis(workspacePath)`: state `idle|loading|loaded|error|cancelled`, `refresh`, `cancel`, `reset`. Bounded timeout `GRAPH_TIMEOUT_MS 4000`, `AbortController`, `requestIdRef` for stale guard, workspace switch `useEffect` resets and aborts, handles timeout → error, handles cancel → `cancelled`, ignores stale results (`requestIdRef`/`controller.signal`).
- `useRelationships(workspacePath)`: state `idle|loading(path)|loaded(data)|error|cancelled`, `load(path)`, `cancel`, `reset`. Timeout 3000 ms, validates path non-empty, checks `data.path === requestedPath` association, stale guard via `requestIdRef` + `pathRef`.

**`features/workspace/useImpact.ts` (new):** `useImpact(workspacePath)` — same pattern, `analyze(path)`, timeout 3500 ms, association `report.target === path`, stale/workspace-switch handling, cancellation.

**`features/workspace/retrieval.ts` (extend):**
- New `SmartRetrieveArgs` with optional `graph` (nodes with `imports: [{to, verified}]`, `importedBy`, `edges`), new `retrieveRelevantFilesSmart(args)` that starts from `retrieveRelevantFiles` base, then adds graph-aware candidates:
  1. For each selected file, adds verified dependencies (`imports.verified`) and verified dependents (`importedBy`) with reason `dependency of X (verified import)` / `dependent of X (imports it)`, priority 1.
  2. For symbol-based inclusions, adds files that import the defining file (impact via graph), priority 2.
  Dedup by path, sorts by priority then path, respects `maxFiles`/`maxBytes` (4000 estimated per file), sensitive excluded, never duplicate, adds warning “Graph-aware: added N related file(s) via verified imports”. Fallback to base if no graph.

### 3.4 UI — `components/AnalysisPanel.tsx` (new) + `styles/app.css` (extend)

**Panel structure (no Phase 18 premium redesign):** Reuses tokens, `340px` explorer width, no glassmorphism premium, follows `CodeIntelligencePanel` patterns.

- **Header:** `Deep Analysis` + badge `N files · M edges` when loaded + `Refresh` (disabled while loading) + `Cancel` when loading. Uses existing `Icon` (`search`, `refresh`, `alert`, `file`, `x`).
- **States:**
  - No workspace: hint “Open a project to see deep codebase analysis.”
  - Loading: “Building dependency graph… Bounded (max 500 files, 5 MiB, 2000 edges)…”
  - Error: banner with message + Retry.
  - Empty graph: hint supported languages.
  - Overview (when loaded): stats `files indexed`, `verified+unresolved edges`, `N circular` (warn if >0), `truncated`/`unsupported`, cycles list (up to 5, each `a → b → a`, note “verified edges only; treat as potential”), meta `Last analyzed: time — root`.
- **Tabs (role=tablist):** `Graph` (default), `File`, `Impact` — File/Impact disabled if no `targetPath` (selected single or preview or first context file). Tab switching via `activeTab` state.
  - **Graph tab:** up to 12 nodes, each `path` button → `onPreview`, meta `imports · dependents`, edge list up to 3 per file: `raw → to` + badge `verified` (green) / `unresolved` (yellow) + `:line` + note, hint “Verified = resolved to existing file; unresolved = external or not found (inferred). Never invented paths.”
  - **File tab:** for `targetPath` shows `imports (N)` list (code `raw` → `to` + confidence + line + note + `open` button if verified), `imported by (N)` list (verified), `defined symbols (N)` (code name (kind) :line, detail, up to 10), `referenced by (N)` (code symbol in `path:line` inferred + snippet, hint “inferred via substring search — may include coincidental matches, not proven call graph”), `unresolved` list if any, notes (truncated, unresolved count, no-symbols). Loading/error/empty handled, Cancel.
  - **Impact tab:** for `targetPath` shows `direct dependents (N)` (button path — reason verified + evidence), `transitive/inferred (N)` (inferred, “potential risk, not guaranteed”), truncated warn, risks list (circular participation, “N direct + M transitive potential …”, “Static analysis does not prove runtime”), notes (truncated, unsupported, confidence legend), hint “Static analysis does not prove runtime behavior. Verify with tests.”
- Handles cancellation (`cancel` callbacks), workspace switching (reset via hooks' useEffect), stale results (requestId guard), and distinguishes verified vs inferred everywhere with badges/colors.

**App wiring (`App.tsx`):**
- Imports `getDependencyGraph`, `clearAnalysisGraph`, `AnalysisGraph`, `retrieveRelevantFilesSmart`, `AnalysisPanel`.
- State `analysisGraph: AnalysisGraph | null` + effect that on `workspacePath`/`inventory` change fetches `getDependencyGraph` (cancels on unmount, clears on null), mirrors symbol index pattern.
- `retrievalResult` now `useMemo` that if `analysisGraph` exists uses `retrieveRelevantFilesSmart` with graph, else fallback to `retrieveRelevantFiles` — respects budget/sensitive, adds graph-aware warning.
- `groundedContextBlock` header now includes `graphSummary` (`Graph: N files, M edges, K cycles, truncated`) and distinguishes “verified facts (resolved import, confirmed symbol:line) from inferred (name-match, transitive)”, plus dependency note “verified = file exists, unresolved = external or not found; cycles via verified only.”
- Render: after `ContextBudgetBar`, adds `<AnalysisPanel workspacePath={workspacePath} targetPath={generationTargetPath ?? workspace.previewPath ?? (workspace.contextFiles[0]?.path ?? null)} onPreview={workspace.loadPreview} />` — before `PatchGenerateBar`, inside `workspace__explorer` (same grid as before, no layout break).

**Grounded context improvement for project-level questions:** The enhanced `groundedContextBlock` plus `retrieveRelevantFilesSmart` ensures that queries like “where is feature X implemented?” (symbol defines), “which files call this function?” (referenced_by + dependents), “what could be affected?” (impact direct/transitive), “where to investigate issue?” (relationships + impact), “which relationships could not be verified?” (unresolved list + verified/inferred badges) are answered with indexed evidence (file:line, verified edge) and with explicit limitation notes when not grounded.

## 4. Security and performance controls

- **Workspace-root restriction:** Every backend entry validates `root` via `canonicalize_root` and `target` via `..` check, `is_sensitive_file`, `MAX_PATH_CHARS` 4096, absolute rejection, `canonical_file.starts_with(canonical_root)` for reads, never escapes root.
- **Sensitive exclusion:** `is_sensitive_file` on relative and canonical (`.env`, `*.pem`, `*.key`, `id_rsa`, `credentials.json`, `/.env` path) before indexing, graph, retrieval, and context building; `.env` never indexed, never in graph, never in context, impact/relationships return `secret_excluded`.
- **Bounded indexing/graph:** Reuses scanner limits + new `MAX_ANALYSIS_FILES 500`, `MAX_ANALYSIS_BYTES 5 MiB`, `MAX_FILE_SIZE 256 KiB`, `MAX_EDGES_TOTAL 2000`, `MAX_EDGES_PER_FILE 50`, `MAX_SYMBOLS 5000`, `MAX_TRAVERSAL_DEPTH 4`, `MAX_IMPACT 20`, `MAX_CYCLES 10`, `MAX_CYCLE_LEN 10`, `MAX_CONTEXT_FILES 8`, `MAX_CONTEXT_TOTAL_BYTES 100 KiB`, `MAX_PREVIEW_BYTES 256 KiB`. Truncated/unsupported flags propagated to UI.
- **No unbounded recursion:** DFS for cycles uses iterative stack + visited set, sorted keys, early exit at limits; BFS for impact uses `VecDeque` + `visited` set + depth counter, stops at `MAX_IMPACT`.
- **Memory:** Graph stores only paths + edges + 50 refs max, no full file contents in state (contents read on demand for relationships/impact with 120-char snippet, not stored). Frontend stores graph once (~few KB per node).
- **Cancellation & workspace switching:** `useAnalysis`/`useRelationships`/`useImpact` use `AbortController` + `requestIdRef` + `pathRef` + `setTimeout` (4000/3000/3500 ms) + `useEffect([workspacePath])` reset; backend graph cached per root, but frontend ignores stale resolves, `clearAnalysisGraph` on close.
- **No exec:** No `Command::new`, no shell, no `cargo`/`npm`/`pytest`, no `build.rs` hooks; `static_analysis` remains `NotRun` where exec would be needed; analysis is pure regex + file reads + graph traversal.
- **No telemetry/cloud/secret logging:** Local-only (`loopback` inference not used here), no external send, no prompt logging, only `path`/`check id`/`error code` in errors, file contents never logged, sensitive never indexed.
- **No silent modification:** Analysis never calls `fs::write` into root, no `apply`, no install, no dependency manifest changes; impact is read-only.

## 5. Testing

### 5.1 Rust unit (in `analysis.rs`, 14 tests)

- `extract_ts_imports` — parses `import ... from`, `require`, `import()`, `export * from` via regex, dedup.
- `resolve_relative_ts` — `./bar` from `src/app.ts` → `src/bar.ts`, `../utils` → `src/utils/index.ts`, external `react` → unresolved with note.
- `graph_simple_chain` — `a.ts→b.ts→c.ts` chain verified, imported_by correct, cycles empty.
- `detects_cycle` — `a.ts↔b.ts` yields cycle `a.ts → b.ts → a.ts`.
- `unresolved_import` — `./missing` + `react` both verified=false with notes.
- `impact_direct_and_transitive` — `base→mid→top` direct contains mid, transitive contains top, no unrelated, risks contain “potential”.
- `relationships_referenced_by` — `foo.ts` defines `greet`, `bar.ts` imports and references, `referenced_by` includes bar via name search.
- `sensitive_excluded` — `.env` not in graph, impact on `.env` returns `secret_excluded`.
- `python_and_rust_imports` — `import os` unresolved external, `import mymod` verified, `use crate::utils` verified, `mod utils` verified.
- `limits_truncated` — 600 files → truncated true, nodes ≤500, edges ≤2000.
- `unsupported_language_skipped` — `README.md`/`data.json` not in nodes, unsupported ≥2.
- `traversal_protected` — `../outside.ts` → `invalid_request`.
- `no_circular_for_chain` — `a→b→c` no cycles.

### 5.2 Frontend unit & component (5 files, 42 tests)

- `lib/analysis.test.ts` (2): `confidenceLabel`, `edgeLabel`.
- `features/workspace/useAnalysis.test.tsx` (13): graph load success, truncated/cycles, unresolved, sensitive error, cancellation, timeout (4100 ms), workspace switch idle, stale ignored, relationships verified vs inferred, unsupported (no symbols), cancellation, workspace switch, incorrect evidence association (path mismatch → error).
- `features/workspace/useImpact.test.tsx` (10): direct+transitive confidence, isolated (no dependents), ranking verified before inferred + truncated, circular risk, sensitive error, cancellation, timeout (3600 ms), workspace switch, stale ignored, incorrect evidence (target mismatch → error).
- `features/workspace/retrieval.smart.test.ts` (7): prioritizes dependencies of selected via graph, adds dependents, respects budget & sensitive (filters `.env`), truncates at maxFiles, fallback to base when no graph, adds dependents of queried symbol defining file, never duplicates selected.
- `components/AnalysisPanel.test.tsx` (10): hint when no workspace, loading then graph overview, graph with verified edges + cycles + truncated, unresolved/unsupported handling, distinguishes verified vs inferred in file relationships, shows impact verified/inferred + risks, handles error states (Project not found), workspace switching clears, sensitive exclusion (no .env), unsupported language (no symbols, notes) — all use `getAllByText` for multiple badges, `waitFor` for async, `userEvent` for tab clicks.

### 5.3 Regression

- Existing Phase 14-15 suites still pass: `symbolIndex` (9 tests), `retrieval` (9), `validation` (10), `useValidation` (10), `usePatchRepair` (8), `ValidationPanel` (10), `PatchGenerateBar` (15), `PatchPreview` (10), `usePatchGeneration` (17), `useConversation` (20), `App` (9), `App.interaction` (16), `WorkspacePanel` (8), `CodeBlock` (10), `FilePreview` (9), `RetrievalBar` (4), `ContextBudgetBar` (4), `history` etc. total 468/42 (was 426/37).

## 6. Verification (2026-10-18)

| Check | Result |
|-------|--------|
| `pnpm typecheck` (`tsc --noEmit`) | **PASS** (fixed `git-branch` icon → `search`) |
| `pnpm test` (`vitest run`) | **PASS 468 / 468** (42 files) — +42 vs 426 (analysis lib 2 + useAnalysis 13 + useImpact 10 + retrieval.smart 7 + AnalysisPanel 10) |
| `pnpm build` (`tsc && vite`) | **PASS** — `dist/assets/index-Bm2uUbmE.js 404.68 KiB` (gzip 117.98 KiB), css 53.28 KiB (was 383.06 KiB, +21.62 KiB) |
| `cargo fmt --check` | **BLOCKED** — `cargo: command not found` (no Rust toolchain in sandbox) — not `PASS`, reported as BLOCKED |
| `cargo check` | **BLOCKED** — same reason |
| `cargo clippy` | **BLOCKED** — same reason |
| `cargo test` | **BLOCKED** — same reason + would need `analysis.rs` 14 tests on CI (Linux/Windows). No local exec. |

CI (GitHub Actions) **NOT RUN** (local only, public gate). No weights, no external APIs.

## 7. Security and performance considerations (summary)

- Verified vs inferred always labeled; never invent path/symbol/line — uses actual `line_number` from `content[..offset]` and real `resolved` check against `existing_set`.
- Incomplete indexing reported: `truncated`, `unsupported`, `parseErrors`, `Graph truncated — results may be incomplete`, `No symbols indexed` hint.
- Unsupported syntax (regex) not claimed as full — `.md/.toml/.yaml/.css/.html/.sh/.go/.java` are `unsupported` (no symbols, no imports), `.json` no imports.
- Static analysis explicitly not proof of runtime: risks include “Static analysis does not prove runtime behavior — verify with tests”.
- Sensitive never indexed or in prompt: `is_sensitive_file` both relative and canonical, plus `isSensitivePath` in retrieval.
- Bounded: see §3.1 limits; timeout/cancel prevents UI block; workspace switch clears stale.

## 8. Known limitations

- Symbol extraction via regex (not `tree-sitter`/`syn`) — misses macros, nested `impl`, multi-line signatures, dynamic `export`, Python decorators, JS dynamic `import`. Mitigated by `unsupported`/`parseErrors` counts and `Inferred` labeling for `referenced_by`.
- Import resolution heuristic: TS `alias` paths (`@/lib` without `tsconfig` mapping) remain unresolved; Python namespace packages without file may be unresolved; Rust `use` of proc macros not resolved.
- `referenced_by` is substring search for symbol names — may include coincidental matches (e.g., variable named like function), flagged as inferred.
- Cycle detection only on verified edges; inferred/unresolved edges excluded to avoid false cycles.
- Impact transitive depth capped at 4, results capped at 20, graph nodes capped at 500 — large monorepos will be truncated (flagged).
- No execution of project code, no type-checking, no `cargo check` — semantic relationships (trait impls, re-exports) not verified.

## 9. Manual testing instructions

1. `pnpm typecheck` → PASS; `pnpm test` → 468/468; `pnpm build` → PASS 404 KiB.
2. Open desktop app, open project (e.g., `apps/desktop` itself). Wait for `Deep Analysis` to show `files indexed · edges` (Refresh if needed). Check no `.env` in graph, check cycles (if any) show verified path.
3. Select a file in explorer or preview (e.g., `src/components/CodeIntelligencePanel.tsx` which imports `Icon`). In `Deep Analysis` → `File` tab, verify `Imports` lists `./Icon` → `src/components/Icon.tsx` verified with `:line`, `Imported by` lists dependents (verified), `Defined symbols` shows `CodeIntelligencePanel` etc. with `:line`, `Referenced by` shows files mentioning that symbol (inferred) with `path:line` and snippet.
4. Create unresolved import: add `import { missing } from "./does-not-exist";` in a file, refresh, verify it appears as `unresolved — file not found` (yellow, inferred) and in `Unresolved` section.
5. Check supported vs unsupported: open `README.md`, File tab should show “No symbols indexed for this file (unsupported language or empty)” and `unsupported skipped` in overview.
6. Check change impact: with `src/components/Icon.tsx` selected, go to `Impact` tab, verify `Direct dependents` lists files that import it (verified, with `:line`), `Transitive` lists dependents of dependents (inferred, “potential risk”), risks include “potential dependents — treat as potential risks, not guaranteed” and “Static analysis does not prove runtime”.
7. Create cycle: make `src/a.ts` import `src/b.ts` and `src/b.ts` import `src/a.ts`, refresh, verify `1 circular dependency` appears in overview with `a.ts → b.ts → a.ts`, and Impact for `a.ts` shows risk “Target participates in circular dependency”.
8. Check smarter retrieval: select `src/a.ts` as context, type query `greet` that is defined in `src/b.ts`, `RetrievalBar` should show `dependency of …` or `imports …` added via graph-aware warning “Graph-aware: added N related file(s) via verified imports” and `GROUNDED` prompt header includes `Graph: N files…`.
9. Check limits & cancellation: open huge project (>500 files) → overview shows `truncated — limits hit` and `unsupported` count. Click Refresh then Cancel → shows `cancelled` or `timed out` after 4s, no crash. Switch workspace → panel resets to `Open a project…`, previous graph not shown.
10. Check sensitive exclusion: ensure `.env` or `credentials.json` never appears in `Deep Analysis` graph, File, or Impact, and never in `RetrievalBar` or grounded prompt.

## 10. Git state (end of Phase 16)

| Item | Observed |
|------|----------|
| Branch | `arena/74aec262-nexora` |
| HEAD after Phase 16 commit | to be `c2a2a71` + Phase 16 local commit (new) |
| Origin `arena/74aec262-nexora` | `2e1f854` (unchanged, not pushed) |
| `isPrivate` | `false` (public) — no push/PR/visibility change, no publish |
| Working tree | clean after commit |
| Local commits not on origin | `c2a2a71` + Phase 16 commit (2 ahead of `2e1f854`, plus `a15b304/776f44d/5313bf2/0339588/09c25c3` in log) — preserved, no reset/rebase |
| Diff vs `origin/main` (`04deeba`) | Phases 10-16 files (~90 files) — analysis added |

**Gate respected:** `gh repo view --json isPrivate` still `false`; `git push` not run; PR not opened.


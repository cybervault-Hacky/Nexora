# Phase 27: Code Intelligence & Dependency Analysis

## 1. Overview & Architectural Scope

Phase 27 implements and formalizes the **Code Intelligence** system for Nexora, fulfilling the requirements of the original 32-phase product roadmap.

Nexora provides project-aware code intelligence that operates locally, deterministically, and with zero network leakage:
- **Local AST & Symbol Indexing**: Extracts functions, methods, classes, types, interfaces, structs, enums, traits, and constants across Rust (`.rs`), TypeScript/JavaScript (`.ts`, `.tsx`, `.js`, `.jsx`), and Python (`.py`).
- **Verified Dependency Graphs**: Maps source-to-source import relationships, separates verified relative imports from unresolved dependencies, and detects circular import cycles.
- **Change Impact & Blast Radius Audit**: Evaluates direct and transitive dependents for any selected target file, surfacing potential modification risks before patches are generated or applied.
- **Strict Honesty & Bounded Execution**: Files are analyzed with static parser patterns without running untrusted code; unsupported languages are explicitly tallied and reported rather than invented or hallucinated.
- **Offline Sovereignty**: Zero code, symbol, or dependency data is ever transmitted to cloud services or telemetry collectors.

---

## 2. Key Capabilities

### 2.1 Symbol Indexing & Filtered Search
- **Bounded Ingestion**: Ingests files up to strict project boundaries (`MAX_FILES = 500`, `MAX_BYTES = 5 MiB`), automatically excluding build outputs (`node_modules`, `target`, `dist`, `.git`).
- **Kind-Based Classification**: Categorizes symbols into functions, types/classes, and constants.
- **Interactive Kind Filtering**: Developers can filter search results dynamically across:
  - **All**: All matching symbols.
  - **Functions**: Functions and class methods.
  - **Types & Classes**: Classes, interfaces, types, structs, enums, and traits.
  - **Constants**: Constants and read-only definitions.
- **Workspace Navigation**: Clicking any search result opens the target file in the read-only previewer at the designated line number.

### 2.2 Dependency Graph & Relationship Analysis
- **Full Graph Overview**: Displays node count, total edges, circular dependency count, and truncated flags.
- **Verified vs. Unresolved Edges**: Distinguishes verified local module imports from external packages or unresolvable module paths.
- **Circular Dependency Detection**: Flags circular dependency chains (e.g. `A → B → C → A`) to help developers prevent architectural regressions.

### 2.3 Single-File Impact & Blast Radius
- **Direct Dependents**: Identifies files that import the target file directly.
- **Transitive Dependents**: Computes the secondary blast radius of changes.
- **Risk Assessment**: Surfaces circular dependency risks and high-fanout warnings before patch application.

### 2.4 User Interface & Routing
- **Dual Routing**: Fully accessible via both `#/code` and `#/intelligence`.
- **Honest Empty State**: When no workspace is active, displays an accessible empty state with an "Open project folder" action.
- **View Switcher Tabs**:
  - `All views`: Full split view with Symbol Index & Search and Deep Analysis.
  - `Symbol index & search`: Focused symbol searching and filtering.
  - `Dependency & impact analysis`: Focused dependency graph and change impact audit.
- **Header Actions**: Displays active project name, root filesystem path, and project switching controls.

---

## 3. Implementation Summary

### Files Created & Modified
1. **`apps/desktop/ui/src/pages/CodeIntelligencePage.tsx`**:
   - Modernized layout with header controls, compact status summary, view switcher tabs, and honest empty state card.
   - Connected `onOpenWorkspace` and `workspaceName` properties.
2. **`apps/desktop/ui/src/components/CodeIntelligencePanel.tsx`**:
   - Added interactive symbol kind filtering (`All`, `Functions`, `Types & Classes`, `Constants`).
   - Integrated `useMemo` for instant client-side filtering without redundant IPC queries.
3. **`apps/desktop/ui/src/components/AnalysisPanel.tsx`**:
   - Added accessible landmark `role="region"` for automated screen readers and tests.
4. **`apps/desktop/ui/src/lib/router.ts`**:
   - Added route alias `/intelligence` mapping to the `code` route ID.
5. **`apps/desktop/ui/src/App.tsx`**:
   - Passed `workspaceName` and `onOpenWorkspace` to `CodeIntelligencePage`.
6. **`apps/desktop/ui/src/styles/app.css`**:
   - Added CSS classes for `.code-intel__filters`, `.code-intel-summary`, `.code-intel-summary__item`, and `.code-intel-tabs`.

---

## 4. Verification & Test Results

### 4.1 Unit & Component Tests
- **`apps/desktop/ui/src/pages/CodeIntelligencePage.test.tsx`** (4 tests, all passing):
  - Renders empty state call to action when no project is open.
  - Renders active project header, summary metrics, and panels when project is open.
  - Switches views using tabs between all, symbols, and analysis.
  - Handles switch project button click in header.
- **`apps/desktop/ui/src/components/CodeIntelligencePanel.test.tsx`** (12 tests, all passing):
  - Overview display, file type badges, search input, clear button, error handling, stale query cancellation, kind filtering, and truncation warnings.
- **`apps/desktop/ui/src/components/AnalysisPanel.test.tsx`** (10 tests, all passing):
  - Graph overview, cycles display, file relationships, change impact audit, and cancel/retry actions.

### 4.2 Integration Tests
- **`apps/desktop/ui/src/App.phase27Intelligence.test.tsx`** (5 integration tests, all passing):
  - Renders empty state on `#/code` and `#/intelligence` when no workspace is active.
  - Loads symbol index and displays genuine codebase metrics when workspace is open.
  - Searches indexed symbols and filters results by symbol kind.
  - Switches views using tabs between symbols and dependency analysis.
  - Refreshes symbol index and dependency graph without corrupting workspace.

### 4.3 Full Test Suite Pass Rate
- **Vitest**: **68 test files passed, 664 tests passed (0 failures)**.
- **TypeScript (`pnpm typecheck`)**: 0 errors.
- **Production Build (`pnpm build`)**: 118 modules transformed, client bundle built cleanly in 1.82s.
- **Git diff check (`git diff --check`)**: Clean pass with zero formatting or trailing whitespace errors.

---

## 5. Limitations & Windows Verification Notice

### Current Environment
- Host: Linux Sandbox (`x86_64`).
- All tests and builds were executed directly in this environment with real Node.js and Vitest runners.
- Linux verification does not replace real Windows runtime verification.

### Remaining Verification Steps for Windows
1. **Windows Native Runner**: Run GitHub Actions CI workflow on `windows-latest`.
2. **File Path Normalization**: Verify Windows backslash path separators (`\`), drive letters (`C:\`), and UNC paths in symbol and dependency graphs.
3. **Rust Compiler Toolchain**: Verify compilation with `cargo check --all-targets` and `pnpm tauri build` on Windows.

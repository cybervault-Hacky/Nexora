# Evidence index

Campaign date/time: 2026-10-26, timestamp directory `20261026_030506_AsiaKolkata`.

## Captured logs and records

- `workspace-initial-state.md` — dirty-tree baseline and preservation record.
- `host-inventory.md` — relevant OS/toolchain/memory/display/runtime availability observations.
- `pnpm-install.log` — `corepack pnpm install --frozen-lockfile` result and pre-install hashes.
- `lockfile-hashes-after.txt` — after-install SHA-256 comparison; both lockfiles unchanged.
- `pnpm-audit.log` — JavaScript dependency audit.
- `frontend-typecheck.log`, `frontend-typecheck-rerun.log` — TypeScript checks.
- `frontend-tests.log` — initial full frontend run (two stale/incomplete contract failures; retained as failure evidence).
- `rust-contract-regression.log` — focused contract/parser regression run after correction (60/60 passed).
- `frontend-tests-rerun.log` — complete post-fix frontend suite (90 files, 891/891 tests passed).
- `installer-contract-tests.log` — initial standalone run (17/17); `installer-contract-tests-rerun.log` — post-fix standalone rerun (17/17).
- `frontend-production-build.log` — production TypeScript/Vite build plus config-security and installer-contract scripts; records advisory bundler warnings.
- `tauri-environment.log` — Tauri environment diagnostic; native toolchain/system dependency blockers.
- `git-diff-check.log` — final tracked-diff whitespace/conflict-marker check (`git diff --check`, exit 0).
- `final-integrity-check.log` — new Markdown whitespace/final-newline and evidence-inventory checks.
- `workspace-final-status.txt` — final `git status --short` snapshot; no changes staged.
- `media-attempts.md` — explicit per-operation gate and honest capability classification.

## Screenshots and artifacts

No genuine screenshot was captured: the app could not be launched (missing Rust/Cargo and Linux Tauri system dependencies; no GUI session was established). No synthetic/static UI screenshot is presented as app evidence. No real media artifact exists because no generation/edit/enhancement operation was run. The Vite `apps/desktop/ui/dist/` directory is build output only, not a desktop app or model/runtime artifact.

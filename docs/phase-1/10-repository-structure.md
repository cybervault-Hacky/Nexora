# 10 — Proposed Private GitHub Repository Structure

**Status:** Proposed for owner approval  
**Important:** This document describes a repository layout only. **No repository has been created, initialized, or published.** The existing workspace contains planning documents only.

---

## 1. Principles

1. **Monorepo, clear boundaries.** One private repository for the desktop app, Rust crates, UI, templates, tests, and docs; dependencies flow in one direction.
2. **Privilege lives in Rust.** Security-sensitive code is isolated in `nexora-policy`, `nexora-security`, and `nexora-tools`, with enforced dependency rules.
3. **No generated or binary blobs in Git.** Models, runtime packs, installers, and build output are release artifacts, not commits.
4. **Everything reproducible.** Pinned toolchains, locked dependencies, documented build commands.
5. **Docs next to decisions.** ADRs and architecture notes are versioned with the code.
6. **Private by default.** No public release, package, or repository without explicit owner approval.
7. **No secrets, ever.** CI secrets live in protected environments; local secrets never enter Git.

---

## 2. Proposed tree

```
nexora/
├─ .github/
│  ├─ workflows/
│  │  ├─ ci.yml                 # PR: fmt, lint, tests, typecheck, build
│  │  ├─ nightly.yml            # E2E, eval subset, perf smoke, installer
│  │  ├─ release.yml            # tag: sign, SBOM, draft release, approval
│  │  └─ security.yml           # CodeQL, dependency review, secret scan
│  ├─ CODEOWNERS
│  ├─ pull_request_template.md
│  ├─ ISSUE_TEMPLATE/
│  │  ├─ bug_report.yml
│  │  ├─ feature_request.yml
│  │  └─ security_note.yml      # redirects to private disclosure
│  └─ dependabot.yml
│
├─ apps/
│  ├─ desktop/
│  │  ├─ src-tauri/
│  │  │  ├─ Cargo.toml
│  │  │  ├─ build.rs
│  │  │  ├─ tauri.conf.json
│  │  │  ├─ capabilities/       # least-privilege Tauri permissions
│  │  │  ├─ icons/              # generated ICO/PNG from approved master
│  │  │  └─ src/
│  │  │     ├─ main.rs
│  │  │     ├─ app.rs           # lifecycle, single instance, tray (later)
│  │  │     ├─ commands/        # typed IPC handlers (thin)
│  │  │     ├─ events/          # event/channel emitters
│  │  │     └─ updater.rs
│  │  ├─ ui/
│  │  │  ├─ index.html
│  │  │  ├─ vite.config.ts
│  │  │  ├─ src/
│  │  │  │  ├─ app/             # shell, routing, providers, error boundaries
│  │  │  │  ├─ features/
│  │  │  │  │  ├─ chat/
│  │  │  │  │  ├─ composer/
│  │  │  │  │  ├─ projects/
│  │  │  │  │  ├─ editor/
│  │  │  │  │  ├─ terminal/
│  │  │  │  │  ├─ preview/
│  │  │  │  │  ├─ task/
│  │  │  │  │  ├─ models/
│  │  │  │  │  ├─ permissions/
│  │  │  │  │  ├─ settings/
│  │  │  │  │  └─ onboarding/
│  │  │  │  ├─ components/      # design-system primitives
│  │  │  │  ├─ styles/          # tokens, glass, themes
│  │  │  │  ├─ lib/             # ipc client, formatters, guards
│  │  │  │  ├─ hooks/
│  │  │  │  └─ stores/
│  │  │  ├─ tests/              # component + visual tests
│  │  │  └─ public/
│  │  └─ README.md
│  └─ cli/                      # optional headless agent (post-1.0)
│
├─ crates/
│  ├─ nexora-core/              # domain types, errors, config, IDs
│  ├─ nexora-protocol/          # IPC DTOs, schemas, TS export
│  ├─ nexora-store/             # SQLite, migrations, blobs, checkpoints
│  ├─ nexora-inference/         # runtime packs, model registry, supervisor, client
│  ├─ nexora-context/           # repo scan, ignore, tree-sitter, search, index
│  ├─ nexora-tools/             # fs, patch, process, search, git-read, preview, browser
│  ├─ nexora-policy/            # permissions, path guard, command policy, risk
│  ├─ nexora-security/          # secret scan, signature verify, Windows containment
│  ├─ nexora-agent/             # state machine, planner, budgets, verification, report
│  └─ nexora-diagnostics/       # logging, redaction, crash capture, bundle export
│
├─ packages/
│  ├─ design-tokens/            # token source of truth (JSON/CSS)
│  ├─ shared-types/             # generated TS from nexora-protocol (not hand-edited)
│  └─ config/                   # eslint, prettier, tsconfig presets
│
├─ resources/
│  ├─ icons/                    # master SVG, ICO, PNG, installer graphics
│  ├─ templates/                # bundled project scaffolds
│  ├─ model-manifests/          # signed catalogue + schemas
│  ├─ runtime-manifests/        # llama.cpp build pins and hashes
│  └─ licenses/                 # third-party licence texts
│
├─ tests/
│  ├─ fixtures/                 # sample projects, broken repos, adversarial repos
│  ├─ e2e/                      # Tauri/WebDriver + Playwright
│  ├─ agent-evals/              # tasks, checkers, expected outcomes
│  ├─ security/                 # path/command/patch fuzz corpora, injection suites
│  ├─ performance/              # startup, UI, inference benchmarks
│  └─ installers/               # clean-VM scripts and checklists
│
├─ docs/
│  ├─ phase-1/                  # this planning set
│  ├─ adr/                      # numbered decision records
│  ├─ architecture/             # diagrams, IPC, storage, runtime
│  ├─ user/                     # install, models, permissions, troubleshooting
│  ├─ security/                 # threat model, disclosure, privacy
│  └─ contributing/             # setup, style, testing, release
│
├─ scripts/                     # dev/build/release helpers (no secrets)
├─ .editorconfig
├─ .gitattributes
├─ .gitignore
├─ Cargo.toml                   # workspace
├─ Cargo.lock                   # committed
├─ package.json                 # workspace scripts
├─ pnpm-workspace.yaml
├─ pnpm-lock.yaml               # committed
├─ rust-toolchain.toml
├─ LICENSE                      # Decision pending (Q-04); proprietary default
├─ README.md
├─ SECURITY.md
├─ CONTRIBUTING.md
├─ CHANGELOG.md
└─ THIRD_PARTY_NOTICES.md
```

---

## 3. Directory responsibilities

| Path | Owns | Must not contain |
|------|------|------------------|
| `apps/desktop/src-tauri` | Window lifecycle, IPC registration, updater wiring, capabilities | Business logic, policy decisions, direct process spawning |
| `apps/desktop/ui` | Rendering, interaction, accessibility, design system | Privilege logic, filesystem access, shell execution |
| `nexora-core` | Shared domain types, errors, config | I/O, process, UI |
| `nexora-protocol` | Command/event schemas, versioning, TS generation | Runtime state |
| `nexora-store` | DB, migrations, repositories, blobs, checkpoints | Policy decisions |
| `nexora-inference` | Runtime packs, model catalogue/download, supervisor, streaming client | Tool execution |
| `nexora-context` | Repo scan, ignore rules, symbol map, retrieval | Writes, model output interpretation |
| `nexora-tools` | Filesystem, patch, process, search, preview, browser, git-read | Permission prompts (UI), model calls |
| `nexora-policy` | Permissions, path guard, command classifier, risk, redaction rules | UI, model calls |
| `nexora-security` | Secret scanning, signature verification, Windows containment | Product policy decisions |
| `nexora-agent` | State machine, planning, budgets, verification, reporting | Raw OS access |
| `nexora-diagnostics` | Logging, crash capture, bundle export | Network upload |
| `resources/templates` | Bundled scaffolds | Secrets, remote trackers |
| `tests/fixtures` | Reproducible test projects | Real user data |

---

## 4. Dependency rules

```
core ← protocol ← desktop
core ← store ← inference, context, tools
core ← policy ← tools, agent
core ← security ← inference, tools
core ← diagnostics ← desktop
tools, policy, context, store, inference ← agent ← desktop
ui → protocol (types only)
```

Rules enforced by CI:

- No cycles between crates.
- `core` depends on no Nexora crate.
- `policy` and `security` never depend on `agent` or `ui`.
- Only `apps/desktop` registers Tauri commands.
- Only `nexora-tools` may spawn processes; only `nexora-policy` may authorize paths.
- UI packages may not import Rust internals except generated protocol types.
- Shared types are generated, never hand-edited; a CI check fails on drift.

---

## 5. Engineering conventions

### Rust

- Edition 2024, stable toolchain pinned, `rustfmt` default, `clippy -D warnings`.
- Errors: `thiserror` for typed domain errors; `anyhow` only at application boundaries; no `unwrap`/`expect` in production paths (tests excepted).
- Async: Tokio; no blocking calls in async contexts; CPU work on `spawn_blocking`.
- Serialization: `serde`; schemas via `schemars`; IDs as ULID strings.
- Time: UTC, `time` or `chrono` consistently (choose one at Phase 2).
- Logging: `tracing` with spans; no `println!` in production.
- Unsafe: deny by default; each block documented and reviewed.
- Tests: unit in-module, integration in `tests/`, fixtures in `tests/fixtures`.

### TypeScript / React

- TypeScript strict; no `any` without a written justification.
- Function components and hooks; no class components.
- State: Zustand stores per feature; no global mutable singletons.
- Styling: design tokens + Tailwind + component CSS; no inline hex values.
- Accessibility: semantic HTML first; Radix primitives for complex widgets.
- Testing: Vitest + Testing Library; Playwright for E2E and visuals.
- Imports: feature-boundary rules enforced by ESLint.

### Data and files

- SQL migrations numbered (`0001_init.sql`), forward-only, reviewed.
- Model/runtime manifests versioned; schema changes require a migration note.
- Templates versioned; breaking changes bump the template version.
- No generated code committed unless it is a reviewed contract (protocol types may be generated at build time instead).

---

## 6. Git workflow

- **Trunk-based:** short-lived branches, squash merge to `main`.
- `main` protected: linear history, required checks, review, no force-push, no direct commits.
- **Conventional Commits** (`feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `chore:`, `security:`).
- Signed commits recommended; release tags signed.
- PR template: summary, risk, tests, screenshots for UI, security impact, rollback.
- CODEOWNERS for `nexora-policy`, `nexora-security`, installer/signing, workflows, and docs/phase-1.
- Dependabot for Cargo/npm/Actions with grouped updates and manual review; no auto-merge for security-sensitive crates.
- No release branch at 1.0; tags drive releases. Hotfixes branch from the tag.

---

## 7. CI workflows

| Workflow | Trigger | Jobs |
|----------|---------|------|
| `ci.yml` | PR, push to main | fmt, clippy, test, typecheck, lint, vitest, playwright smoke, unsigned build |
| `security.yml` | PR, schedule | CodeQL, dependency review, gitleaks, licence check |
| `nightly.yml` | Schedule | E2E, eval subset, perf smoke, installer build, accessibility |
| `release.yml` | Tag `v*`, manual approval | build, sign, SBOM, checksums, draft release, publish after approval |

- Actions pinned by commit SHA.
- `GITHUB_TOKEN` least privilege; release jobs use a protected environment with required reviewers.
- No secrets in fork PRs; signing keys only in the release environment/HSM.
- Artifacts retained per policy; release artifacts retained indefinitely.

---

## 8. Documentation and ADRs

```
docs/adr/0001-use-tauri.md
docs/adr/0002-use-llamacpp-sidecar.md
docs/adr/0003-rust-agent-orchestrator.md
docs/adr/0004-sqlite-and-content-addressed-checkpoints.md
docs/adr/0005-sandbox-baseline.md
docs/adr/0006-installer-and-updater.md
...
```

ADR format: context, decision, alternatives, consequences, status, date, owner. A decision that changes architecture requires a new ADR, not an edit to history.

`docs/user/` covers installation, first model, permissions, offline use, troubleshooting, data locations, and uninstall. `docs/security/` covers the threat model, disclosure policy, and privacy statement.

---

## 9. Secrets and ignore rules

`.gitignore` must exclude:

```
/target
/node_modules
/dist
/out
*.log
.env
.env.*
!.env.example
**/src-tauri/target
**/.nexora/
models/
runtime/
artifacts/
*.gguf
*.safetensors
*.msi
*.nsis
```

- No API keys, tokens, certificates, or signing keys in the repository.
- `.env.example` contains names and placeholders only.
- gitleaks runs pre-commit (optional) and in CI; a finding blocks merge.
- GitHub environment secrets are scoped to release; no secrets in workflow logs.
- Diagnostics and crash dumps are never committed.

---

## 10. Large files and release artifacts

- Models, runtime packs, installers, and build caches are **not** committed.
- Git LFS is avoided for binaries; small design assets (icons, sample images) live in Git if under ~1 MB each.
- Release binaries are attached to GitHub Releases (private repo) or a controlled artifact store.
- Model and runtime downloads are hosted separately with signed manifests; the repository stores only manifests and hashes.
- Test fixtures are small, synthetic, and licence-clean; no real user code.

---

## 11. Licensing

- The product licence is **undecided**. Default assumption: proprietary, all rights reserved, until the owner chooses otherwise (Q-04).
- Third-party licences are tracked in `THIRD_PARTY_NOTICES.md` and `resources/licenses/`.
- Model licences are recorded per manifest.
- Generated project templates may use permissive dependencies; their licences are listed in template READMEs.
- No copyleft dependency may be introduced into distributed application code without legal review.

---

## 12. Future growth

- `apps/cli`: a headless agent for CI and power users, reusing `nexora-agent` and tools.
- `crates/nexora-providers`: optional external inference providers.
- `packages/plugin-sdk`: only after a security model for third-party extensions exists (post-1.0).
- Splitting the repo is not planned; if it becomes necessary, the crate boundaries make extraction straightforward.

---

## 13. Acceptance criteria

- [ ] Layout and crate boundaries approved.
- [ ] Dependency rules agreed and enforceable in CI.
- [ ] Git workflow and protection rules approved.
- [ ] Secret and large-file policies approved.
- [ ] Licence decision recorded before repository creation.
- [ ] No repository is created or published until the owner authorizes Phase 2.
- [ ] The first commit, when authorized, contains only code/docs approved for Phase 2, with no secrets and no binary artifacts.

---

## 14. Sources

- Cargo workspaces — https://doc.rust-lang.org/cargo/reference/workspaces.html
- pnpm workspaces — https://pnpm.io/workspaces
- Conventional Commits — https://www.conventionalcommits.org/
- GitHub Actions security hardening — https://docs.github.com/actions/security-guides/security-hardening-for-github-actions
- GitHub protected branches — https://docs.github.com/repositories/configuring-branches-and-merges-in-your-repository/managing-protected-branches
- ADR guidance — https://adr.github.io/
- Tauri capabilities — https://v2.tauri.app/security/capabilities/

Access date: 2026-10-17.

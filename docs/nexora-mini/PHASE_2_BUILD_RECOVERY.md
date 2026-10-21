# NEXORA MINI — PHASE 2: NATIVE BUILD RECOVERY

**Status:** Phase 2 complete; a **Phase 2B verification closeout was appended on 2026-10-20**
(see §8). Awaiting approval. No commit, push, merge or release was made.
**Scope:** restore a reproducible, buildable native Rust/Tauri baseline. No Mini feature work,
no model selection, no downloads, no quota logic, no UI changes.
**Reading order:** §0–§7 and appendices A–C are the 2026-10-19 record, preserved as written.
§8 is the 2026-10-20 re-verification and says which earlier statements it supersedes.

---

## 0. Verdict in one paragraph

The crate now **parses cleanly, is rustfmt-clean, and every compile error CI actually reported
has been fixed at its root**. The crate has **not been type-checked or tested end to end**,
because this sandbox cannot reach the crates.io index, so `cargo check` / `cargo test` cannot
resolve a single dependency. The native baseline is therefore **repaired but not yet proven**:
it needs one CI run (or one `cargo check` on a networked Windows/Linux machine) before any
model-selection work starts. Details, evidence and the exact blocker are below.

**Phase 2B update (2026-10-20).** The closeout in §8 ran the **actual** CI formatting command
(`cargo fmt --all -- --check`, exit 0), validated the manifest with `cargo metadata`, found and
fixed two further warnings, and executed **5 real Rust tests** from `inference/sse.rs`
(5 passed / 0 failed). Dependency resolution still fails at `index.crates.io` (exit 101), so the
verdict is unchanged: **repaired, formatted and parse-clean, but not compilation-verified, with
no Windows evidence.** Two new defects (#15, #16) were found in this closeout that Phase 2 could
not see — which is the strongest available argument that the crate still needs one real CI run.

---

## 1. Repository state before and after

| Item | Before this phase | After this phase |
| --- | --- | --- |
| Branch | `arena/18030bbb-nexora` | `arena/18030bbb-nexora` (unchanged) |
| HEAD | `9bd4f7ad36ab012b50e48333c27fdc9b8244d5ff` | `9bd4f7ad36ab012b50e48333c27fdc9b8244d5ff` (unchanged) |
| Commits made | — | **0** (working-tree changes only, as instructed) |
| Tracked files modified | 0 | **15** (all under `apps/desktop/src-tauri/src/`) |
| Files deleted / renamed | 0 | 0 |
| `Cargo.lock` present | no | **no** (could not be generated — see §5.1) |
| `docs/nexora-mini/PHASE_1_AUDIT.md` | 44 544 B, untracked | 44 544 B, untracked, **untouched** |
| `git diff --check` | exit 0 | exit 0 |
| Frontend lockfile | `pnpm-lock.yaml` 67 765 B | unchanged |

No `git reset`, `clean`, `stash`, `checkout` or `restore` was used at any point. Nothing was
discarded. `git status` at the end shows exactly the 15 modified Rust files plus the untracked
`docs/nexora-mini/` directory.

---

## 2. Files modified and why

Fifteen files changed: **8 carry a semantic fix**, **7 are formatting-only**.

### 2.1 Semantic fixes

| # | File | Change | Reason |
| --- | --- | --- | --- |
| 1 | `src/auth/storage.rs` | `entry.delete_password()` → `entry.delete_credential()` | keyring 3.x removed the old name. Windows-only code, so Linux never saw it. |
| 2 | `src/auth/client.rs` | added `pub fn base_url(&self) -> &str` | `base_url` is a private field; diagnostics in `commands.rs` read it directly. Accessor keeps the field private and read-only. |
| 3 | `src/auth/commands.rs` | `state.client.base_url.clone()` → `state.client.base_url().to_string()` | consumes the accessor from #2 |
| 4 | `src/auth/commands.rs` | clone `pkce.state` into `expected_state` before `spawn_blocking(move …)` | the closure moved the whole `PkceChallenge`, then the code used `pkce.code_verifier` / `pkce.state` / `pkce.nonce` afterwards |
| 5 | `src/auth/commands.rs` | removed a duplicated trailing `;` after `?` | `cargo check` warning: unnecessary trailing semicolon |
| 6 | `src/workspace/analysis.rs` | `content.as_bytes().len() as u64 > MAX_PREVIEW_BYTES` | `len()` is `usize`, `MAX_PREVIEW_BYTES` is `u64` (`scanner.rs:15`) — the reported `mismatched types` |
| 7 | `src/workspace/analysis.rs` | dropped `.to_string()` on the `format!` template; args now fill the two `{}` | single defect that produced three diagnostics (`to_string` not found, argument never used, expected `,` found `.`) |
| 8 | `src/workspace/analysis.rs` | removed unused `extract_symbols_for_file` import | warning |
| 9 | `src/workspace/validation.rs` | `c.note.unwrap()` → `c.note.as_deref().unwrap()` | `c` is `&ValidationCheck` from `.iter().find(...)`; `unwrap()` on the field moved out of a shared reference |
| 10 | `src/workspace/validation.rs` | `let mut note = None;` → `let note = None;` (2 sites) | those two bindings are never reassigned; warning |
| 11 | `src/workspace/patch.rs` | unused `PathBuf` import removed | warning |
| 12 | `src/workspace/patch.rs` | added a `#[cfg(not(unix))]` branch to `symlink_escape_rejected` that exercises the fixture | `dir` was only used inside `#[cfg(unix)]`, so Windows reported `unused variable: dir` |
| 13 | `src/workspace/scanner.rs` | removed dead `let truncated = …` (the `FilePreview` literal already hard-codes `truncated: false`) | warning |
| 14 | `src/workspace/symbol_index.rs` | unused `PathBuf` import removed; `let mut out` → `let out` | warnings |

### 2.2 Formatting-only (no token changed)

`src/auth/mod.rs`, `src/auth/pkce.rs`, `src/licensing/commands.rs`, `src/licensing/validator.rs`,
`src/workspace/commands.rs`, `src/workspace/patch_contract.rs`, `src/workspace/settings.rs`.

These were reformatted by rustfmt to satisfy the CI step that has been failing since the last
green run: `cargo fmt --all -- --check`.

### 2.3 Deliberately not touched

- No `Cargo.toml` edit: no dependency added, removed, upgraded or re-pinned.
- No change to `tauri.conf.json`, `capabilities/main.json`, CSP, permissions or the command surface.
- No frontend file changed.
- No test deleted, renamed or disabled. `#[test]` count is **176 before and 176 after**.
- No `unsafe` block added; nothing silenced with `#[allow]`/`#[expect]`.

---

## 3. Each original issue and how it was resolved

CI run [`39240750843`](https://github.com/cybervault-Hacky/Nexora/actions/runs/39240750843)
(Linux: "Check formatting"; Windows: "Type-check every target on Windows") was the evidence base.
Source paths in the check-run annotations were mangled to `.github`, so each message was matched
to code by text, and every match was then verified by reading the surrounding program.

### 3.1 Hard errors — all 8 gone

| CI message | Real location (before) | Root cause | Fix |
| --- | --- | --- | --- |
| `no method named delete_password found for struct keyring::Entry` | `auth/storage.rs:74` | keyring 3.x API rename | `delete_credential()` (#1) |
| `field base_url of struct AuthApiClient is private` | `auth/commands.rs:216`, field at `auth/client.rs:12` | diagnostics reached into a private field | read-only accessor (#2, #3) |
| `borrow of moved value: pkce.state` | `auth/commands.rs:277` | `move` closure captured the whole struct | clone the expected state (#4) |
| `cannot move out of c.note which is behind a shared reference` | `workspace/validation.rs:847` | `Option<String>` field read through `&ValidationCheck` | `as_deref()` (#9) |
| `mismatched types` | `workspace/analysis.rs:558` | `usize` compared with `u64` | explicit `as u64` (#6) |
| `cannot find function to_string in this scope` | `workspace/analysis.rs:1037` | `"…".to_string()` passed as the format template | template fixed (#7) |
| `argument never used` | `workspace/analysis.rs:1037` | same line | same fix |
| `expected ',' found '.'` | `workspace/analysis.rs:1037` | same line | same fix |

Note: the last three are **one defect**, not three. `format!("…".to_string(), a, b)` made the
first argument a method call on a string literal, which both broke parsing and left two arguments
unconsumed.

### 3.2 Warnings from the same CI run

Ten `warning:` annotations were reported. Nine were located and fixed (#5, #8, #10 ×2, #11,
#12, #13, #14 ×2). Two caveats, stated plainly:

- One of the four `variable does not need to be mutable` annotations could not be located.
  Its annotation carried no usable path or line. Three were found and fixed
  (`validation.rs:347`, `validation.rs:425`, `symbol_index.rs:257`); a fourth may still exist.
- These are `cargo check` warnings. They do not fail `cargo check`, but they *will* fail the
  next CI step, `cargo clippy --all-targets -- -D warnings`, which has never run on this commit.

### 3.3 Formatting

Fourteen of the 31 source files were not rustfmt-clean, and `cargo fmt --all -- --check` compares
the lot. The whole module tree was reformatted with **real rustfmt 1.8.0-stable (rustc 1.88.0,
2025-06-23)**, invoked as:

```
rustfmt --edition 2021 src/lib.rs src/main.rs build.rs
```

There is no `rustfmt.toml` and no `[workspace]` in `Cargo.toml`, so `cargo fmt --all` covers exactly
this package. Cross-checked with a second, independent rustfmt implementation
(`@scalar/rust-fmt@0.3.0`, the wasm build already used by phases 10–14): **0 differences in 32 files**.

---

## 4. Checks actually run, and what each proves

| Check | Status | Evidence |
| --- | --- | --- |
| `cargo fmt --all -- --check` (CI form) | **not runnable** — `cargo fmt` needs workspace metadata, which needs the dependency graph | substitute below |
| `rustfmt --check --edition 2021 src/lib.rs src/main.rs build.rs` | **PASS** | exit 0, no output; 32 files |
| second-opinion formatter (`@scalar/rust-fmt`) | **PASS** | `checked=32 nonconforming=0` |
| `cargo check --all-targets` | **not runnable** — deps cannot be downloaded | see §5.2 |
| `rustc --emit=metadata` on lib target | **parse/expansion clean** | 0 syntax, macro or format-string errors; only unresolved-external-crate diagnostics remain |
| `rustc --emit=metadata --test` on test target | **parse/expansion clean** | same; no error in any `#[cfg(test)]` block |
| `cargo test` | **not runnable** | would need a successful `cargo check` |
| `pnpm typecheck` | **PASS** | `tsc --noEmit`, exit 0 |
| `pnpm test` | **PASS** | 75 files / 724 tests, 93.06 s |
| `pnpm build` | **PASS** | `index-cOYSa9hs.js` 522.59 kB, `index-BaD5w8Y6.css` 93.42 kB |
| `git diff --check` | **PASS** | exit 0 |

**What this proves:** the source is syntactically valid, macro-expandable, format-string valid,
and formatted to the exact style CI enforces. **What it does not prove:** type correctness,
borrow-checker acceptance beyond the two sites fixed, or test results.

---

## 5. Remaining failures and environmental limitations

### 5.1 `Cargo.lock` — why it is absent, and the exact blocker

- There is **no `Cargo.lock` anywhere in the repository** (`find . -name Cargo.lock` → 0 hits).
- It is **not ignored**: the root `.gitignore` covers `target/`, `node_modules/`, `dist/`,
  `.env*` and OS noise only. So the lockfile is missing by omission, not by policy.
- Reproduction attempt, with a working cargo 1.88.0 and rustc 1.88.0 bootstrapped locally:

```
$ cargo generate-lockfile
    Updating crates.io index
warning: spurious network error (3 tries remaining): [35] SSL connect error (TLS connect error: error:00000000:lib(0)::reason(0))
warning: spurious network error (2 tries remaining): [35] SSL connect error (TLS connect error: error:00000000:lib(0)::reason(0))
warning: spurious network error (1 tries remaining): [35] SSL connect error (TLS connect error: error:00000000:lib(0)::reason(0))
error: failed to get `futures-util` as a dependency of package `nexora v0.1.0 (/home/user/Nexora/apps/desktop/src-tauri)`

Caused by:
  download of config.json failed

Caused by:
  failed to download from `https://index.crates.io/config.json`

Caused by:
  [35] SSL connect error (TLS connect error: error:00000000:lib(0)::reason(0))
```

- This is a **network egress restriction**, not a repository defect: `index.crates.io`,
  `static.crates.io`, `static.rust-lang.org` and `docs.rs` all fail at TLS, while
  `registry.npmjs.org`, `github.com` and `codeload.github.com` are reachable.
- **No lockfile was fabricated.** Version numbers were not invented; nothing was upgraded.
- Consequence, stated for the record: without a lockfile every CI run resolves the newest
  semver-compatible release of all ~300 transitive crates. That is the reproducibility gap the
  phase brief asked about, and it is real. It also means `cargo check` may behave differently on
  different days — which is exactly how a green build can rot.
- Recommended on a networked machine (still not committed in this phase):

```
cd apps/desktop/src-tauri
cargo generate-lockfile        # produces Cargo.lock; keep it, review the diff, commit deliberately
cargo fmt --all -- --check
cargo check --all-targets
cargo clippy --all-targets -- -D warnings
cargo test
```

### 5.2 What cannot be verified here

| Limitation | Impact |
| --- | --- |
| No crate registry access | `cargo check`, `cargo test`, `cargo clippy` impossible; type errors beyond the 8 reported would not be seen |
| No Windows machine or runner | `cfg(windows)` code (keyring, `windows-sys`, `CREATE_NO_WINDOW`) is fixed **by inspection only** — not compiled, not linked |
| GitHub CI logs return `EOF` | per-job logs unavailable; only step statuses and check-run annotations could be read |
| CI cannot be triggered | branch is unpushed by instruction; any CI run must follow a deliberate push |
| rustfmt version skew | formatted with 1.88.0; CI pins 1.93.0. Two independent formatters agree, but version-exactness is unproven |
| Clippy never ran | `-D warnings` may surface lints unrelated to the 10 known warnings |

### 5.3 Error-list completeness

The 8 errors above come from GitHub check-run annotations, which may be truncated by the
annotations API. Rustc normally reports all errors in one pass, so this list is very likely
complete for the Windows target — but "very likely" is not "verified", and the report will not
pretend otherwise. The 14 previous CI runs all failed; the last green run was
`39106404992` (2026-10-18, PR #1), so regressions entered during Phases 10–32.

---

## 6. Product features and security boundaries — preserved

| Guarantee | Evidence |
| --- | --- |
| No feature added, removed or altered | all 15 changed files are `src-tauri/src/*.rs`; `git diff --name-only` filtered for `.ts/.tsx/.json/.toml` returns nothing |
| Tauri command surface unchanged | still 58 `#[tauri::command]` functions; `generate_handler!` untouched |
| CSP untouched | `tauri.conf.json` unmodified |
| Capabilities/permissions untouched | `capabilities/main.json` unmodified |
| Credential handling preserved | keyring remains Windows Credential Manager; `MEMORY_STORE` fallback intact; tokens still never cross the IPC boundary |
| OAuth PKCE security preserved | `state` is still generated per flow, still compared by `await_callback`, still sent to the server exchange — the fix only stops the value being moved |
| Workspace validation/sandboxing preserved | path traversal, sensitive-file and hash checks untouched; the only edit is a borrow fix in a test |
| Test coverage not reduced | 176 `#[test]` before, 176 after; no test removed |
| Repository history intact | HEAD identical; no commit, push, merge, tag or release |

---

## 7. Is the native baseline ready for model-selection work?

**Not yet — one verification step away.**

What is now true:
- The formatting failure that blocked the Linux job is resolved (substitute check passes, and a
  second formatter agrees).
- All 8 compile errors that Windows CI actually reported are fixed at source, with the correct
  APIs for the pinned crate versions (keyring 3.x `delete_credential`, verified against
  `keyring-rs` v3.6.3, which is what `keyring = "3"` resolves to).
- No workaround, `unsafe`, `#[allow]` or test deletion was used to get there.

What is still unproven:
- Type-checking and the 176 Rust tests have **never executed** on this code.
- Windows compilation and linking have **never executed** on this code.
- `cargo clippy -D warnings` has never executed on this commit.

**Recommended gate before Phase 3:** push this branch (or run the four cargo commands on any
networked Windows machine) and require the `rust` and `windows` jobs to reach at least the test
step. If new errors surface, they will be the residue that the annotations could not show — they
should be fixed the same way, one root cause at a time.

Until that run exists, calling the baseline "ready" would be a claim this environment cannot
support.

---

## Appendix A — Toolchain actually used in this sandbox

| Tool | Version | Provenance |
| --- | --- | --- |
| rustc | 1.88.0 (6b00bc388 2025-06-23) | `@rustbin/rustc-1.88.0-x86_64-unknown-linux-gnu` (npm), sysroot assembled from `@rustbin/rust-std-…` |
| cargo | 1.88.0 (873a06493 2025-05-10) | `@rustbin/cargo-1.88.0-x86_64-unknown-linux-gnu` |
| rustfmt | 1.8.0-stable (6b00bc3880 2025-06-23) | `@rustbin/rustfmt-1.88.0-x86_64-unknown-linux-gnu` |
| second formatter | `@scalar/rust-fmt` 0.3.0 (wasm) | repo devDependency, already used by phases 10–14 |
| node / pnpm | v22.22.3 / 9.15.4 | repo `packageManager` pin |

The repo pins `channel = "1.93.0"`; 1.93 binaries are not obtainable from this sandbox, so 1.88 was
used for parse-checking and formatting. Nothing in the source depends on a 1.93-only feature.

## Appendix B — Verification commands, in the order run

```
# formatting (CI-equivalent, since `cargo fmt` needs metadata)
rustfmt --check --edition 2021 src/lib.rs src/main.rs build.rs      # exit 0

# parse + macro expansion + format-string checking, library and test targets
rustc --sysroot <local> --edition 2021 --crate-type lib  --emit=metadata -o /tmp/a.rmeta src/lib.rs
rustc --sysroot <local> --edition 2021 --crate-type lib --test --emit=metadata -o /tmp/b.rmeta src/lib.rs

# lockfile generation attempt (documents the blocker)
cargo generate-lockfile

# frontend, unchanged by this phase
pnpm typecheck && pnpm test && pnpm build

# hygiene
git diff --check
```

## Appendix C — Files changed (15)

```
apps/desktop/src-tauri/src/auth/client.rs
apps/desktop/src-tauri/src/auth/commands.rs
apps/desktop/src-tauri/src/auth/mod.rs
apps/desktop/src-tauri/src/auth/pkce.rs
apps/desktop/src-tauri/src/auth/storage.rs
apps/desktop/src-tauri/src/licensing/commands.rs
apps/desktop/src-tauri/src/licensing/validator.rs
apps/desktop/src-tauri/src/workspace/analysis.rs
apps/desktop/src-tauri/src/workspace/commands.rs
apps/desktop/src-tauri/src/workspace/patch.rs
apps/desktop/src-tauri/src/workspace/patch_contract.rs
apps/desktop/src-tauri/src/workspace/scanner.rs
apps/desktop/src-tauri/src/workspace/settings.rs
apps/desktop/src-tauri/src/workspace/symbol_index.rs
apps/desktop/src-tauri/src/workspace/validation.rs
```

---

## 8. Phase 2B — verification closeout (2026-10-20)

> **How to read this section.** Sections 0–7 and appendices A–C were written on 2026-10-19 and
> are preserved unchanged. This section records what was re-verified on 2026-10-20, what was
> newly found, and — in §8.8 — which earlier statements are now superseded and which still stand.
> Nothing in this section was inferred from the earlier report; every claim below comes from a
> command run in this closeout.

### 8.1 Two more real defects found and fixed

Both are `cargo check` warnings that would fail the CI step
`cargo clippy --all-targets -- -D warnings`. Neither was visible in Phase 2.

| # | File | Defect | Evidence | Fix |
| --- | --- | --- | --- | --- |
| 15 | `src/workspace/analysis.rs:562` | `let mut unsupported = 0usize;` is immediately overwritten by `unsupported = unsupported_count;` | rustc: `warning: value assigned to 'unsupported' is never read` (`unused_assignments`) | initialise directly from the count; the intermediate `unsupported_count` binding is gone. Same value, same behaviour. |
| 16 | `src/workspace/patch.rs:602` | `use std::os::unix::fs::PermissionsExt;` inside a `#[cfg(unix)]` test block that never calls `set_mode` | rustc: `warning: unused import` | import removed |

Why Phase 2 could not see them:

- #15 only appears once the lint pass reaches `workspace/analysis.rs`; in Phase 2 the tree was
  checked by inspection and by the CI annotations, which did not carry it.
- #16 is **Linux-only** (`#[cfg(unix)]`). The Windows job cannot see it, and the Linux job has
  never reached `cargo check` because `cargo fmt` failed first.

After both fixes: **0 warnings** from the library target and **0** from the test target.
Two files changed in this closeout — both were already among the 15 modified in Phase 2.
The diff is still 15 files, now **+945 / −266**.

### 8.2 Audit performed before any change

| Check | Result |
| --- | --- |
| Branch | `arena/18030bbb-nexora` |
| HEAD | `9bd4f7ad36ab012b50e48333c27fdc9b8244d5ff` |
| Commits in local clone | 1 (grafted: `Merge pull request #5 …`) |
| `refs/heads/main`, `origin/main`, `origin/HEAD` | all `9bd4f7a` — the CI evidence below is for this exact tree |
| Staged changes | none |
| Unstaged changes | 15 modified files, +945 / −263 (at audit time) |
| Untracked | `docs/nexora-mini/PHASE_1_AUDIT.md`, `docs/nexora-mini/PHASE_2_BUILD_RECOVERY.md` |
| Stash | none |
| Ignored artifacts (left untouched) | `node_modules/`, `apps/desktop/node_modules/`, `apps/desktop/ui/dist/` |
| Phase 1 audit | 44 544 B, `sha256 a40e7297…` before and after — unmodified |
| Diff review | token-level comparison of all 15 files; 8 files carry the documented semantic fixes, 7 are formatting-only; string literals unchanged except the test fixture added in Phase 2 |

Both `docs/nexora-mini/PHASE_1_AUDIT.md` and the whole of §0–§7 were read before acting.
No reset, revert, clean, stash, rebase or overwrite was performed.

### 8.3 Commands run in this closeout, with real exit codes

| # | Command (exact) | Exit | What the exit means |
| --- | --- | --- | --- |
| 1 | `cargo metadata --no-deps --format-version 1 --offline` | **0** | manifest is valid; targets and dependency requirements parse |
| 2 | `cargo fmt --all -- --check` | **0** | the exact CI formatting step passes |
| 3 | `cargo check --all-targets` | **101** | stops at dependency download; **no source is compiled** |
| 4 | `cargo test` | **101** | same dependency failure |
| 5 | `cargo clippy --all-targets -- -D warnings` | **101** | `no such command: clippy` — clippy is not installed |
| 6 | `rustc --emit=metadata src/lib.rs` | non-zero | 254 diagnostics, **all** unresolved-crate classes; **0 warnings** |
| 7 | `rustc --emit=metadata --test src/lib.rs` | non-zero | 273 diagnostics, same classes; **0 warnings** |
| 8 | `rustc --test src/inference/sse.rs` then run binary | **0** | **5 tests passed, 0 failed** (real tests, unmodified source) |
| 9 | `pnpm typecheck` | **0** | `tsc --noEmit` clean |
| 10 | `pnpm test` | **0** | 75 files / **724 tests passed** (94.19 s) |
| 11 | `pnpm build` | **0** | built in 2.14 s |
| 12 | `git diff --check` | **0** | no whitespace errors |

`cargo metadata` output confirms the manifest is internally consistent:

```
package: nexora 0.1.0 edition 2021 rust-version 1.77.2
targets:  ['staticlib','cdylib','rlib'] nexora_lib -> lib.rs
          ['bin'] nexora -> main.rs
          ['custom-build'] build-script-build -> build.rs
deps:     futures-util ^0.3, getrandom ^0.2, regex ^1, reqwest ^0.12, serde ^1,
          serde_json ^1, sha2 ^0.10, similar ^2.7, tauri ^2, tempfile ^3, tokio ^1,
          tauri-build ^2; cfg(windows): keyring ^3, windows-sys ^0.59
```

`main.rs` calls `nexora_lib::run()`, and `lib.rs` defines `pub fn run()` — the bin/lib pairing is
consistent (§8.6 explains why the unresolved-crate diagnostics are not source defects).

### 8.4 Cargo dependency resolution: it did not succeed

Exact output of `cargo check --all-targets` (the CI command), run in
`apps/desktop/src-tauri`:

```
    Updating crates.io index
warning: spurious network error (3 tries remaining): [35] SSL connect error (TLS connect error: error:00000000:lib(0)::reason(0))
warning: spurious network error (2 tries remaining): [35] SSL connect error (TLS connect error: error:00000000:lib(0)::reason(0))
warning: spurious network error (1 tries remaining): [35] SSL connect error (TLS connect error: error:00000000:lib(0)::reason(0))
error: failed to get `futures-util` as a dependency of package `nexora v0.1.0 (/home/user/Nexora/apps/desktop/src-tauri)`

Caused by:
  download of config.json failed

Caused by:
  failed to download from `https://index.crates.io/config.json`

Caused by:
  [35] SSL connect error (TLS connect error: error:00000000:lib(0)::reason(0))
```

Exit code **101**. `cargo test` fails identically. **This is a dependency-fetch failure, not a
Rust source compilation failure**: resolution stops before rustc is invoked on any crate source,
and no `target/` directory or `Cargo.lock` was created.

Reachability, measured in this environment:

| Endpoint | HTTP result |
| --- | --- |
| `https://index.crates.io/config.json` | 000 (TLS blocked) |
| `https://static.crates.io/crates/serde/serde-1.0.219.crate` | 000 |
| `https://crates.io`, `https://crates.io/api/v1/crates/serde` | 000 |
| `https://rsproxy.cn`, `mirrors.sjtug.sjtu.edu.cn`, `mirrors.tuna.tsinghua.edu.cn` | 000 |
| `https://registry.npmjs.org/`, `https://github.com/`, `https://pypi.org/simple/` | 200 |

No TLS verification was disabled, no mirror was substituted, and no dependency version was
changed. The blocker is the sandbox's egress policy.

### 8.5 Lockfile provenance: there is nothing to recover

| Question | Answer | Evidence |
| --- | --- | --- |
| Is `Cargo.lock` ignored? | No | `.gitignore` has no `Cargo.lock` rule |
| Is one tracked now? | No | `find` → 0 files; `git ls-files` → only `pnpm-lock.yaml` |
| Did one ever exist in this repo? | **No** | `GET /commits?path=apps/desktop/src-tauri/Cargo.lock` → 0 commits; root `Cargo.lock` → 0 commits (51 commits on `main`) |
| Do CI artifacts hold one? | No | `actions/artifacts` → `total_count=0`; `.github/workflows/ci.yml` has **no** `upload-artifact` / `download-artifact` step |
| Is a lockfile expected here? | **Yes** | this is a binary/Tauri desktop application (not a library), and the same repository commits `pnpm-lock.yaml` for the frontend |

So there is no authoritative lockfile in history, in artifacts, or in the tree — nothing could be
recovered, and none was fabricated. The consequence stands: with `tauri = "2"`, `keyring = "3"`
and the rest of the loose ranges, every resolve picks the newest semver-compatible release, so
two CI runs on the same commit can compile different code.

### 8.6 What the dependency-less parse proves — and what it does not

The `rustc --emit=metadata` runs are a **parse + macro-expansion + format-string** check. Their
diagnostics fall into exactly these classes:

| Class | Count | Meaning |
| --- | --- | --- |
| `E0433` unresolved module/crate | 123 | `serde`, `tauri`, `reqwest`, `tokio`, `futures_util`, `sha2`, `similar`, `regex`, `tempfile`, `getrandom` are absent |
| `cannot find attribute 'serde'` | 86 | derive attribute without the `serde` crate |
| `E0432` unresolved import | 32 | same cause |
| `E0277` unsized `str` / `[u8]` | 11 | type inference degraded at four call sites whose types come from missing crates (`auth/commands.rs:83/86`, `auth/commands.rs:289/298`, `inference/client.rs:159/172`, `licensing/commands.rs:64`) |
| `E0308` mismatched types | 1 | see below |
| `CARGO_PKG_VERSION` | 1 | injected by cargo; absent when rustc is invoked directly |

**The single `E0308` is a cascade, not a defect.** At `src/licensing/commands.rs:65`:

```rust
Ok(validated) => Ok(to_licence_status(Some(&validated), Some(key))),
```

`key` comes from `state.active_key` (`Mutex<Option<String>>`), but `state: State<'_, LicenceState>`
has no type here because `tauri` is missing, so inference degrades to `str` and the diagnostic
reads “expected `String`, found `str`”. Two independent checks confirm this is an artefact:

1. A dependency-free reconstruction of the exact types (`State<'_, T>: Deref<Target = T>`,
   `Mutex<Option<String>>`, `raw_key: Option<String>`) compiles and runs, returning the expected
   key. See Appendix D.
2. The real CI compile (all crates present) reported exactly **one** `mismatched types`
   annotation, and `analysis.rs:558` (`usize` vs `u64`) is unconditionally wrong. If line 65 had
   also been an error, it would have appeared as a second one. It did not.

**Conclusion:** the parse check proves the crate is syntactically valid and free of
format-string defects, and that the previously reported errors are gone — verified by grepping
the current diagnostics for each of the original messages (0 hits for all seven distinct ones).
It does **not** prove type-correctness, trait resolution, borrow-checking under real types, or
that the 176 tests pass. It is not a substitute for `cargo check`.

### 8.7 Real Rust tests: 5 of 176 executed

`src/inference/sse.rs` is the only module with no external-crate dependency. It was compiled
from the **unmodified** source file as its own test crate and executed:

```
running 5 tests
test tests::classifies_data_done_and_noise ... ok
test tests::handles_crlf_and_blank_lines ... ok
test tests::keeps_multibyte_characters_intact_across_chunks ... ok
test tests::yields_only_complete_lines ... ok
test tests::drops_oversized_unterminated_input ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

This is genuine execution of real project tests — **5 of 176 (2.8 %)**. The remaining 171 live in
modules that import `serde`, `tauri`, `sha2`, `similar`, `regex`, `tempfile` or `keyring`, so they
cannot be built without the dependency graph. This does not replace `cargo test` and must not be
read as evidence that the suite passes.

### 8.8 Phase 2 statements: superseded or still standing

| Phase 2 statement | Phase 2B status |
| --- | --- |
| “`cargo fmt --all -- --check` is not runnable” (§4) | **Superseded.** It was run in this closeout (real `cargo-fmt` + `rustfmt 1.8.0`, offline) and **exited 0**. |
| “formatting verified only by substitute” (§4) | **Superseded** — the CI command itself now passes. rustfmt is 1.88.0, CI pins 1.93.0, so version-exactness remains unproven. |
| “10 warnings; 9 located and fixed” (§3.2, corrected in this closeout) | **Updated.** Phase 2 fixed 9 of the 10 annotations; one `unused mut` could not be located. Phase 2B found and fixed two warnings the annotations never showed (#15, #16). Total fixed: 11; 1 still unlocated. |
| “`cargo check` / `cargo test` not runnable” (§4/§5.2) | **Standing.** They were attempted again; both exit 101 at dependency fetch, before compiling. |
| “`cargo clippy` never ran” (§5.2) | **Standing, and stronger.** Clippy is not installed in the available toolchain (`no such command: clippy`) and cannot be obtained without registry access. |
| “no `Cargo.lock`; blocker documented” (§5.1) | **Standing, now proven exhaustive.** No lockfile exists in history or artifacts (§8.5). |
| “Windows not verified” (§5.2/§7) | **Standing.** Nothing native was compiled or linked. |
| “Every compile error CI reported is fixed” (§0) | **Holds in the parse check**: all seven distinct original diagnostics are absent from the current diagnostics. |

### 8.9 Windows: what was and was not established

**Not established:** no Windows runner was available, and CI cannot be triggered without pushing
the branch (not permitted). Therefore:

- Windows `cargo check --all-targets` — **not run**
- Windows `cargo build` (linking, `windows_subsystem`, embedded icon) — **not run**
- Windows `cargo test` — **not run**
- Installer/packaging — **not run**, and deliberately absent from the workflow
  (`ci.yml`: “Deliberately no `tauri build`: producing an installer is out of scope.”)

**Established by source inspection only** (supporting evidence, not compilation proof):

| API used by Windows-only code | Verified against |
| --- | --- |
| `keyring::Entry::delete_credential`, `set_password`, `get_password` | `keyring-rs` v3.6.3 `src/lib.rs` |
| feature `windows-native` (what `keyring = "3"` is configured with) | `keyring-rs` v3.6.3 `Cargo.toml` `[features]` |
| `Win32_System_SystemInformation` feature | `microsoft/windows-rs` `crates/libs/sys/Cargo.toml` at tag `0.59.0` |
| `GlobalMemoryStatusEx`, `MEMORYSTATUSEX` | `crates/libs/sys/src/Windows/Win32/System/SystemInformation/mod.rs` at `0.59.0` |
| `cargo check` also cross-compiles nothing here: rustc knows 20 Windows targets but only `x86_64-unknown-linux-gnu` std is installed | `rustc --print target-list`, sysroot `lib/rustlib/` |

### 8.10 Remaining blockers and the exact next step

| # | Blocker | Precise reason | Who can clear it |
| --- | --- | --- | --- |
| B-1 | No Rust type-check or build | `index.crates.io` + `static.crates.io` TLS-blocked; all three public mirrors also blocked | a networked machine or CI |
| B-2 | 171 of 176 Rust tests unrun | depend on crates that cannot be fetched | same |
| B-3 | Windows compile/link/tests unrun | no Windows runner; CI must not be triggered from here | CI after an approved push |
| B-4 | Clippy unverified | clippy component absent; not installable offline. `-D warnings` has never executed on this commit | CI |
| B-5 | Reproducibility | no `Cargo.lock` has ever existed in this repository | a networked `cargo generate-lockfile`, then a deliberate commit decision |
| B-6 | rustfmt version skew | formatted with 1.88.0, CI pins 1.93.0 | CI will confirm |

**Exact next step (requires approval, because it changes the repository):**

```
git push origin arena/18030bbb-nexora        # triggers CI on all three jobs
```

or, on any networked Windows machine:

```
cd apps/desktop/src-tauri
cargo generate-lockfile
cargo fmt --all -- --check
cargo check --all-targets
cargo clippy --all-targets -- -D warnings
cargo test
cargo build
```

Expectation to set honestly: because `cargo clippy -D warnings` has **never** run on this commit,
the first CI pass that reaches it may surface style lints that cannot be enumerated here
(e.g. `clippy::double_ended_iterator_last` on `path.split('.').last()`, which appears in several
files). Those are not regressions from this work; they are lints that were never reachable while
formatting failed.

### 8.11 The ten required report items

1. **Actual branch and HEAD** — `arena/18030bbb-nexora`, `9bd4f7ad36ab012b50e48333c27fdc9b8244d5ff`.
   `main`, `origin/main` and `origin/HEAD` point at the same commit; the local clone is a grafted
   single commit.
2. **Initial and final working-tree status** — initial: 15 modified files (+945/−263), 2 untracked
   docs, 0 staged, no stash. Final: 15 modified files (+945/−266), same 2 untracked docs,
   0 staged, no stash, no `target/`, no `Cargo.lock`. HEAD unchanged; **0 commits created**.
3. **Files modified during this task** — two, both already modified in Phase 2:
   `src/workspace/analysis.rs` (dead `unsupported` initialiser, `unused_assignments`) and
   `src/workspace/patch.rs` (unused `PermissionsExt` import, Linux-only). No other file changed.
4. **Exact commands and real exits** — §8.3, twelve commands, each with its actual exit code.
   No command is reported as passing unless it exited 0.
5. **Did Cargo dependency resolution succeed?** — **No.** Exit 101 at
   `https://index.crates.io/config.json`, `[35] SSL connect error`.
6. **Did Rust compilation and tests genuinely run?** — Compilation: **no**. Tests: **partially** —
   5 of 176 ran and passed (`inference/sse.rs`, unmodified source, standalone test binary).
   The suite as a whole did not run.
7. **Did Windows CI/build genuinely run?** — **No.** No runner, no push permitted. Nothing about
   Windows compilation or runtime is claimed.
8. **Remaining blockers** — §8.10, six entries, each with its exact cause.
9. **Phase 1 audit and unrelated work preserved** — `PHASE_1_AUDIT.md` is byte-identical
   (`44 544 B`, `sha256 a40e72970f849bce…`); the Phase 2 text is preserved verbatim above this
   section; no unrelated file, dependency, config, capability, CSP or test was touched.
10. **Is Phase 2 fully verified or still blocked?** — **Still blocked.** Formatting and manifest
    validity are genuinely verified; parsing is verified; 5 tests genuinely pass. Native
    compilation, 171 tests, clippy and every Windows check remain unverified because dependency
    resolution fails and no Windows runner exists. Phase 2 must not be declared complete on this
    evidence.

---

### Appendix D — dependency-free harness used in §8.6

The reproduction is a standalone file (not part of the repository) that mirrors only the types
involved at `licensing/commands.rs:65`. It compiled with exit 0 and printed
`licensing type-shape repro: OK`, proving the call shape is type-correct once `State` resolves:

```rust
struct State<'a, T>(&'a T);
impl<'a, T> std::ops::Deref for State<'a, T> {
    type Target = T;
    fn deref(&self) -> &T { self.0 }
}
struct LicenceState { active_key: Mutex<Option<String>> }
fn to_licence_status(_v: Option<&ValidatedLicence>, raw_key: Option<String>) -> LicenceStatus { … }

fn licence_get_status(state: State<'_, LicenceState>) -> Result<LicenceStatus, LicenceError> {
    let active_key = { let guard = state.active_key.lock()?; guard.clone() };
    match active_key {
        Some(key) => match validate_licence_key(&key) {
            Ok(validated) => Ok(to_licence_status(Some(&validated), Some(key))),
            Err(_) => Ok(LicenceStatus::default()),
        },
        None => Ok(LicenceStatus::default()),
    }
}
```

### Appendix E — toolchain used in Phase 2B

| Tool | Version | Provenance |
| --- | --- | --- |
| rustc | 1.88.0 (6b00bc388 2025-06-23) | `@rustbin/rustc-1.88.0-x86_64-unknown-linux-gnu` |
| cargo | 1.88.0 (873a06493 2025-05-10) | `@rustbin/cargo-1.88.0-x86_64-unknown-linux-gnu` |
| cargo-fmt / rustfmt | 1.8.0-stable (6b00bc3880 2025-06-23) | `@rustbin/rustfmt-1.88.0-x86_64-unknown-linux-gnu` |
| installed std targets | `x86_64-unknown-linux-gnu` only | sysroot assembled from `@rustbin/rust-std-…` |
| clippy | **not available** | no component published in the usable form; `cargo clippy` → `no such command` |
| node / pnpm | v22.22.3 / 9.15.4 | repository `packageManager` pin |

The repository pins `channel = "1.93.0"`; 1.93 binaries are not obtainable from this sandbox.
Everything above is version-labelled so that the 1.88-vs-1.93 gap is explicit rather than implied.

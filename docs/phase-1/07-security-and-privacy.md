# 07 — Security and Privacy Design

**Status:** Proposed for owner approval  
**Related:** [03-application-architecture.md](03-application-architecture.md), [05-coding-agent-workflow.md](05-coding-agent-workflow.md), [09-build-installer-and-release.md](09-build-installer-and-release.md)

---

## 1. Security principles

1. **Local by default.** Inference, indexing, conversations, and project files stay on the device unless the user explicitly starts a network action.
2. **Least privilege.** The renderer, the model, and tool processes receive the minimum authority needed for the current action.
3. **No ambient authority.** The agent cannot read arbitrary paths, run arbitrary commands, or reach the network without a scoped grant.
4. **The model is untrusted.** Its output is data until validated; prompt injection is assumed possible.
5. **Deterministic enforcement.** Security decisions are made by Rust policy code, not by prompt instructions or model self-restraint.
6. **Reversible actions.** Checkpoints and confirmations are security controls, not just UX.
7. **Transparent network.** Every app-initiated destination is documented and visible; offline mode actually disables network features.
8. **Defence in depth.** Sandboxing, policy, validation, confirmations, and auditing overlap; no single control is trusted alone.
9. **Secure supply chain.** Pinned, hashed, signed artifacts; reviewed dependencies; least-privilege CI.
10. **Privacy by design.** No telemetry by default; data is exportable and deletable; logs are redacted.

**Offline inference does not mean the entire product is offline or private.** Model downloads, dependency installation, update checks, and any user-approved external requests use the network. The product must disclose this precisely rather than marketing "fully offline" without qualification.

---

## 2. Assets and data classification

| Class | Examples | Storage | Protection |
|-------|----------|---------|------------|
| **Public** | Installer, docs, marketing site | Release artifacts | Code signing, checksums |
| **Internal** | App settings, hardware report, feature flags | SQLite / local files | User ACL, no telemetry |
| **Confidential** | Source code, prompts, conversations, previews, checkpoints | Project folder, SQLite, blob store | Local-only, ACL, redaction, permission scopes |
| **Restricted** | API keys, tokens, `.env` values, credentials, private keys | Never copied by Nexora; Windows Credential Manager for app secrets | DPAPI, exclusion, redaction, never logged |
| **Derived** | Embeddings, symbol index, summaries | App data | Same as confidential; deletable |

**Data handling rules:**

- Confidential data never leaves the device except through an explicit, logged network action (for example, a package install that reads `package.json`).
- Restricted data is excluded from prompts, indexes, checkpoints, logs, and diagnostics by default.
- The model does not receive secrets unless the user explicitly attaches them after a warning; even then, the app redacts them from logs.
- Derived data inherits the classification of its source.

---

## 3. Trust boundaries and data flow

```
┌─────────────┐   user intent    ┌──────────────┐   validated tool call   ┌─────────────┐
│ UI renderer │─────────────────►│ Policy engine│────────────────────────►│ Tool runtime│
│ (untrusted) │◄──── events ─────│ (trusted)    │◄──── results/evidence ──│ (contained) │
└─────────────┘                  └──────┬───────┘                         └──────┬──────┘
                                        │                                        │
                                 ┌──────▼───────┐                         ┌──────▼──────┐
                                 │ Model output │                         │ OS / files  │
                                 │ (untrusted)  │                         │ child procs │
                                 └──────────────┘                         └─────────────┘
```

### 3.1 Network destinations

| Destination | When | Data sent | User control |
|-------------|------|-----------|--------------|
| Model host (Hugging Face or approved mirror) | User downloads a model | HTTP request for pinned files; no project data | Explicit download; licence acceptance |
| Update endpoint (GitHub Releases or signed manifest host) | Update check, if enabled | App version, OS version, channel | Toggle; disabled in offline mode |
| npm/other package registry | Dependency install requested by agent/user | Package names, versions, lockfile metadata | Per-session permission; logged |
| Crash-report endpoint | Only if user opts in and submits | Sanitized crash metadata; no prompts/files | Default off; preview before send |
| Project runtime | When the generated project itself calls an API | Determined by the user's project | Visible in preview console/network log |
| Local inference | Always local | Prompts and context | Never leaves the device |

No analytics, ad, or tracking endpoint exists at 1.0. There is no silent prompt or file upload.

---

## 4. Threat model

Likelihood/impact: L/M/H. Phase: when the mitigation must exist (A = alpha, B = beta, 1.0, Post).

| ID | Threat | L | I | Mitigation | Phase |
|----|--------|---|---|------------|-------|
| T-01 | Prompt injection in repository files instructs the agent to run destructive or exfiltrating actions | H | H | Instruction hierarchy; content marked untrusted; policy engine independent of model; confirmations; adversarial evals; suspicious-content flag | A/B |
| T-02 | Model proposes a destructive command (`rm -rf`, `format`, registry edit) | M | H | Command allowlist; no shell; argv arrays; deny-list; job containment; confirmation; checkpoint | A |
| T-03 | Path traversal or symlink/junction escape writes outside the project | M | H | Canonicalization; reparse-point checks; root confinement; deny UNC/device paths; tests | A |
| T-04 | Agent reads or writes user documents, SSH keys, browser data | M | H | `outside.read/write` denied by default; path guard; secret exclusions; no broad home access | A |
| T-05 | Secret exfiltration through network-enabled tool or generated code | M | H | Network permission; secret scanning; no secrets in context; egress logging; strict sandbox mode | B |
| T-06 | Secrets leak into logs, diagnostics, checkpoints, or crash reports | M | H | Redaction layer; secret scanner; exclude restricted files; diagnostics preview; tests | A/B |
| T-07 | Malicious npm package or postinstall script compromises the machine | H | H | Lockfiles; pinned versions; `--ignore-scripts` default; scoped exceptions; job containment; registry allowlist; audit | B |
| T-08 | Tampered or typosquatted model file | M | H | Signed manifest; pinned revision; SHA-256; GGUF header validation; trusted catalogue; licence display | A |
| T-09 | Tampered runtime pack or DLL hijack | M | H | Signed packs; hash verification; absolute DLL paths; safe DLL search order; code signing | A/B |
| T-10 | Local inference API exposed to LAN | M | H | Bind 127.0.0.1; no 0.0.0.0; firewall not opened; port check | A |
| T-11 | Another local user connects to the inference server | L | M | Per-install bearer token in user-only file; ACL; short-lived rotation; no token in argv | A |
| T-12 | XSS via model output, markdown, or file content | M | H | Raw HTML disabled; sanitizer; CSP; no `innerHTML`; escaped code; no remote images | A |
| T-13 | Preview iframe reaches Nexora IPC or app data | M | H | Separate origin; sandbox attribute; no IPC in frames; CSP frame-src limited to localhost; navigation blocked | B |
| T-14 | Generated site opens popups, navigates the app, or phishes | M | M | Sandbox without `allow-popups`/`allow-top-navigation`; external links open browser after confirmation; URL bar shows loopback | B |
| T-15 | Update channel or signing key compromise | L | H | Signed manifests; TLS; offline public key; key rotation; staged rollout; rollback; incident plan | 1.0 |
| T-16 | Installer tampering or SmartScreen bypass | L | H | Authenticode signing; timestamping; hash publication; reproducible builds; clean-VM tests | 1.0 |
| T-17 | Privilege escalation through UAC or service | L | H | App runs as standard user; no elevation; no service; agent cannot request admin | A |
| T-18 | DoS via huge files, deep trees, or context floods | M | M | Size caps; depth limits; timeouts; virtualized UI; context budgets; cancellation | A/B |
| T-19 | Disk exhaustion via models, checkpoints, logs | M | M | Preflight checks; quotas; retention; usage display; safe failure | B |
| T-20 | Clipboard/screen capture of sensitive data | L | H | Agent has no clipboard/screen APIs; screenshots limited to preview viewport; user-initiated copy only | A |
| T-21 | Shared PC exposes another user's conversations/projects | M | M | Per-user `%LOCALAPPDATA%` ACLs; no machine-wide data; no cross-user indexing; documentation | B |
| T-22 | Remote images in markdown leak IP/metadata | M | M | Remote images blocked; local protocol only; external links require user action | A |
| T-23 | Browser automation SSRF to other localhost services or cloud metadata | M | H | Allowlist only the current preview port; block link-local/metadata IPs; no arbitrary navigation; temp profile | B |
| T-24 | Git credentials or SSH keys exposed through read-only Git | L | H | Never read credential helpers; sanitized env; read-only commands; no `git config` changes | B |
| T-25 | CI supply-chain compromise via Actions or dependencies | M | H | Pin actions by SHA; least-privilege tokens; protected environments; dependency review; SBOM; no secrets in PR builds | 1.0 |
| T-26 | Telemetry or crash report leaks prompts/files | L | H | No telemetry; opt-in; preview payload; redaction; no file contents; local-only default | B |
| T-27 | Malicious template or generated dependency | M | H | Bundled reviewed templates; pinned deps; no remote templates; audit | B |
| T-28 | Model licence violation | M | M | Catalogue gate; licence display; legal review; no blocked models | A |
| T-29 | Rollback overwrites user changes made outside Nexora | M | H | Hash comparison; conflict confirmation; per-file restore; audit | B |
| T-30 | Local database or checkpoints readable by malware running as the same user | M | H | OS ACLs; DPAPI for secrets; document residual risk; optional encryption post-1.0 | 1.0 |

### 4.1 STRIDE summary

| Category | Primary examples | Primary controls |
|----------|------------------|------------------|
| **Spoofing** | Fake model manifest, fake update, fake runtime | Signatures, pinned hashes, TLS, code signing |
| **Tampering** | Model/runtime files, project files, DB | Hash verification, atomic writes, checkpoints, ACLs |
| **Repudiation** | Agent claims it did not run a command | Append-only audit log, correlation IDs |
| **Information disclosure** | Secrets in logs, prompt injection exfiltration, preview escapes | Redaction, exclusion, sandboxing, network gating |
| **Denial of service** | Huge files, context floods, runaway processes | Limits, timeouts, job objects, budgets |
| **Elevation of privilege** | Command escaping, path escape, UAC abuse | No shell, path guard, standard user, AppContainer |

---

## 5. Permission enforcement

The policy engine is the single enforcement point. It receives a structured `ToolCall`, the project context, the current grants, and the risk classification, and returns a decision with an audit record.

**Risk classification:**

| Risk | Examples | Default |
|------|----------|---------|
| Low | Read file in project, search, list | Allow with `workspace.read` |
| Moderate | Write file, run build/test, start preview | Ask once per project/task; remember scoped grants |
| High | Install dependencies, network access, delete, restore checkpoint | Ask per session/action; explicit consequence text |
| Critical | Outside-workspace access, secrets, system changes, Git write | Deny; no persistent grant at 1.0 |

**Rules:**

- A grant is bound to a project, action class, and expiry; it cannot be reused for a different path or command.
- A model cannot widen its own grant. Only a user click can.
- Denials are final for the run; the agent may not retry through another tool.
- Permission prompts show the exact command/path, not a summary generated solely by the model.
- Every decision is logged with the actor, request, and result.
- A "panic switch" cancels work and kills child processes without requiring a model turn.

---

## 6. Windows containment

### 6.1 Baseline (alpha/beta): restricted process + job object

- Spawn tool processes with a **restricted token**: administrator SIDs deny-only, privileges removed, integrity level unchanged unless a stricter mode is selected.
- Assign every child to a **job object** with:
  - `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`
  - active process limit (e.g., 64)
  - per-process memory limit (configurable, default generous enough for Node builds)
  - UI restrictions (no clipboard, no global atoms, no system parameters, no desktop switch)
  - `PROC_THREAD_ATTRIBUTE_CHILD_PROCESS_POLICY = PROCESS_CREATION_CHILD_PROCESS_RESTRICTED` where supported.
- Sanitize the environment: explicit allowlist (`PATH`, `TEMP`, `TMP`, `HOME`, `USERPROFILE`, locale, `CI`), never tokens or credentials.
- Fix the working directory to the project root; deny UNC paths and network shares.
- Kill the job tree on timeout, cancellation, app exit, or panic.
- Create processes suspended, assign to the job, then resume, to close the escape window.

**Limitation:** this is strong against accidental damage and casual malicious commands, but it is not a complete security boundary against a determined native-code exploit. It must be described as containment, not a guarantee.

### 6.2 Strict mode (beta, opt-in): AppContainer/LPAC

- Run supported tool processes in a **Less Privileged AppContainer (LPAC)** with zero capabilities (no internet, no loopback unless explicitly granted, no user-profile access).
- Grant ACLs only to the project workspace, a private temp directory, and the runtime/tool installation folders (read/execute).
- Use a per-project container SID derived from a stable hash; grants are idempotent and revocable.
- Allow network only through an approved localhost proxy that enforces a destination allowlist and logs requests; the container itself has no `internetClient` capability.
- If a tool is incompatible with the container (common for Node/npm native builds), fall back to baseline containment with a warning rather than silently disabling the sandbox.
- Test app compatibility extensively; strict mode is not the default until proven.

**Why not a full VM or Windows Sandbox:** heavy startup, poor file integration, GPU passthrough complexity, and a poor fit for iterative builds. A VM may be offered as a future "high-assurance" mode.

### 6.3 Filesystem guard

- Canonicalize paths with `GetFinalPathNameByHandleW`; reject symlinks and junctions that resolve outside the project.
- Deny device paths (`\\.\`), UNC paths, alternate data streams, and reserved names (`CON`, `NUL`, `COM1`, etc.).
- Compare paths case-insensitively using ordinal rules; normalize separators and trailing dots/spaces.
- Refuse to follow reparse points unless the resolved target is inside the project.
- Enforce per-file (2 MB text) and per-run write limits.
- Use atomic writes: temp file in the same directory, flush, rename, verify hash.
- Never delete recursively without a path guard and confirmation.

### 6.4 Browser and preview isolation

- Preview served from loopback only; iframe sandbox without `allow-top-navigation`, `allow-popups`, or `allow-modals`.
- No Nexora IPC bridge exposed to frames; the custom protocol rejects frame requests.
- CSP for the app disallows remote scripts, frames other than loopback, and `eval`.
- Browser automation navigates only to the active preview origin/port; all other URLs are blocked, including other localhost ports and cloud metadata endpoints (`169.254.169.254`).
- External links open in the system browser after a user click; the app never auto-opens a URL from model output.
- The URL bar displays the real loopback URL, not a friendly alias.

### 6.5 Inference server hardening

- Bind `127.0.0.1` only; never `0.0.0.0`.
- Random per-install bearer token; write to a per-user file with an ACL limited to the current user; pass via `--api-key-file` (not argv, which is readable by other local users).
- Disable the web UI and experimental server-side agent/tool features.
- Single parallel slot; request size and concurrency limits enforced by the app.
- Verify `/props` matches the expected model and context after load.
- Kill the process with its job on app exit; rotate the token on each launch where practical.
- No CORS for browser origins; the app uses native HTTP, so a malicious website cannot read responses even if it can send requests (token still required).
- Log authentication failures; stop accepting requests after repeated failures.

### 6.6 Process execution policy

- No shell by default; argv arrays only.
- Executable allowlist: `node`, `npm`, `pnpm`, `yarn`, `npx` (restricted), `git` (read-only), `python` (only if the project is Python and the user approves), platform tools required by the stack.
- Subcommand policy: `npm install/ci/run build/run test/run lint` allowed with grants; `npm publish`, `npm config set`, `npm exec` arbitrary packages denied; `git push/commit/reset/clean` denied.
- Deny: `cmd /c`, `powershell -enc`, `Invoke-Expression`, `curl | sh`, `wget | sh`, `certutil -urlcache`, `bitsadmin`, `reg add`, `sc`, `schtasks`, `shutdown`, `format`, `diskpart`, `bcdedit`, `takeown`, `icacls` outside temp, `runas`.
- Environment sanitization and `PATH` allowlist prevent binary planting.
- Output is capped and treated as untrusted text.

---

## 7. Prompt injection and untrusted content

The agent reads code, READMEs, comments, issues, dependency output, and preview content. Any of it may contain adversarial instructions.

**Controls:**

1. **Instruction hierarchy:** system and policy outrank repository content; the prompt explicitly labels file content as untrusted data.
2. **Provenance:** every context block carries source path, hash, and trust level; the UI can show what the model saw.
3. **Deterministic policy:** destructive or network actions require policy approval regardless of model intent.
4. **No auto-run of documentation:** commands in READMEs are not executed unless the user asks or the plan verifies them against project scripts.
5. **Suspicious-content detection:** flag phrases such as "ignore previous instructions", "system prompt", "exfiltrate", "send to", "curl |", "powershell -enc", hidden Unicode bidi controls, and instructions to read secrets. Flagging does not block legitimate code; it raises the risk level for tool calls.
6. **Secret exclusion:** `.env`, keys, credential files, and browser profiles are never included.
7. **Network gating:** no outbound data from tool processes without a grant; strict mode blocks network at the OS capability level.
8. **Preview isolation:** generated web content cannot reach app IPC or the filesystem.
9. **Adversarial fixtures:** CI includes repositories with injection attempts; the agent must not execute them or leak secrets.
10. **User visibility:** when an injection attempt is detected, the task report says so; the user can inspect the file and decide.

No prompt-only defense is sufficient. The design assumes the model can be manipulated and relies on enforcement outside the model.

---

## 8. Secrets management

### 8.1 Detection

- File-name patterns: `.env`, `.env.*`, `*.pem`, `*.key`, `*.p12`, `*.pfx`, `id_rsa*`, `id_ed25519*`, `.npmrc`, `.git-credentials`, `credentials.json`, cloud CLI config, browser profile paths.
- Content patterns: provider token formats (AWS, GitHub, GitLab, Stripe, Slack, OpenAI-style keys), private-key blocks, high-entropy strings near `key`/`token`/`secret`/`password`.
- Generated-code scan before the final report: if a secret pattern appears in a file that will be committed, block "verified" and show the location.
- Client-bundle scan: ensure `NEXT_PUBLIC_*` or Vite `VITE_*` variables do not contain secrets.

### 8.2 Handling

- Exclude secret files from context, indexing, checkpoints, and diagnostics by default.
- Never log values; log only the rule and the path with a redacted value.
- If the user explicitly attaches a secret, warn, redact logs, and do not persist it in conversation metadata.
- App credentials (future provider keys) live in Windows Credential Manager via DPAPI, never SQLite or config files.
- Diagnostics bundles run a final secret scan and show a warning if anything matches.
- Deletion of secrets is the user's responsibility; Nexora explains where files live but does not edit them silently.

### 8.3 Redaction pipeline

Every log line, audit detail, diagnostic entry, and crash report passes through:

1. Exact-match redaction of known app tokens.
2. Pattern-based redaction of common credential formats.
3. Entropy-based detection with a conservative threshold to avoid mangling code.
4. Path hashing for home directories in shared diagnostics (optional).

Redaction failures are treated as security bugs, not cosmetic issues.

---

## 9. Supply chain security

### 9.1 Models

- Signed catalogue manifest (Ed25519/minisign) with model ID, pinned revision, file hashes, licence, and hardware requirements.
- HTTPS only; certificate validation; no insecure redirects to HTTP.
- SHA-256 of every file verified before load; size and GGUF header checked; corrupt files quarantined.
- Downloads resumable but never silently retried with a different source.
- The catalogue is reviewed before a model is marked `approved`; `experimental` models require an explicit warning.
- Model provenance and licence are displayed; blocked models cannot be downloaded.

### 9.2 Runtime packs

- Built from a pinned llama.cpp commit with recorded build flags; signed and hashed.
- DLL search hardening; no user-writable directories in the search path.
- Optional antivirus/AMSI scan of downloaded packs before first use (best-effort, not a guarantee).
- Rollback to the previous runtime pack if the new one fails health checks.

### 9.3 Dependencies

- Lockfiles committed; installs use the lockfile (`npm ci`, `pnpm install --frozen-lockfile`).
- No `latest`; no git URLs or tarballs without approval.
- Install scripts disabled by default; scoped exceptions with visibility.
- Registry allowlist; no untrusted mirrors.
- SBOM for the app; licence report; vulnerability scan in CI.
- Generated projects get a dependency report, not an automatic upgrade cascade.

### 9.4 CI/CD

- GitHub Actions pinned by commit SHA; least-privilege `GITHUB_TOKEN`; no secrets in pull-request builds from forks.
- Protected `main`; required reviews; signed commits recommended.
- Release builds run in a protected environment with manual approval.
- Artifacts signed (Authenticode + update manifest), hashed, and accompanied by an SBOM.
- CodeQL, dependency review, secret scanning, and licence checks are required status checks.
- No release or repository publication without explicit owner approval (Phase 1 requirement).

### 9.5 Updates

- Update manifests signed with an offline key; the public key is embedded in the app.
- TLS plus signature verification; downgrade prevention; version monotonicity.
- Staged rollout (e.g., 5% → 25% → 100%) with a kill switch for a bad release.
- The previous version remains installed until the new one launches successfully; automatic rollback on failure.
- Major-version updates require explicit user action.
- Key rotation documented; compromise triggers an advisory and a signed update.

---

## 10. Privacy and data lifecycle

### 10.1 Defaults

| Data | Default | User control |
|------|---------|--------------|
| Telemetry | Off | No opt-in UI at 1.0 (can be added later) |
| Crash reports | Off | Opt-in with preview |
| Update checks | On (can be disabled) | Toggle; disabled in offline mode |
| Model download | User-initiated | Always explicit |
| Dependency network | Permission per session | Revocable |
| Conversation storage | Local | Export/delete |
| Logs | Local, 14 days | Clear; export diagnostics |
| Checkpoints | Local, 30 days / 20 per project | Retention settings; delete |
| Index | Local | Rebuild/delete |

### 10.2 Transparency

- Settings → Privacy & Network lists every app-initiated destination and the last time it was contacted.
- The first-run disclosure explains what is local and what needs network.
- Offline mode disables update checks, model downloads, and any optional network feature; local model, editing, build (with cached dependencies), and preview continue to work.
- The app never claims "fully offline" while a network feature is enabled.

### 10.3 Data subject rights

- **Export:** conversations as Markdown/JSON; project data as a folder; diagnostics as a reviewed bundle.
- **Delete:** per conversation, per project, per model, per log category, or all app data; deletion scope is explained.
- **Portability:** projects are ordinary folders; no proprietary format.
- **Retention:** documented; no hidden copies after deletion except OS backups/recycle bin, which are explained.
- **Secure deletion caveat:** SSD wear-levelling and Windows restore points mean deletion cannot be guaranteed at the physical level; the app deletes its references and files, and explains the limitation.

### 10.4 Shared computers

Nexora stores data per Windows user under `%LOCALAPPDATA%` with user ACLs. It does not create machine-wide project indexes or share conversations between users. The app warns that a Windows administrator or malware running as the same user can access local data. Full-disk encryption and OS account hygiene are the user's responsibility.

### 10.5 Compliance posture

- Nexora itself does not collect personal data at 1.0; the user's projects may contain personal data, and the user remains the controller.
- A privacy policy and licence agreement are required before distribution.
- Model licences and third-party notices are shipped and viewable.
- If telemetry is added later, it requires a lawful basis, data minimization, retention limits, and an opt-in.

---

## 11. Logging, diagnostics, and crash reporting

- **Logs:** structured, local, rotated, redacted. Levels: error, warn, info, debug (debug off by default).
- **Audit log:** append-only, separate from debug logs, records privileged actions and decisions; never contains secret values.
- **Diagnostics bundle:** user-initiated, previewed, zipped; includes versions, hardware, settings (redacted), recent logs, task summaries; excludes prompts, file contents, secrets, and model files by default. The user may add specific files.
- **Crash reports:** local by default; opt-in submission; no prompts, file contents, or secrets; a hash and stack trace only. The user sees exactly what will be sent.
- **No background uploads.** No third-party analytics SDK at 1.0.

---

## 12. Incident response and vulnerability handling

1. **Reporting:** `security.txt`, a dedicated security contact, and a private disclosure channel. No public issue for vulnerabilities.
2. **Triage:** acknowledge within 3 business days; assess severity; assign an owner.
3. **Containment:** for app vulnerabilities, prepare a signed patch; for model/runtime issues, mark the affected model/runtime blocked in the catalogue.
4. **Disclosure:** coordinated advisory with affected versions, severity, mitigation, and credit (if desired).
5. **Post-incident:** root-cause review, regression test, threat-model update.
6. **Signing-key compromise:** rotate keys, publish a signed advisory, require manual update verification, and consider a temporary download halt.
7. **User notification:** only for confirmed impacts; no alarmist messaging; in-app notice with details.

Severity targets: critical patch within 7 days of a confirmed fix being available; high within 30 days; medium in the next scheduled release.

---

## 13. Security testing

| Test | What it covers | Phase |
|------|----------------|-------|
| Threat-model review | Design gaps, trust boundaries | Phase 1/2 |
| Path-guard fuzzing | Traversal, symlinks, UNC, ADS, reserved names | Phase 2 |
| Command-policy tests | Bypass attempts, shell metacharacters, encoded commands | Phase 2 |
| Patch-parser fuzzing | Malformed diffs, huge inputs, encoding attacks | Phase 3 |
| Sandbox escape tests | Job breakaway, child-process policy, AppContainer grants | Phase 3 |
| Prompt-injection fixtures | Repo instructions, README attacks, hidden Unicode | Phase 4 |
| Secret-scanning tests | Known token formats, entropy, false positives | Phase 3 |
| IPC fuzzing | Malformed payloads, oversized messages, replay | Phase 3 |
| WebView2 tests | CSP, navigation, frame isolation, XSS payloads | Phase 3 |
| Inference-server tests | Auth bypass, LAN bind, CORS, token leakage | Phase 3 |
| Dependency/SBOM review | Licences, vulnerabilities, typosquats | Every release |
| Installer/update tests | Signature, downgrade, rollback, tamper | Phase 6 |
| External penetration test | End-to-end attack surface | Before 1.0 |

Findings are tracked with severity, owner, and fix version. No release with an open critical or high security finding.

---

## 14. Residual risks (accepted or monitored)

| Risk | Why it remains | Mitigation / monitoring |
|------|----------------|-------------------------|
| Baseline containment is not a full sandbox | Builds need broad filesystem/process access | Strict mode where compatible; confirmations; checkpoints; documentation |
| Malicious package can still damage the project | Builds execute project code by design | Job containment, script gating, checkpoints, network permission |
| User can approve a dangerous action | Autonomy requires consent | Clear consequence text; no "always allow" for critical actions |
| Local data readable by same-user malware | Desktop OS model | ACLs, DPAPI for secrets, optional encryption post-1.0 |
| Model can be socially engineered | LLM limitation | Deterministic policy; adversarial evals; user visibility |
| Runtime/WebView2 zero-days | Third-party code | Pinned versions, prompt updates, sandboxing |
| Local model quality varies | Hardware/model limits | Honest status, evals, fallback |
| No guaranteed secure deletion | SSD/OS behaviour | Explain; provide best-effort deletion |
| Generated code may be insecure | AI limitation | Security checks, dependency reports, disclaimers, no production guarantee |

These risks must be documented in the user-facing security page, not hidden in an internal document.

---

## 15. Decisions

| ID | Decision | Recommendation | Status |
|----|----------|----------------|--------|
| D-05 | Sandbox baseline | Job object + restricted token; AppContainer/LPAC strict mode | Proposed |
| D-38 | Network default | Deny for tools except approved installs; no telemetry | Proposed |
| D-39 | Secrets | Never in prompts/logs; Credential Manager for app secrets | Proposed |
| D-40 | Local inference auth | Per-install bearer token via file; loopback only | Proposed |
| D-41 | Update signing | Offline key, signed manifests, staged rollout | Proposed |
| D-42 | Crash reports | Opt-in, previewed, no content | Proposed |
| D-43 | Data encryption | OS ACLs + DPAPI; full-disk encryption recommended; app-level DB encryption deferred | Deferred |
| D-44 | Vulnerability disclosure | Private channel, coordinated advisories | Proposed |

Open: Q-26 whether to pursue a security certification (e.g., SOC 2) later; Q-27 whether to offer a VM-based high-assurance mode; Q-28 data-retention defaults; Q-29 enterprise policy controls.

---

## 16. Sources

- OWASP Top 10 for LLM Applications — https://owasp.org/www-project-top-10-for-large-language-model-applications/
- OWASP ASVS — https://owasp.org/www-project-application-security-verification-standard/
- Microsoft Windows AppContainer — https://learn.microsoft.com/windows/win32/secauthz/appcontainer-isolation
- Microsoft Job Objects — https://learn.microsoft.com/windows/win32/procthread/job-objects
- Microsoft restricted tokens — https://learn.microsoft.com/windows/win32/secauthz/restricted-tokens
- Microsoft DPAPI — https://learn.microsoft.com/windows/win32/seccng/cng-dpapi
- Chromium sandbox design — https://chromium.googlesource.com/chromium/src/+/refs/heads/main/docs/design/sandbox.md
- Tauri security — https://v2.tauri.app/security/
- Electron security — https://www.electronjs.org/docs/latest/tutorial/security
- llama.cpp server — https://github.com/ggml-org/llama.cpp/blob/master/tools/server/README.md
- SLSA — https://slsa.dev/
- Sigstore — https://www.sigstore.dev/
- gitleaks — https://github.com/gitleaks/gitleaks
- CISA KEV — https://www.cisa.gov/known-exploited-vulnerabilities-catalog

Access date: 2026-10-17.

# Phases 3–4 — Registry and acquisition integration

## 1. Registry shape

`apps/desktop/src-tauri/src/inference/registry.rs`

```rust
pub enum ArtifactState {
    Approved(ModelSource),
    Pending { note: &'static str },
}

pub struct ModelProfile {
    pub id: &'static str,
    pub display_name: &'static str,
    pub publisher: &'static str,
    pub upstream_model: &'static str,
    pub upstream_revision: Option<&'static str>, // None until recorded
    pub format: &'static str,
    pub quantization: &'static str,
    pub licence: LicenceRef,
    pub artifact: ArtifactState,
    pub context: ContextGuidance,
    pub compatibility_notes: &'static [&'static str],
    pub locally_verified: bool,
}
```

Two changes made the second model possible without a parallel implementation:

1. `source: ModelSource` became `artifact: ArtifactState`, so "known model, unapproved
   artefact" is representable without inventing a digest.
2. `upstream_revision` became `Option`, because the base-model commit is equally unverifiable
   offline.

Registration is still a static table:

```rust
static PROFILES: [ModelProfile; 2] = [NEXORA_MINI, NEXORA_SONNET];
pub static APPROVED: ModelRegistry = ModelRegistry::new(&PROFILES);
pub const DEFAULT_MODEL_ID: &str = NEXORA_MINI_ID;
```

`ModelRegistry` gained `all()` and `selected(Option<&str>)`. `selected` treats `None` and blank
as the default and **errors** on an unknown id rather than falling back — a silent fallback
would be the bug this whole phase exists to prevent.

### Backward compatibility

| Concern | Behaviour |
| --- | --- |
| Settings files written before selection existed | `modelId` is `Option` with `#[serde(default)]`; absent → `None` → Mini |
| Mini's metadata | unchanged: same ids, revision, size, digest, context, notes |
| Existing UI call sites | `get_model_profile` still returns a profile; it now returns the *selected* one |
| Unknown configured file | still allowed while Mini is selected, so renamed files keep working |
| Unknown `modelId` | rejected by `select_model` and `save_settings`, never rewritten |

## 2. Selection guard

`registry::check_selection(selected, loaded)` is the single place that decides whether a
generation may run:

1. The selected id must resolve.
2. Its artefact must be approved — otherwise `artifact_pending` with an explanation.
3. If a profile was loaded, it must be the selected one — otherwise `model_mismatch`.
4. If nothing identifiable was loaded, Mini (the default) is still allowed; a non-default
   selection is refused.

Rule 4 is what keeps existing installations working while making "answer a Sonnet request
with an unidentified file" impossible.

## 3. Download path

`inference/acquisition.rs`:

| Function | Change |
| --- | --- |
| `plan_for(profile)` | now returns `Result`; `Pending` → `artifact_pending`, no URL built |
| `verify_file(path, profile)` | takes the profile; `Pending` → `artifact_pending` |
| `installed_path(profile, candidates)` | new: a candidate counts only if the **file name matches** and the **length matches exactly** |
| `profile_for_path(path)` | new: attributes a loaded file to a profile by name, for the routing guard |

The previously reported download hardening is preserved unchanged:

- `STALL_TIMEOUT` (60 s) covers both the request (`send()` is wrapped in
  `tokio::time::timeout`) and the body stream;
- `redirect::Policy::limited(5)` plus an explicit `https` scheme check after redirects;
- cancellation is honoured during the body **and** immediately before the rename, so a late
  Cancel cannot install a file;
- `.part` files are removed on failure, cancellation, and publish refusal;
- publishing re-checks the length and never overwrites an existing model.

## 4. Command surface

| Command | Change |
| --- | --- |
| `get_model_profile` | returns the selected profile (Mini when nothing is chosen) |
| `get_model_catalog` | new: every profile with `status`, `active`, `installedPath`, `downloadable`, plus `selectionError` computed by `check_selection` |
| `select_model` | new: persists only the id; does not touch paths, files, or the engine |
| `save_settings` | validates the id; persists `modelId` |
| `start_model_download` | takes `profileId` |
| `verify_model` | takes `profileId` (defaults to the selection) |
| `load_model` | records the loaded profile; clears on failure |
| `unload_model` | clears the loaded profile |
| `start_generation` | runs `check_selection` before spending a usage unit |

An unused `list_model_profiles` command was written and then **removed** during this phase:
`get_model_catalog` supersedes it, and the contract test that asserts every registered command
is reachable from the UI caught the orphan. No dead surface was left behind.

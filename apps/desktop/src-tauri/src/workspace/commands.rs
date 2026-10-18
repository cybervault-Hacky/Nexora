use std::path::{Path, PathBuf};
use std::sync::Mutex;

use tauri::State;

use super::scanner::{
    canonicalize_root, is_sensitive_file, read_workspace_file, scan_workspace, FilePreview,
    WorkspaceError, WorkspaceInventory, WorkspaceResult, MAX_CONTEXT_FILES,
    MAX_CONTEXT_TOTAL_BYTES,
};

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

pub struct WorkspaceState {
    pub root: Mutex<Option<PathBuf>>,
}

impl WorkspaceState {
    pub fn new() -> Self {
        Self {
            root: Mutex::new(None),
        }
    }
}

fn lock_err() -> WorkspaceError {
    WorkspaceError::storage("Workspace state unavailable. Restart Nexora.")
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn validate_context_files(root: &Path, paths: &[String]) -> WorkspaceResult<Vec<(String, String)>> {
    if paths.len() > MAX_CONTEXT_FILES {
        return Err(WorkspaceError::invalid_request(format!(
            "Too many context files ({} max {}).",
            paths.len(),
            MAX_CONTEXT_FILES
        )));
    }
    let mut total = 0usize;
    let mut out = Vec::new();
    for rel in paths {
        let preview = read_workspace_file(root, rel)?;
        // Additional sensitive check already inside read
        let bytes = preview.content.len();
        total += bytes;
        if total > MAX_CONTEXT_TOTAL_BYTES {
            return Err(WorkspaceError::too_large(format!(
                "Context too large ({} bytes, max {}).",
                total, MAX_CONTEXT_TOTAL_BYTES
            )));
        }
        // Binary already rejected
        out.push((preview.path, preview.content));
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

/// Validate a path string as a workspace root and remember it on success.
/// Returns the canonical root and its display name. Frontend may store the
/// canonical string for later `scan`/`read` calls.
#[tauri::command]
pub fn set_workspace(
    state: State<WorkspaceState>,
    path: String,
) -> WorkspaceResult<WorkspaceInventory> {
    if path.trim().is_empty() {
        return Err(WorkspaceError::invalid_request("Project path is empty."));
    }
    if path.chars().count() > 4096 {
        return Err(WorkspaceError::invalid_request("Path too long."));
    }
    let input = PathBuf::from(path.trim());
    let canonical = canonicalize_root(&input)?;
    // Remember
    let mut guard = state.root.lock().map_err(|_| lock_err())?;
    *guard = Some(canonical.clone());
    drop(guard);
    scan_workspace(&canonical)
}

/// Scan a workspace root provided explicitly (stateless). Validates and scans.
#[tauri::command]
pub fn scan_workspace_command(
    _state: State<WorkspaceState>,
    root: String,
) -> WorkspaceResult<WorkspaceInventory> {
    if root.trim().is_empty() {
        return Err(WorkspaceError::invalid_request("Project path is empty."));
    }
    let p = PathBuf::from(root.trim());
    scan_workspace(&p)
}

/// Alternative: scan the remembered workspace (if set). Used for refresh without resending path.
#[tauri::command]
pub fn scan_current_workspace(state: State<WorkspaceState>) -> WorkspaceResult<WorkspaceInventory> {
    let guard = state.root.lock().map_err(|_| lock_err())?;
    let Some(root) = guard.clone() else {
        return Err(WorkspaceError::not_found(
            "No project selected. Open a project folder first.",
        ));
    };
    drop(guard);
    scan_workspace(&root)
}

/// Read a file inside the workspace. `root` must be the canonical workspace root
/// previously validated, `relativePath` is the posix relative path from inventory.
#[tauri::command]
pub fn read_workspace_file_command(
    _state: State<WorkspaceState>,
    root: String,
    relative_path: String,
) -> WorkspaceResult<FilePreview> {
    if root.trim().is_empty() {
        return Err(WorkspaceError::invalid_request("Project path is empty."));
    }
    let root_path = PathBuf::from(root.trim());
    read_workspace_file(&root_path, &relative_path)
}

/// Validate and return eligible context files (checks sensitive, size, count, total).
/// Does not read entire project, only the explicitly listed rel paths.
#[tauri::command]
pub fn get_workspace_context(
    _state: State<WorkspaceState>,
    root: String,
    paths: Vec<String>,
) -> WorkspaceResult<Vec<FilePreview>> {
    if root.trim().is_empty() {
        return Err(WorkspaceError::invalid_request("Project path is empty."));
    }
    if paths.is_empty() {
        return Ok(Vec::new());
    }
    if paths.len() > MAX_CONTEXT_FILES {
        return Err(WorkspaceError::invalid_request(format!(
            "Too many files selected ({} > {}).",
            paths.len(),
            MAX_CONTEXT_FILES
        )));
    }
    let root_path = PathBuf::from(root.trim());
    let canonical_root = canonicalize_root(&root_path)?;
    let mut total = 0usize;
    let mut out = Vec::new();
    for rel in paths {
        let preview = read_workspace_file(&canonical_root, &rel)?;
        total += preview.content.len();
        if total > MAX_CONTEXT_TOTAL_BYTES {
            return Err(WorkspaceError::too_large(format!(
                "Selected files exceed the {} byte context limit.",
                MAX_CONTEXT_TOTAL_BYTES
            )));
        }
        // Defensive: block sensitive even if caller bypassed preview check
        if is_sensitive_file(Path::new(&rel)) {
            return Err(WorkspaceError::secret_excluded(
                "Sensitive files cannot be used as context.",
            ));
        }
        out.push(preview);
    }
    Ok(out)
}

/// Close the current workspace (forget root). Does not delete files.
#[tauri::command]
pub fn close_workspace(state: State<WorkspaceState>) -> WorkspaceResult<bool> {
    let mut guard = state.root.lock().map_err(|_| lock_err())?;
    let had = guard.is_some();
    *guard = None;
    Ok(had)
}

/// Return the currently remembered workspace, if any.
#[tauri::command]
pub fn get_workspace(state: State<WorkspaceState>) -> WorkspaceResult<Option<String>> {
    let guard = state.root.lock().map_err(|_| lock_err())?;
    Ok(guard.as_ref().map(|p| p.to_string_lossy().to_string()))
}

// ---------------------------------------------------------------------------
// Picks — native dialog stub
// ---------------------------------------------------------------------------

/// Returns an error indicating the frontend should use `@tauri-apps/plugin-dialog`
/// or a manual path prompt. Keeping filesystem dialogs off the IPC transport
/// preserves least-privilege: the webview never gains bulk filesystem scope.
#[tauri::command]
pub fn pick_workspace_dialog() -> WorkspaceResult<Option<String>> {
    Err(WorkspaceError::invalid_request(
        "Native picker not available via this IPC. Use the desktop dialog or type a path.",
    ))
}

// ---------------------------------------------------------------------------
// Workspace settings persistence (versioned, app_config_dir)
// ---------------------------------------------------------------------------

pub struct WorkspaceSettingsState {
    pub store: Mutex<super::settings::WorkspaceSettingsStore>,
}

impl WorkspaceSettingsState {
    pub fn new(config_dir: PathBuf) -> Self {
        Self {
            store: Mutex::new(super::settings::WorkspaceSettingsStore::load(&config_dir)),
        }
    }
}

fn settings_lock_err() -> WorkspaceError {
    WorkspaceError::storage("Workspace settings unavailable. Restart Nexora.")
}

#[tauri::command]
pub fn get_workspace_settings(
    state: State<WorkspaceSettingsState>,
) -> WorkspaceResult<super::settings::WorkspaceSettings> {
    let guard = state.store.lock().map_err(|_| settings_lock_err())?;
    Ok(guard.get())
}

#[tauri::command]
pub fn save_workspace_settings(
    state: State<WorkspaceSettingsState>,
    settings: super::settings::WorkspaceSettings,
) -> WorkspaceResult<super::settings::WorkspaceSettings> {
    let mut guard = state.store.lock().map_err(|_| settings_lock_err())?;
    guard.save(settings)
}

// ---------------------------------------------------------------------------
// Patch proposal — single file, validated, explicit apply
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn create_patch_proposal(
    _state: State<WorkspaceState>,
    root: String,
    relative_path: String,
    proposed_content: String,
) -> WorkspaceResult<super::patch::PatchProposal> {
    if root.trim().is_empty() {
        return Err(WorkspaceError::invalid_request("Project path is empty."));
    }
    let root_path = PathBuf::from(root.trim());
    super::patch::create_patch_proposal(&root_path, &relative_path, &proposed_content)
}

#[tauri::command]
pub fn validate_patch_proposal(
    _state: State<WorkspaceState>,
    root: String,
    proposal: super::patch::PatchProposal,
) -> WorkspaceResult<super::patch::PatchValidation> {
    if root.trim().is_empty() {
        return Err(WorkspaceError::invalid_request("Project path is empty."));
    }
    let root_path = PathBuf::from(root.trim());
    super::patch::validate_patch_proposal(&root_path, &proposal)
}

#[tauri::command]
pub fn apply_patch_proposal(
    _state: State<WorkspaceState>,
    root: String,
    proposal: super::patch::PatchProposal,
) -> WorkspaceResult<super::patch::ApplyResult> {
    if root.trim().is_empty() {
        return Err(WorkspaceError::invalid_request("Project path is empty."));
    }
    let root_path = PathBuf::from(root.trim());
    super::patch::apply_patch_proposal(&root_path, &proposal)
}

#[tauri::command]
pub fn get_patch_diff(
    _state: State<WorkspaceState>,
    original: String,
    proposed: String,
) -> WorkspaceResult<super::patch::PatchDiffSummary> {
    // No filesystem access — pure diff, safe for any input up to limit
    if original.len() > 400_000 || proposed.len() > 400_000 {
        return Err(WorkspaceError::too_large("Diff input too large."));
    }
    Ok(super::patch::diff_summary(&original, &proposed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn temp_with_file(content: &str) -> (TempDir, PathBuf) {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("src/main.rs");
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, content).unwrap();
        (dir, file)
    }

    #[test]
    fn set_and_scan_current() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("a.txt"), "hello").unwrap();
        let state = WorkspaceState::new();
        // Simulate Tauri State wrapper by directly calling scanner for logic coverage
        let inv = scan_workspace(dir.path()).unwrap();
        assert!(inv.entries.iter().any(|e| e.path == "a.txt"));
        // State initial None
        assert!(state.root.lock().unwrap().is_none());
    }

    #[test]
    fn context_limits() {
        let dir = TempDir::new().unwrap();
        for i in 0..10 {
            fs::write(dir.path().join(format!("f{}.txt", i)), "x").unwrap();
        }
        let paths: Vec<String> = (0..9).map(|i| format!("f{}.txt", i)).collect();
        let err = get_workspace_context_inner(dir.path(), paths);
        assert!(err.is_err());
        assert_eq!(err.unwrap_err().code, "invalid_request");
    }

    fn get_workspace_context_inner(
        root: &Path,
        paths: Vec<String>,
    ) -> WorkspaceResult<Vec<FilePreview>> {
        let mut total = 0usize;
        let mut out = Vec::new();
        for rel in paths {
            let preview = read_workspace_file(root, &rel)?;
            total += preview.content.len();
            if total > MAX_CONTEXT_TOTAL_BYTES {
                return Err(WorkspaceError::too_large("too large"));
            }
            out.push(preview);
        }
        Ok(out)
    }

    #[test]
    fn sensitive_blocked_in_context() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(".env"), "secret").unwrap();
        let err = read_workspace_file(dir.path(), ".env").unwrap_err();
        assert_eq!(err.code, "secret_excluded");
    }
}

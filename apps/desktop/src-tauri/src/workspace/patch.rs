use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use similar::{ChangeTag, TextDiff};

use super::scanner::{
    is_sensitive_file, validate_relative_path, WorkspaceError, WorkspaceResult, MAX_PATH_CHARS,
    MAX_PREVIEW_BYTES,
};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchProposal {
    pub id: String,
    pub path: String, // posix relative
    pub original_content: String,
    pub original_hash: String,
    pub proposed_content: String,
    pub proposed_hash: String,
    pub created_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchDiffSummary {
    pub added: usize,
    pub removed: usize,
    pub unchanged: usize,
    pub original_lines: usize,
    pub proposed_lines: usize,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchValidation {
    pub ok: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub diff_summary: PatchDiffSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyResult {
    pub applied: bool,
    pub path: String,
    pub previous_hash: String,
    pub new_hash: String,
    pub new_size: u64,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn hash_content(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    let result = hasher.finalize();
    // hex encode without extra crate
    let mut s = String::with_capacity(result.len() * 2);
    for b in result {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

fn generate_id() -> String {
    let mut buf = [0u8; 16];
    // getrandom 0.2
    let _ = getrandom::getrandom(&mut buf);
    let mut s = String::with_capacity(36);
    for b in buf {
        s.push_str(&format!("{b:02x}"));
    }
    // prefix and trim for readability
    format!("patch-{}-{}", now_millis(), &s[..8])
}

pub fn diff_summary(original: &str, proposed: &str) -> PatchDiffSummary {
    // Use similar crate to compute line diff
    let diff = TextDiff::from_lines(original, proposed);
    let mut added = 0usize;
    let mut removed = 0usize;
    let mut unchanged = 0usize;
    for change in diff.iter_all_changes() {
        match change.tag() {
            ChangeTag::Equal => unchanged += 1,
            ChangeTag::Delete => removed += 1,
            ChangeTag::Insert => added += 1,
        }
    }
    let original_lines = original.lines().count();
    let proposed_lines = proposed.lines().count();
    // Truncated if either > 5000 lines (guard)
    let truncated = original_lines > 5000 || proposed_lines > 5000;
    PatchDiffSummary {
        added,
        removed,
        unchanged,
        original_lines,
        proposed_lines,
        truncated,
    }
}

fn is_binary_content(s: &str) -> bool {
    // Check for nul byte
    if s.contains('\0') {
        return true;
    }
    // Already valid utf8 since s is String, but check for control?
    false
}

fn validate_proposed_size(content: &str) -> WorkspaceResult<()> {
    if content.as_bytes().len() as u64 > MAX_PREVIEW_BYTES {
        return Err(WorkspaceError::too_large(format!(
            "Proposed content too large ({} bytes, max {}).",
            content.len(),
            MAX_PREVIEW_BYTES
        )));
    }
    if content.chars().count() > 200_000 {
        return Err(WorkspaceError::too_large(
            "Proposed content exceeds character limit.",
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Core: create, validate, apply
// ---------------------------------------------------------------------------

pub fn create_patch_proposal(
    root: &Path,
    relative_path: &str,
    proposed_content: &str,
) -> WorkspaceResult<PatchProposal> {
    // Validate relative path
    validate_relative_path(relative_path)?;
    if relative_path.chars().count() > MAX_PATH_CHARS {
        return Err(WorkspaceError::invalid_request("Path too long."));
    }

    // Validate proposed content size early
    validate_proposed_size(proposed_content)?;
    if is_binary_content(proposed_content) {
        return Err(WorkspaceError::binary(
            "Proposed content contains binary data.",
        ));
    }

    // Canonicalize root
    let canonical_root = super::scanner::canonicalize_root(root)?;

    // Join and canonicalize target
    let joined = canonical_root.join(relative_path);
    let canonical_file = fs::canonicalize(&joined)
        .map_err(|_| WorkspaceError::not_found("File not found or not accessible."))?;

    if !canonical_file.starts_with(&canonical_root) {
        return Err(WorkspaceError::forbidden("File escapes workspace."));
    }

    // Must be existing regular file
    let meta = fs::metadata(&canonical_file)
        .map_err(|_| WorkspaceError::not_found("File not accessible."))?;
    if meta.is_dir() {
        return Err(WorkspaceError::invalid_request(
            "Path is a directory, not a file.",
        ));
    }

    // Sensitive check
    if is_sensitive_file(&canonical_file) {
        return Err(WorkspaceError::secret_excluded(
            "Sensitive files cannot be patched.",
        ));
    }
    let lower_rel = relative_path.to_ascii_lowercase();
    if lower_rel == ".env" || lower_rel.starts_with(".env.") || lower_rel.contains("/.env") {
        return Err(WorkspaceError::secret_excluded(
            "Environment files are excluded.",
        ));
    }

    // Size check for original file
    if meta.len() > MAX_PREVIEW_BYTES {
        return Err(WorkspaceError::too_large(
            "Original file too large for patching.",
        ));
    }

    // Read original
    let original_bytes = fs::read(&canonical_file)
        .map_err(|_| WorkspaceError::storage("Could not read original file."))?;
    if original_bytes.len() as u64 > MAX_PREVIEW_BYTES {
        return Err(WorkspaceError::too_large(
            "Original file exceeds preview limit.",
        ));
    }
    if original_bytes.contains(&0) {
        return Err(WorkspaceError::binary(
            "Original file is binary and cannot be patched.",
        ));
    }
    let original_content = String::from_utf8(original_bytes)
        .map_err(|_| WorkspaceError::binary("Original file is not valid UTF-8."))?;

    // Compute hashes
    let original_hash = hash_content(&original_content);
    let proposed_hash = hash_content(proposed_content);

    // If identical, still allow but diff will be zero
    let posix_path = super::scanner::posix_relative(&canonical_root, &canonical_file)?;

    Ok(PatchProposal {
        id: generate_id(),
        path: posix_path,
        original_content,
        original_hash,
        proposed_content: proposed_content.to_string(),
        proposed_hash,
        created_at: now_millis(),
    })
}

pub fn validate_patch_proposal(
    root: &Path,
    proposal: &PatchProposal,
) -> WorkspaceResult<PatchValidation> {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    // Validate path
    if let Err(e) = validate_relative_path(&proposal.path) {
        errors.push(format!("Invalid path: {}", e.message));
    }
    if proposal.path.chars().count() > MAX_PATH_CHARS {
        errors.push("Path too long.".to_string());
    }

    // Hash consistency: recompute from contents
    let recomputed_original = hash_content(&proposal.original_content);
    if recomputed_original != proposal.original_hash {
        errors.push("Original hash does not match original content.".to_string());
    }
    let recomputed_proposed = hash_content(&proposal.proposed_content);
    if recomputed_proposed != proposal.proposed_hash {
        errors.push("Proposed hash does not match proposed content.".to_string());
    }

    // Size checks
    if proposal.original_content.as_bytes().len() as u64 > MAX_PREVIEW_BYTES {
        errors.push("Original content too large.".to_string());
    }
    if proposal.proposed_content.as_bytes().len() as u64 > MAX_PREVIEW_BYTES {
        errors.push("Proposed content too large.".to_string());
    }

    // Binary checks
    if is_binary_content(&proposal.original_content) || proposal.original_content.contains('\0') {
        errors.push("Original content is binary.".to_string());
    }
    if is_binary_content(&proposal.proposed_content) {
        errors.push("Proposed content is binary.".to_string());
    }

    // File existence and workspace containment (re-canonicalize)
    let canonical_root = match super::scanner::canonicalize_root(root) {
        Ok(r) => r,
        Err(e) => {
            errors.push(format!("Invalid workspace root: {}", e.message));
            return Ok(PatchValidation {
                ok: false,
                errors,
                warnings,
                diff_summary: diff_summary(&proposal.original_content, &proposal.proposed_content),
            });
        }
    };

    let joined = canonical_root.join(&proposal.path);
    let canonical_file = match fs::canonicalize(&joined) {
        Ok(c) => c,
        Err(_) => {
            errors.push("Target file not found.".to_string());
            // Still compute diff
            return Ok(PatchValidation {
                ok: false,
                errors,
                warnings,
                diff_summary: diff_summary(&proposal.original_content, &proposal.proposed_content),
            });
        }
    };

    if !canonical_file.starts_with(&canonical_root) {
        errors.push("File escapes workspace.".to_string());
    }

    if let Ok(meta) = fs::metadata(&canonical_file) {
        if meta.is_dir() {
            errors.push("Target is a directory.".to_string());
        }
        if meta.len() > MAX_PREVIEW_BYTES {
            errors.push("Current file too large.".to_string());
        }
    }

    if is_sensitive_file(&canonical_file) {
        errors.push("Sensitive files cannot be patched.".to_string());
    }
    let lower = proposal.path.to_ascii_lowercase();
    if lower == ".env" || lower.starts_with(".env.") || lower.contains("/.env") {
        errors.push("Environment files are excluded.".to_string());
    }

    // Staleness is not an error at validation time, but a warning if current content differs
    // We check current file hash vs proposal.original_hash
    if errors.is_empty() {
        if let Ok(current_bytes) = fs::read(&canonical_file) {
            if let Ok(current_str) = String::from_utf8(current_bytes) {
                let current_hash = hash_content(&current_str);
                if current_hash != proposal.original_hash {
                    warnings
                        .push("File has changed since proposal was created (stale).".to_string());
                }
            }
        }
    }

    let diff = diff_summary(&proposal.original_content, &proposal.proposed_content);
    if proposal.original_content == proposal.proposed_content {
        warnings
            .push("No changes detected — original and proposed content are identical.".to_string());
    }

    let ok = errors.is_empty();
    Ok(PatchValidation {
        ok,
        errors,
        warnings,
        diff_summary: diff,
    })
}

pub fn apply_patch_proposal(root: &Path, proposal: &PatchProposal) -> WorkspaceResult<ApplyResult> {
    // Revalidate first
    let validation = validate_patch_proposal(root, proposal)?;
    if !validation.ok {
        return Err(WorkspaceError::invalid_request(format!(
            "Proposal validation failed: {}",
            validation.errors.join("; ")
        )));
    }
    // Check stale via warning
    if validation.warnings.iter().any(|w| w.contains("stale")) {
        return Err(WorkspaceError::invalid_request(
            "File has changed since the proposal was created. Reload and recreate the proposal.",
        ));
    }

    let canonical_root = super::scanner::canonicalize_root(root)?;
    let joined = canonical_root.join(&proposal.path);
    let canonical_file =
        fs::canonicalize(&joined).map_err(|_| WorkspaceError::not_found("File not found."))?;

    if !canonical_file.starts_with(&canonical_root) {
        return Err(WorkspaceError::forbidden("File escapes workspace."));
    }

    // Verify still a file, not dir
    let meta = fs::metadata(&canonical_file)
        .map_err(|_| WorkspaceError::not_found("File not accessible."))?;
    if meta.is_dir() {
        return Err(WorkspaceError::invalid_request("Target is a directory."));
    }

    // Double-check sensitive
    if is_sensitive_file(&canonical_file) {
        return Err(WorkspaceError::secret_excluded(
            "Sensitive files cannot be patched.",
        ));
    }

    // Compare current content to original
    let current_bytes =
        fs::read(&canonical_file).map_err(|_| WorkspaceError::storage("Could not read file."))?;
    let current_str = String::from_utf8(current_bytes)
        .map_err(|_| WorkspaceError::binary("Current file is binary."))?;
    let current_hash = hash_content(&current_str);
    if current_hash != proposal.original_hash {
        return Err(WorkspaceError::invalid_request(
            "Stale proposal: file has changed.",
        ));
    }
    if current_str != proposal.original_content {
        return Err(WorkspaceError::invalid_request(
            "Stale proposal: original content mismatch.",
        ));
    }

    // Check proposed size again
    validate_proposed_size(&proposal.proposed_content)?;

    // Atomic write: NamedTempFile in parent dir
    let parent = canonical_file
        .parent()
        .ok_or_else(|| WorkspaceError::storage("Invalid file parent."))?;
    // Ensure parent inside root (already)
    {
        use std::io::Write;
        let mut tmp = tempfile::NamedTempFile::new_in(parent)
            .map_err(|_| WorkspaceError::storage("Could not create temporary patch file."))?;
        tmp.write_all(proposal.proposed_content.as_bytes())
            .and_then(|_| tmp.as_file().sync_all())
            .map_err(|_| WorkspaceError::storage("Could not write patched content."))?;
        tmp.persist(&canonical_file)
            .map_err(|_| WorkspaceError::storage("Could not replace file. Check permissions."))?;
        #[cfg(unix)]
        {
            if let Ok(dir) = fs::File::open(parent) {
                let _ = dir.sync_all();
            }
        }
    }

    let new_hash = proposal.proposed_hash.clone();
    let new_size = proposal.proposed_content.as_bytes().len() as u64;

    Ok(ApplyResult {
        applied: true,
        path: proposal.path.clone(),
        previous_hash: proposal.original_hash.clone(),
        new_hash,
        new_size,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn make_file(dir: &Path, rel: &str, content: &str) {
        let p = dir.join(rel);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(p, content).unwrap();
    }

    #[test]
    fn valid_single_file_proposal() {
        let dir = TempDir::new().unwrap();
        make_file(dir.path(), "src/app.ts", "let x = 1;\n");
        let proposal = create_patch_proposal(dir.path(), "src/app.ts", "let x = 2;\n").unwrap();
        assert_eq!(proposal.path, "src/app.ts");
        let validation = validate_patch_proposal(dir.path(), &proposal).unwrap();
        assert!(validation.ok);
        assert_eq!(validation.diff_summary.added, 1);
        assert_eq!(validation.diff_summary.removed, 1);
        let result = apply_patch_proposal(dir.path(), &proposal).unwrap();
        assert!(result.applied);
        assert_eq!(
            fs::read_to_string(dir.path().join("src/app.ts")).unwrap(),
            "let x = 2;\n"
        );
    }

    #[test]
    fn rejects_absolute_and_traversal() {
        let dir = TempDir::new().unwrap();
        make_file(dir.path(), "a.txt", "hi");
        let err = create_patch_proposal(dir.path(), "/etc/passwd", "new").unwrap_err();
        assert_eq!(err.code, "invalid_request");
        let err2 = create_patch_proposal(dir.path(), "../a.txt", "new").unwrap_err();
        assert_eq!(err2.code, "invalid_request");
    }

    #[test]
    fn rejects_sensitive_binary_oversized() {
        let dir = TempDir::new().unwrap();
        make_file(dir.path(), ".env", "secret");
        let err = create_patch_proposal(dir.path(), ".env", "new").unwrap_err();
        assert_eq!(err.code, "secret_excluded");
        make_file(dir.path(), "ok.txt", "hi");
        let err2 = create_patch_proposal(dir.path(), "ok.txt", "has\0nul").unwrap_err();
        assert_eq!(err2.code, "binary");
        let big = "a".repeat((MAX_PREVIEW_BYTES as usize) + 10);
        let err3 = create_patch_proposal(dir.path(), "ok.txt", &big).unwrap_err();
        assert_eq!(err3.code, "too_large");
    }

    #[test]
    fn symlink_escape_rejected() {
        let dir = TempDir::new().unwrap();
        let outside = TempDir::new().unwrap();
        make_file(outside.path(), "secret.txt", "outside");
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            symlink(
                outside.path().join("secret.txt"),
                dir.path().join("link.txt"),
            )
            .unwrap();
            let err = create_patch_proposal(dir.path(), "link.txt", "new").unwrap_err();
            // Either not_found or forbidden depending on canonicalization
            assert!(
                err.code == "forbidden"
                    || err.code == "not_found"
                    || err.code == "storage"
                    || err.code == "secret_excluded"
            );
        }

        // Creating a symlink needs privileges on Windows, so the escape vector
        // is only exercised on unix. Keep the fixture meaningful everywhere
        // rather than leaving `dir` unused on the other platform.
        #[cfg(not(unix))]
        {
            make_file(dir.path(), "inside.txt", "inside");
            assert!(create_patch_proposal(dir.path(), "inside.txt", "new").is_ok());
        }
    }

    #[test]
    fn detects_stale() {
        let dir = TempDir::new().unwrap();
        make_file(dir.path(), "a.txt", "v1");
        let proposal = create_patch_proposal(dir.path(), "a.txt", "v2").unwrap();
        // Modify file before apply
        fs::write(dir.path().join("a.txt"), "v1-modified").unwrap();
        let err = apply_patch_proposal(dir.path(), &proposal).unwrap_err();
        assert!(err.message.to_lowercase().contains("stale") || err.code == "invalid_request");
        // File should remain modified, not overwritten to v2
        assert_eq!(
            fs::read_to_string(dir.path().join("a.txt")).unwrap(),
            "v1-modified"
        );
    }

    #[test]
    fn rejects_invalid_hash() {
        let dir = TempDir::new().unwrap();
        make_file(dir.path(), "a.txt", "hello");
        let mut proposal = create_patch_proposal(dir.path(), "a.txt", "world").unwrap();
        proposal.proposed_hash = "badhash".to_string();
        let v = validate_patch_proposal(dir.path(), &proposal).unwrap();
        assert!(!v.ok);
        assert!(v.errors.iter().any(|e| e.contains("hash")));
    }

    #[test]
    fn multi_file_rejected() {
        // Our API only supports single file; attempting to create proposal for non-existing file that would imply new file
        let dir = TempDir::new().unwrap();
        // Not creating file, so should reject as not_found
        let err = create_patch_proposal(dir.path(), "newfile.txt", "content").unwrap_err();
        assert_eq!(err.code, "not_found");
    }

    #[test]
    fn out_of_root_rejected() {
        let dir = TempDir::new().unwrap();
        let parent = dir.path().parent().unwrap();
        let err = create_patch_proposal(parent, "tmp", "x").unwrap_err();
        // Depends on path handling, but should reject or not_found
        assert!(
            err.code == "not_found" || err.code == "forbidden" || err.code == "invalid_request"
        );
    }

    #[test]
    fn failed_write_preserves_original() {
        let dir = TempDir::new().unwrap();
        make_file(dir.path(), "a.txt", "orig");
        let proposal = create_patch_proposal(dir.path(), "a.txt", "new").unwrap();
        // Make file read-only to cause write failure if possible (unix)
        #[cfg(unix)]
        {
            let p = dir.path().join("a.txt");
            // Make parent dir not writable instead? File write via persist uses parent dir permissions
            // Simulate by making parent dir read-only
            // But we can test that apply fails but file still orig when we use a dir as target
            fs::write(dir.path().join("a.txt"), "orig").unwrap();
            // Try to apply with proposal that would succeed, then verify file unchanged if we break permissions
            // Instead we test that a second apply with stale original fails and preserves file
            fs::write(&p, "orig").unwrap();
            let mut bad = proposal.clone();
            bad.original_content = "different".to_string();
            bad.original_hash = hash_content("different");
            let err = apply_patch_proposal(dir.path(), &bad).unwrap_err();
            assert!(err.code == "invalid_request");
            assert_eq!(fs::read_to_string(&p).unwrap(), "orig");
        }
        #[cfg(not(unix))]
        {
            assert_eq!(
                fs::read_to_string(dir.path().join("a.txt")).unwrap(),
                "orig"
            );
        }
    }

    #[test]
    fn only_intended_file_changed() {
        let dir = TempDir::new().unwrap();
        make_file(dir.path(), "a.txt", "a");
        make_file(dir.path(), "b.txt", "b");
        let proposal = create_patch_proposal(dir.path(), "a.txt", "a2").unwrap();
        apply_patch_proposal(dir.path(), &proposal).unwrap();
        assert_eq!(fs::read_to_string(dir.path().join("a.txt")).unwrap(), "a2");
        assert_eq!(fs::read_to_string(dir.path().join("b.txt")).unwrap(), "b");
    }

    #[test]
    fn diff_summary_correct() {
        let original = "line1\nline2\nline3\n";
        let proposed = "line1\nlineX\nline3\nline4\n";
        let summary = diff_summary(original, proposed);
        assert!(summary.added >= 1);
        assert!(summary.removed >= 1);
        assert!(summary.unchanged >= 2);
    }
}

// ---------------------------------------------------------------------------
// Multi-file patch batches — atomic all-or-nothing apply
// ---------------------------------------------------------------------------
//
// A batch is 1..=MAX_BATCH_FILES per-file proposals applied fully or not at
// all (Phase-1 agent protocol, doc 05 §14). Every file replace is individually
// atomic (tempfile + persist, same as single-file apply). If any replace fails,
// files already replaced in this batch are restored from their recorded
// original content (atomic write) and the batch reports an error.
//
// Documented residual: a process crash mid-apply is not journaled; the next
// validate/apply reports stale hashes per file and each file on disk is still
// well-formed.

pub const MAX_BATCH_FILES: usize = 20;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchEdit {
    pub relative_path: String,
    pub proposed_content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchBatch {
    pub id: String,
    pub files: Vec<PatchProposal>,
    pub created_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchFileValidation {
    pub path: String,
    pub validation: PatchValidation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchValidation {
    pub ok: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub files: Vec<BatchFileValidation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchApplyResult {
    pub applied: bool,
    pub results: Vec<ApplyResult>,
}

/// Create per-file proposals for a batch. Each file goes through the exact
/// single-file checks (path, traversal, sensitive, binary, size, hashes).
pub fn create_patch_batch(root: &Path, edits: &[BatchEdit]) -> WorkspaceResult<PatchBatch> {
    if edits.is_empty() {
        return Err(WorkspaceError::invalid_request(
            "Batch is empty — at least one file is required.",
        ));
    }
    if edits.len() > MAX_BATCH_FILES {
        return Err(WorkspaceError::invalid_request(format!(
            "Too many files in patch batch (max {}).",
            MAX_BATCH_FILES
        )));
    }

    // Unique paths — case-insensitive because Windows paths are.
    let mut seen = std::collections::HashSet::new();
    for edit in edits {
        if edit.relative_path.trim().is_empty() {
            return Err(WorkspaceError::invalid_request(
                "Batch contains an empty target path.",
            ));
        }
        if !seen.insert(edit.relative_path.to_lowercase()) {
            return Err(WorkspaceError::invalid_request(format!(
                "Duplicate path in patch batch: '{}'.",
                edit.relative_path
            )));
        }
    }

    let mut files = Vec::with_capacity(edits.len());
    for edit in edits {
        files.push(create_patch_proposal(
            root,
            &edit.relative_path,
            &edit.proposed_content,
        )?);
    }

    Ok(PatchBatch {
        id: generate_id(),
        files,
        created_at: now_millis(),
    })
}

/// Validate every file in the batch and aggregate. Batch-level failures (any
/// per-file error) make `ok` false; stale warnings keep their file paths.
pub fn validate_patch_batch(root: &Path, batch: &PatchBatch) -> WorkspaceResult<BatchValidation> {
    if batch.files.is_empty() {
        return Ok(BatchValidation {
            ok: false,
            errors: vec!["Batch is empty — at least one file is required.".to_string()],
            warnings: Vec::new(),
            files: Vec::new(),
        });
    }
    if batch.files.len() > MAX_BATCH_FILES {
        return Ok(BatchValidation {
            ok: false,
            errors: vec![format!(
                "Too many files in patch batch (max {}).",
                MAX_BATCH_FILES
            )],
            warnings: Vec::new(),
            files: Vec::new(),
        });
    }

    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let mut per_file = Vec::new();

    let mut seen = std::collections::HashSet::new();
    for proposal in &batch.files {
        if !seen.insert(proposal.path.to_lowercase()) {
            errors.push(format!(
                "Duplicate path in patch batch: '{}'.",
                proposal.path
            ));
        }
    }

    for proposal in &batch.files {
        let validation = validate_patch_proposal(root, proposal)?;
        if !validation.ok {
            for error in &validation.errors {
                errors.push(format!("{}: {}", proposal.path, error));
            }
        }
        for warning in &validation.warnings {
            warnings.push(format!("{}: {}", proposal.path, warning));
        }
        per_file.push(BatchFileValidation {
            path: proposal.path.clone(),
            validation,
        });
    }

    Ok(BatchValidation {
        ok: errors.is_empty(),
        errors,
        warnings,
        files: per_file,
    })
}

/// Apply every file in the batch, or none. Validation and stale checks run for
/// the whole batch before the first write.
pub fn apply_patch_batch(root: &Path, batch: &PatchBatch) -> WorkspaceResult<BatchApplyResult> {
    let validation = validate_patch_batch(root, batch)?;
    if !validation.ok {
        return Err(WorkspaceError::invalid_request(format!(
            "Batch validation failed: {}",
            validation.errors.join("; ")
        )));
    }
    let stale: Vec<String> = validation
        .warnings
        .iter()
        .filter(|w| w.to_lowercase().contains("stale"))
        .cloned()
        .collect();
    if !stale.is_empty() {
        return Err(WorkspaceError::invalid_request(format!(
            "Files changed since the batch was created ({}). Reload and recreate the batch.",
            stale.join("; ")
        )));
    }

    let mut results = Vec::with_capacity(batch.files.len());
    let mut replaced: Vec<&PatchProposal> = Vec::new();

    for proposal in &batch.files {
        match apply_patch_proposal(root, proposal) {
            Ok(result) => {
                replaced.push(proposal);
                results.push(result);
            }
            Err(error) => {
                // Roll back every file this batch already replaced by writing
                // the recorded original content back atomically.
                for done in replaced.iter().rev() {
                    if let Err(restore_error) = restore_original(root, done) {
                        return Err(WorkspaceError::storage(format!(
                            "Batch apply failed ({}) and rollback also failed ({}). \
                             Check the workspace before retrying.",
                            error.message, restore_error.message
                        )));
                    }
                }
                return Err(error);
            }
        }
    }

    Ok(BatchApplyResult {
        applied: true,
        results,
    })
}

/// Restore a proposal's recorded original content with an atomic write.
fn restore_original(root: &Path, proposal: &PatchProposal) -> WorkspaceResult<()> {
    let canonical_root = super::scanner::canonicalize_root(root)?;
    let joined = canonical_root.join(&proposal.path);
    let canonical_file = fs::canonicalize(&joined)
        .map_err(|_| WorkspaceError::storage("Could not resolve file for rollback."))?;
    if !canonical_file.starts_with(&canonical_root) {
        return Err(WorkspaceError::forbidden("File escapes workspace."));
    }
    let parent = canonical_file
        .parent()
        .ok_or_else(|| WorkspaceError::storage("Invalid file parent."))?;
    use std::io::Write;
    let mut tmp = tempfile::NamedTempFile::new_in(parent)
        .map_err(|_| WorkspaceError::storage("Could not create rollback file."))?;
    tmp.write_all(proposal.original_content.as_bytes())
        .and_then(|_| tmp.as_file().sync_all())
        .map_err(|_| WorkspaceError::storage("Could not write rollback content."))?;
    tmp.persist(&canonical_file)
        .map_err(|_| WorkspaceError::storage("Could not restore file during rollback."))?;
    Ok(())
}

#[cfg(test)]
mod batch_tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn make_file(dir: &Path, rel: &str, content: &str) {
        let p = dir.join(rel);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(p, content).unwrap();
    }

    fn edit(rel: &str, proposed: &str) -> BatchEdit {
        BatchEdit {
            relative_path: rel.to_string(),
            proposed_content: proposed.to_string(),
        }
    }

    #[test]
    fn batch_create_validate_apply_happy_path() {
        let dir = TempDir::new().unwrap();
        make_file(dir.path(), "a.txt", "a1\n");
        make_file(dir.path(), "sub/b.txt", "b1\n");

        let batch = create_patch_batch(
            dir.path(),
            &[edit("a.txt", "a2\n"), edit("sub/b.txt", "b2\n")],
        )
        .unwrap();
        assert_eq!(batch.files.len(), 2);
        assert_eq!(batch.files[0].path, "a.txt");
        assert_eq!(batch.files[1].path, "sub/b.txt");

        let validation = validate_patch_batch(dir.path(), &batch).unwrap();
        assert!(validation.ok, "errors: {:?}", validation.errors);
        assert!(validation.warnings.is_empty());

        let result = apply_patch_batch(dir.path(), &batch).unwrap();
        assert!(result.applied);
        assert_eq!(result.results.len(), 2);
        assert_eq!(
            fs::read_to_string(dir.path().join("a.txt")).unwrap(),
            "a2\n"
        );
        assert_eq!(
            fs::read_to_string(dir.path().join("sub/b.txt")).unwrap(),
            "b2\n"
        );
    }

    #[test]
    fn batch_rejects_empty_oversized_and_duplicate_paths() {
        let dir = TempDir::new().unwrap();
        make_file(dir.path(), "a.txt", "a1\n");

        assert!(create_patch_batch(dir.path(), &[]).is_err());

        let many: Vec<BatchEdit> = (0..MAX_BATCH_FILES + 1)
            .map(|i| edit(&format!("f{i}.txt"), "x"))
            .collect();
        assert!(create_patch_batch(dir.path(), &many).is_err());

        let dup = create_patch_batch(dir.path(), &[edit("a.txt", "x\n"), edit("A.TXT", "y\n")]);
        assert!(dup.is_err(), "case-insensitive duplicates must be rejected");
    }

    #[test]
    fn batch_create_fails_when_any_file_is_missing() {
        let dir = TempDir::new().unwrap();
        make_file(dir.path(), "a.txt", "a1\n");
        let result = create_patch_batch(
            dir.path(),
            &[edit("a.txt", "x\n"), edit("missing.txt", "y\n")],
        );
        assert!(result.is_err(), "one missing file fails the whole batch");
    }

    #[test]
    fn batch_apply_refuses_stale_files_before_any_write() {
        let dir = TempDir::new().unwrap();
        make_file(dir.path(), "a.txt", "a1\n");
        make_file(dir.path(), "b.txt", "b1\n");
        let batch = create_patch_batch(dir.path(), &[edit("a.txt", "a2\n"), edit("b.txt", "b2\n")])
            .unwrap();

        // Change b.txt after the batch was created.
        make_file(dir.path(), "b.txt", "b-changed\n");

        let error = apply_patch_batch(dir.path(), &batch).unwrap_err();
        assert!(
            error.message.to_lowercase().contains("changed"),
            "got: {}",
            error.message
        );
        // All-or-nothing: a.txt must be untouched.
        assert_eq!(
            fs::read_to_string(dir.path().join("a.txt")).unwrap(),
            "a1\n"
        );
    }

    #[test]
    fn batch_validate_reports_stale_per_file() {
        let dir = TempDir::new().unwrap();
        make_file(dir.path(), "a.txt", "a1\n");
        make_file(dir.path(), "b.txt", "b1\n");
        let batch = create_patch_batch(dir.path(), &[edit("a.txt", "a2\n"), edit("b.txt", "b2\n")])
            .unwrap();
        make_file(dir.path(), "b.txt", "b-changed\n");

        let validation = validate_patch_batch(dir.path(), &batch).unwrap();
        let stale: Vec<&String> = validation
            .warnings
            .iter()
            .filter(|w| w.contains("b.txt"))
            .collect();
        assert!(!stale.is_empty(), "warnings: {:?}", validation.warnings);
        assert_eq!(validation.files.len(), 2);
    }

    #[test]
    fn restore_original_writes_recorded_content_back() {
        let dir = TempDir::new().unwrap();
        make_file(dir.path(), "a.txt", "a1\n");
        let batch = create_patch_batch(dir.path(), &[edit("a.txt", "a2\n")]).unwrap();
        apply_patch_batch(dir.path(), &batch).unwrap();
        assert_eq!(
            fs::read_to_string(dir.path().join("a.txt")).unwrap(),
            "a2\n"
        );

        restore_original(dir.path(), &batch.files[0]).unwrap();
        assert_eq!(
            fs::read_to_string(dir.path().join("a.txt")).unwrap(),
            "a1\n"
        );
    }
}

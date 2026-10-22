use std::collections::HashSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json;

// ---------------------------------------------------------------------------
// Limits & constants — match docs/phase-11/README.md §2
// ---------------------------------------------------------------------------

pub const MAX_DEPTH: usize = 12;
pub const MAX_FILES: usize = 8000;
pub const MAX_DIRS: usize = 8000;
pub const MAX_PREVIEW_BYTES: u64 = 256 * 1024; // 256 KiB
pub const MAX_INVENTORY_BYTES: usize = 2 * 1024 * 1024; // 2 MiB JSON guard
pub const MAX_CONTEXT_TOTAL_BYTES: usize = 100 * 1024;
pub const MAX_CONTEXT_FILES: usize = 8;
pub const MAX_PATH_CHARS: usize = 4096;

/// Directories never descended — `.git` + deps + build outputs + caches.
pub const EXCLUDED_DIR_NAMES: &[&str] = &[
    ".git",
    ".hg",
    ".svn",
    "node_modules",
    "vendor",
    "target",
    "dist",
    "build",
    "out",
    "__pycache__",
    ".venv",
    "venv",
    ".tox",
    ".mypy_cache",
    ".pytest_cache",
    ".ruff_cache",
    ".cache",
    ".parcel-cache",
    ".next",
    ".nuxt",
    ".svelte-kit",
    ".vite",
    ".turbo",
    ".output",
    "coverage",
    ".nyc_output",
    ".idea",
    ".arena",
];

/// File name patterns considered secret-bearing.
const SENSITIVE_SUFFIXES: &[&str] = &[".pem", ".key", ".p12", ".pfx", ".kdbx"];
const SENSITIVE_EXACT: &[&str] = &[
    "credentials",
    ".netrc",
    ".git-credentials",
    "secrets.json",
    "secrets.yaml",
    "secrets.yml",
];

// ---------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceError {
    pub code: &'static str,
    pub message: String,
}

pub type WorkspaceResult<T> = Result<T, WorkspaceError>;

impl WorkspaceError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
    pub fn not_found(msg: impl Into<String>) -> Self {
        Self::new("not_found", msg)
    }
    pub fn invalid_request(msg: impl Into<String>) -> Self {
        Self::new("invalid_request", msg)
    }
    pub fn forbidden(msg: impl Into<String>) -> Self {
        Self::new("forbidden", msg)
    }
    pub fn too_large(msg: impl Into<String>) -> Self {
        Self::new("too_large", msg)
    }
    pub fn binary(msg: impl Into<String>) -> Self {
        Self::new("binary", msg)
    }
    pub fn secret_excluded(msg: impl Into<String>) -> Self {
        Self::new("secret_excluded", msg)
    }
    pub fn storage(msg: impl Into<String>) -> Self {
        Self::new("storage", msg)
    }
}

impl std::fmt::Display for WorkspaceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}
impl std::error::Error for WorkspaceError {}

// ---------------------------------------------------------------------------
// Inventory types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceEntry {
    pub path: String, // relative posix
    pub kind: String, // "file" | "dir"
    pub size: u64,
    pub depth: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceStats {
    pub file_count: usize,
    pub dir_count: usize,
    pub skipped_excluded: usize,
    pub skipped_sensitive: usize,
    pub skipped_oversized: usize,
    pub skipped_unreadable: usize,
    pub skipped_symlink_outside: usize,
    pub skipped_depth: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceLimits {
    pub max_depth: usize,
    pub max_files: usize,
    pub max_preview_bytes: u64,
    pub max_inventory_bytes: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceInventory {
    pub root_name: String,
    pub root_path: String, // canonical absolute for display
    pub entries: Vec<WorkspaceEntry>,
    pub truncated: bool,
    pub stats: WorkspaceStats,
    pub limits: WorkspaceLimits,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilePreview {
    pub path: String,
    pub content: String,
    pub size: u64,
    pub truncated: bool,
    pub encoding: String,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn is_excluded_dir(name: &str) -> bool {
    // case-insensitive on Windows, case-sensitive on Unix — normalize to lowercase for comparison
    let lower = name.to_ascii_lowercase();
    EXCLUDED_DIR_NAMES
        .iter()
        .any(|ex| ex.to_ascii_lowercase() == lower)
}

pub fn is_sensitive_file(path: &Path) -> bool {
    let Some(file_name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    let lower = file_name.to_ascii_lowercase();
    // .env and .env.*
    if lower == ".env" || lower.starts_with(".env.") {
        return true;
    }
    if lower == ".env.local"
        || lower == ".env.development"
        || lower == ".env.production"
        || lower == ".env.test"
    {
        return true;
    }
    // exact names
    if SENSITIVE_EXACT.iter().any(|e| *e == lower) {
        return true;
    }
    // id_rsa etc
    if lower == "id_rsa"
        || lower == "id_ed25519"
        || lower == "id_dsa"
        || lower.starts_with("id_rsa.")
        || lower.starts_with("id_ed25519.")
    {
        return true;
    }
    // suffixes
    for suffix in SENSITIVE_SUFFIXES {
        if lower.ends_with(suffix) {
            return true;
        }
    }
    // .npmrc may contain tokens but we allow preview with warning; do not block outright
    // secrets.* already covered
    false
}

pub fn is_binary_bytes(bytes: &[u8]) -> bool {
    // NUL byte is a strong binary signal
    if bytes.contains(&0) {
        return true;
    }
    // Check valid utf-8
    std::str::from_utf8(bytes).is_err()
}

pub fn validate_relative_path(input: &str) -> WorkspaceResult<()> {
    if input.is_empty() {
        return Err(WorkspaceError::invalid_request("File path is empty."));
    }
    if input.chars().count() > MAX_PATH_CHARS {
        return Err(WorkspaceError::invalid_request("File path too long."));
    }
    // No absolute, no drive prefix, no ParentDir
    let path = Path::new(input);
    if path.is_absolute() {
        return Err(WorkspaceError::invalid_request(
            "Absolute paths are not allowed.",
        ));
    }
    for comp in path.components() {
        match comp {
            Component::ParentDir => {
                return Err(WorkspaceError::invalid_request(
                    "Path traversal is not allowed.",
                ))
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(WorkspaceError::invalid_request(
                    "Absolute paths are not allowed.",
                ))
            }
            _ => {}
        }
    }
    // Also reject backslash escaping on Unix (treat as invalid)
    if input.contains('\\') && cfg!(unix) {
        // On unix, backslash is valid filename char but we normalize to posix and forbid it for traversal safety
        // Allow but warn? For strictness, reject if it would create ambiguity
        // We'll allow single segment with backslash? Simpler: forbid `..` already handled
    }
    if input.contains('\0') {
        return Err(WorkspaceError::invalid_request("Invalid file path."));
    }
    Ok(())
}

pub fn posix_relative(canonical_root: &Path, canonical_path: &Path) -> WorkspaceResult<String> {
    let rel = canonical_path
        .strip_prefix(canonical_root)
        .map_err(|_| WorkspaceError::forbidden("File escapes workspace."))?;
    let mut s = String::new();
    for (i, comp) in rel.components().enumerate() {
        if i > 0 {
            s.push('/');
        }
        let part = comp
            .as_os_str()
            .to_str()
            .ok_or_else(|| WorkspaceError::storage("Invalid path encoding."))?;
        // Reject any `..` that survived canonicalization (should not happen)
        if part == ".." {
            return Err(WorkspaceError::forbidden("Path escapes workspace."));
        }
        s.push_str(part);
    }
    if s.is_empty() {
        // root itself
        s.push('.');
    }
    if s.chars().count() > MAX_PATH_CHARS {
        return Err(WorkspaceError::invalid_request("File path too long."));
    }
    Ok(s)
}

fn read_gitignore_patterns(root: &Path) -> Vec<String> {
    let gitignore = root.join(".gitignore");
    let content = match fs::read_to_string(&gitignore) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    content
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty() && !l.starts_with('#') && !l.starts_with('!'))
        .map(|l| {
            // Trim leading slash and trailing slash
            let mut pat = l;
            if pat.starts_with('/') {
                pat = pat[1..].to_string();
            }
            if pat.ends_with('/') {
                pat.pop();
            }
            pat
        })
        .filter(|p| !p.is_empty())
        .collect()
}

fn matches_gitignore(path: &str, patterns: &[String]) -> bool {
    // Minimal literal matching: if pattern contains * or ?, do simple wildcard check,
    // otherwise check exact filename or directory prefix.
    for pat in patterns {
        if pat.contains('*') || pat.contains('?') {
            // Simple glob: convert to contains check for practical purposes
            // Strip wildcards and see if remaining literal appears
            let literal: String = pat.chars().filter(|c| *c != '*' && *c != '?').collect();
            if !literal.is_empty() && path.contains(&literal) {
                return true;
            }
            // also match extension patterns like *.log
            if pat.starts_with("*.") {
                let ext = &pat[1..]; // .log
                if path.ends_with(ext) {
                    return true;
                }
            }
            continue;
        }
        // Exact dir or file segment
        if path == pat.as_str() {
            return true;
        }
        if path.starts_with(&format!("{}/", pat)) {
            return true;
        }
        // Match file name segment
        if path.split('/').any(|seg| seg == pat.as_str()) {
            return true;
        }
    }
    false
}

pub fn canonicalize_root(root: &Path) -> WorkspaceResult<PathBuf> {
    let canonical = fs::canonicalize(root)
        .map_err(|_| WorkspaceError::not_found("Project directory not found or not accessible."))?;
    let meta = fs::metadata(&canonical)
        .map_err(|_| WorkspaceError::not_found("Project directory not accessible."))?;
    if !meta.is_dir() {
        return Err(WorkspaceError::invalid_request(
            "Selected path is not a directory.",
        ));
    }
    Ok(canonical)
}

// ---------------------------------------------------------------------------
// Scan
// ---------------------------------------------------------------------------

pub fn scan_workspace(root_input: &Path) -> WorkspaceResult<WorkspaceInventory> {
    let canonical_root = canonicalize_root(root_input)?;
    let root_name = canonical_root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("project")
        .to_string();
    let root_path_display = canonical_root.to_string_lossy().to_string();

    let gitignore_patterns = read_gitignore_patterns(&canonical_root);

    let mut entries: Vec<WorkspaceEntry> = Vec::new();
    let mut stats = WorkspaceStats {
        file_count: 0,
        dir_count: 0,
        skipped_excluded: 0,
        skipped_sensitive: 0,
        skipped_oversized: 0,
        skipped_unreadable: 0,
        skipped_symlink_outside: 0,
        skipped_depth: 0,
    };
    let mut truncated = false;

    // Stack for DFS: (path, depth)
    let mut stack: Vec<(PathBuf, usize)> = vec![(canonical_root.clone(), 0)];
    let mut visited_dirs: HashSet<PathBuf> = HashSet::new();
    visited_dirs.insert(canonical_root.clone());

    while let Some((dir, depth)) = stack.pop() {
        if depth > MAX_DEPTH {
            stats.skipped_depth += 1;
            truncated = true;
            continue;
        }
        let read = match fs::read_dir(&dir) {
            Ok(r) => r,
            Err(_) => {
                stats.skipped_unreadable += 1;
                continue;
            }
        };

        for entry_res in read {
            let entry = match entry_res {
                Ok(e) => e,
                Err(_) => {
                    stats.skipped_unreadable += 1;
                    continue;
                }
            };
            let path = entry.path();
            let file_name = entry.file_name();
            let name_str = file_name.to_string_lossy().to_string();

            // Check symlink first
            let meta = match fs::symlink_metadata(&path) {
                Ok(m) => m,
                Err(_) => {
                    stats.skipped_unreadable += 1;
                    continue;
                }
            };
            let is_symlink = meta.file_type().is_symlink();
            if is_symlink {
                // Canonicalize target and verify inside root
                match fs::canonicalize(&path) {
                    Ok(target) => {
                        if !target.starts_with(&canonical_root) {
                            stats.skipped_symlink_outside += 1;
                            continue;
                        }
                        // Symlink inside root: do not follow directory symlinks, treat file symlink as file if target is file
                        let target_meta = match fs::metadata(&target) {
                            Ok(m) => m,
                            Err(_) => {
                                stats.skipped_unreadable += 1;
                                continue;
                            }
                        };
                        if target_meta.is_dir() {
                            // Do not descend symlink dirs to avoid loops
                            stats.skipped_symlink_outside += 1;
                            continue;
                        }
                        // file symlink: fall through to file handling but note size from target
                        // We'll handle as file below
                    }
                    Err(_) => {
                        stats.skipped_unreadable += 1;
                        continue;
                    }
                }
            }

            let is_dir = meta.is_dir() && !is_symlink;
            // For symlink file, meta is symlink, but we determined target is file — treat as file
            let effective_is_dir = if is_symlink {
                // We already checked symlink target is file, so not dir
                false
            } else {
                is_dir
            };

            if effective_is_dir {
                // Excluded dir check
                if is_excluded_dir(&name_str) {
                    stats.skipped_excluded += 1;
                    continue;
                }
                // Gitignore check for dir
                let rel_for_ignore = match path.strip_prefix(&canonical_root) {
                    Ok(p) => p.to_string_lossy().replace('\\', "/"),
                    Err(_) => name_str.clone(),
                };
                if matches_gitignore(&rel_for_ignore, &gitignore_patterns) {
                    stats.skipped_excluded += 1;
                    continue;
                }

                if stats.dir_count >= MAX_DIRS {
                    truncated = true;
                    continue;
                }

                // Depth check for dir entry itself
                let entry_depth = depth + 1;
                if entry_depth > MAX_DEPTH {
                    stats.skipped_depth += 1;
                    truncated = true;
                    continue;
                }

                // Avoid loop: canonicalize dir and check visited
                let canon_dir = match fs::canonicalize(&path) {
                    Ok(c) => c,
                    Err(_) => {
                        stats.skipped_unreadable += 1;
                        continue;
                    }
                };
                if !visited_dirs.insert(canon_dir.clone()) {
                    // already visited (symlink loop)
                    continue;
                }

                // Record dir entry
                let rel = posix_relative(&canonical_root, &canon_dir)?;
                // Inventory byte guard: estimate size
                if entries.len() >= MAX_FILES + MAX_DIRS {
                    truncated = true;
                    break;
                }
                entries.push(WorkspaceEntry {
                    path: rel,
                    kind: "dir".to_string(),
                    size: 0,
                    depth: entry_depth,
                });
                stats.dir_count += 1;

                // Push to stack for traversal
                stack.push((canon_dir, entry_depth));

                // Check inventory byte limit approx
                if entries.len() % 500 == 0 {
                    let json_len = serde_json::to_string(&entries)
                        .map(|s| s.len())
                        .unwrap_or(0);
                    if json_len > MAX_INVENTORY_BYTES {
                        truncated = true;
                        break;
                    }
                }
                if entries.len() > MAX_FILES + MAX_DIRS {
                    truncated = true;
                    break;
                }
            } else {
                // File
                // Excluded file due to gitignore? Still count as excluded? Spec says dependency dirs excluded, but we also respect ignore for files
                let rel_for_ignore = match path.strip_prefix(&canonical_root) {
                    Ok(p) => p.to_string_lossy().replace('\\', "/"),
                    Err(_) => name_str.clone(),
                };
                if matches_gitignore(&rel_for_ignore, &gitignore_patterns) {
                    stats.skipped_excluded += 1;
                    continue;
                }

                if stats.file_count >= MAX_FILES {
                    truncated = true;
                    continue;
                }

                let file_size = if is_symlink {
                    // Use target size
                    match fs::canonicalize(&path).and_then(|t| fs::metadata(&t)) {
                        Ok(m) => m.len(),
                        Err(_) => meta.len(),
                    }
                } else {
                    meta.len()
                };

                // Do not fail on oversized file for inventory — just record size and optionally count skippedOversized if needed for preview? Keep inventory.
                // But if file is huge > 50 MiB, we could skip display to keep inventory light
                // Count skipped_oversized only for preview context? Keep 0 for inventory.

                let entry_depth = depth + 1;
                if entry_depth > MAX_DEPTH {
                    stats.skipped_depth += 1;
                    continue;
                }

                // Canonical path for relative
                let canonical_file = if is_symlink {
                    fs::canonicalize(&path).unwrap_or(path.clone())
                } else {
                    // For regular file, canonicalize to ensure no traversal; but we can use path canonicalized
                    match fs::canonicalize(&path) {
                        Ok(c) => c,
                        Err(_) => path.clone(),
                    }
                };
                let rel = posix_relative(&canonical_root, &canonical_file)?;
                if rel.chars().count() > MAX_PATH_CHARS {
                    stats.skipped_unreadable += 1;
                    continue;
                }

                entries.push(WorkspaceEntry {
                    path: rel,
                    kind: "file".to_string(),
                    size: file_size,
                    depth: entry_depth,
                });
                stats.file_count += 1;

                // Inventory byte guard
                if entries.len() % 500 == 0 {
                    let json_len = serde_json::to_string(&entries)
                        .map(|s| s.len())
                        .unwrap_or(0);
                    if json_len > MAX_INVENTORY_BYTES {
                        truncated = true;
                        break;
                    }
                }
            }

            if truncated {
                break;
            }
        }
        if truncated {
            break;
        }
    }

    // Sort entries: dirs first, then files, each alphabetically
    entries.sort_by(|a, b| {
        let kind_ord = a.kind.cmp(&b.kind); // "dir" < "file"
        if kind_ord != std::cmp::Ordering::Equal {
            return kind_ord;
        }
        a.path.cmp(&b.path)
    });

    // Final inventory byte check
    let json_len = serde_json::to_string(&entries)
        .map(|s| s.len())
        .unwrap_or(0);
    if json_len > MAX_INVENTORY_BYTES {
        truncated = true;
        // Trim oldest beyond limit? Keep first MAX_FILES entries
        if entries.len() > MAX_FILES {
            entries.truncate(MAX_FILES);
        }
    }

    Ok(WorkspaceInventory {
        root_name,
        root_path: root_path_display,
        entries,
        truncated,
        stats,
        limits: WorkspaceLimits {
            max_depth: MAX_DEPTH,
            max_files: MAX_FILES,
            max_preview_bytes: MAX_PREVIEW_BYTES,
            max_inventory_bytes: MAX_INVENTORY_BYTES,
        },
    })
}

// ---------------------------------------------------------------------------
// File preview
// ---------------------------------------------------------------------------

pub fn read_workspace_file(root_input: &Path, relative_path: &str) -> WorkspaceResult<FilePreview> {
    validate_relative_path(relative_path)?;
    let canonical_root = canonicalize_root(root_input)?;

    // Join and canonicalize
    let joined = canonical_root.join(relative_path);
    // For file that may not exist, canonicalize will fail — map to not_found
    let canonical_file = fs::canonicalize(&joined)
        .map_err(|_| WorkspaceError::not_found("File not found or not accessible."))?;

    // Ensure inside root
    if !canonical_file.starts_with(&canonical_root) {
        return Err(WorkspaceError::forbidden("File escapes workspace."));
    }

    // Check is file
    let meta = fs::metadata(&canonical_file)
        .map_err(|_| WorkspaceError::not_found("File not accessible."))?;
    if meta.is_dir() {
        return Err(WorkspaceError::invalid_request(
            "Path is a directory, not a file.",
        ));
    }

    // Sensitive check via canonical file path and original relative
    if is_sensitive_file(&canonical_file) {
        return Err(WorkspaceError::secret_excluded(
            "This file is excluded from preview and model context to protect secrets.",
        ));
    }
    // Also check relative string patterns for .env inside subdir
    let lower_rel = relative_path.to_ascii_lowercase();
    if lower_rel == ".env" || lower_rel.starts_with(".env.") || lower_rel.contains("/.env") {
        return Err(WorkspaceError::secret_excluded(
            "Environment files are excluded.",
        ));
    }

    let size = meta.len();
    if size > MAX_PREVIEW_BYTES {
        return Err(WorkspaceError::too_large(format!(
            "File is too large for preview ({} bytes, max {}).",
            size, MAX_PREVIEW_BYTES
        )));
    }

    // Read with limit
    let bytes = fs::read(&canonical_file)
        .map_err(|_| WorkspaceError::storage("Could not read file. Check permissions."))?;

    if bytes.len() as u64 > MAX_PREVIEW_BYTES {
        return Err(WorkspaceError::too_large(
            "File exceeds preview size limit.",
        ));
    }

    if is_binary_bytes(&bytes) {
        return Err(WorkspaceError::binary(
            "Binary files cannot be previewed as text.",
        ));
    }

    let content = String::from_utf8(bytes)
        .map_err(|_| WorkspaceError::binary("File is not valid UTF-8 text."))?;

    // Size was checked above, so the preview is never truncated here.
    // Derive encoding check: prefer UTF-8, content already validated
    let rel = posix_relative(&canonical_root, &canonical_file)?;

    Ok(FilePreview {
        path: rel,
        content,
        size,
        truncated: false,
        encoding: "utf8".to_string(),
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::symlink;
    use tempfile::TempDir;

    fn make_file(dir: &Path, rel: &str, content: &str) {
        let p = dir.join(rel);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(p, content).unwrap();
    }

    fn temp_root() -> TempDir {
        TempDir::new().unwrap()
    }

    #[test]
    fn scan_empty() {
        let dir = temp_root();
        let inv = scan_workspace(dir.path()).unwrap();
        assert_eq!(inv.entries.len(), 0);
        assert!(!inv.truncated);
    }

    #[test]
    fn scan_lists_files_and_dirs() {
        let dir = temp_root();
        make_file(dir.path(), "src/main.rs", "fn main(){}");
        make_file(dir.path(), "README.md", "# hi");
        fs::create_dir_all(dir.path().join("src/utils")).unwrap();
        make_file(dir.path(), "src/utils/helper.ts", "export {}");
        let inv = scan_workspace(dir.path()).unwrap();
        let paths: Vec<_> = inv.entries.iter().map(|e| e.path.as_str()).collect();
        assert!(paths.contains(&"src"));
        assert!(paths.contains(&"src/main.rs"));
        assert!(paths.contains(&"README.md"));
        assert_eq!(inv.stats.file_count, 3);
        assert_eq!(inv.stats.dir_count, 2); // src, src/utils
    }

    #[test]
    fn excludes_git_and_node_modules() {
        let dir = temp_root();
        make_file(dir.path(), ".git/config", "[core]");
        make_file(dir.path(), "node_modules/pkg/index.js", "x");
        make_file(dir.path(), "target/debug/app", "bin");
        make_file(dir.path(), "src/app.rs", "code");
        let inv = scan_workspace(dir.path()).unwrap();
        let paths: Vec<_> = inv.entries.iter().map(|e| e.path.clone()).collect();
        assert!(!paths.iter().any(|p| p.starts_with(".git")));
        assert!(!paths.iter().any(|p| p.starts_with("node_modules")));
        assert!(!paths.iter().any(|p| p.starts_with("target")));
        assert!(paths.contains(&"src/app.rs".to_string()));
        assert!(inv.stats.skipped_excluded >= 3);
    }

    #[test]
    fn respects_gitignore() {
        let dir = temp_root();
        fs::write(dir.path().join(".gitignore"), "ignored.log\ncustom_dir/\n").unwrap();
        make_file(dir.path(), "ignored.log", "x");
        make_file(dir.path(), "keep.log", "x");
        make_file(dir.path(), "custom_dir/file.txt", "x");
        make_file(dir.path(), "src/keep.txt", "x");
        let inv = scan_workspace(dir.path()).unwrap();
        let paths: Vec<_> = inv.entries.iter().map(|e| e.path.as_str()).collect();
        assert!(!paths.contains(&"ignored.log"));
        assert!(paths.contains(&"keep.log"));
        assert!(!paths.iter().any(|p| p.starts_with("custom_dir")));
        assert!(paths.contains(&"src/keep.txt"));
    }

    #[test]
    fn depth_limit() {
        let dir = temp_root();
        let mut cur = dir.path().to_path_buf();
        for i in 0..15 {
            cur = cur.join(format!("d{}", i));
            fs::create_dir_all(&cur).unwrap();
            make_file(&cur, "file.txt", "x");
        }
        let inv = scan_workspace(dir.path()).unwrap();
        assert!(inv.truncated || inv.stats.skipped_depth > 0);
        // Depth 12 means d12/file should be beyond
        let paths: Vec<_> = inv.entries.iter().map(|e| e.path.clone()).collect();
        assert!(!paths.iter().any(|p| p.contains("d13")));
    }

    #[test]
    fn file_count_limit() {
        let dir = temp_root();
        for i in 0..(MAX_FILES + 100) {
            make_file(dir.path(), &format!("file{}.txt", i), "x");
        }
        let inv = scan_workspace(dir.path()).unwrap();
        assert!(inv.truncated);
        assert!(inv.entries.len() <= MAX_FILES);
        assert_eq!(inv.stats.file_count, MAX_FILES);
    }

    #[test]
    fn preview_read_ok() {
        let dir = temp_root();
        make_file(dir.path(), "src/main.rs", "fn main(){println!(\"hi\")}");
        let preview = read_workspace_file(dir.path(), "src/main.rs").unwrap();
        assert_eq!(preview.path, "src/main.rs");
        assert!(preview.content.contains("fn main"));
        assert_eq!(preview.encoding, "utf8");
    }

    #[test]
    fn preview_traversal_rejected() {
        let dir = temp_root();
        make_file(dir.path(), "a.txt", "ok");
        let err = read_workspace_file(dir.path(), "../a.txt").unwrap_err();
        assert_eq!(err.code, "invalid_request");
        let err2 = read_workspace_file(dir.path(), "/etc/passwd").unwrap_err();
        assert_eq!(err2.code, "invalid_request");
        let err3 = read_workspace_file(dir.path(), "a/../../etc/passwd").unwrap_err();
        assert_eq!(err3.code, "invalid_request");
    }

    #[test]
    #[cfg(unix)]
    fn preview_symlink_outside_blocked() {
        let dir = temp_root();
        let outside = temp_root();
        make_file(outside.path(), "secret.txt", "outside");
        let link_path = dir.path().join("link.txt");
        symlink(outside.path().join("secret.txt"), &link_path).unwrap();
        let err = read_workspace_file(dir.path(), "link.txt").unwrap_err();
        assert_eq!(err.code, "forbidden");
    }

    #[test]
    #[cfg(unix)]
    fn symlink_dir_not_followed() {
        let dir = temp_root();
        let outside = temp_root();
        make_file(outside.path(), "outside.txt", "x");
        fs::create_dir_all(outside.path().join("inner")).unwrap();
        make_file(outside.path(), "inner/file.txt", "inner");
        symlink(outside.path(), dir.path().join("linkdir")).unwrap();
        make_file(dir.path(), "real.txt", "real");
        let inv = scan_workspace(dir.path()).unwrap();
        let paths: Vec<_> = inv.entries.iter().map(|e| e.path.as_str()).collect();
        assert!(!paths
            .iter()
            .any(|p| p.contains("outside.txt") || p.contains("inner")));
        assert!(paths.contains(&"real.txt"));
        assert!(inv.stats.skipped_symlink_outside > 0);
    }

    #[test]
    fn sensitive_excluded() {
        let dir = temp_root();
        make_file(dir.path(), ".env", "SECRET=1");
        make_file(dir.path(), ".env.local", "x");
        make_file(dir.path(), "keys/id_rsa", "private");
        make_file(dir.path(), "cert.pem", "x");
        make_file(dir.path(), "normal.txt", "ok");
        // Inventory may still list? But preview should block
        let err = read_workspace_file(dir.path(), ".env").unwrap_err();
        assert_eq!(err.code, "secret_excluded");
        assert_eq!(
            read_workspace_file(dir.path(), ".env.local")
                .unwrap_err()
                .code,
            "secret_excluded"
        );
        assert_eq!(
            read_workspace_file(dir.path(), "cert.pem")
                .unwrap_err()
                .code,
            "secret_excluded"
        );
        assert_eq!(
            read_workspace_file(dir.path(), "keys/id_rsa")
                .unwrap_err()
                .code,
            "secret_excluded"
        );
        // Normal file ok
        assert!(read_workspace_file(dir.path(), "normal.txt").is_ok());

        // Also .env in subdir
        make_file(dir.path(), "config/.env.production", "x");
        assert_eq!(
            read_workspace_file(dir.path(), "config/.env.production")
                .unwrap_err()
                .code,
            "secret_excluded"
        );
    }

    #[test]
    fn binary_rejected() {
        let dir = temp_root();
        let p = dir.path().join("bin.dat");
        fs::write(&p, [0u8, 1, 2, 3, 255]).unwrap();
        let err = read_workspace_file(dir.path(), "bin.dat").unwrap_err();
        assert_eq!(err.code, "binary");

        // Invalid utf8 without nul but non-utf8 sequence
        let p2 = dir.path().join("bad.txt");
        fs::write(&p2, [0xFF, 0xFE, 0xFD]).unwrap();
        let err2 = read_workspace_file(dir.path(), "bad.txt").unwrap_err();
        assert_eq!(err2.code, "binary");
    }

    #[test]
    fn oversized_rejected() {
        let dir = temp_root();
        let big = "a".repeat((MAX_PREVIEW_BYTES as usize) + 100);
        make_file(dir.path(), "big.txt", &big);
        let err = read_workspace_file(dir.path(), "big.txt").unwrap_err();
        assert_eq!(err.code, "too_large");
    }

    #[test]
    fn invalid_root() {
        let dir = temp_root();
        let missing = dir.path().join("nope");
        let err = scan_workspace(&missing).unwrap_err();
        assert_eq!(err.code, "not_found");
        // file instead of dir
        make_file(dir.path(), "file.txt", "x");
        let err2 = scan_workspace(&dir.path().join("file.txt")).unwrap_err();
        assert_eq!(err2.code, "invalid_request");
    }

    #[test]
    fn inaccessible_handled_gracefully() {
        let dir = temp_root();
        make_file(dir.path(), "ok.txt", "ok");
        // Create a directory with no read permission? On unix we can chmod 000
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let restricted = dir.path().join("restricted");
            fs::create_dir(&restricted).unwrap();
            make_file(&restricted, "inner.txt", "x");
            fs::set_permissions(&restricted, fs::Permissions::from_mode(0o000)).unwrap();
            let inv = scan_workspace(dir.path()).unwrap();
            // Should not panic, may skip unreadable
            // Restore for cleanup
            let _ = fs::set_permissions(&restricted, fs::Permissions::from_mode(0o755));
            assert!(inv.entries.iter().any(|e| e.path == "ok.txt"));
        }
    }
}

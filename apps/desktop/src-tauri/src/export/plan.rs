//! Turning the remembered workspace into a reviewed list of files to export.
//!
//! Planning reuses the workspace scanner, so the export sees exactly the files
//! the Projects page shows: the same build/dependency/cache exclusions, the same
//! `.gitignore` handling, and the same depth and file ceilings. On top of that,
//! planning removes secret-bearing names and symbolic links, which the scanner
//! lists but must never be archived.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use super::zip::MAX_ARCHIVE_BYTES;
use super::{
    is_unsafe_component, validate_entry_name, ExportCandidate, ExportError, ExportPlan,
    ExportResult,
};
use crate::workspace::scanner::{is_sensitive_file, scan_workspace};

/// A planned file resolved to an absolute path for the archive writer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFile {
    pub name: String,
    pub path: PathBuf,
    pub size: u64,
}

/// Hard ceiling on entries in one archive (the ZIP field is 16-bit).
const MAX_ENTRIES: usize = 60_000;

fn too_big(name: &str) -> ExportError {
    ExportError::too_large(format!(
        "{name} is 4 GiB or larger. Nexora writes standard ZIP files without ZIP64, so this file cannot be archived. Export it on its own instead."
    ))
}

/// Builds the reviewable plan for a whole project or an explicit selection.
pub fn build_plan(root: &Path, selection: Option<&[String]>) -> ExportResult<ExportPlan> {
    let canonical_root = fs::canonicalize(root)
        .map_err(|_| ExportError::not_found("The project folder is not available."))?;

    let root_name = canonical_root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("project")
        .to_string();

    match selection {
        None => plan_whole_project(&canonical_root, root_name),
        Some(paths) => plan_selection(&canonical_root, root_name, paths),
    }
}

fn plan_whole_project(root: &Path, root_name: String) -> ExportResult<ExportPlan> {
    let inventory = scan_workspace(root)?;

    if inventory.truncated {
        return Err(ExportError::too_large(
            "This project is larger than Nexora's workspace ceiling (8,000 files or 12 folder levels), so a whole-project archive could be silently incomplete. Select a smaller group of files and export that instead.",
        ));
    }

    let mut files = Vec::new();
    let mut excluded_sensitive = 0usize;
    let mut excluded_symlink = 0usize;
    let mut excluded_unsafe = 0usize;
    let mut total_bytes = 0u64;
    let mut seen: HashSet<String> = HashSet::new();

    for entry in inventory
        .entries
        .iter()
        .filter(|entry| entry.kind == "file")
    {
        let name = entry.path.clone();

        if !seen.insert(name.clone()) {
            continue;
        }

        if is_sensitive_file(Path::new(&name)) {
            excluded_sensitive += 1;
            continue;
        }

        if is_unsafe_component(
            Path::new(&name)
                .file_name()
                .and_then(|part| part.to_str())
                .unwrap_or(""),
        ) || validate_entry_name(&name).is_err()
        {
            excluded_unsafe += 1;
            continue;
        }

        let absolute = root.join(&name);
        let Ok(metadata) = fs::symlink_metadata(&absolute) else {
            excluded_unsafe += 1;
            continue;
        };

        if metadata.file_type().is_symlink() {
            excluded_symlink += 1;
            continue;
        }

        if !metadata.is_file() {
            excluded_unsafe += 1;
            continue;
        }

        if metadata.len() >= MAX_ARCHIVE_BYTES {
            return Err(too_big(&name));
        }

        total_bytes = total_bytes.saturating_add(metadata.len());

        if total_bytes >= MAX_ARCHIVE_BYTES {
            return Err(ExportError::too_large(
                "The project is 4 GiB or larger. Nexora writes standard ZIP files without ZIP64; export a smaller selection.",
            ));
        }

        files.push(ExportCandidate {
            path: name,
            size: metadata.len(),
        });
    }

    if files.is_empty() {
        return Err(ExportError::no_files(
            if excluded_sensitive + excluded_symlink + excluded_unsafe > 0 {
                "Nothing can be exported: every eligible file was withheld as a secret or a symbolic link.".to_string()
            } else {
                "This project has no exportable files. Build output, dependencies and caches are excluded by design.".to_string()
            },
        ));
    }

    Ok(ExportPlan {
        root_name,
        files,
        excluded_sensitive,
        excluded_symlink,
        excluded_unsafe,
        total_bytes,
        truncated: false,
    })
}

fn plan_selection(root: &Path, root_name: String, paths: &[String]) -> ExportResult<ExportPlan> {
    if paths.is_empty() {
        return Err(ExportError::no_files("No files are selected."));
    }

    if paths.len() > MAX_ENTRIES {
        return Err(ExportError::too_large(format!(
            "Too many files selected ({}). The limit is {MAX_ENTRIES}.",
            paths.len()
        )));
    }

    let mut files = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut total_bytes = 0u64;

    for raw in paths {
        let name = raw.trim().to_string();

        validate_entry_name(&name)?;

        if is_sensitive_file(Path::new(&name)) {
            return Err(ExportError::secret_excluded(&name));
        }

        if !seen.insert(name.clone()) {
            continue;
        }

        let absolute = root.join(&name);

        let metadata = fs::symlink_metadata(&absolute)
            .map_err(|_| ExportError::not_found(format!("{name} is missing or not accessible.")))?;

        if metadata.file_type().is_symlink() {
            return Err(ExportError::symlink_excluded(&name));
        }

        if metadata.is_dir() {
            return Err(ExportError::invalid_request(format!(
                "{name} is a folder, not a file."
            )));
        }

        if !metadata.is_file() {
            return Err(ExportError::invalid_request(format!(
                "{name} is not a regular file."
            )));
        }

        let canonical = absolute
            .canonicalize()
            .map_err(|_| ExportError::not_found(format!("{name} is not accessible.")))?;

        if !canonical.starts_with(root) {
            return Err(ExportError::forbidden(format!(
                "{name} resolves outside the project folder."
            )));
        }

        if metadata.len() >= MAX_ARCHIVE_BYTES {
            return Err(too_big(&name));
        }

        total_bytes = total_bytes.saturating_add(metadata.len());

        if total_bytes >= MAX_ARCHIVE_BYTES {
            return Err(ExportError::too_large(
                "The selection is 4 GiB or larger. Nexora writes standard ZIP files without ZIP64.",
            ));
        }

        files.push(ExportCandidate {
            path: name,
            size: metadata.len(),
        });
    }

    if files.is_empty() {
        return Err(ExportError::no_files("No files are selected."));
    }

    Ok(ExportPlan {
        root_name,
        files,
        excluded_sensitive: 0,
        excluded_symlink: 0,
        excluded_unsafe: 0,
        total_bytes,
        truncated: false,
    })
}

/// Re-resolves the plan immediately before writing, so a file replaced by a
/// link between planning and export is still refused.
pub fn resolve_sources(root: &Path, plan: &ExportPlan) -> ExportResult<Vec<SourceFile>> {
    let canonical_root = fs::canonicalize(root)
        .map_err(|_| ExportError::not_found("The project folder is not available."))?;

    let mut sources = Vec::with_capacity(plan.files.len());

    for candidate in &plan.files {
        validate_entry_name(&candidate.path)?;

        if is_sensitive_file(Path::new(&candidate.path)) {
            return Err(ExportError::secret_excluded(&candidate.path));
        }

        let absolute = canonical_root.join(&candidate.path);

        let metadata = fs::symlink_metadata(&absolute).map_err(|_| {
            ExportError::not_found(format!(
                "{} disappeared before it could be exported.",
                candidate.path
            ))
        })?;

        if metadata.file_type().is_symlink() {
            return Err(ExportError::symlink_excluded(&candidate.path));
        }

        if !metadata.is_file() {
            return Err(ExportError::invalid_request(format!(
                "{} is no longer a regular file.",
                candidate.path
            )));
        }

        let canonical = absolute.canonicalize().map_err(|_| {
            ExportError::not_found(format!("{} is not accessible.", candidate.path))
        })?;

        if !canonical.starts_with(&canonical_root) {
            return Err(ExportError::forbidden(format!(
                "{} resolves outside the project folder.",
                candidate.path
            )));
        }

        sources.push(SourceFile {
            name: candidate.path.clone(),
            path: canonical,
            size: metadata.len(),
        });
    }

    Ok(sources)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);

    fn scratch(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "nexora-plan-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Mirrors `build_plan`: the folder is canonicalised before entries are
    /// resolved. Without that, Windows' short 8.3 temporary path and the long
    /// form `canonicalize` returns for the file do not share a prefix.
    fn select(root: &Path, paths: &[&str]) -> ExportResult<ExportPlan> {
        let owned: Vec<String> = paths.iter().map(|path| (*path).to_string()).collect();
        let canonical_root = fs::canonicalize(root)
            .map_err(|_| ExportError::not_found("The project folder is not available."))?;
        plan_selection(&canonical_root, "project".to_string(), &owned)
    }

    #[test]
    fn selection_refuses_secrets_unsafe_names_and_traversal() {
        let dir = scratch("unsafe");
        fs::write(dir.join("keep.txt"), b"ok").unwrap();

        assert_eq!(select(&dir, &[".env"]).unwrap_err().code, "secret_excluded");
        assert_eq!(
            select(&dir, &["../escape.txt"]).unwrap_err().code,
            "forbidden"
        );
        assert_eq!(
            select(&dir, &["C:/Windows/System32/drivers/etc/hosts"])
                .unwrap_err()
                .code,
            "forbidden"
        );
        assert_eq!(
            select(&dir, &["//server/share/secret.txt"])
                .unwrap_err()
                .code,
            "forbidden"
        );
        // Reserved device names are refused as unsafe names, not as traversal.
        assert_eq!(select(&dir, &["CON"]).unwrap_err().code, "invalid_request");
        assert_eq!(
            select(&dir, &["nested\\path.txt"]).unwrap_err().code,
            "invalid_request"
        );
        assert_eq!(select(&dir, &[]).unwrap_err().code, "no_files");

        let plan = select(&dir, &["keep.txt"]).unwrap();
        assert_eq!(plan.files.len(), 1);
        assert_eq!(plan.files[0].path, "keep.txt");
        assert_eq!(plan.total_bytes, 2);
        assert!(!plan.truncated);

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn selection_refuses_missing_paths_and_directories() {
        let dir = scratch("missing");
        fs::create_dir_all(dir.join("src")).unwrap();

        assert_eq!(
            select(&dir, &["src/missing.rs"]).unwrap_err().code,
            "not_found"
        );
        assert_eq!(select(&dir, &["src"]).unwrap_err().code, "invalid_request");

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn selection_deduplicates_repeated_paths() {
        let dir = scratch("dedupe");
        fs::write(dir.join("a.txt"), b"12345").unwrap();
        fs::write(dir.join("b.txt"), b"67").unwrap();

        let plan = select(&dir, &["a.txt", "b.txt", "a.txt", " a.txt "]).unwrap();

        assert_eq!(plan.files.len(), 2);
        assert_eq!(plan.total_bytes, 7);
        assert_eq!(plan.files[0].path, "a.txt");
        assert_eq!(plan.files[1].path, "b.txt");

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn selection_refuses_symlinks() {
        let dir = scratch("symlink");
        fs::write(dir.join("real.txt"), b"content").unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            symlink(dir.join("real.txt"), dir.join("link.txt")).unwrap();

            assert_eq!(
                select(&dir, &["link.txt"]).unwrap_err().code,
                "symlink_excluded"
            );
        }

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn resolve_sources_rechecks_secrets_and_links() {
        let dir = scratch("resolve");
        fs::write(dir.join("a.txt"), b"hello").unwrap();

        let plan = |path: &str| ExportPlan {
            root_name: "project".to_string(),
            files: vec![ExportCandidate {
                path: path.to_string(),
                size: 5,
            }],
            excluded_sensitive: 0,
            excluded_symlink: 0,
            excluded_unsafe: 0,
            total_bytes: 5,
            truncated: false,
        };

        let sources = resolve_sources(&dir, &plan("a.txt")).unwrap();
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].name, "a.txt");
        assert_eq!(sources[0].size, 5);
        assert!(sources[0].path.ends_with("a.txt"));

        assert_eq!(
            resolve_sources(&dir, &plan(".env")).unwrap_err().code,
            "secret_excluded"
        );
        assert_eq!(
            resolve_sources(&dir, &plan("gone.txt")).unwrap_err().code,
            "not_found"
        );

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            symlink(dir.join("a.txt"), dir.join("swapped.txt")).unwrap();
            assert_eq!(
                resolve_sources(&dir, &plan("swapped.txt"))
                    .unwrap_err()
                    .code,
                "symlink_excluded"
            );
        }

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn build_plan_reports_a_missing_root() {
        let dir = scratch("root");
        let missing = dir.join("does-not-exist");

        assert_eq!(build_plan(&missing, None).unwrap_err().code, "not_found");

        let _ = fs::remove_dir_all(dir);
    }
}

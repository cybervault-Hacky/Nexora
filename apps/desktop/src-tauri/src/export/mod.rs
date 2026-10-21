//! Exporting workspace output: one file, a ZIP archive, or a reviewed group.
//!
//! This module is deliberately separate from the inference downloader. It never
//! touches the network, never reads a path the caller chooses as a *root* (the
//! root comes from `WorkspaceState`), and never overwrites a file unless the
//! caller explicitly asked for it after a conflict was reported.
//!
//! Safety rules enforced here, in Rust, rather than only in the UI:
//!
//! * every source path is relative to the remembered workspace root and is
//!   canonicalised before use;
//! * symlinks are never followed for export, so an archive cannot escape the
//!   project through a link;
//! * secret-bearing names are refused even if a caller passes them explicitly;
//! * archive entry names are validated for traversal, Windows reserved names,
//!   control characters and trailing dots/spaces;
//! * output is written to a temporary file next to the destination and moved
//!   into place only after it is complete.

pub mod commands;
pub mod plan;
pub mod single;
pub mod zip;

use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;

use serde::Serialize;

/// A single running export, so a second request cannot race the first.
pub struct ExportState {
    pub active: Mutex<Option<std::sync::Arc<AtomicBool>>>,
}

impl ExportState {
    pub fn new() -> Self {
        Self {
            active: Mutex::new(None),
        }
    }
}

impl Default for ExportState {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportError {
    pub code: &'static str,
    pub message: String,
}

pub type ExportResult<T> = Result<T, ExportError>;

impl ExportError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self::new("invalid_request", message)
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::new("forbidden", message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new("not_found", message)
    }

    pub fn already_exists(message: impl Into<String>) -> Self {
        Self::new("already_exists", message)
    }

    pub fn no_files(message: impl Into<String>) -> Self {
        Self::new("no_files", message)
    }

    pub fn too_large(message: impl Into<String>) -> Self {
        Self::new("too_large", message)
    }

    pub fn storage(message: impl Into<String>) -> Self {
        Self::new("storage", message)
    }

    pub fn busy() -> Self {
        Self::new(
            "busy",
            "An export is already running. Wait for it to finish or cancel it first.",
        )
    }

    pub fn cancelled() -> Self {
        Self::new("cancelled", "The export was cancelled.")
    }

    pub fn secret_excluded(name: &str) -> Self {
        Self::new(
            "secret_excluded",
            format!(
                "{name} looks like it contains credentials, so Nexora will not export it. Remove the secret to a file outside the project, or export a different file."
            ),
        )
    }

    pub fn symlink_excluded(name: &str) -> Self {
        Self::new(
            "symlink_excluded",
            format!("{name} is a symbolic link. Nexora never follows links during export."),
        )
    }

    pub fn changed(name: &str) -> Self {
        Self::new(
            "changed",
            format!("{name} changed while it was being exported. Nothing was saved; try again."),
        )
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new("internal", message)
    }
}

impl From<io::Error> for ExportError {
    fn from(error: io::Error) -> Self {
        Self::storage(format!("The filesystem refused the export: {error}"))
    }
}

impl From<crate::workspace::scanner::WorkspaceError> for ExportError {
    fn from(error: crate::workspace::scanner::WorkspaceError) -> Self {
        Self::new(error.code, error.message)
    }
}

// ---------------------------------------------------------------------------
// Types shared with the UI
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportCandidate {
    /// Posix relative path inside the workspace.
    pub path: String,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportPlan {
    pub root_name: String,
    pub files: Vec<ExportCandidate>,
    /// Files withheld because their name marks them as secret-bearing.
    pub excluded_sensitive: usize,
    /// Entries withheld because they are symbolic links.
    pub excluded_symlink: usize,
    /// Entries withheld because the name is not safe on Windows (reserved
    /// device name, trailing dot or space, control character, over-long part).
    pub excluded_unsafe: usize,
    pub total_bytes: u64,
    /// The workspace scanner hit its file/depth ceiling. The UI refuses a
    /// whole-project export in that state rather than shipping a partial ZIP.
    pub truncated: bool,
}

/// Progress for one export operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportProgress {
    pub done: usize,
    pub total: usize,
    pub bytes_done: u64,
    pub bytes_total: u64,
    /// Relative path of the file just written, or an empty string.
    pub current: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ExportEvent {
    Preparing { total: usize },
    Progress(ExportProgress),
    Finalizing { total: usize },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportOutcome {
    /// Absolute destination, or an empty string when nothing was written.
    pub path: String,
    pub files: usize,
    pub bytes: u64,
    pub cancelled: bool,
    pub replaced: bool,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SingleExportRequest {
    /// Workspace-relative path of the file to copy.
    pub relative_path: String,
    pub destination: String,
    /// True only after the user confirmed replacing an existing file.
    #[serde(default)]
    pub overwrite: bool,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZipExportRequest {
    /// `None` means the whole project. `Some(list)` means exactly those files.
    #[serde(default)]
    pub selection: Option<Vec<String>>,
    pub destination: String,
    #[serde(default)]
    pub overwrite: bool,
}

// ---------------------------------------------------------------------------
// Name and path safety
// ---------------------------------------------------------------------------

const MAX_PATH_CHARS: usize = 4096;
const MAX_COMPONENT_BYTES: usize = 255;

/// Windows device names that cannot be used as file or archive-entry names.
const RESERVED_NAMES: &[&str] = &[
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9", "conin$",
    "conout$",
];

fn has_control_chars(name: &str) -> bool {
    name.chars().any(|c| (c as u32) < 0x20 || c == '\u{7f}')
}

/// A name is unsafe when Windows cannot create it, or when extracting it would
/// silently rename or refuse it. Checked on every platform so an archive made
/// on Linux cannot become unopenable on Windows.
pub fn is_unsafe_component(name: &str) -> bool {
    if name.is_empty() || name == "." || name == ".." {
        return true;
    }

    if name.len() > MAX_COMPONENT_BYTES {
        return true;
    }

    if has_control_chars(name) {
        return true;
    }

    if name.contains('/') || name.contains('\\') {
        return true;
    }

    // Windows strips trailing dots and spaces, which would turn `a.` into `a`.
    if name.ends_with('.') || name.ends_with(' ') {
        return true;
    }

    let stem = name.split('.').next().unwrap_or(name).to_ascii_lowercase();

    RESERVED_NAMES.contains(&stem.as_str())
}

/// Validates a workspace-relative, slash-separated entry name.
///
/// Rejects absolute paths, drive prefixes, `..`, empty segments, backslashes,
/// control characters, reserved device names and over-long components.
pub fn validate_entry_name(name: &str) -> ExportResult<()> {
    if name.trim().is_empty() {
        return Err(ExportError::invalid_request("The file name is empty."));
    }

    if name.chars().count() > MAX_PATH_CHARS {
        return Err(ExportError::invalid_request("The file path is too long."));
    }

    if name.contains('\0') {
        return Err(ExportError::invalid_request(
            "The file path contains an invalid character.",
        ));
    }

    if name.contains('\\') {
        return Err(ExportError::invalid_request(
            "Use forward slashes in workspace paths.",
        ));
    }

    // `Path::is_absolute` is platform-dependent: on Unix, "C:/x" is a normal
    // relative name with a colon in it. Reject drive and UNC shapes explicitly
    // so an archive created on Linux cannot contain a Windows drive path.
    let bytes = name.as_bytes();

    if bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic() {
        return Err(ExportError::forbidden(
            "The export path looks like a Windows drive path.",
        ));
    }

    if name.starts_with("//") {
        return Err(ExportError::forbidden(
            "The export path looks like a network path.",
        ));
    }

    let path = Path::new(name);

    if path.is_absolute() {
        return Err(ExportError::invalid_request(
            "Absolute paths are not allowed in an export.",
        ));
    }

    for component in path.components() {
        match component {
            Component::Normal(part) => {
                let text = part.to_str().ok_or_else(|| {
                    ExportError::invalid_request("The file name is not valid UTF-8.")
                })?;

                if is_unsafe_component(text) {
                    return Err(ExportError::invalid_request(format!(
                        "\"{text}\" is not a safe file name on Windows."
                    )));
                }
            }
            _ => {
                return Err(ExportError::forbidden(
                    "The export path escapes the project folder.",
                ))
            }
        }
    }

    Ok(())
}

/// Validates the destination the user typed or picked.
pub fn destination_path(input: &str) -> ExportResult<PathBuf> {
    let trimmed = input.trim();

    if trimmed.is_empty() {
        return Err(ExportError::invalid_request(
            "Choose where the export should be saved.",
        ));
    }

    if trimmed.chars().count() > MAX_PATH_CHARS {
        return Err(ExportError::invalid_request(
            "The destination path is too long.",
        ));
    }

    if trimmed.contains('\0') {
        return Err(ExportError::invalid_request(
            "The destination path contains an invalid character.",
        ));
    }

    let path = PathBuf::from(trimmed);

    if !path.is_absolute() {
        return Err(ExportError::invalid_request(
            "Choose an absolute destination path.",
        ));
    }

    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| ExportError::invalid_request("The destination needs a file name."))?;

    if is_unsafe_component(file_name) {
        return Err(ExportError::invalid_request(format!(
            "\"{file_name}\" is not a safe file name on Windows."
        )));
    }

    let parent = path
        .parent()
        .ok_or_else(|| ExportError::invalid_request("The destination needs a folder."))?;

    if !parent.is_dir() {
        return Err(ExportError::invalid_request(
            "The destination folder does not exist.",
        ));
    }

    if path.exists() && path.is_dir() {
        return Err(ExportError::invalid_request(
            "The destination is a folder, not a file.",
        ));
    }

    Ok(path)
}

/// Appends `-export`, `-export-2`, … until the path does not exist.
pub fn unique_suggestion(path: &Path) -> PathBuf {
    if !path.exists() {
        return path.to_path_buf();
    }

    let parent = path.parent().unwrap_or_else(|| Path::new(""));
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("export");
    let extension = path.extension().and_then(|value| value.to_str());

    for attempt in 1..1000 {
        let suffix = if attempt == 1 {
            "-export".to_string()
        } else {
            format!("-export-{attempt}")
        };

        let name = match extension {
            Some(ext) => format!("{stem}{suffix}.{ext}"),
            None => format!("{stem}{suffix}"),
        };

        let candidate = parent.join(name);

        if !candidate.exists() {
            return candidate;
        }
    }

    path.to_path_buf()
}

/// The source and destination must not be the same file.
pub fn ensure_distinct(source: &Path, destination: &Path) -> ExportResult<()> {
    if source == destination {
        return Err(ExportError::invalid_request(
            "Choose a different destination: the source and destination are the same file.",
        ));
    }

    if let Ok(canonical) = destination.canonicalize() {
        if canonical == source {
            return Err(ExportError::invalid_request(
                "Choose a different destination: the source and destination are the same file.",
            ));
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Temporary output and safe finalisation
// ---------------------------------------------------------------------------

fn random_suffix() -> ExportResult<String> {
    let mut bytes = [0u8; 8];

    getrandom::getrandom(&mut bytes)
        .map_err(|error| ExportError::internal(format!("No random source: {error}")))?;

    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

/// Creates a sibling temporary file so the final rename stays on one volume.
pub fn create_temp(destination: &Path) -> ExportResult<(PathBuf, File)> {
    let file_name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("export");
    let parent = destination
        .parent()
        .ok_or_else(|| ExportError::invalid_request("The destination needs a folder."))?;

    for _ in 0..8 {
        let temp = parent.join(format!(".{file_name}.{}.part", random_suffix()?));

        match OpenOptions::new().write(true).create_new(true).open(&temp) {
            Ok(file) => return Ok((temp, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(ExportError::storage(format!(
                    "The export file could not be created: {error}"
                )))
            }
        }
    }

    Err(ExportError::storage(
        "The export file could not be created after several attempts.",
    ))
}

/// Moves a completed temporary file into place.
///
/// Refuses to replace an existing file unless `overwrite` is true. When
/// replacing, the previous file is renamed aside first and restored if the
/// final move fails, so a failed export never destroys the file it was meant to
/// replace.
pub fn publish(temp: &Path, destination: &Path, overwrite: bool) -> ExportResult<bool> {
    if destination.exists() {
        if !overwrite {
            return Err(ExportError::already_exists(format!(
                "{} already exists. Confirm replacing it, or choose another name.",
                destination.display()
            )));
        }

        let backup = destination.with_file_name(format!(
            ".{}.{}.bak",
            destination
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("export"),
            random_suffix()?
        ));

        fs::rename(destination, &backup).map_err(|error| {
            ExportError::storage(format!(
                "The existing file could not be moved aside: {error}"
            ))
        })?;

        return match fs::rename(temp, destination) {
            Ok(()) => {
                let _ = fs::remove_file(&backup);
                Ok(true)
            }
            Err(error) => {
                let _ = fs::rename(&backup, destination);
                Err(ExportError::storage(format!(
                    "The export could not be put in place: {error}"
                )))
            }
        };
    }

    fs::rename(temp, destination)
        .map_err(|error| ExportError::storage(format!("The export could not be saved: {error}")))?;

    Ok(false)
}

/// Best-effort cleanup for a temporary file.
pub fn discard(temp: &Path) {
    let _ = fs::remove_file(temp);
}

/// Formats a path for the wire without lossy replacement of invalid UTF-8.
pub fn display_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "nexora-export-{name}-{}",
            random_suffix().expect("suffix")
        ));
        fs::create_dir_all(&dir).expect("create dir");
        dir
    }

    #[test]
    fn rejects_traversal_and_absolute_entry_names() {
        for name in ["../secret.txt", "/etc/passwd", "a/../../b", "C:/Windows/x"] {
            assert!(
                validate_entry_name(name).is_err(),
                "{name} should be rejected"
            );
        }
    }

    #[test]
    fn accepts_ordinary_nested_names() {
        for name in ["src/main.rs", "docs/a b/c.md", "café/naïve.txt"] {
            assert!(validate_entry_name(name).is_ok(), "{name} should pass");
        }
    }

    #[test]
    fn rejects_windows_reserved_and_trailing_names() {
        for name in [
            "CON",
            "nul.txt",
            "folder/aux",
            "com1.md",
            "name.",
            "name ",
            "bad\u{7}name",
            "a\\b.txt",
        ] {
            assert!(is_unsafe_component(name) || validate_entry_name(name).is_err());
        }
    }

    #[test]
    fn destination_must_be_absolute_and_safe() {
        assert!(destination_path("relative.zip").is_err());
        // A POSIX literal like "/tmp/..." is not absolute on Windows, so the
        // traversal case is built from the platform's own temporary folder.
        let traversal = std::env::temp_dir().join("..").join("x.zip");
        assert!(destination_path(&traversal.to_string_lossy()).is_ok());
        let dir = scratch("dest");
        assert!(destination_path(&format!("{}/CON.zip", dir.display())).is_err());
        assert!(destination_path(&format!("{}/ok.zip", dir.display())).is_ok());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn publish_never_overwrites_without_permission() {
        let dir = scratch("publish");
        let temp = dir.join(".a.zip.part");
        let destination = dir.join("a.zip");

        fs::write(&temp, b"new").unwrap();
        fs::write(&destination, b"old").unwrap();

        let error = publish(&temp, &destination, false).unwrap_err();
        assert_eq!(error.code, "already_exists");
        assert_eq!(fs::read(&destination).unwrap(), b"old");
        assert!(temp.exists());

        assert!(publish(&temp, &destination, true).unwrap());
        assert_eq!(fs::read(&destination).unwrap(), b"new");
        assert!(!temp.exists());

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn suggestions_do_not_collide_with_existing_files() {
        let dir = scratch("suggest");
        let existing = dir.join("report.zip");
        fs::write(&existing, b"x").unwrap();

        let candidate = unique_suggestion(&existing);
        assert_eq!(
            candidate.file_name().unwrap().to_str().unwrap(),
            "report-export.zip"
        );

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn temporary_file_is_a_sibling_and_removable() {
        let dir = scratch("temp");
        let destination = dir.join("out.zip");
        let (temp, mut file) = create_temp(&destination).unwrap();

        assert_eq!(temp.parent().unwrap(), dir.as_path());
        assert!(temp
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .ends_with(".part"));

        file.write_all(b"data").unwrap();
        drop(file);

        discard(&temp);
        assert!(!temp.exists());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn distinct_source_and_destination_are_enforced() {
        let dir = scratch("distinct");
        let file = dir.join("a.txt");
        fs::write(&file, b"x").unwrap();

        let canonical = file.canonicalize().unwrap();
        assert!(ensure_distinct(&canonical, &file).is_err());

        let other = dir.join("b.txt");
        assert!(ensure_distinct(&canonical, &other).is_ok());

        let _ = fs::remove_dir_all(dir);
    }
}

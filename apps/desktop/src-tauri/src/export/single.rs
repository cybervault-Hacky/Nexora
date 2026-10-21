//! Copying one workspace file to a destination the user chose.
//!
//! The copy is streamed (never loaded into memory), written to a sibling
//! temporary file, and moved into place only when complete. A cancelled or
//! failed copy leaves no file behind, and an existing destination is replaced
//! only after the caller confirms.

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use super::{
    create_temp, destination_path, discard, display_path, ensure_distinct, publish,
    validate_entry_name, ExportError, ExportEvent, ExportOutcome, ExportProgress, ExportResult,
    SingleExportRequest,
};
use crate::workspace::scanner::is_sensitive_file;

const CHUNK: usize = 256 * 1024;
const PROGRESS_EVERY: u64 = 4 * 1024 * 1024;

/// Copies `request.relative_path` out of `root`.
///
/// The caller must have acquired the single-export slot; this function does not
/// touch `ExportState` itself.
pub fn export_file<F: FnMut(ExportEvent)>(
    root: &Path,
    request: &SingleExportRequest,
    cancel: &AtomicBool,
    on_event: &mut F,
) -> ExportResult<ExportOutcome> {
    validate_entry_name(&request.relative_path)?;

    if is_sensitive_file(Path::new(&request.relative_path)) {
        return Err(ExportError::secret_excluded(&request.relative_path));
    }

    let canonical_root = fs::canonicalize(root)
        .map_err(|_| ExportError::not_found("The project folder is not available."))?;

    let joined = canonical_root.join(&request.relative_path);

    let metadata = fs::symlink_metadata(&joined).map_err(|_| {
        ExportError::not_found(format!(
            "{} is missing or not accessible.",
            request.relative_path
        ))
    })?;

    if metadata.file_type().is_symlink() {
        return Err(ExportError::symlink_excluded(&request.relative_path));
    }

    if metadata.is_dir() {
        return Err(ExportError::invalid_request(format!(
            "{} is a folder. Choose a file to export.",
            request.relative_path
        )));
    }

    if !metadata.is_file() {
        return Err(ExportError::invalid_request(format!(
            "{} is not a regular file.",
            request.relative_path
        )));
    }

    let source = joined.canonicalize().map_err(|_| {
        ExportError::not_found(format!("{} is not accessible.", request.relative_path))
    })?;

    if !source.starts_with(&canonical_root) {
        return Err(ExportError::forbidden(format!(
            "{} resolves outside the project folder.",
            request.relative_path
        )));
    }

    let destination = destination_path(&request.destination)?;

    ensure_distinct(&source, &destination)?;

    if destination.exists() && !request.overwrite {
        return Err(ExportError::already_exists(format!(
            "{} already exists. Confirm replacing it, or choose another name.",
            destination.display()
        )));
    }

    on_event(ExportEvent::Preparing { total: 1 });

    let expected = metadata.len();
    let (temp, mut output) = create_temp(&destination)?;

    let result = copy_into(&source, &mut output, expected, cancel, on_event);

    let copied = match result {
        Ok(bytes) => bytes,
        Err(error) => {
            discard(&temp);
            return if error.code == "cancelled" {
                Ok(ExportOutcome {
                    path: String::new(),
                    files: 0,
                    bytes: 0,
                    cancelled: true,
                    replaced: false,
                })
            } else {
                Err(error)
            };
        }
    };

    if let Err(error) = output.flush().and_then(|_| output.sync_all()) {
        discard(&temp);
        return Err(ExportError::storage(format!(
            "The exported file could not be flushed: {error}"
        )));
    }

    drop(output);

    let replaced = match publish(&temp, &destination, request.overwrite) {
        Ok(replaced) => replaced,
        Err(error) => {
            discard(&temp);
            return Err(error);
        }
    };

    on_event(ExportEvent::Finalizing { total: 1 });

    Ok(ExportOutcome {
        path: display_path(&destination),
        files: 1,
        bytes: copied,
        cancelled: false,
        replaced,
    })
}

fn copy_into<F: FnMut(ExportEvent)>(
    source: &Path,
    output: &mut File,
    expected: u64,
    cancel: &AtomicBool,
    on_event: &mut F,
) -> ExportResult<u64> {
    let mut input = File::open(source).map_err(|error| {
        ExportError::not_found(format!("The file could not be opened: {error}"))
    })?;

    let mut buffer = vec![0u8; CHUNK];
    let mut copied = 0u64;
    let mut last_report = 0u64;

    loop {
        if cancel.load(Ordering::SeqCst) {
            return Err(ExportError::cancelled());
        }

        let read = input.read(&mut buffer).map_err(|error| {
            ExportError::storage(format!("The file could not be read: {error}"))
        })?;

        if read == 0 {
            break;
        }

        output
            .write_all(&buffer[..read])
            .map_err(|error| ExportError::storage(format!("The copy failed: {error}")))?;

        copied += read as u64;

        if copied - last_report >= PROGRESS_EVERY {
            last_report = copied;
            on_event(ExportEvent::Progress(ExportProgress {
                done: 0,
                total: 1,
                bytes_done: copied,
                bytes_total: expected,
                current: String::new(),
            }));
        }
    }

    if copied != expected {
        return Err(ExportError::changed(
            "The source file changed while it was being copied.",
        ));
    }

    on_event(ExportEvent::Progress(ExportProgress {
        done: 1,
        total: 1,
        bytes_done: copied,
        bytes_total: expected,
        current: String::new(),
    }));

    Ok(copied)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "nexora-single-{name}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn request(relative: &str, destination: &Path, overwrite: bool) -> SingleExportRequest {
        SingleExportRequest {
            relative_path: relative.to_string(),
            destination: destination.to_string_lossy().into_owned(),
            overwrite,
        }
    }

    #[test]
    fn copies_bytes_exactly_into_a_new_file() {
        let dir = scratch("copy");
        let root = dir.join("project");
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/main.rs"), b"fn main() {}\n").unwrap();

        let destination = dir.join("out.rs");
        let cancel = AtomicBool::new(false);
        let mut events = Vec::new();

        let outcome = export_file(
            &root,
            &request("src/main.rs", &destination, false),
            &cancel,
            &mut |event| events.push(event),
        )
        .expect("export");

        assert_eq!(outcome.files, 1);
        assert_eq!(outcome.bytes, 13);
        assert!(!outcome.cancelled);
        assert!(!outcome.replaced);
        assert_eq!(fs::read(&destination).unwrap(), b"fn main() {}\n");
        assert!(matches!(
            events.first(),
            Some(ExportEvent::Preparing { total: 1 })
        ));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn refuses_missing_sensitive_and_symlinked_sources() {
        let dir = scratch("refuse");
        let root = dir.join("project");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join(".env"), b"SECRET=1").unwrap();

        let destination = dir.join("out.txt");
        let cancel = AtomicBool::new(false);
        let mut sink = |_event: ExportEvent| {};

        let missing = export_file(
            &root,
            &request("nope.txt", &destination, false),
            &cancel,
            &mut sink,
        )
        .unwrap_err();
        assert_eq!(missing.code, "not_found");

        let secret = export_file(
            &root,
            &request(".env", &destination, false),
            &cancel,
            &mut sink,
        )
        .unwrap_err();
        assert_eq!(secret.code, "secret_excluded");

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            fs::write(root.join("real.txt"), b"x").unwrap();
            symlink(root.join("real.txt"), root.join("link.txt")).unwrap();

            let linked = export_file(
                &root,
                &request("link.txt", &destination, false),
                &cancel,
                &mut sink,
            )
            .unwrap_err();
            assert_eq!(linked.code, "symlink_excluded");
        }

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn never_replaces_without_permission_and_keeps_the_old_file() {
        let dir = scratch("overwrite");
        let root = dir.join("project");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("a.txt"), b"new-content").unwrap();

        let destination = dir.join("a.txt");
        fs::write(&destination, b"old").unwrap();

        let cancel = AtomicBool::new(false);
        let mut sink = |_event: ExportEvent| {};

        let error = export_file(
            &root,
            &request("a.txt", &destination, false),
            &cancel,
            &mut sink,
        )
        .unwrap_err();
        assert_eq!(error.code, "already_exists");
        assert_eq!(fs::read(&destination).unwrap(), b"old");

        let outcome = export_file(
            &root,
            &request("a.txt", &destination, true),
            &cancel,
            &mut sink,
        )
        .expect("replace");
        assert!(outcome.replaced);
        assert_eq!(fs::read(&destination).unwrap(), b"new-content");

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn refuses_to_copy_a_file_onto_itself() {
        let dir = scratch("self");
        let root = dir.join("project");
        fs::create_dir_all(&root).unwrap();
        let source = root.join("a.txt");
        fs::write(&source, b"content").unwrap();

        let cancel = AtomicBool::new(false);
        let mut sink = |_event: ExportEvent| {};

        let error =
            export_file(&root, &request("a.txt", &source, true), &cancel, &mut sink).unwrap_err();
        assert_eq!(error.code, "invalid_request");

        assert_eq!(fs::read(&source).unwrap(), b"content");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn cancellation_writes_nothing() {
        let dir = scratch("cancel");
        let root = dir.join("project");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("a.txt"), b"content").unwrap();

        let destination = dir.join("out.txt");
        let cancel = AtomicBool::new(true);
        let mut sink = |_event: ExportEvent| {};

        let outcome = export_file(
            &root,
            &request("a.txt", &destination, false),
            &cancel,
            &mut sink,
        )
        .expect("cancelled outcome");

        assert!(outcome.cancelled);
        assert!(!destination.exists());
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1); // only the project dir

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn copies_an_empty_file() {
        let dir = scratch("empty");
        let root = dir.join("project");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("empty.txt"), b"").unwrap();

        let destination = dir.join("empty-copy.txt");
        let cancel = AtomicBool::new(false);
        let mut sink = |_event: ExportEvent| {};

        let outcome = export_file(
            &root,
            &request("empty.txt", &destination, false),
            &cancel,
            &mut sink,
        )
        .expect("export");

        assert_eq!(outcome.bytes, 0);
        assert_eq!(fs::read(&destination).unwrap(), b"");

        let _ = fs::remove_dir_all(dir);
    }
}

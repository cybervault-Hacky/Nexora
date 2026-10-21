//! Tauri commands for exporting workspace output.
//!
//! Every command takes the project root from `WorkspaceState`; the webview can
//! never nominate a different root. Destinations are validated here, in Rust,
//! and the single running-export slot lives in `ExportState`.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tauri::ipc::Channel;
use tauri::State;

use super::plan::{build_plan, resolve_sources};
use super::single::export_file;
use super::zip::{write_archive, Entry};
use super::{
    create_temp, destination_path, discard, display_path, publish, unique_suggestion,
    validate_entry_name, ExportError, ExportEvent, ExportOutcome, ExportPlan, ExportProgress,
    ExportResult, ExportState, SingleExportRequest, ZipExportRequest,
};
use crate::workspace::commands::WorkspaceState;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn workspace_root(state: &WorkspaceState) -> ExportResult<PathBuf> {
    let guard = state.root.lock().map_err(|_| {
        ExportError::internal("The workspace state is unavailable. Restart Nexora.")
    })?;

    guard.clone().ok_or_else(|| {
        ExportError::not_found("No project is open. Open a project folder before exporting.")
    })
}

fn lock_error() -> ExportError {
    ExportError::internal("The export state is unavailable. Restart Nexora.")
}

/// Canonical parent plus the chosen file name, so `..` cannot disguise the real
/// destination when it is compared with the project root.
fn canonical_destination(destination: &Path) -> PathBuf {
    match destination
        .parent()
        .and_then(|parent| parent.canonicalize().ok())
    {
        Some(parent) => match destination.file_name() {
            Some(name) => parent.join(name),
            None => destination.to_path_buf(),
        },
        None => destination.to_path_buf(),
    }
}

// ---------------------------------------------------------------------------
// Read-only planning
// ---------------------------------------------------------------------------

/// Lists exactly what a whole-project or selected-files export would contain.
#[tauri::command]
pub fn plan_workspace_export(
    state: State<'_, WorkspaceState>,
    selection: Option<Vec<String>>,
) -> ExportResult<ExportPlan> {
    let root = workspace_root(&state)?;
    build_plan(&root, selection.as_deref())
}

/// A non-colliding default destination next to the project folder.
#[tauri::command]
pub fn suggest_export_path(
    state: State<'_, WorkspaceState>,
    kind: String,
    relative_path: Option<String>,
) -> ExportResult<String> {
    let root = workspace_root(&state)?;

    let root_name = root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("project");

    let parent = root.parent().ok_or_else(|| {
        ExportError::invalid_request("The project folder has no parent folder to save beside.")
    })?;

    let base = match kind.as_str() {
        "zip" => parent.join(format!("{root_name}.zip")),
        "file" => {
            let relative = relative_path
                .ok_or_else(|| ExportError::invalid_request("Choose a file to export first."))?;

            validate_entry_name(&relative)?;

            let file_name = Path::new(&relative)
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| ExportError::invalid_request("That file has no usable name."))?;

            parent.join(file_name)
        }
        _ => {
            return Err(ExportError::invalid_request(
                "Unknown export type. Choose a file or a ZIP archive.",
            ))
        }
    };

    Ok(display_path(&unique_suggestion(&base)))
}

// ---------------------------------------------------------------------------
// Single file
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn export_single_file(
    state: State<'_, WorkspaceState>,
    export: State<'_, ExportState>,
    request: SingleExportRequest,
    on_event: Channel<ExportEvent>,
) -> ExportResult<ExportOutcome> {
    let root = workspace_root(&state)?;

    let cancel = Arc::new(AtomicBool::new(false));

    {
        let mut slot = export.active.lock().map_err(|_| lock_error())?;

        if slot.is_some() {
            return Err(ExportError::busy());
        }

        *slot = Some(Arc::clone(&cancel));
    }

    let joined = tauri::async_runtime::spawn_blocking(move || {
        let mut send = |event: ExportEvent| {
            let _ = on_event.send(event);
        };

        export_file(&root, &request, &cancel, &mut send)
    })
    .await;

    if let Ok(mut slot) = export.active.lock() {
        *slot = None;
    }

    joined.map_err(|error| ExportError::internal(format!("The export task failed: {error}")))?
}

// ---------------------------------------------------------------------------
// ZIP archive
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn export_zip(
    state: State<'_, WorkspaceState>,
    export: State<'_, ExportState>,
    request: ZipExportRequest,
    on_event: Channel<ExportEvent>,
) -> ExportResult<ExportOutcome> {
    let root = workspace_root(&state)?;

    // Validate the destination before doing any work.
    let destination = destination_path(&request.destination)?;

    if canonical_destination(&destination).starts_with(&root) {
        return Err(ExportError::invalid_request(
            "Save the archive outside the project folder so it cannot include itself.",
        ));
    }

    if destination.exists() && !request.overwrite {
        return Err(ExportError::already_exists(format!(
            "{} already exists. Confirm replacing it, or choose another name.",
            destination.display()
        )));
    }

    let cancel = Arc::new(AtomicBool::new(false));

    {
        let mut slot = export.active.lock().map_err(|_| lock_error())?;

        if slot.is_some() {
            return Err(ExportError::busy());
        }

        *slot = Some(Arc::clone(&cancel));
    }

    let joined = tauri::async_runtime::spawn_blocking(move || {
        write_archive_to(&root, &destination, &request, &cancel, &on_event)
    })
    .await;

    if let Ok(mut slot) = export.active.lock() {
        *slot = None;
    }

    joined.map_err(|error| ExportError::internal(format!("The export task failed: {error}")))?
}

fn write_archive_to(
    root: &Path,
    destination: &Path,
    request: &ZipExportRequest,
    cancel: &AtomicBool,
    on_event: &Channel<ExportEvent>,
) -> ExportResult<ExportOutcome> {
    let plan = build_plan(root, request.selection.as_deref())?;
    let sources = resolve_sources(root, &plan)?;

    let _ = on_event.send(ExportEvent::Preparing {
        total: sources.len(),
    });

    let (temp, mut file) = create_temp(destination)?;

    let entries: Vec<Entry> = sources
        .iter()
        .map(|source| Entry {
            name: source.name.clone(),
            source: source.path.clone(),
            size: source.size,
        })
        .collect();

    let total_bytes = plan.total_bytes;

    let written = write_archive(
        &mut file,
        &entries,
        cancel,
        &mut |name, done, total, bytes| {
            let _ = on_event.send(ExportEvent::Progress(ExportProgress {
                done,
                total,
                bytes_done: bytes,
                bytes_total: total_bytes,
                current: name.to_string(),
            }));
        },
    );

    let (count, bytes) = match written {
        Ok(value) => value,
        Err(error) if error.code == "cancelled" => {
            discard(&temp);
            return Ok(ExportOutcome {
                path: String::new(),
                files: 0,
                bytes: 0,
                cancelled: true,
                replaced: false,
            });
        }
        Err(error) => {
            discard(&temp);
            return Err(error);
        }
    };

    if let Err(error) = std::io::Write::flush(&mut file).and_then(|_| file.sync_all()) {
        discard(&temp);
        return Err(ExportError::storage(format!(
            "The archive could not be flushed: {error}"
        )));
    }

    drop(file);

    let replaced = match publish(&temp, destination, request.overwrite) {
        Ok(replaced) => replaced,
        Err(error) => {
            discard(&temp);
            return Err(error);
        }
    };

    let _ = on_event.send(ExportEvent::Finalizing { total: count });

    Ok(ExportOutcome {
        path: display_path(destination),
        files: count,
        bytes,
        cancelled: false,
        replaced,
    })
}

// ---------------------------------------------------------------------------
// Cancellation
// ---------------------------------------------------------------------------

/// Stops the running export. Returns whether one was active.
#[tauri::command]
pub fn cancel_export(state: State<'_, ExportState>) -> bool {
    match state.active.lock() {
        Ok(slot) => match slot.as_ref() {
            Some(flag) => {
                flag.store(true, Ordering::SeqCst);
                true
            }
            None => false,
        },
        Err(_) => false,
    }
}

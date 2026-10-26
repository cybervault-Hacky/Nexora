//! Secure single-artifact transfer primitive used by the media-model manager.
//! URLs and destination filenames come only from the static registry.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use serde::Serialize;
use sha2::{Digest, Sha256};
use tauri::ipc::Channel;
use tokio::io::AsyncWriteExt;

use super::error::{MediaModelError, Result};
use super::registry::{self, ModelFileDefinition};
use super::storage;

const MAX_FILE_BYTES: u64 = 256 * 1024 * 1024 * 1024;
const MAX_REDIRECTS: usize = 5;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
const TOTAL_TIMEOUT: Duration = Duration::from_secs(6 * 60 * 60);
const STALL_TIMEOUT: Duration = Duration::from_secs(60);
const CANCEL_POLL: Duration = Duration::from_millis(250);
const PROGRESS_INTERVAL: Duration = Duration::from_millis(250);

struct TemporaryArtifact {
    path: PathBuf,
    owned: bool,
}

impl Drop for TemporaryArtifact {
    fn drop(&mut self) {
        if self.owned {
            // The temp entry is uniquely created by this request; remove_file
            // never follows a symlink and cleanup is safe on early-return paths.
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgress {
    pub model_id: String,
    pub file_id: Option<String>,
    pub received_bytes: u64,
    pub total_bytes: Option<u64>,
}

pub struct DownloadControl {
    pub cancel: AtomicBool,
    pub received_bytes: AtomicU64,
    pub total_bytes: AtomicU64,
    pub current_file_id: std::sync::Mutex<Option<String>>,
}

impl DownloadControl {
    pub fn new() -> Self {
        Self {
            cancel: AtomicBool::new(false),
            received_bytes: AtomicU64::new(0),
            total_bytes: AtomicU64::new(0),
            current_file_id: std::sync::Mutex::new(None),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DownloadedArtifact {
    pub bytes: u64,
    pub sha256: String,
    pub integrity_verified: bool,
}

/// Download one registered file. A completed file is atomically published
/// without replacing any existing artifact; an unknown checksum remains marked
/// unvalidated even though a local digest is calculated for future inspection.
pub async fn download_file(
    model_id: &str,
    file: &ModelFileDefinition,
    root: &Path,
    model_remaining_limit: u64,
    control: &Arc<DownloadControl>,
    on_event: &Channel<DownloadProgress>,
) -> Result<DownloadedArtifact> {
    let url = file.download_url.ok_or_else(|| {
        MediaModelError::new("source_unavailable", "This registered model file has no approved download source.")
    })?;
    validate_file_metadata(file)?;
    let file_limit = MAX_FILE_BYTES.min(model_remaining_limit);
    if file_limit == 0 || file.expected_size_bytes.is_some_and(|size| size > file_limit) {
        return Err(MediaModelError::new("size_limit", "The registered model exceeds the supported total download size."));
    }
    let mut current = validate_registered_url(reqwest::Url::parse(url).map_err(|_| {
        MediaModelError::new("source_unavailable", "The registered download URL is malformed.")
    })?)?;

    let final_path = storage::artifact_path(root, model_id, file)?;
    let staging_dir = storage::staging_dir(root, model_id)?;
    storage::ensure_layout(root, model_id)?;
    ensure_absent_regular_target(&final_path)?;
    let part_path = staging_dir.join(format!("{}.{}.part", file.id, random_suffix()?));
    let client = reqwest::Client::builder()
        .user_agent(concat!("Nexora/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(TOTAL_TIMEOUT)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| MediaModelError::network("The secure download client could not be initialized."))?;

    let started = Instant::now();
    let response = fetch_checked_redirects(&client, &current, control, started).await?;
    if started.elapsed() > TOTAL_TIMEOUT {
        return Err(MediaModelError::network("The model file exceeded the transfer time limit."));
    }
    if !response.status().is_success() {
        return Err(MediaModelError::network(format!("The trusted model host returned HTTP {}.", response.status().as_u16())));
    }

    let advertised = response.content_length();
    if let (Some(expected), Some(advertised)) = (file.expected_size_bytes, advertised) {
        if expected != advertised {
            return Err(MediaModelError::integrity("The source size does not match the registered file size."));
        }
    }
    let total = file.expected_size_bytes.or(advertised);
    if total.is_some_and(|size| size == 0 || size > file_limit) {
        return Err(MediaModelError::new("size_limit", "The registered model file exceeds the per-file download limit."));
    }
    if let Some(size) = total {
        let free = crate::inference::hardware::disk_space(&staging_dir).free_bytes;
        if free.is_some_and(|available| available < size) {
            return Err(MediaModelError::insufficient_storage());
        }
    }
    control.total_bytes.store(total.unwrap_or(0), Ordering::SeqCst);
    control.received_bytes.store(0, Ordering::SeqCst);

    // Declare the guard before the open file so it is dropped only after the
    // handle closes (required for cleanup on Windows).
    let mut temporary_artifact = TemporaryArtifact { path: part_path.clone(), owned: false };
    let mut output = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&part_path)
        .await
        .map_err(|_| MediaModelError::storage("A temporary media-model file could not be created safely."))?;
    temporary_artifact.owned = true;
    let mut stream = response.bytes_stream();
    let mut hasher = Sha256::new();
    let mut received = 0u64;
    let mut last_progress = Instant::now();
    let transfer_started = Instant::now();
    let mut last_activity = Instant::now();

    loop {
        if control.cancel.load(Ordering::SeqCst) {
            drop(output);
            let _ = tokio::fs::remove_file(&part_path).await;
            return Err(MediaModelError::interrupted());
        }
        let chunk = match tokio::time::timeout(CANCEL_POLL, stream.next()).await {
            Ok(Some(Ok(chunk))) => chunk,
            Ok(Some(Err(_))) => {
                drop(output);
                let _ = tokio::fs::remove_file(&part_path).await;
                return Err(MediaModelError::network("The model transfer was interrupted. Partial data was removed."));
            }
            Ok(None) => break,
            Err(_) if last_activity.elapsed() >= STALL_TIMEOUT => {
                drop(output);
                let _ = tokio::fs::remove_file(&part_path).await;
                return Err(MediaModelError::network("The model source stopped sending data. Partial data was removed."));
            }
            Err(_) if transfer_started.elapsed() >= TOTAL_TIMEOUT => {
                drop(output);
                let _ = tokio::fs::remove_file(&part_path).await;
                return Err(MediaModelError::network("The model file exceeded the transfer time limit. Partial data was removed."));
            }
            Err(_) => continue,
        };
        last_activity = Instant::now();
        let next = received.checked_add(chunk.len() as u64)
            .ok_or_else(|| MediaModelError::new("size_limit", "The downloaded size exceeded the supported limit."))?;
        if next > file_limit || file.expected_size_bytes.is_some_and(|size| next > size) {
            drop(output);
            let _ = tokio::fs::remove_file(&part_path).await;
            return Err(MediaModelError::integrity("The download exceeded its registered size limit; partial data was removed."));
        }
        output.write_all(&chunk).await.map_err(|_| MediaModelError::storage("The model file could not be written. Check available storage."))?;
        hasher.update(&chunk);
        received = next;
        control.received_bytes.store(received, Ordering::SeqCst);
        if last_progress.elapsed() >= PROGRESS_INTERVAL {
            let _ = on_event.send(DownloadProgress {
                model_id: model_id.to_string(),
                file_id: Some(file.id.to_string()),
                received_bytes: received,
                total_bytes: total,
            });
            last_progress = Instant::now();
        }
    }
    output.flush().await.map_err(|_| MediaModelError::storage("The temporary model file could not be flushed."))?;
    output.sync_all().await.map_err(|_| MediaModelError::storage("The temporary model file could not be synchronized to storage."))?;
    drop(output);

    if received == 0 {
        let _ = tokio::fs::remove_file(&part_path).await;
        return Err(MediaModelError::integrity("The model source returned an empty file."));
    }
    if total.is_some_and(|expected| received != expected) {
        let _ = tokio::fs::remove_file(&part_path).await;
        return Err(MediaModelError::integrity("The downloaded file size does not match the source or registered metadata."));
    }
    let sha256 = hex_digest(hasher.clone().finalize().as_slice());
    if let Some(expected) = file.expected_sha256 {
        if !sha256.eq_ignore_ascii_case(expected) {
            let _ = tokio::fs::remove_file(&part_path).await;
            return Err(MediaModelError::integrity("The downloaded file checksum does not match trusted registry metadata."));
        }
    }
    if control.cancel.load(Ordering::SeqCst) {
        let _ = tokio::fs::remove_file(&part_path).await;
        return Err(MediaModelError::interrupted());
    }

    ensure_absent_regular_target(&final_path)?;
    tokio::fs::hard_link(&part_path, &final_path).await
        .map_err(|_| MediaModelError::new("already_exists", "The model destination could not be published without replacing an existing file."))?;
    // The destination is committed. A leftover same-volume hard link is safe
    // and manager-owned; the drop guard retries cleanup without invalidating the
    // completed file if Windows transiently refuses removal.
    let _ = tokio::fs::remove_file(&part_path).await;

    let integrity_verified = file.expected_sha256.is_some();
    let _ = on_event.send(DownloadProgress {
        model_id: model_id.to_string(),
        file_id: Some(file.id.to_string()),
        received_bytes: received,
        total_bytes: total,
    });
    Ok(DownloadedArtifact { bytes: received, sha256, integrity_verified })
}

async fn fetch_checked_redirects(
    client: &reqwest::Client,
    initial: &reqwest::Url,
    control: &Arc<DownloadControl>,
    started: Instant,
) -> Result<reqwest::Response> {
    let mut url = initial.clone();
    for hop in 0..=MAX_REDIRECTS {
        if control.cancel.load(Ordering::SeqCst) {
            return Err(MediaModelError::interrupted());
        }
        let request = client.get(url.clone()).send();
        tokio::pin!(request);
        let response = loop {
            if control.cancel.load(Ordering::SeqCst) { return Err(MediaModelError::interrupted()); }
            if started.elapsed() >= TOTAL_TIMEOUT {
                return Err(MediaModelError::network("The model source exceeded the transfer time limit."));
            }
            match tokio::time::timeout(CANCEL_POLL, &mut request).await {
                Ok(Ok(response)) => break response,
                Ok(Err(_)) => return Err(MediaModelError::network("The model host could not be reached securely.")),
                Err(_) => continue,
            }
        };
        if !response.status().is_redirection() {
            return Ok(response);
        }
        if hop == MAX_REDIRECTS {
            return Err(MediaModelError::network("The model source exceeded the redirect limit."));
        }
        let location = response.headers().get(reqwest::header::LOCATION)
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| MediaModelError::network("The trusted model host returned an invalid redirect."))?;
        let next = response.url().join(location)
            .map_err(|_| MediaModelError::network("The model host returned an invalid redirect URL."))?;
        url = validate_registered_url(next)?;
    }
    Err(MediaModelError::network("The model source could not be resolved."))
}

pub fn validate_registered_url(url: reqwest::Url) -> Result<reqwest::Url> {
    if url.as_str().len() > 4096 {
        return Err(MediaModelError::new("untrusted_source", "The registered model URL exceeds the safe metadata limit."));
    }
    let allowed = registry::TRUSTED_DOWNLOAD_HOSTS.iter().any(|host| {
        url.host_str().is_some_and(|actual| actual.eq_ignore_ascii_case(host))
    });
    if url.scheme() != "https" || !allowed || url.username() != "" || url.password().is_some()
        || url.port_or_known_default() != Some(443)
    {
        return Err(MediaModelError::new("untrusted_source", "The model download URL or redirect is not on the HTTPS allowlist."));
    }
    Ok(url)
}

fn validate_file_metadata(file: &ModelFileDefinition) -> Result<()> {
    if !registry::validate_model_id(file.id) {
        return Err(MediaModelError::invalid("The registered file identifier is invalid."));
    }
    storage::validate_leaf_name(file.file_name)?;
    if file.expected_size_bytes.is_some_and(|size| size == 0 || size > MAX_FILE_BYTES) {
        return Err(MediaModelError::new("size_limit", "The registered model file size is outside the supported range."));
    }
    if let Some(hash) = file.expected_sha256 {
        if hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(MediaModelError::invalid("The registered checksum is not a valid SHA-256 digest."));
        }
    }
    Ok(())
}

fn ensure_absent_regular_target(path: &Path) -> Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => Err(MediaModelError::new("already_exists", "A file already exists at the registered model destination; it was not overwritten.")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(MediaModelError::storage("The registered model destination could not be inspected safely.")),
    }
}

fn random_suffix() -> Result<String> {
    let mut bytes = [0u8; 16];
    getrandom::getrandom(&mut bytes)
        .map_err(|_| MediaModelError::storage("A unique temporary filename could not be created."))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn hex_digest(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

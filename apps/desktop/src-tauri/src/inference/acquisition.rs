//! Verified acquisition of the approved model artefact.
//!
//! Nexora never downloads anything on its own. A download starts only when the
//! user asks for it, writes into a private `.part` file inside a validated
//! destination folder, hashes the bytes while receiving them, and renames into
//! place only after the count and the SHA-256 match the registry. A failed,
//! cancelled, or interrupted download therefore never leaves a file that looks
//! installed.
//!
//! Nothing here launches a process, touches runtime settings, or rewrites the
//! user's configured model path. Publishing a file is the end of this module's
//! responsibility; choosing it is still an explicit user action.

use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use serde::Serialize;
use sha2::{Digest, Sha256};
use tauri::ipc::Channel;
use tokio::io::AsyncWriteExt;

use super::error::{InferenceError, InferenceResult};
use super::registry::{self, ArtifactState, ModelProfile};

/// At most one progress event per interval, plus one on the final byte.
const PROGRESS_INTERVAL: Duration = Duration::from_millis(250);
/// Read size when hashing an existing file (strictly bounded 1 MiB heap slice).
const HASH_BUFFER_BYTES: usize = 1024 * 1024;
/// TCP/TLS handshake timeout per hop.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
/// The network stream is polled in slices of this length so cancellation responds
/// promptly even if the remote host stops sending bytes mid-stream.
const CANCEL_POLL: Duration = Duration::from_millis(250);
/// No bytes for this long is treated as a failed transfer, not a slow one.
/// 4.7 GB over a slow link still delivers a chunk far more often than this.
const STALL_TIMEOUT: Duration = Duration::from_secs(60);
/// Redirect hops allowed before the request is refused. The approved URL is
/// answered with a single hop to the model host's CDN; a longer chain means
/// something other than the approved download is being followed.
const MAX_REDIRECTS: usize = 5;

const WINDOWS_RESERVED_STEMS: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Transfer timing and transport policy.
///
/// Production builds strictly enforce `https` on both the initial request and
/// the final resolved redirect target; local `127.0.0.1` HTTP servers are
/// constructible only inside `#[cfg(test)]` suites.
#[derive(Debug, Clone, Copy)]
pub struct DownloadPolicy {
    pub connect_timeout: Duration,
    pub stall_timeout: Duration,
    pub cancel_poll: Duration,
    pub require_https: bool,
}

impl DownloadPolicy {
    pub const fn production() -> Self {
        Self {
            connect_timeout: CONNECT_TIMEOUT,
            stall_timeout: STALL_TIMEOUT,
            cancel_poll: CANCEL_POLL,
            require_https: true,
        }
    }

    #[cfg(test)]
    pub const fn loopback_for_tests(stall_timeout: Duration) -> Self {
        Self {
            connect_timeout: Duration::from_secs(2),
            stall_timeout,
            cancel_poll: Duration::from_millis(25),
            require_https: false,
        }
    }
}

/// Everything needed to fetch the approved artefact.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadPlan {
    pub url: String,
    pub file_name: String,
    pub size_bytes: u64,
    pub sha256: String,
}

/// Progress reported to the UI while a download runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(serde::Deserialize))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DownloadEvent {
    Progress { received: u64, total: u64 },
    Verifying { total: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadOutcome {
    /// `None` when the user cancelled; no file was published in that case.
    pub path: Option<String>,
    pub file_name: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub cancelled: bool,
}

/// The result of hashing a file the user chose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrityReport {
    pub path: String,
    pub file_name: String,
    pub profile_id: &'static str,
    pub size_bytes: u64,
    pub size_matches: bool,
    pub sha256: String,
    pub expected_size_bytes: u64,
    pub expected_sha256: &'static str,
    pub digest_matches: bool,
}

/// Why a profile cannot be downloaded yet, in words a person can act on.
fn pending_error(profile: &ModelProfile) -> InferenceError {
    let note = match profile.artifact {
        ArtifactState::Pending { note } => note,
        ArtifactState::Approved(_) => "The artefact is not approved.",
    };

    InferenceError::new(
        "artifact_pending",
        format!("{} cannot be downloaded yet. {note}", profile.display_name),
    )
}

fn is_windows_reserved_segment(segment: &str) -> bool {
    let stem = segment.split('.').next().unwrap_or("");
    WINDOWS_RESERVED_STEMS
        .iter()
        .any(|reserved| stem.eq_ignore_ascii_case(reserved))
}

fn has_unsafe_segment_chars(segment: &str) -> bool {
    segment.is_empty()
        || segment.ends_with(' ')
        || segment.ends_with('.')
        || segment.chars().any(|ch| ch.is_control() || ch == '\0')
        || is_windows_reserved_segment(segment)
}

/// Validates that an artifact file name is a single safe `.gguf` leaf name with
/// zero traversal, device-name, hidden-file, or path-separator components.
pub fn validate_artifact_file_name(file_name: &str) -> InferenceResult<()> {
    if file_name.is_empty()
        || file_name.starts_with('.')
        || file_name.contains('/')
        || file_name.contains('\\')
        || file_name.contains(':')
        || has_unsafe_segment_chars(file_name)
    {
        return Err(InferenceError::invalid_request(
            "The model artifact file name is not safe to write on disk.",
        ));
    }

    if !file_name
        .rsplit('.')
        .next()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("gguf"))
    {
        return Err(InferenceError::invalid_request(
            "Only single-file .gguf model artifacts are supported.",
        ));
    }

    let mut components = Path::new(file_name).components();
    match (components.next(), components.next()) {
        (Some(Component::Normal(_)), None) => Ok(()),
        _ => Err(InferenceError::invalid_request(
            "The model artifact file name must not contain path components.",
        )),
    }
}

/// Validates that a destination download directory is an absolute, non-root,
/// non-traversal, non-symlink directory path.
pub fn validate_download_directory(path: &Path) -> InferenceResult<()> {
    let raw = path.to_string_lossy();
    if raw.trim().is_empty() || raw.chars().any(|ch| ch.is_control() || ch == '\0') {
        return Err(InferenceError::invalid_request(
            "Choose a valid folder path for the model download.",
        ));
    }

    if !path.is_absolute() {
        return Err(InferenceError::invalid_request(
            "Choose an absolute folder for the model download.",
        ));
    }

    let mut normal_count = 0usize;
    let mut prefix = PathBuf::new();

    for component in path.components() {
        match component {
            Component::Prefix(p) => prefix.push(p.as_os_str()),
            Component::RootDir => prefix.push(component.as_os_str()),
            Component::Normal(seg) => {
                let Some(seg_str) = seg.to_str() else {
                    return Err(InferenceError::invalid_request(
                        "The download folder path is not valid UTF-8.",
                    ));
                };
                if has_unsafe_segment_chars(seg_str) {
                    return Err(InferenceError::invalid_request(
                        "The download folder contains an unsupported or reserved name.",
                    ));
                }
                normal_count += 1;
                prefix.push(seg);

                if let Ok(meta) = std::fs::symlink_metadata(&prefix) {
                    if meta.file_type().is_symlink() {
                        return Err(InferenceError::invalid_request(
                            "The model download folder must not traverse a symbolic link.",
                        ));
                    }
                }
            }
            Component::CurDir | Component::ParentDir => {
                return Err(InferenceError::invalid_request(
                    "The model download folder must not contain relative '..' or '.' segments.",
                ));
            }
        }
    }

    if normal_count == 0 {
        return Err(InferenceError::invalid_request(
            "Choose a subfolder rather than the filesystem root for model downloads.",
        ));
    }

    if let Ok(meta) = std::fs::symlink_metadata(path) {
        if !meta.is_dir() {
            return Err(InferenceError::invalid_request(
                "The chosen model download destination is a file, not a folder.",
            ));
        }
    }

    Ok(())
}

/// Removes only orphaned `.{file_name}.<hex>.part` temporary files created by
/// an earlier interrupted Nexora download. Never deletes `.gguf` model files or
/// symbolic links; must only be called while holding the exclusive download lock.
pub fn purge_stale_part_files(directory: &Path, file_name: &str) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    let prefix = format!(".{file_name}.");

    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };

        if !name.starts_with(&prefix) || !name.ends_with(".part") {
            continue;
        }

        let middle = &name[prefix.len()..name.len() - ".part".len()];
        if middle.len() != 16 || !middle.chars().all(|c| c.is_ascii_hexdigit()) {
            continue;
        }

        let candidate = entry.path();
        if let Ok(meta) = std::fs::symlink_metadata(&candidate) {
            if meta.file_type().is_file() {
                let _ = std::fs::remove_file(&candidate);
            }
        }
    }
}

/// Scrubs signed CDN query parameters (`?X-Amz-Signature=...`, `?token=...`),
/// embedded URL credentials (`https://user:pass@host`), and `Bearer` headers
/// from any error message before it can reach logs, state, or the desktop UI.
pub fn redact_sensitive_transport_text(raw: &str) -> String {
    let mut scrubbed = String::with_capacity(raw.len());
    let mut rest = raw;

    while let Some(scheme_pos) = rest
        .find("https://")
        .into_iter()
        .chain(rest.find("http://"))
        .min()
    {
        scrubbed.push_str(&rest[..scheme_pos]);
        let after_start = &rest[scheme_pos..];
        let end = after_start
            .find(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | ')' | '>' | ']'))
            .unwrap_or(after_start.len());
        let url_slice = &after_start[..end];
        scrubbed.push_str(&scrub_single_url(url_slice));
        rest = &after_start[end..];
    }
    scrubbed.push_str(rest);

    // Also scrub any standalone bearer credential tokens.
    let mut final_out = String::with_capacity(scrubbed.len());
    let mut tail = scrubbed.as_str();
    while let Some(pos) = tail.to_ascii_lowercase().find("bearer ") {
        final_out.push_str(&tail[..pos]);
        final_out.push_str("Bearer <redacted>");
        let after = &tail[pos + "bearer ".len()..];
        let token_end = after
            .find(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | ',' | ';'))
            .unwrap_or(after.len());
        tail = &after[token_end..];
    }
    final_out.push_str(tail);
    final_out
}

fn scrub_single_url(url: &str) -> String {
    let (scheme, remainder) = match url.split_once("://") {
        Some(parts) => parts,
        None => return url.to_string(),
    };

    let without_query = match remainder.split_once('?') {
        Some((before_q, _)) => format!("{before_q}?<redacted>"),
        None => remainder.to_string(),
    };
    let without_fragment = match without_query.split_once('#') {
        Some((before_hash, _)) => before_hash.to_string(),
        None => without_query,
    };

    let authority_end = without_fragment
        .find('/')
        .unwrap_or_else(|| without_fragment.find('?').unwrap_or(without_fragment.len()));
    let authority = &without_fragment[..authority_end];
    let path_and_query = &without_fragment[authority_end..];

    if let Some((_, host_port)) = authority.rsplit_once('@') {
        format!("{scheme}://<redacted>@{host_port}{path_and_query}")
    } else {
        format!("{scheme}://{authority}{path_and_query}")
    }
}

fn sanitize_reqwest_error(error: reqwest::Error) -> String {
    redact_sensitive_transport_text(&error.without_url().to_string())
}

/// Builds the download plan from the registry rather than from the UI, so the
/// host, revision, size, and digest cannot drift from the approved profile.
///
/// A profile whose artefact has not been approved has no URL, length, or digest
/// to check against, so this refuses instead of producing an unverifiable
/// download. That is the whole reason the pending state exists: Nexora never
/// fetches a file it cannot verify.
pub fn plan_for(profile: &ModelProfile) -> InferenceResult<DownloadPlan> {
    let Some(source) = profile.artifact.source() else {
        return Err(pending_error(profile));
    };

    validate_artifact_file_name(source.file_name)?;

    Ok(DownloadPlan {
        url: format!(
            "https://huggingface.co/{}/resolve/{}/{}",
            source.repository, source.revision, source.file_name
        ),
        file_name: source.file_name.to_string(),
        size_bytes: source.size_bytes,
        sha256: source.sha256.to_string(),
    })
}

/// Finds an installed copy of a profile's artefact among `candidates`.
///
/// A candidate counts as installed only when its file name matches the approved
/// artefact and its length matches exactly. A truncated file that happens to
/// share the name is not reported as installed, so the UI can never claim a
/// model is ready when the bytes on disk are incomplete.
pub fn installed_path(
    profile: &ModelProfile,
    candidates: &[std::path::PathBuf],
) -> Option<std::path::PathBuf> {
    let source = profile.artifact.source()?;

    candidates
        .iter()
        .filter(|candidate| {
            candidate.file_name().and_then(|name| name.to_str()) == Some(source.file_name)
        })
        .find(|candidate| {
            std::fs::symlink_metadata(candidate)
                .map(|metadata| {
                    metadata.file_type().is_file() && metadata.len() == source.size_bytes
                })
                .unwrap_or(false)
        })
        .cloned()
}

/// The registered profile a chosen file belongs to, identified by file name
/// only. Used to record what the engine actually loaded; it never claims the
/// bytes are correct, because that is `verify_file`'s job.
pub fn profile_for_path(path: &Path) -> Option<&'static ModelProfile> {
    let name = path.file_name()?.to_str()?;

    registry::APPROVED.by_file_name(name)
}

/// Downloads the artefact into `directory` and publishes it only if it verifies.
///
/// Returns `cancelled: true` when the caller flipped `cancel`; the partial file
/// is removed in that case.
#[cfg(test)]
pub async fn download(
    plan: &DownloadPlan,
    directory: &Path,
    cancel: &AtomicBool,
    verifying: &AtomicBool,
    on_event: &Channel<DownloadEvent>,
) -> InferenceResult<DownloadOutcome> {
    download_with_policy(
        plan,
        directory,
        cancel,
        verifying,
        on_event,
        DownloadPolicy::production(),
    )
    .await
}

pub async fn download_with_policy(
    plan: &DownloadPlan,
    directory: &Path,
    cancel: &AtomicBool,
    verifying: &AtomicBool,
    on_event: &Channel<DownloadEvent>,
    policy: DownloadPolicy,
) -> InferenceResult<DownloadOutcome> {
    validate_artifact_file_name(&plan.file_name)?;
    validate_download_directory(directory)?;

    let final_path = directory.join(&plan.file_name);

    if std::fs::symlink_metadata(&final_path).is_ok() {
        return Err(InferenceError::already_exists(format!(
            "{} already exists. Nexora never overwrites or deletes an existing model file; verify it if it matches the approved download, or move it first.",
            final_path.display()
        )));
    }

    purge_stale_part_files(directory, &plan.file_name);

    let part_path = directory.join(format!(".{}.{}.part", plan.file_name, random_suffix()?));

    let completed = match receive(plan, &part_path, cancel, verifying, on_event, policy).await {
        Ok(completed) => completed,
        Err(error) => {
            verifying.store(false, Ordering::SeqCst);
            // A failed download must not leave a partial file behind.
            let _ = tokio::fs::remove_file(&part_path).await;
            return Err(error);
        }
    };

    verifying.store(false, Ordering::SeqCst);

    // Cancellation is honoured up to the moment the file is installed. The file
    // is complete and verified at this point, but pressing Cancel during the
    // final digest must not install it anyway: the UI would then report a
    // cancelled download for a file that exists.
    if !completed || cancel.load(Ordering::SeqCst) {
        let _ = tokio::fs::remove_file(&part_path).await;

        return Ok(DownloadOutcome {
            path: None,
            file_name: plan.file_name.clone(),
            size_bytes: 0,
            sha256: String::new(),
            cancelled: true,
        });
    }

    if let Err(error) = publish(&part_path, &final_path, plan.size_bytes) {
        let _ = tokio::fs::remove_file(&part_path).await;
        return Err(error);
    }

    Ok(DownloadOutcome {
        path: Some(final_path.to_string_lossy().into_owned()),
        file_name: plan.file_name.clone(),
        size_bytes: plan.size_bytes,
        sha256: plan.sha256.clone(),
        cancelled: false,
    })
}

/// Streams the response into `part_path`. `Ok(false)` means cancelled.
async fn receive(
    plan: &DownloadPlan,
    part_path: &Path,
    cancel: &AtomicBool,
    verifying: &AtomicBool,
    on_event: &Channel<DownloadEvent>,
    policy: DownloadPolicy,
) -> InferenceResult<bool> {
    if policy.require_https && !plan.url.starts_with("https://") {
        return Err(InferenceError::network(
            "Model downloads require an HTTPS URL. No file was written.",
        ));
    }

    let client = reqwest::Client::builder()
        .user_agent(concat!("Nexora/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(policy.connect_timeout)
        // Bound the chain explicitly instead of relying on the client default.
        // The registry URL is the only approved entry point; the redirect it
        // answers with is a single hop to the host's CDN.
        .redirect(reqwest::redirect::Policy::limited(MAX_REDIRECTS))
        .build()
        .map_err(|error| {
            InferenceError::network(format!(
                "The download client could not be created: {}",
                sanitize_reqwest_error(error)
            ))
        })?;

    // The request itself is bounded. `stall_timeout` below covers both header
    // arrival and inter-chunk silence so a silent server can never stall the
    // UI or the Cancel button indefinitely.
    let response = tokio::time::timeout(policy.stall_timeout, client.get(&plan.url).send())
        .await
        .map_err(|_| {
            InferenceError::network(format!(
                "The model host did not answer within {} seconds. Nothing was installed.",
                policy.stall_timeout.as_secs().max(1)
            ))
        })?
        .map_err(|error| {
            InferenceError::network(format!(
                "The model could not be downloaded: {}",
                sanitize_reqwest_error(error)
            ))
        })?;

    // A redirect that leaves TLS is refused before any body byte is written.
    // The digest would catch tampering later, but there is no reason to accept
    // the downgrade at all.
    if policy.require_https && response.url().scheme() != "https" {
        return Err(InferenceError::network(
            "The model host redirected to an insecure connection. No file was written.",
        ));
    }

    let status = response.status();

    if !status.is_success() {
        return Err(InferenceError::network(format!(
            "The model host answered {status}. No file was written."
        )));
    }

    // Validate `Content-Length` against the pinned registry size when advertised.
    // When `Content-Length` is omitted (chunked transfer), report `total = 0` so
    // the UI displays honest received bytes without fabricating a percentage,
    // while strictly enforcing `plan.size_bytes` as the maximum allowed length.
    let advertised_total = match response.content_length() {
        Some(length) if length != plan.size_bytes => {
            return Err(InferenceError::integrity_failed(format!(
                "The host offered {length} bytes, but the approved file is {} bytes. No file was written.",
                plan.size_bytes
            )));
        }
        Some(length) => length,
        None => 0,
    };

    let mut file = tokio::fs::File::create(part_path).await.map_err(|error| {
        InferenceError::storage(format!("The download file could not be created: {error}"))
    })?;

    let mut hasher = Sha256::new();
    let mut received = 0u64;
    let mut emitted_any = false;
    let mut last_event = Instant::now();
    let mut stream = response.bytes_stream();

    let mut stalled = Duration::ZERO;

    loop {
        if cancel.load(Ordering::SeqCst) {
            return Ok(false);
        }

        // Poll in short slices so cancellation responds promptly while the host
        // is silent, and so a dead connection fails instead of hanging forever.
        let next = match tokio::time::timeout(policy.cancel_poll, stream.next()).await {
            Ok(next) => {
                stalled = Duration::ZERO;
                next
            }
            Err(_) => {
                stalled += policy.cancel_poll;

                if stalled >= policy.stall_timeout {
                    return Err(InferenceError::network(format!(
                        "The download received no data for {} seconds. Nothing was installed.",
                        policy.stall_timeout.as_secs().max(1)
                    )));
                }

                continue;
            }
        };

        let Some(chunk) = next else { break };

        let chunk = chunk.map_err(|error| {
            InferenceError::network(format!(
                "The download was interrupted: {}",
                sanitize_reqwest_error(error)
            ))
        })?;

        received = received.saturating_add(chunk.len() as u64);

        if received > plan.size_bytes {
            return Err(InferenceError::integrity_failed(
                "The host sent more data than the approved file contains. Nothing was installed.",
            ));
        }

        hasher.update(&chunk);

        file.write_all(&chunk).await.map_err(|error| {
            InferenceError::storage(format!("The download could not be written: {error}"))
        })?;

        if !emitted_any || last_event.elapsed() >= PROGRESS_INTERVAL || received == plan.size_bytes
        {
            let _ = on_event.send(DownloadEvent::Progress {
                received,
                total: advertised_total,
            });
            emitted_any = true;
            last_event = Instant::now();
        }
    }

    file.flush().await.map_err(|error| {
        InferenceError::storage(format!("The download could not be flushed: {error}"))
    })?;

    // Force the bytes to the device before the rename. An OS crash after the
    // rename must not leave a correct-looking file with an empty tail.
    file.sync_all().await.map_err(|error| {
        InferenceError::storage(format!("The download could not be completed: {error}"))
    })?;

    drop(file);

    if received != plan.size_bytes {
        return Err(InferenceError::integrity_failed(format!(
            "The download ended after {received} of {} bytes. Nothing was installed.",
            plan.size_bytes
        )));
    }

    verifying.store(true, Ordering::SeqCst);
    let _ = on_event.send(DownloadEvent::Verifying {
        total: plan.size_bytes,
    });

    let digest = hex_digest(&hasher.finalize());

    if !digest.eq_ignore_ascii_case(&plan.sha256) {
        return Err(InferenceError::integrity_failed(format!(
            "The downloaded file did not match the approved SHA-256 (expected {}, received {digest}). Nothing was installed.",
            plan.sha256
        )));
    }

    Ok(true)
}

/// Moves a fully received file into place, refusing to overwrite anything
/// (including broken symbolic links).
///
/// Split from the network loop so the rule that matters — never publish a file
/// of the wrong length, never replace an existing model — can be tested without
/// a network.
pub fn publish(part: &Path, final_path: &Path, expected_size: u64) -> InferenceResult<()> {
    let metadata = std::fs::symlink_metadata(part).map_err(|error| {
        InferenceError::storage(format!("The download file is missing: {error}"))
    })?;

    if !metadata.file_type().is_file() || metadata.len() != expected_size {
        return Err(InferenceError::integrity_failed(
            "The download is not the expected size. Nothing was installed.",
        ));
    }

    if std::fs::symlink_metadata(final_path).is_ok() {
        return Err(InferenceError::already_exists(format!(
            "{} already exists. Nexora never overwrites a model file.",
            final_path.display()
        )));
    }

    std::fs::rename(part, final_path)
        .map_err(|error| InferenceError::storage(format!("The model could not be placed: {error}")))
}

/// Hashes a file the user already has, and compares it with the profile.
///
/// A profile with no approved digest cannot be verified: returning a report
/// would imply there is something to compare against. It fails with the same
/// `artifact_pending` code the download uses.
pub fn verify_file(path: &str, profile: &ModelProfile) -> InferenceResult<IntegrityReport> {
    let Some(source) = profile.artifact.source() else {
        return Err(pending_error(profile));
    };

    let trimmed = path.trim();

    if trimmed.is_empty() {
        return Err(InferenceError::not_configured(
            "Choose a model file to verify.",
        ));
    }

    let file_path = Path::new(trimmed);

    let metadata = std::fs::symlink_metadata(file_path)
        .map_err(|_| InferenceError::not_configured(format!("No file exists at {trimmed}.")))?;

    if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
        return Err(InferenceError::not_configured(format!(
            "No regular model file exists at {trimmed}."
        )));
    }

    let size_bytes = metadata.len();

    let sha256 = hash_file(file_path, &AtomicBool::new(false))?;
    let expected = source.sha256;
    let digest_matches = sha256.eq_ignore_ascii_case(expected);

    Ok(IntegrityReport {
        path: trimmed.to_string(),
        file_name: file_path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("model.gguf")
            .to_string(),
        profile_id: profile.id,
        size_bytes,
        size_matches: size_bytes == source.size_bytes,
        sha256,
        expected_size_bytes: source.size_bytes,
        expected_sha256: expected,
        digest_matches,
    })
}

/// Streaming SHA-256 of a file on disk using a bounded 1 MiB heap buffer.
pub fn hash_file(path: &Path, cancel: &AtomicBool) -> InferenceResult<String> {
    let mut file = std::fs::File::open(path).map_err(|error| {
        InferenceError::invalid_model(format!("The file could not be opened: {error}"))
    })?;

    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; HASH_BUFFER_BYTES];

    loop {
        if cancel.load(Ordering::SeqCst) {
            return Err(InferenceError::new(
                "cancelled",
                "The verification was cancelled.",
            ));
        }

        let read = file.read(&mut buffer).map_err(|error| {
            InferenceError::invalid_model(format!("The file could not be read: {error}"))
        })?;

        if read == 0 {
            break;
        }

        hasher.update(&buffer[..read]);
    }

    Ok(hex_digest(&hasher.finalize()))
}

fn hex_digest(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn random_suffix() -> InferenceResult<String> {
    let mut bytes = [0u8; 8];

    getrandom::getrandom(&mut bytes)
        .map_err(|error| InferenceError::internal(format!("No random source: {error}")))?;

    Ok(hex_digest(&bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex as StdMutex};
    use tauri::ipc::InvokeResponseBody;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("nexora-acquisition-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    fn write_file(path: &Path, bytes: &[u8]) {
        let mut file = std::fs::File::create(path).expect("create");
        file.write_all(bytes).expect("write");
    }

    fn sha256_of(bytes: &[u8]) -> String {
        hex_digest(&Sha256::digest(bytes))
    }

    fn event_sink() -> (Channel<DownloadEvent>, Arc<StdMutex<Vec<DownloadEvent>>>) {
        let captured = Arc::new(StdMutex::new(Vec::new()));
        let copy = Arc::clone(&captured);
        let channel = Channel::new(move |body: InvokeResponseBody| {
            if let InvokeResponseBody::Json(raw) = body {
                if let Ok(event) = serde_json::from_str::<DownloadEvent>(&raw) {
                    copy.lock().expect("events lock").push(event);
                }
            }
            Ok(())
        });
        (channel, captured)
    }

    /// A profile with no approved artefact. Built locally so the refusal path
    /// stays covered now that both registered profiles are approved.
    fn pending_profile() -> ModelProfile {
        ModelProfile {
            id: "nexora-test-pending",
            display_name: "Pending Test Model",
            publisher: "test",
            upstream_model: "test/model",
            upstream_revision: None,
            format: "GGUF",
            quantization: "Q4_K_M",
            licence: registry::NEXORA_MINI.licence,
            artifact: ArtifactState::Pending {
                note: "No artefact recorded.",
            },
            context: registry::NEXORA_MINI.context,
            kv_bytes_per_token: registry::NEXORA_MINI.kv_bytes_per_token,
            compatibility_notes: &["test"],
            locally_verified: false,
        }
    }

    #[test]
    fn builds_the_approved_url_from_the_registry() {
        let profile = registry::APPROVED
            .resolve(registry::NEXORA_MINI_ID)
            .expect("profile");
        let plan = plan_for(profile).expect("approved plan");

        assert_eq!(
            plan.url,
            "https://huggingface.co/Qwen/Qwen2.5-Coder-7B-Instruct-GGUF/resolve/13fb94bfda8c8cf22497dc57b78f391a9acb426a/qwen2.5-coder-7b-instruct-q4_k_m.gguf"
        );
        assert_eq!(plan.size_bytes, 4_683_073_536);
        assert_eq!(plan.sha256.len(), 64);
    }

    #[test]
    fn hashes_a_file_with_the_standard_test_vector() {
        let dir = temp_dir("hash");
        let path = dir.join("abc.bin");
        write_file(&path, b"abc");

        let digest = hash_file(&path, &AtomicBool::new(false)).expect("hash");

        assert_eq!(
            digest,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn refuses_to_publish_a_partial_file() {
        let dir = temp_dir("partial");
        let part = dir.join("model.part");
        write_file(&part, b"short");

        let error = publish(&part, &dir.join("model.gguf"), 4096).unwrap_err();

        assert_eq!(error.code, "integrity_failed");
        assert!(part.exists(), "the caller removes the partial file");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn refuses_to_overwrite_an_existing_model() {
        let dir = temp_dir("overwrite");
        let part = dir.join("model.part");
        let final_path = dir.join("model.gguf");
        write_file(&part, b"new");
        write_file(&final_path, b"old");

        let error = publish(&part, &final_path, 3).unwrap_err();

        assert_eq!(error.code, "already_exists");

        let kept = std::fs::read(&final_path).expect("existing file");
        assert_eq!(kept, b"old");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn publishes_a_complete_file() {
        let dir = temp_dir("publish");
        let part = dir.join("model.part");
        let final_path = dir.join("model.gguf");
        write_file(&part, b"complete");

        publish(&part, &final_path, 8).expect("publish");

        assert!(final_path.exists());
        assert!(!part.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn verification_reports_a_mismatch_instead_of_failing() {
        let dir = temp_dir("verify");
        let path = dir.join("model.gguf");
        write_file(&path, b"not the approved file");

        let profile = registry::APPROVED
            .resolve(registry::NEXORA_MINI_ID)
            .expect("profile");
        let report = verify_file(path.to_str().expect("path"), profile).expect("report");

        assert!(!report.size_matches);
        assert!(!report.digest_matches);
        assert_eq!(report.expected_size_bytes, 4_683_073_536);
        assert_eq!(report.profile_id, registry::NEXORA_MINI_ID);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn verification_refuses_a_missing_file() {
        let profile = registry::APPROVED
            .resolve(registry::NEXORA_MINI_ID)
            .expect("profile");
        let error = verify_file("/definitely/not/here.gguf", profile).unwrap_err();

        assert_eq!(error.code, "not_configured");
    }

    #[test]
    fn a_pending_artefact_has_no_download_plan() {
        let profile = pending_profile();
        let error = plan_for(&profile).unwrap_err();

        assert_eq!(error.code, "artifact_pending");
        assert!(error.message.contains("Pending Test Model"));
    }

    #[test]
    fn the_sonnet_plan_points_at_the_pinned_artefact() {
        let profile = registry::APPROVED
            .resolve(registry::NEXORA_SONNET_ID)
            .expect("sonnet profile");
        let plan = plan_for(profile).expect("approved plan");

        assert_eq!(
            plan.url,
            "https://huggingface.co/Qwen/Qwen2.5-Coder-14B-Instruct-GGUF/resolve/d0a692ef765eefbf2fabb130b3cb2e8917e3d225/qwen2.5-coder-14b-instruct-q4_k_m.gguf"
        );
        assert_eq!(plan.file_name, "qwen2.5-coder-14b-instruct-q4_k_m.gguf");
        assert_eq!(plan.size_bytes, 8_988_110_272);
        assert_eq!(plan.sha256.len(), 64);
        assert!(plan.url.starts_with("https://"));
    }

    #[test]
    fn a_pending_artefact_cannot_be_verified() {
        let profile = pending_profile();
        let error = verify_file("/tmp/whatever.gguf", &profile).unwrap_err();

        assert_eq!(error.code, "artifact_pending");
    }

    #[test]
    fn an_installed_artefact_is_found_only_at_the_expected_length() {
        let dir = temp_dir("installed");
        let profile = registry::APPROVED
            .resolve(registry::NEXORA_MINI_ID)
            .expect("profile");
        let expected = profile.artifact.source().expect("source");
        let right_name = dir.join(expected.file_name);

        // Absent file.
        assert!(installed_path(profile, std::slice::from_ref(&right_name)).is_none());

        // Present but the wrong length: a truncated download must not look
        // installed.
        write_file(&right_name, b"short");
        assert!(installed_path(profile, std::slice::from_ref(&right_name)).is_none());

        // A different file name is never accepted, however large.
        let renamed = dir.join("some-other-model.gguf");
        write_file(&renamed, b"short");
        assert!(installed_path(profile, &[renamed]).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_file_is_attributed_to_the_registered_profile_by_name() {
        let profile = registry::APPROVED
            .resolve(registry::NEXORA_MINI_ID)
            .expect("profile");
        let path = std::path::PathBuf::from("/models/qwen2.5-coder-7b-instruct-q4_k_m.gguf");

        assert_eq!(
            profile_for_path(&path).map(|found| found.id),
            Some(profile.id)
        );
        assert!(profile_for_path(Path::new("/models/unknown.gguf")).is_none());
    }

    #[test]
    fn a_pending_profile_never_looks_installed() {
        let profile = pending_profile();

        assert!(installed_path(
            &profile,
            &[std::path::PathBuf::from("/models/anything.gguf")]
        )
        .is_none());
    }

    #[test]
    fn the_sonnet_artefact_is_found_only_at_its_published_length() {
        let profile = registry::APPROVED
            .resolve(registry::NEXORA_SONNET_ID)
            .expect("sonnet profile");
        let source = profile.artifact.source().expect("source");
        let dir = temp_dir("sonnet-installed");
        let path = dir.join(source.file_name);

        // Absent.
        assert!(installed_path(profile, std::slice::from_ref(&path)).is_none());

        // Present with the wrong length: a truncated 8.99 GB file must not
        // read as installed.
        write_file(&path, b"not the real model");
        assert!(installed_path(profile, std::slice::from_ref(&path)).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn validates_destination_paths_filenames_symlinks_and_redacts_signed_urls() {
        let dir = temp_dir("path-safety");
        let regular_file = dir.join("existing-file");
        write_file(&regular_file, b"file");

        // Relative, root, parent-traversal, reserved-device, and file destinations are rejected.
        assert_eq!(
            validate_download_directory(Path::new("relative/models"))
                .unwrap_err()
                .code,
            "invalid_request"
        );
        assert_eq!(
            validate_download_directory(Path::new("/"))
                .unwrap_err()
                .code,
            "invalid_request"
        );
        assert_eq!(
            validate_download_directory(&dir.join("sub").join("..").join("escape"))
                .unwrap_err()
                .code,
            "invalid_request"
        );
        assert_eq!(
            validate_download_directory(&dir.join("CON"))
                .unwrap_err()
                .code,
            "invalid_request"
        );
        assert_eq!(
            validate_download_directory(&regular_file).unwrap_err().code,
            "invalid_request"
        );

        #[cfg(unix)]
        {
            let real_sub = dir.join("real");
            std::fs::create_dir_all(&real_sub).expect("create real dir");
            let symlink_dir = dir.join("symlink-models");
            std::os::unix::fs::symlink(&real_sub, &symlink_dir).expect("create symlink");
            assert_eq!(
                validate_download_directory(&symlink_dir).unwrap_err().code,
                "invalid_request"
            );
        }

        // Unsafe artifact names are rejected.
        assert!(validate_artifact_file_name("../evil.gguf").is_err());
        assert!(validate_artifact_file_name("nested/model.gguf").is_err());
        assert!(validate_artifact_file_name(".hidden.gguf").is_err());
        assert!(validate_artifact_file_name("NUL.gguf").is_err());
        assert!(validate_artifact_file_name("not-gguf.bin").is_err());
        assert!(validate_artifact_file_name("qwen2.5-coder-7b-instruct-q4_k_m.gguf").is_ok());

        // Signed CDN URLs, query parameters, userinfo, and Bearer tokens never leak.
        let leaked = "error sending request for url (https://token_user:supersecret@cdn-lfs.hf.co/repos/ab/cd?X-Amz-Security-Token=SECRET_SESSION_TOKEN&X-Amz-Signature=deadbeef999&token=hf_secret123): Authorization: Bearer sk-live-secret";
        let scrubbed = redact_sensitive_transport_text(leaked);
        assert!(!scrubbed.contains("SECRET_SESSION_TOKEN"), "{scrubbed}");
        assert!(!scrubbed.contains("deadbeef999"), "{scrubbed}");
        assert!(!scrubbed.contains("hf_secret123"), "{scrubbed}");
        assert!(!scrubbed.contains("supersecret"), "{scrubbed}");
        assert!(!scrubbed.contains("sk-live-secret"), "{scrubbed}");
        assert!(scrubbed.contains("?<redacted>"), "{scrubbed}");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn streaming_download_handles_chunked_unknown_length_digest_verification_and_https_guard()
    {
        let dir = temp_dir("streaming-ok");
        // Seed a stale `.part` file from a prior crash; download must purge it cleanly,
        // while keeping unrelated user files intact.
        let stale_part = dir.join(".fixture-model.gguf.0123456789abcdef.part");
        let unrelated_user_file = dir.join("keep-my-model.gguf");
        write_file(&stale_part, b"stale partial bytes");
        write_file(&unrelated_user_file, b"precious user bytes");

        let payload: Vec<u8> = (0..65_536u32).map(|i| (i % 251) as u8).collect();
        let expected_sha = sha256_of(&payload);

        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock http");
        let port = listener.local_addr().expect("addr").port();
        let body_copy = payload.clone();

        // Serve HTTP/1.1 chunked transfer WITHOUT `Content-Length` to verify both
        // unknown content length handling (`total == 0`) AND exact digest promotion.
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut req = [0u8; 1024];
            let _ = stream.read(&mut req);
            stream
                .write_all(
                    b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n",
                )
                .expect("headers");
            for chunk in body_copy.chunks(8_192) {
                write!(stream, "{:X}\r\n", chunk.len()).expect("chunk size");
                stream.write_all(chunk).expect("chunk payload");
                stream.write_all(b"\r\n").expect("chunk crlf");
            }
            stream.write_all(b"0\r\n\r\n").expect("eof chunk");
        });

        let plan = DownloadPlan {
            url: format!("http://127.0.0.1:{port}/fixture-model.gguf"),
            file_name: "fixture-model.gguf".to_string(),
            size_bytes: payload.len() as u64,
            sha256: expected_sha.clone(),
        };

        // 1. Production policy strictly rejects non-HTTPS URLs before opening sockets or files.
        let (prod_ch, _) = event_sink();
        let https_err = download(
            &plan,
            &dir,
            &AtomicBool::new(false),
            &AtomicBool::new(false),
            &prod_ch,
        )
        .await
        .expect_err("production policy must block plain http");
        assert_eq!(https_err.code, "network");

        // 2. Controlled loopback test policy downloads chunked stream, reports unknown total (`0`),
        //    emits `Verifying`, promotes file atomically, and cleans stale `.part`.
        let (ch, events) = event_sink();
        let verifying = AtomicBool::new(false);
        let outcome = download_with_policy(
            &plan,
            &dir,
            &AtomicBool::new(false),
            &verifying,
            &ch,
            DownloadPolicy::loopback_for_tests(Duration::from_secs(2)),
        )
        .await
        .expect("chunked streaming download succeeds");

        server.join().expect("server thread");

        assert!(!outcome.cancelled);
        let final_path = dir.join("fixture-model.gguf");
        assert_eq!(
            outcome.path.as_deref(),
            Some(final_path.to_string_lossy().as_ref())
        );
        assert_eq!(outcome.size_bytes, payload.len() as u64);
        assert_eq!(outcome.sha256, expected_sha);
        assert_eq!(std::fs::read(&final_path).expect("read final"), payload);
        assert!(!stale_part.exists(), "stale .part file cleaned up");
        assert_eq!(
            std::fs::read(&unrelated_user_file).expect("user file preserved"),
            b"precious user bytes"
        );

        let recorded = events.lock().expect("lock events").clone();
        assert!(
            recorded.iter().any(|ev| matches!(
                ev,
                DownloadEvent::Progress {
                    received,
                    total: 0
                } if *received > 0
            )),
            "unknown Content-Length must report total = 0 without fabricating total: {recorded:?}"
        );
        assert_eq!(
            recorded.last(),
            Some(&DownloadEvent::Verifying {
                total: payload.len() as u64
            })
        );

        // 3. Re-running `download_with_policy` when `final_path` exists never overwrites or deletes it.
        let collision_err = download_with_policy(
            &plan,
            &dir,
            &AtomicBool::new(false),
            &AtomicBool::new(false),
            &ch,
            DownloadPolicy::loopback_for_tests(Duration::from_secs(2)),
        )
        .await
        .expect_err("existing model must never be overwritten");
        assert_eq!(collision_err.code, "already_exists");
        assert_eq!(std::fs::read(&final_path).expect("still intact"), payload);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn streaming_download_rejects_bad_digest_bad_size_interruption_timeout_and_cancellation()
    {
        let dir = temp_dir("streaming-failures");
        let good_bytes = b"0123456789abcdef0123456789abcdef";
        let good_sha = sha256_of(good_bytes);

        // Helper spawning a 1-shot controlled TCP server returning raw HTTP bytes.
        let serve_once = |raw_response: Vec<u8>, hold_ms: u64| -> u16 {
            let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
            let port = listener.local_addr().expect("addr").port();
            std::thread::spawn(move || {
                if let Ok((mut stream, _)) = listener.accept() {
                    let mut buf = [0u8; 1024];
                    let _ = stream.read(&mut buf);
                    let _ = stream.write_all(&raw_response);
                    let _ = stream.flush();
                    if hold_ms > 0 {
                        std::thread::sleep(Duration::from_millis(hold_ms));
                    }
                }
            });
            port
        };

        // Case 1: Wrong `Content-Length` rejected before writing anything.
        let p1 = serve_once(
            b"HTTP/1.1 200 OK\r\nContent-Length: 999\r\nConnection: close\r\n\r\nwrong".to_vec(),
            0,
        );
        let (ch, _) = event_sink();
        let err = download_with_policy(
            &DownloadPlan {
                url: format!("http://127.0.0.1:{p1}/m.gguf"),
                file_name: "m.gguf".into(),
                size_bytes: good_bytes.len() as u64,
                sha256: good_sha.clone(),
            },
            &dir,
            &AtomicBool::new(false),
            &AtomicBool::new(false),
            &ch,
            DownloadPolicy::loopback_for_tests(Duration::from_secs(1)),
        )
        .await
        .unwrap_err();
        assert_eq!(err.code, "integrity_failed");
        assert!(!dir.join("m.gguf").exists());

        // Case 2: Exact length but corrupted SHA-256 rejected BEFORE promotion.
        let mut corrupted = *good_bytes;
        corrupted[0] ^= 0xFF;
        let mut resp2 = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            corrupted.len()
        )
        .into_bytes();
        resp2.extend_from_slice(&corrupted);
        let p2 = serve_once(resp2, 0);
        let err = download_with_policy(
            &DownloadPlan {
                url: format!("http://127.0.0.1:{p2}/m.gguf"),
                file_name: "m.gguf".into(),
                size_bytes: good_bytes.len() as u64,
                sha256: good_sha.clone(),
            },
            &dir,
            &AtomicBool::new(false),
            &AtomicBool::new(false),
            &ch,
            DownloadPolicy::loopback_for_tests(Duration::from_secs(1)),
        )
        .await
        .unwrap_err();
        assert_eq!(err.code, "integrity_failed");
        assert!(!dir.join("m.gguf").exists());

        // Case 3: Interrupted mid-stream transfer (connection drops after 10 of 32 bytes).
        let mut resp3 = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            good_bytes.len()
        )
        .into_bytes();
        resp3.extend_from_slice(&good_bytes[..10]);
        let p3 = serve_once(resp3, 0);
        let err = download_with_policy(
            &DownloadPlan {
                url: format!("http://127.0.0.1:{p3}/m.gguf"),
                file_name: "m.gguf".into(),
                size_bytes: good_bytes.len() as u64,
                sha256: good_sha.clone(),
            },
            &dir,
            &AtomicBool::new(false),
            &AtomicBool::new(false),
            &ch,
            DownloadPolicy::loopback_for_tests(Duration::from_secs(1)),
        )
        .await
        .unwrap_err();
        assert!(matches!(err.code, "network" | "integrity_failed"));
        assert!(!dir.join("m.gguf").exists());

        // Case 4: Stalled stream (headers sent, body stalls past `stall_timeout`).
        let p4 = serve_once(
            format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n",
                good_bytes.len()
            )
            .into_bytes(),
            600,
        );
        let err = download_with_policy(
            &DownloadPlan {
                url: format!("http://127.0.0.1:{p4}/m.gguf"),
                file_name: "m.gguf".into(),
                size_bytes: good_bytes.len() as u64,
                sha256: good_sha.clone(),
            },
            &dir,
            &AtomicBool::new(false),
            &AtomicBool::new(false),
            &ch,
            DownloadPolicy::loopback_for_tests(Duration::from_millis(120)),
        )
        .await
        .unwrap_err();
        assert_eq!(err.code, "network");
        assert!(!dir.join("m.gguf").exists());

        // Case 5: Mid-stream cancellation cleans up `.part` and marks `cancelled: true` without installing.
        let cancel = Arc::new(AtomicBool::new(false));
        let p5 = serve_once(
            format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n1234",
                good_bytes.len()
            )
            .into_bytes(),
            1_000,
        );
        let cancel_trigger = Arc::clone(&cancel);
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(40)).await;
            cancel_trigger.store(true, Ordering::SeqCst);
        });
        let cancelled = download_with_policy(
            &DownloadPlan {
                url: format!("http://127.0.0.1:{p5}/m.gguf"),
                file_name: "m.gguf".into(),
                size_bytes: good_bytes.len() as u64,
                sha256: good_sha,
            },
            &dir,
            &cancel,
            &AtomicBool::new(false),
            &ch,
            DownloadPolicy::loopback_for_tests(Duration::from_secs(2)),
        )
        .await
        .expect("cancellation returns clean outcome");
        assert!(cancelled.cancelled);
        assert!(cancelled.path.is_none());
        assert!(!dir.join("m.gguf").exists());

        // Case 6: Redirect to signed CDN URL that fails never leaks signed query parameters.
        let p6_dead_port = {
            let dead = TcpListener::bind("127.0.0.1:0").expect("ephemeral");
            dead.local_addr().expect("addr").port()
        };
        let p6 = serve_once(
            format!(
                "HTTP/1.1 302 Found\r\nLocation: http://cdn_user:cdn_pass@127.0.0.1:{p6_dead_port}/weights.gguf?X-Amz-Signature=TOPSECRET_SIG_99&token=HF_PRIV_TOK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            )
            .into_bytes(),
            0,
        );
        let redirect_err = download_with_policy(
            &DownloadPlan {
                url: format!("http://127.0.0.1:{p6}/m.gguf"),
                file_name: "m.gguf".into(),
                size_bytes: 32,
                sha256: "00".repeat(32),
            },
            &dir,
            &AtomicBool::new(false),
            &AtomicBool::new(false),
            &ch,
            DownloadPolicy::loopback_for_tests(Duration::from_secs(1)),
        )
        .await
        .unwrap_err();
        assert!(!redirect_err.message.contains("TOPSECRET_SIG_99"));
        assert!(!redirect_err.message.contains("HF_PRIV_TOK"));
        assert!(!redirect_err.message.contains("cdn_pass"));

        let _ = std::fs::remove_dir_all(&dir);
    }
}

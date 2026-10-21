//! Verified acquisition of the approved model artefact.
//!
//! Nexora never downloads anything on its own. A download starts only when the
//! user asks for it, writes into a private `.part` file inside the destination
//! folder, hashes the bytes while receiving them, and renames into place only
//! after the count and the SHA-256 match the registry. A failed, cancelled, or
//! interrupted download therefore never leaves a file that looks installed.
//!
//! Nothing here launches a process, touches runtime settings, or rewrites the
//! user's configured model path. Publishing a file is the end of this module's
//! responsibility; choosing it is still an explicit user action.

use std::io::Read;
use std::path::Path;
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
/// Read size when hashing an existing file.
const HASH_BUFFER_BYTES: usize = 1024 * 1024;
/// The network stream is polled in slices of this length. Without it, a host
/// that accepts the connection and then goes silent would leave `next()` — and
/// therefore the Cancel button — waiting indefinitely.
const CANCEL_POLL: Duration = Duration::from_secs(1);
/// No bytes for this long is treated as a failed transfer, not a slow one.
/// 4.7 GB over a slow link still delivers a chunk far more often than this.
const STALL_TIMEOUT: Duration = Duration::from_secs(60);
/// Redirect hops allowed before the request is refused. The approved URL is
/// answered with a single hop to the model host's CDN; a longer chain means
/// something other than the approved download is being followed.
const MAX_REDIRECTS: usize = 5;

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
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DownloadEvent {
    Progress { received: u64, total: u64 },
    Verifying { total: u64 },
}

#[derive(Debug, Clone, Serialize)]
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
#[derive(Debug, Clone, Serialize)]
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
            std::fs::metadata(candidate)
                .map(|metadata| metadata.len() == source.size_bytes)
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
pub async fn download(
    plan: &DownloadPlan,
    directory: &Path,
    cancel: &AtomicBool,
    on_event: &Channel<DownloadEvent>,
) -> InferenceResult<DownloadOutcome> {
    let final_path = directory.join(&plan.file_name);

    if final_path.exists() {
        return Err(InferenceError::already_exists(format!(
            "{} already exists. Nexora never overwrites a model file; move or remove it first.",
            final_path.display()
        )));
    }

    let part_path = directory.join(format!(".{}.{}.part", plan.file_name, random_suffix()?));

    let completed = match receive(plan, &part_path, cancel, on_event).await {
        Ok(completed) => completed,
        Err(error) => {
            // A failed download must not leave a partial file behind.
            let _ = tokio::fs::remove_file(&part_path).await;
            return Err(error);
        }
    };

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
    on_event: &Channel<DownloadEvent>,
) -> InferenceResult<bool> {
    let client = reqwest::Client::builder()
        .user_agent(concat!("Nexora/", env!("CARGO_PKG_VERSION")))
        // Bound the chain explicitly instead of relying on the client default.
        // The registry URL is the only approved entry point; the redirect it
        // answers with is a single hop to the host's CDN.
        .redirect(reqwest::redirect::Policy::limited(MAX_REDIRECTS))
        .build()
        .map_err(|error| {
            InferenceError::network(format!("The download client could not be created: {error}"))
        })?;

    // The request itself is bounded. `STALL_TIMEOUT` below only covers the
    // body: a host that accepts the connection and then never sends response
    // headers would otherwise leave `send()` — and the Cancel button — waiting
    // forever.
    let response = tokio::time::timeout(STALL_TIMEOUT, client.get(&plan.url).send())
        .await
        .map_err(|_| {
            InferenceError::network(format!(
                "The model host did not answer within {} seconds. Nothing was installed.",
                STALL_TIMEOUT.as_secs()
            ))
        })?
        .map_err(|error| {
            InferenceError::network(format!("The model could not be downloaded: {error}"))
        })?;

    // A redirect that leaves TLS is refused before any body byte is written.
    // The digest would catch tampering later, but there is no reason to accept
    // the downgrade at all.
    if response.url().scheme() != "https" {
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

    // A redirect to a captive portal or an HTML error page must not be saved
    // as a model. The registry knows the exact length, so a mismatch is fatal
    // before a single byte is written.
    if let Some(length) = response.content_length() {
        if length != plan.size_bytes {
            return Err(InferenceError::integrity_failed(format!(
                "The host offered {length} bytes, but the approved file is {} bytes. No file was written.",
                plan.size_bytes
            )));
        }
    }

    let mut file = tokio::fs::File::create(part_path).await.map_err(|error| {
        InferenceError::storage(format!("The download file could not be created: {error}"))
    })?;

    let mut hasher = Sha256::new();
    let mut received = 0u64;
    let mut last_event = Instant::now();
    let mut stream = response.bytes_stream();

    let mut stalled = Duration::ZERO;

    loop {
        if cancel.load(Ordering::SeqCst) {
            return Ok(false);
        }

        // Poll in one-second slices so cancellation works while the host is
        // silent, and so a dead connection fails instead of hanging forever.
        let next = match tokio::time::timeout(CANCEL_POLL, stream.next()).await {
            Ok(next) => {
                stalled = Duration::ZERO;
                next
            }
            Err(_) => {
                stalled += CANCEL_POLL;

                if stalled >= STALL_TIMEOUT {
                    return Err(InferenceError::network(format!(
                        "The download received no data for {} seconds. Nothing was installed.",
                        STALL_TIMEOUT.as_secs()
                    )));
                }

                continue;
            }
        };

        let Some(chunk) = next else { break };

        let chunk = chunk.map_err(|error| {
            InferenceError::network(format!("The download was interrupted: {error}"))
        })?;

        received += chunk.len() as u64;

        if received > plan.size_bytes {
            return Err(InferenceError::integrity_failed(
                "The host sent more data than the approved file contains.",
            ));
        }

        hasher.update(&chunk);

        file.write_all(&chunk).await.map_err(|error| {
            InferenceError::storage(format!("The download could not be written: {error}"))
        })?;

        if last_event.elapsed() >= PROGRESS_INTERVAL || received == plan.size_bytes {
            let _ = on_event.send(DownloadEvent::Progress {
                received,
                total: plan.size_bytes,
            });
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

    let _ = on_event.send(DownloadEvent::Verifying {
        total: plan.size_bytes,
    });

    let digest = hex_digest(&hasher.finalize());

    if digest != plan.sha256 {
        return Err(InferenceError::integrity_failed(format!(
            "The downloaded file did not match the approved SHA-256 (expected {}, received {digest}). Nothing was installed.",
            plan.sha256
        )));
    }

    Ok(true)
}

/// Moves a fully received file into place, refusing to overwrite anything.
///
/// Split from the network loop so the rule that matters — never publish a file
/// of the wrong length, never replace an existing model — can be tested without
/// a network.
pub fn publish(part: &Path, final_path: &Path, expected_size: u64) -> InferenceResult<()> {
    let metadata = std::fs::metadata(part).map_err(|error| {
        InferenceError::storage(format!("The download file is missing: {error}"))
    })?;

    if metadata.len() != expected_size {
        return Err(InferenceError::integrity_failed(
            "The download is not the expected size. Nothing was installed.",
        ));
    }

    if final_path.exists() {
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

    if !file_path.is_file() {
        return Err(InferenceError::not_configured(format!(
            "No file exists at {trimmed}."
        )));
    }

    let size_bytes = std::fs::metadata(file_path)
        .map_err(|error| InferenceError::invalid_model(error.to_string()))?
        .len();

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

/// Streaming SHA-256 of a file on disk.
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

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("nexora-acquisition-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    fn write_file(path: &Path, bytes: &[u8]) {
        let mut file = std::fs::File::create(path).expect("create");
        file.write_all(bytes).expect("write");
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
    }

    #[test]
    fn refuses_to_publish_a_partial_file() {
        let dir = temp_dir("partial");
        let part = dir.join("model.part");
        write_file(&part, b"short");

        let error = publish(&part, &dir.join("model.gguf"), 4096).unwrap_err();

        assert_eq!(error.code, "integrity_failed");
        assert!(part.exists(), "the caller removes the partial file");
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
        assert!(installed_path(profile, &[right_name.clone()]).is_none());

        // Present but the wrong length: a truncated download must not look
        // installed.
        write_file(&right_name, b"short");
        assert!(installed_path(profile, &[right_name.clone()]).is_none());

        // A different file name is never accepted, however large.
        let renamed = dir.join("some-other-model.gguf");
        write_file(&renamed, b"short");
        assert!(installed_path(profile, &[renamed]).is_none());
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
        assert!(installed_path(profile, &[path.clone()]).is_none());

        // Present with the wrong length: a truncated 8.99 GB file must not
        // read as installed.
        write_file(&path, b"not the real model");
        assert!(installed_path(profile, &[path.clone()]).is_none());
    }
}

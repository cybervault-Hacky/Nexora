use std::ffi::OsString;
use std::fmt;
use std::io::{ErrorKind, Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use super::error::{InferenceError, InferenceResult};
use super::models;

/// Loopback bind address enforced for every local runtime process.
///
/// Nexora never binds `llama-server` to `0.0.0.0` or any network interface.
pub const LOOPBACK_HOST: &str = "127.0.0.1";

/// How long the engine may take to load a model before we call it a failure.
pub const LOAD_TIMEOUT: Duration = Duration::from_secs(180);
/// The llama.cpp build the integration was written against (see docs/phase-2).
///
/// Nexora never downloads a runtime, so this is a report, not an enforcement:
/// a mismatch is surfaced to the user instead of being silently tolerated.
pub const PINNED_BUILD: &str = "11714";
/// Session key file name inside the app config directory.
///
/// Public so startup can remove a key left behind by an unclean exit: the key
/// is useless without the process that read it, and a secret should not sit on
/// disk across runs.
pub const SESSION_KEY_FILE: &str = "runtime-session.key";
/// How long a stop may wait for the child to be reaped.
///
/// `wait()` on a child whose `kill()` failed blocks forever, and this runs
/// while the supervisor mutex is held, so shutdown is bounded instead.
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);
const STDERR_FLUSH_TIMEOUT: Duration = Duration::from_millis(250);
const STDERR_CAP: u64 = 64 * 1024;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Per-launch bearer secret shared solely between Nexora and its child `llama-server`.
///
/// Redacts itself in `Debug` so formatting runtime structs can never leak the credential.
#[derive(Clone, PartialEq, Eq)]
pub struct RuntimeSecret(String);

impl RuntimeSecret {
    pub fn new(raw: impl Into<String>) -> Self {
        Self(raw.into())
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Debug for RuntimeSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RuntimeSecret(<redacted>)")
    }
}

/// Scrubs any raw runtime session credential or `Bearer ...` token from text
/// surfaced to error messages, diagnostics, or UI status banners.
pub fn scrub_runtime_secret(text: &str, secret: Option<&str>) -> String {
    let mut scrubbed = match secret.filter(|s| !s.is_empty()) {
        Some(raw) => text.replace(raw, "<redacted>"),
        None => text.to_string(),
    };

    // Guard against any header reflection (`Bearer <token>`) in error payloads.
    for prefix in ["Bearer ", "bearer "] {
        let mut cursor = 0usize;
        while let Some(rel) = scrubbed[cursor..].find(prefix) {
            let token_start = cursor + rel + prefix.len();
            let token_end = scrubbed[token_start..]
                .find(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | ',' | ';' | ')'))
                .map(|len| token_start + len)
                .unwrap_or(scrubbed.len());

            if token_end > token_start && &scrubbed[token_start..token_end] != "<redacted>" {
                scrubbed.replace_range(token_start..token_end, "<redacted>");
                cursor = token_start + "<redacted>".len();
            } else {
                cursor = token_end;
            }
        }
    }

    scrubbed
}

/// Optional directories checked when no explicit `runtime_path` is set in settings.
#[derive(Debug, Clone, Default)]
pub struct RuntimeSearchDirs {
    pub exe_dir: Option<PathBuf>,
    pub resource_dir: Option<PathBuf>,
    pub data_dir: Option<PathBuf>,
}

fn candidate_binary_names() -> &'static [&'static str] {
    #[cfg(windows)]
    {
        &[
            "llama-server.exe",
            "llama-server-x86_64-pc-windows-msvc.exe",
        ]
    }

    #[cfg(all(unix, target_arch = "x86_64", target_os = "linux"))]
    {
        &["llama-server", "llama-server-x86_64-unknown-linux-gnu"]
    }

    #[cfg(all(unix, not(all(target_arch = "x86_64", target_os = "linux"))))]
    {
        &["llama-server"]
    }
}

/// Searches managed application bundle/data folders for a sidecar runtime executable.
///
/// Ambient `PATH` and current working directories are deliberately ignored so an
/// untrusted binary in a project directory can never be launched automatically.
pub fn discover_sidecar(dirs: &RuntimeSearchDirs) -> Result<Option<PathBuf>, InferenceError> {
    let mut locations: Vec<PathBuf> = Vec::new();

    if let Some(ref exe_dir) = dirs.exe_dir {
        locations.push(exe_dir.clone());
        locations.push(exe_dir.join("binaries"));
        locations.push(exe_dir.join("runtime"));
    }

    if let Some(ref res_dir) = dirs.resource_dir {
        locations.push(res_dir.clone());
        locations.push(res_dir.join("binaries"));
        locations.push(res_dir.join("runtime"));
    }

    if let Some(ref data_dir) = dirs.data_dir {
        locations.push(data_dir.join("runtime"));
        locations.push(data_dir.join("binaries"));
    }

    let mut first_incompatible: Option<InferenceError> = None;

    for dir in locations {
        for name in candidate_binary_names() {
            let candidate = dir.join(name);
            if !candidate.exists() {
                continue;
            }

            let as_str = candidate.to_string_lossy();
            match models::looks_like_executable(&as_str) {
                Ok(()) => return Ok(Some(candidate)),
                Err(err) => {
                    if first_incompatible.is_none() {
                        first_incompatible = Some(err);
                    }
                }
            }
        }
    }

    if let Some(err) = first_incompatible {
        return Err(err);
    }

    Ok(None)
}

/// Resolves and validates the local sidecar executable path.
///
/// An explicit user setting always takes precedence and fails loudly if missing
/// or incompatible rather than silently substituting another binary.
pub fn resolve_runtime_path(
    configured: Option<&str>,
    dirs: &RuntimeSearchDirs,
) -> InferenceResult<String> {
    if let Some(explicit) = configured.map(str::trim).filter(|p| !p.is_empty()) {
        models::looks_like_executable(explicit)?;
        return Ok(explicit.to_string());
    }

    match discover_sidecar(dirs)? {
        Some(found) => Ok(found.to_string_lossy().into_owned()),
        None => Err(InferenceError::not_configured(
            "Choose the llama-server executable before loading.",
        )),
    }
}

/// Builds structured launch arguments for `llama-server`.
///
/// Kept pure so tests verify directly that `--host 127.0.0.1` is enforced and
/// that no raw session secret appears anywhere in process `argv`.
pub fn build_launch_args(
    model_path: &str,
    port: u16,
    context_size: u32,
    key_path: &Path,
) -> Vec<OsString> {
    vec![
        OsString::from("--model"),
        OsString::from(model_path),
        OsString::from("--host"),
        OsString::from(LOOPBACK_HOST),
        OsString::from("--port"),
        OsString::from(port.to_string()),
        OsString::from("--ctx-size"),
        OsString::from(context_size.to_string()),
        OsString::from("--parallel"),
        OsString::from("1"),
        OsString::from("--no-ui"),
        OsString::from("--jinja"),
        OsString::from("--api-key-file"),
        key_path.as_os_str().to_os_string(),
        OsString::from("--timeout"),
        OsString::from("600"),
    ]
}

fn map_spawn_io_error(error: std::io::Error) -> InferenceError {
    #[cfg(unix)]
    if error.raw_os_error() == Some(8) {
        return InferenceError::incompatible_runtime(format!(
            "The runtime executable format is incompatible with this machine ({error})."
        ));
    }

    #[cfg(windows)]
    if matches!(error.raw_os_error(), Some(193) | Some(216)) {
        return InferenceError::incompatible_runtime(format!(
            "The runtime executable is not compatible with this Windows installation ({error})."
        ));
    }

    match error.kind() {
        ErrorKind::NotFound => InferenceError::runtime_missing(format!(
            "The engine executable was not found ({error})."
        )),
        ErrorKind::PermissionDenied => InferenceError::incompatible_runtime(format!(
            "The runtime file does not have permission to execute ({error})."
        )),
        _ => InferenceError::spawn_failed(format!(
            "The engine could not be started. Check that the runtime path is correct. ({error})"
        )),
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeInfo {
    pub path: String,
    pub version_line: String,
    /// Whether the reported build matches the pinned target.
    pub pinned_build_matched: bool,
}

/// Result of probing the loopback `/health` endpoint on `llama-server`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HealthProbe {
    /// Engine answered `200 OK` with `{"status": "ok"}`.
    Ready,
    /// Engine has not bound the socket yet or answered `503` while loading weights.
    Loading,
    /// Engine rejected our per-launch session credential (`401`/`403`).
    AuthFailed,
    /// Port answered with an unexpected status or non-`llama-server` body.
    Unhealthy(String),
}

struct RunningServer {
    child: Child,
    port: u16,
    key: RuntimeSecret,
    /// Removed when the engine stops, so a session key never outlives its use.
    key_path: PathBuf,
    stderr: Arc<Mutex<String>>,
    /// Signals once the stderr pipe reaches EOF so crash output is captured reliably.
    stderr_done: Option<mpsc::Receiver<()>>,
    /// Keeps the verified model file unchanged while the child may still open it.
    _model_file: models::LockedFile,
}

/// Owns the single `llama-server` child process.
///
/// There is deliberately one slot: the product serves one conversation at a
/// time, and a second large model is never loaded implicitly.
pub struct Supervisor {
    running: Option<RunningServer>,
    pub last_error: Option<String>,
}

impl Default for Supervisor {
    fn default() -> Self {
        Self::new()
    }
}

impl Supervisor {
    pub fn new() -> Self {
        Self {
            running: None,
            last_error: None,
        }
    }

    pub fn endpoint(&self) -> Option<(u16, String)> {
        self.running
            .as_ref()
            .map(|server| (server.port, server.key.as_str().to_string()))
    }

    fn flush_exited_stderr(&mut self) {
        if let Some(rx) = self
            .running
            .as_mut()
            .and_then(|server| server.stderr_done.take())
        {
            let _ = rx.recv_timeout(STDERR_FLUSH_TIMEOUT);
        }
    }

    /// Reaps a crashed child and records why it died.
    pub fn is_running(&mut self) -> bool {
        let status = match self.running.as_mut() {
            None => return false,
            Some(server) => server.child.try_wait(),
        };

        match status {
            Ok(None) => true,
            Ok(Some(exit)) => {
                self.flush_exited_stderr();
                let tail = self.stderr_tail();
                let reason = format!("The local engine stopped unexpectedly ({exit}).{tail}");
                self.last_error = Some(reason.clone());
                if let Err(stop_error) = self.stop() {
                    self.last_error = Some(format!(
                        "{reason} Shutdown was not confirmed: {}",
                        stop_error.message
                    ));
                }
                false
            }
            Err(error) => {
                let reason = format!(
                    "The local engine could not be checked; shutdown confirmation is required: {error}"
                );
                self.last_error = Some(reason.clone());
                if let Err(stop_error) = self.stop() {
                    self.last_error = Some(format!(
                        "{reason} Shutdown was not confirmed: {}",
                        stop_error.message
                    ));
                    // An inaccessible child is treated as possibly alive. This
                    // keeps loaded attribution and the file guard from being
                    // discarded based on an unverified status read.
                    true
                } else {
                    false
                }
            }
        }
    }

    /// Spawns the runtime with structured arguments only.
    ///
    /// No shell is involved, no argument is interpolated into a command line,
    /// and the API key is passed through a file rather than argv.
    /// Stops any prior child before starting the runtime with a retained model
    /// file guard. A failed stop returns without spawning a replacement.
    pub fn start(
        &mut self,
        runtime_path: &str,
        model_file: models::LockedFile,
        context_size: u32,
        config_dir: &Path,
    ) -> InferenceResult<()> {
        self.stop()?;
        self.start_stopped(runtime_path, model_file, context_size, config_dir)
    }

    /// Starts a new child after the caller has already confirmed the old child
    /// is reaped. The file guard is retained until `stop` confirms child exit.
    pub fn start_stopped(
        &mut self,
        runtime_path: &str,
        model_file: models::LockedFile,
        context_size: u32,
        config_dir: &Path,
    ) -> InferenceResult<()> {
        if self.running.is_some() {
            return Err(InferenceError::new(
                "runtime_stop_failed",
                "The previous local engine is still tracked. Nexora will not start a replacement.",
            ));
        }

        self.last_error = None;
        models::looks_like_executable(runtime_path)?;
        let model_path = model_file.path().to_string_lossy().into_owned();

        let port = free_loopback_port()?;
        let raw_key = session_key()?;
        let key_path = config_dir.join(SESSION_KEY_FILE);
        write_key_file(&key_path, &raw_key)?;

        let mut command = Command::new(runtime_path);
        command
            .args(build_launch_args(&model_path, port, context_size, &key_path))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());

        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(CREATE_NO_WINDOW);
        }

        let mut child = command.spawn().map_err(|error| {
            // The key file is useless without the process that reads it.
            let _ = std::fs::remove_file(&key_path);

            map_spawn_io_error(error)
        })?;

        let stderr = Arc::new(Mutex::new(String::new()));
        let stderr_pipe = child.stderr.take();
        self.running = Some(RunningServer {
            child,
            port,
            key: RuntimeSecret::new(raw_key),
            key_path,
            stderr: Arc::clone(&stderr),
            stderr_done: None,
            _model_file: model_file,
        });

        if let Some(mut pipe) = stderr_pipe {
            let sink = Arc::clone(&stderr);
            let (done_tx, done_rx) = mpsc::channel::<()>();

            // Drain continuously and retain only the tail. Stopping the read
            // early would let the child block forever once the OS pipe buffer
            // filled, which looks exactly like a hung model load.
            let drain = move || {
                let mut chunk = [0u8; 4096];
                let mut retained: Vec<u8> = Vec::new();

                loop {
                    match pipe.read(&mut chunk) {
                        Ok(0) => break,
                        Ok(read) => {
                            retained.extend_from_slice(&chunk[..read]);

                            if retained.len() > STDERR_CAP as usize {
                                let excess = retained.len() - STDERR_CAP as usize;
                                retained.drain(..excess);
                            }

                            if let Ok(mut slot) = sink.lock() {
                                *slot = String::from_utf8_lossy(&retained).into_owned();
                            }
                        }
                        Err(_) => break,
                    }
                }

                let _ = done_tx.send(());
            };

            match std::thread::Builder::new()
                .name("nexora-runtime-stderr".to_string())
                .spawn(drain)
            {
                Ok(_reader) => {
                    if let Some(server) = self.running.as_mut() {
                        server.stderr_done = Some(done_rx);
                    }
                }
                Err(error) => {
                    let primary =
                        format!("The local engine output reader could not be started: {error}");
                    let stop_error = self.stop().err().map(|stop| stop.message);
                    let message = match stop_error {
                        Some(stop_error) => {
                            format!("{primary} Shutdown was not confirmed: {stop_error}")
                        }
                        None => primary,
                    };
                    self.last_error = Some(message.clone());
                    return Err(InferenceError::spawn_failed(message));
                }
            }
        }

        Ok(())
    }

    /// Stops and reaps the child, without ever blocking indefinitely.
    ///
    /// The child handle, session key, and model-file guard remain in the
    /// supervisor unless process exit is confirmed. Callers must not replace
    /// the runtime or clear loaded state after an error.
    pub fn stop(&mut self) -> InferenceResult<()> {
        self.stop_with_timeout(SHUTDOWN_TIMEOUT, true)
    }

    fn stop_with_timeout(
        &mut self,
        timeout: Duration,
        request_termination: bool,
    ) -> InferenceResult<()> {
        let Some(server) = self.running.as_mut() else {
            return Ok(());
        };

        let kill_error = if request_termination {
            server.child.kill().err()
        } else {
            None
        };
        let deadline = Instant::now() + timeout;
        let mut reaped = false;
        let mut wait_error = None;

        loop {
            match self
                .running
                .as_mut()
                .expect("the running child remains owned until reaped")
                .child
                .try_wait()
            {
                Ok(Some(_)) => {
                    reaped = true;
                    break;
                }
                Ok(None) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(20));
                }
                Ok(None) => {
                    wait_error = Some(
                        "The local engine did not exit after being asked to stop; it may still be running."
                            .to_string(),
                    );
                    break;
                }
                Err(error) => {
                    wait_error = Some(format!(
                        "The local engine could not be confirmed stopped: {error}"
                    ));
                    break;
                }
            }
        }

        if !reaped {
            let detail = wait_error
                .unwrap_or_else(|| "The local engine could not be confirmed stopped.".to_string());
            let message = match kill_error {
                Some(error) => format!("{detail} Termination request failed: {error}"),
                None => detail,
            };
            self.last_error = Some(message.clone());
            return Err(InferenceError::new("runtime_stop_failed", message));
        }

        let mut server = self
            .running
            .take()
            .expect("the reaped child remains owned until cleanup");
        if let Some(rx) = server.stderr_done.take() {
            let _ = rx.recv_timeout(STDERR_FLUSH_TIMEOUT);
        }

        // The key is useless once the process that was given it is reaped.
        let _ = std::fs::remove_file(&server.key_path);
        Ok(())
    }

    /// `Some(reason)` once the engine is no longer running.
    ///
    /// Loading is bounded by wall-clock time in the caller, but a runtime that
    /// dies during startup must fail immediately with its own words instead of
    /// looking like a three-minute load. `is_running` records the exit status
    /// and the stderr tail, so the reason is already human-readable here.
    pub fn poll_failure(&mut self) -> Option<String> {
        if self.is_running() {
            return None;
        }

        Some(
            self.last_error
                .clone()
                .unwrap_or_else(|| "The local engine stopped before it was ready.".to_string()),
        )
    }

    /// Bounded tail of engine output, for a readable failure message.
    pub fn diagnostics(&self) -> String {
        self.stderr_tail()
    }

    /// Last lines of engine stderr, scrubbed of any runtime session credential.
    ///
    /// The engine is never given prompt text on the command line, and any accidental
    /// reflection of the per-launch key is redacted before leaving this module.
    fn stderr_tail(&self) -> String {
        let Some(server) = self.running.as_ref() else {
            return String::new();
        };

        let Ok(text) = server.stderr.lock() else {
            return String::new();
        };

        let scrubbed = scrub_runtime_secret(&text, Some(server.key.as_str()));

        let tail: Vec<&str> = scrubbed
            .lines()
            .filter(|line| !line.trim().is_empty())
            .rev()
            .take(6)
            .collect();

        if tail.is_empty() {
            return String::new();
        }

        let joined = tail.into_iter().rev().collect::<Vec<_>>().join(" | ");

        // Keep the message short enough for a UI banner.
        let trimmed: String = joined.chars().take(600).collect();

        format!(" Engine output: {trimmed}")
    }
}

impl Drop for Supervisor {
    fn drop(&mut self) {
        if self.stop().is_err() {
            // The owner is going away, but the child must not be detached from
            // its model-file guard. The shared slot preserves ownership even
            // if the operating system cannot create a reaper thread.
            if let Some(server) = self.running.take() {
                let retained = Arc::new(Mutex::new(Some(server)));
                let reaper = Arc::clone(&retained);
                let result = std::thread::Builder::new()
                    .name("nexora-runtime-reaper".to_string())
                    .spawn(move || {
                        let server = reaper
                            .lock()
                            .unwrap_or_else(|error| error.into_inner())
                            .take();
                        if let Some(server) = server {
                            reap_until_stopped(server);
                        }
                    });

                if result.is_err() {
                    // Thread creation is best-effort, but dropping a live
                    // child's file guard is not. Retry synchronously rather
                    // than allowing the process to outlive its locked model.
                    let server = retained
                        .lock()
                        .unwrap_or_else(|error| error.into_inner())
                        .take();
                    if let Some(server) = server {
                        reap_until_stopped(server);
                    }
                }
            }
        }
    }
}

fn reap_until_stopped(mut server: RunningServer) {
    loop {
        let _ = server.child.kill();
        match server.child.try_wait() {
            Ok(Some(_)) => {
                if let Some(rx) = server.stderr_done.take() {
                    let _ = rx.recv_timeout(STDERR_FLUSH_TIMEOUT);
                }
                let _ = std::fs::remove_file(&server.key_path);
                return;
            }
            Ok(None) | Err(_) => std::thread::sleep(Duration::from_millis(250)),
        }
    }
}

fn free_loopback_port() -> InferenceResult<u16> {
    let listener = TcpListener::bind((LOOPBACK_HOST, 0)).map_err(|error| {
        InferenceError::spawn_failed(format!("No loopback port was available: {error}"))
    })?;

    let port = listener
        .local_addr()
        .map_err(|error| InferenceError::spawn_failed(error.to_string()))?
        .port();

    drop(listener);

    Ok(port)
}

fn session_key() -> InferenceResult<String> {
    let mut bytes = [0u8; 32];

    getrandom::getrandom(&mut bytes)
        .map_err(|error| InferenceError::internal(format!("No random source: {error}")))?;

    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn write_key_file(path: &Path, key: &str) -> InferenceResult<()> {
    if key.len() != 64
        || !key
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(InferenceError::storage(
            "The local runtime session key is malformed and was not written.",
        ));
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|_| {
            InferenceError::storage("The local runtime key directory could not be prepared.")
        })?;
    }

    // `create_new` prevents following/replacing a pre-existing path. Unix
    // permissions are set atomically at creation; Windows inherits the ACL of
    // Tauri's per-user application data directory.
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }

    let mut file = options.open(path).map_err(|error| {
        if error.kind() == ErrorKind::AlreadyExists {
            InferenceError::storage(
                "A previous local runtime key file still exists; the new key was not written.",
            )
        } else {
            InferenceError::storage("The local runtime key file could not be created.")
        }
    })?;

    if file.write_all(format!("{key}\n").as_bytes()).is_err() {
        drop(file);
        let _ = std::fs::remove_file(path);
        return Err(InferenceError::storage(
            "The local runtime key file could not be written safely.",
        ));
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = file
            .metadata()
            .map(|metadata| metadata.permissions().mode() & 0o777)
            .unwrap_or(0);
        if permissions & 0o077 != 0 || permissions & 0o400 == 0 {
            drop(file);
            let _ = std::fs::remove_file(path);
            return Err(InferenceError::storage(
                "The local runtime key file permissions are not private enough.",
            ));
        }
    }

    Ok(())
}

/// Reads `llama-server --version`, bounded to five seconds.
pub fn probe_runtime(runtime_path: &str) -> InferenceResult<RuntimeInfo> {
    models::looks_like_executable(runtime_path)?;

    let mut command = Command::new(runtime_path);
    command
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = command.spawn().map_err(map_spawn_io_error)?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let (sender, receiver) = mpsc::channel::<(Stream, String)>();

    drain_pipe(stdout, Stream::Stdout, sender.clone());
    drain_pipe(stderr, Stream::Stderr, sender.clone());
    drop(sender);

    let deadline = Instant::now() + Duration::from_secs(5);

    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();

                    return Err(InferenceError::timeout(
                        "The runtime did not answer --version within five seconds.",
                    ));
                }

                std::thread::sleep(Duration::from_millis(50));
            }
            Err(error) => {
                let _ = child.kill();
                return Err(InferenceError::spawn_failed(error.to_string()));
            }
        }
    }

    // Both pipes are read concurrently: draining only stdout would leave the
    // version text unread, and draining only stderr would risk a blocked pipe.
    let mut captured = CapturedOutput::default();
    let collect_deadline = Instant::now() + Duration::from_secs(2);

    for _ in 0..2 {
        let remaining = collect_deadline.saturating_duration_since(Instant::now());

        match receiver.recv_timeout(remaining) {
            Ok((Stream::Stdout, text)) => captured.stdout = text,
            Ok((Stream::Stderr, text)) => captured.stderr = text,
            Err(_) => break,
        }
    }

    let version_line = first_version_line(&captured);

    Ok(RuntimeInfo {
        pinned_build_matched: reports_pinned_build(&version_line),
        path: runtime_path.to_string(),
        version_line,
    })
}

/// Reads one child pipe to completion on its own thread.
///
/// Draining only one stream would deadlock the handshake as soon as the other
/// pipe filled, so both are consumed concurrently.
fn drain_pipe<R: Read + Send + 'static>(
    pipe: Option<R>,
    stream: Stream,
    sender: mpsc::Sender<(Stream, String)>,
) {
    std::thread::spawn(move || {
        let mut text = String::new();

        if let Some(pipe) = pipe {
            // `take` consumes the reader, so no mutable binding is needed.
            let _ = pipe.take(STDERR_CAP).read_to_string(&mut text);
        }

        let _ = sender.send((stream, text));
    });
}

#[derive(Debug, Clone, Copy)]
enum Stream {
    Stdout,
    Stderr,
}

#[derive(Debug, Default)]
pub struct CapturedOutput {
    pub stdout: String,
    pub stderr: String,
}

/// Picks the build line out of the runtime's output.
///
/// `llama-server --version` writes to **stderr** at b11714
/// (`llama_print_build_info(version)` defaults to `stderr`), which was verified
/// by running the binary: stdout was empty and stderr held
/// `version: 0.7.0-dev (build 11714, commit c0fd786...)`. Reading stdout alone
/// reported "unknown build" for a perfectly good runtime, so stderr is
/// preferred and stdout is kept as a fallback for other builds.
pub fn first_version_line(captured: &CapturedOutput) -> String {
    fn build_line(text: &str) -> Option<&str> {
        text.lines()
            .map(str::trim)
            .find(|line| line.starts_with("version:"))
    }

    fn first_text(text: &str) -> Option<&str> {
        text.lines().map(str::trim).find(|line| !line.is_empty())
    }

    // A build line anywhere beats the first line: some builds print a warning
    // (CUDA, backend selection) before the version block.
    build_line(&captured.stderr)
        .or_else(|| build_line(&captured.stdout))
        .or_else(|| first_text(&captured.stderr))
        .or_else(|| first_text(&captured.stdout))
        .unwrap_or("unknown build")
        .chars()
        .take(200)
        .collect()
}

/// True when the version line contains the pinned build number as a whole
/// number. Anything else is reported as a mismatch rather than assumed.
pub fn reports_pinned_build(version_line: &str) -> bool {
    version_line
        .split(|character: char| !character.is_ascii_digit())
        .any(|part| part == PINNED_BUILD)
}

#[derive(Deserialize)]
struct HealthBody {
    status: String,
}

/// Probes `GET http://127.0.0.1:{port}/health` with the per-launch session credential
/// and verifies both HTTP status and JSON readiness payload (`{"status": "ok"}`).
pub async fn probe_health(port: u16, key: Option<&str>) -> HealthProbe {
    let Ok(client) = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_millis(1200))
        .build()
    else {
        return HealthProbe::Unhealthy("Failed to construct loopback health HTTP client.".into());
    };

    let mut request = client.get(format!("http://{LOOPBACK_HOST}:{port}/health"));
    if let Some(secret) = key.filter(|s| !s.is_empty()) {
        request = request.bearer_auth(secret);
    }

    let response = match request.send().await {
        Ok(response) => response,
        Err(_) => return HealthProbe::Loading,
    };

    let code = response.status().as_u16();
    match code {
        401 | 403 => HealthProbe::AuthFailed,
        503 => HealthProbe::Loading,
        200..=299 => {
            let body = response.text().await.unwrap_or_default();
            match serde_json::from_str::<HealthBody>(&body) {
                Ok(parsed) if parsed.status.eq_ignore_ascii_case("ok") => HealthProbe::Ready,
                Ok(parsed) if parsed.status.to_ascii_lowercase().contains("loading") => {
                    HealthProbe::Loading
                }
                Ok(parsed) => HealthProbe::Unhealthy(format!(
                    "Engine reported unhealthy status: {}",
                    scrub_runtime_secret(&parsed.status, key)
                )),
                Err(_) => HealthProbe::Unhealthy(
                    "Loopback /health did not return a valid llama-server JSON payload.".into(),
                ),
            }
        }
        other => HealthProbe::Unhealthy(format!("Engine /health returned HTTP {other}.")),
    }
}

/// True when the engine answers `/health` with an authenticated `{"status": "ok"}` payload.
pub async fn health_ok_with_key(port: u16, key: Option<&str>) -> bool {
    matches!(probe_health(port, key).await, HealthProbe::Ready)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn failed_stop_retains_the_child_key_and_model_guard_until_reaped() {
        let directory = tempfile::tempdir().expect("temporary runtime directory");
        let model_path = directory.path().join("model.gguf");
        std::fs::write(&model_path, b"model bytes").expect("write model file");
        let model_file = models::LockedFile::open(&model_path).expect("open model guard");
        let key_path = directory.path().join(SESSION_KEY_FILE);
        std::fs::write(&key_path, b"test session key").expect("write key fixture");
        let child = Command::new("sleep")
            .arg("30")
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn child fixture");

        let mut supervisor = Supervisor::new();
        supervisor.running = Some(RunningServer {
            child,
            port: 0,
            key: RuntimeSecret::new("test-key"),
            key_path: key_path.clone(),
            stderr: Arc::new(Mutex::new(String::new())),
            stderr_done: None,
            _model_file: model_file,
        });

        let error = supervisor
            .stop_with_timeout(Duration::ZERO, false)
            .expect_err("unconfirmed child must not be forgotten");
        assert_eq!(error.code, "runtime_stop_failed");
        let running = supervisor.running.as_ref().expect("child retained");
        assert!(running._model_file.path().exists(), "file guard is retained");
        assert!(key_path.exists(), "session key remains while the child may use it");

        supervisor.stop().expect("termination is confirmed");
        assert!(supervisor.running.is_none());
        assert!(!key_path.exists(), "key is removed only after confirmed exit");
    }

    #[test]
    fn recognises_the_pinned_build_number() {
        assert!(reports_pinned_build("version: 11714 (c0ffee)"));
        assert!(reports_pinned_build("b11714"));
        assert!(reports_pinned_build("11714"));
    }

    /// Captured verbatim from the locally built b11714 binary:
    /// `llama-server --version` printed nothing on stdout and the build line on
    /// stderr. Reading stdout only is what this test guards against.
    const OBSERVED_STDERR: &str = "version: 0.7.0-dev (build 11714, commit c0fd78626405c6d9b934efef0ec7291fcc20e3d6)\nbuilt with GNU 12.2.0 for Linux x86_64\n";

    #[test]
    fn reads_the_build_line_from_stderr() {
        let captured = CapturedOutput {
            stdout: String::new(),
            stderr: OBSERVED_STDERR.to_string(),
        };

        let line = first_version_line(&captured);

        assert_eq!(
            line,
            "version: 0.7.0-dev (build 11714, commit c0fd78626405c6d9b934efef0ec7291fcc20e3d6)"
        );
        assert!(reports_pinned_build(&line));
    }

    #[test]
    fn skips_warnings_before_the_build_line() {
        let captured = CapturedOutput {
            stdout: String::new(),
            stderr: "warning: no usable GPU found\n".to_string() + OBSERVED_STDERR,
        };

        assert!(first_version_line(&captured).starts_with("version: 0.7.0-dev"));
    }

    #[test]
    fn falls_back_to_stdout_for_other_builds() {
        let captured = CapturedOutput {
            stdout: "\nversion: 9000 (build 9000, commit deadbeef)\n".to_string(),
            stderr: String::new(),
        };

        assert_eq!(
            first_version_line(&captured),
            "version: 9000 (build 9000, commit deadbeef)"
        );
    }

    #[test]
    fn reports_unknown_when_the_runtime_says_nothing() {
        let captured = CapturedOutput::default();

        assert_eq!(first_version_line(&captured), "unknown build");
        assert!(!reports_pinned_build(&first_version_line(&captured)));
    }

    #[test]
    fn rejects_other_or_partial_build_numbers() {
        assert!(!reports_pinned_build("version: 1171 (abc)"));
        assert!(!reports_pinned_build("version: 117140 (abc)"));
        assert!(!reports_pinned_build("unknown build"));
        assert!(!reports_pinned_build(""));
    }

    #[test]
    fn launch_args_enforce_loopback_host_and_never_embed_raw_secret() {
        let secret = "9f8e7d6c5b4a39281716151413121110fedcba9876543210abcdef0123456789";
        let key_path = PathBuf::from("/tmp/nexora-test-cfg/runtime-session.key");
        let args: Vec<String> = build_launch_args("/models/m.gguf", 49152, 8_192, &key_path)
            .into_iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();

        let host_idx = args
            .iter()
            .position(|a| a == "--host")
            .expect("--host flag");
        assert_eq!(args[host_idx + 1], "127.0.0.1");
        assert!(!args.iter().any(|a| a == "0.0.0.0"));
        assert!(!args.iter().any(|a| a.contains(secret)));

        let dbg = format!("{:?}", RuntimeSecret::new(secret));
        assert!(!dbg.contains(secret));
        assert!(dbg.contains("<redacted>"));

        let leaked = format!("fatal: key={secret} Authorization: Bearer {secret}");
        let scrubbed = scrub_runtime_secret(&leaked, Some(secret));
        assert!(!scrubbed.contains(secret));
    }

    #[test]
    fn session_key_file_is_private_and_never_overwrites_an_existing_path() {
        let directory = tempfile::tempdir().expect("temporary config directory");
        let path = directory.path().join(SESSION_KEY_FILE);
        let key = "a".repeat(64);

        write_key_file(&path, &key).expect("create private runtime key");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), format!("{key}\n"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode & 0o077, 0, "group/other access must be denied");
            assert_ne!(mode & 0o400, 0, "the owner must be able to read the key");
        }

        let replacement = "b".repeat(64);
        let error = write_key_file(&path, &replacement).unwrap_err();
        assert_eq!(error.code, "storage");
        assert!(!error.message.contains(&replacement));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), format!("{key}\n"));
    }

    #[test]
    fn malformed_session_key_is_rejected_before_any_file_is_created() {
        let directory = tempfile::tempdir().expect("temporary config directory");
        let path = directory.path().join(SESSION_KEY_FILE);
        let error = write_key_file(&path, "not-a-session-key").unwrap_err();
        assert_eq!(error.code, "storage");
        assert!(!path.exists());
    }

    #[test]
    fn sidecar_discovery_distinguishes_missing_incompatible_and_valid_binaries() {
        let root = std::env::temp_dir().join("nexora-phase04-sidecar-discovery");
        let _ = std::fs::remove_dir_all(&root);
        let bundle_dir = root.join("binaries");
        std::fs::create_dir_all(&bundle_dir).expect("create bundle dir");

        let empty_dirs = RuntimeSearchDirs {
            exe_dir: Some(root.join("missing-dir")),
            resource_dir: None,
            data_dir: None,
        };
        assert_eq!(
            resolve_runtime_path(None, &empty_dirs).unwrap_err().code,
            "not_configured"
        );
        assert_eq!(
            resolve_runtime_path(Some("/definitely/missing/llama-server"), &empty_dirs)
                .unwrap_err()
                .code,
            "runtime_missing"
        );

        let bin_name = candidate_binary_names()[0];
        let bad_sidecar = bundle_dir.join(bin_name);
        std::fs::write(&bad_sidecar, b"not-an-executable").expect("write bad sidecar");

        let dirs = RuntimeSearchDirs {
            exe_dir: Some(root.clone()),
            resource_dir: None,
            data_dir: None,
        };
        assert_eq!(
            resolve_runtime_path(None, &dirs).unwrap_err().code,
            "incompatible_runtime"
        );

        #[cfg(unix)]
        std::fs::write(&bad_sidecar, b"#!/bin/sh\nexit 0\n").expect("write valid unix stub");
        #[cfg(windows)]
        std::fs::write(&bad_sidecar, b"MZ\x90\x00stub").expect("write valid pe stub");

        let resolved = resolve_runtime_path(None, &dirs).expect("discover valid sidecar");
        assert_eq!(PathBuf::from(resolved), bad_sidecar);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn health_probe_verifies_bearer_auth_and_json_ok_status() {
        use std::io::{Read as _, Write as _};

        let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
        let port = listener.local_addr().expect("addr").port();

        let server = std::thread::spawn(move || {
            for response in [
                "HTTP/1.1 503 Service Unavailable\r\nContent-Type: application/json\r\nContent-Length: 42\r\nConnection: close\r\n\r\n{\"error\":{\"code\":503,\"message\":\"Loading\"}}",
                "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: 13\r\nConnection: close\r\n\r\n<html></html>",
                "HTTP/1.1 401 Unauthorized\r\nContent-Type: application/json\r\nContent-Length: 27\r\nConnection: close\r\n\r\n{\"error\":\"Invalid API Key\"}",
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 15\r\nConnection: close\r\n\r\n{\"status\":\"ok\"}",
            ] {
                let (mut stream, _) = listener.accept().expect("accept");
                let mut req_buf = [0u8; 2048];
                let n = stream.read(&mut req_buf).unwrap_or(0);
                let req_str = String::from_utf8_lossy(&req_buf[..n]);
                assert!(
                    req_str
                        .to_ascii_lowercase()
                        .contains("authorization: bearer test-secret-key"),
                    "missing bearer header in raw HTTP request: {req_str}"
                );
                let _ = stream.write_all(response.as_bytes());
            }
        });

        assert_eq!(
            probe_health(port, Some("test-secret-key")).await,
            HealthProbe::Loading
        );
        assert!(matches!(
            probe_health(port, Some("test-secret-key")).await,
            HealthProbe::Unhealthy(_)
        ));
        assert_eq!(
            probe_health(port, Some("test-secret-key")).await,
            HealthProbe::AuthFailed
        );
        assert_eq!(
            probe_health(port, Some("test-secret-key")).await,
            HealthProbe::Ready
        );

        server.join().expect("server thread cleanly joined");
    }
}

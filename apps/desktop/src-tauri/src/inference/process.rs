use std::io::Read;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;

use super::error::{InferenceError, InferenceResult};
use super::models;

/// How long the engine may take to load a model before we call it a failure.
pub const LOAD_TIMEOUT: Duration = Duration::from_secs(180);
/// The llama.cpp build the integration was written against (see docs/phase-2).
///
/// Nexora never downloads a runtime, so this is a report, not an enforcement:
/// a mismatch is surfaced to the user instead of being silently tolerated.
pub const PINNED_BUILD: &str = "11714";
const KEY_FILE: &str = "runtime-session.key";
const STDERR_CAP: u64 = 64 * 1024;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeInfo {
    pub path: String,
    pub version_line: String,
    /// Whether the reported build matches the pinned target.
    pub pinned_build_matched: bool,
}

struct RunningServer {
    child: Child,
    port: u16,
    key: String,
    /// Removed when the engine stops, so a session key never outlives its use.
    key_path: PathBuf,
    stderr: Arc<Mutex<String>>,
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
            .map(|server| (server.port, server.key.clone()))
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
                let tail = self.stderr_tail();
                self.last_error = Some(format!(
                    "The local engine stopped unexpectedly ({exit}).{tail}"
                ));
                self.stop();
                false
            }
            Err(error) => {
                self.last_error = Some(format!(
                    "The local engine could not be checked and was stopped: {error}"
                ));
                self.stop();
                false
            }
        }
    }

    /// Spawns the runtime with structured arguments only.
    ///
    /// No shell is involved, no argument is interpolated into a command line,
    /// and the API key is passed through a file rather than argv.
    pub fn start(
        &mut self,
        runtime_path: &str,
        model_path: &str,
        context_size: u32,
        config_dir: &Path,
    ) -> InferenceResult<()> {
        self.stop();
        self.last_error = None;

        models::looks_like_executable(runtime_path)?;
        models::inspect(model_path, context_size, None)?;

        let port = free_loopback_port()?;
        let key = session_key()?;
        let key_path = config_dir.join(KEY_FILE);
        write_key_file(&key_path, &key)?;

        let mut command = Command::new(runtime_path);
        command
            .arg("--model")
            .arg(model_path)
            .arg("--host")
            .arg("127.0.0.1")
            .arg("--port")
            .arg(port.to_string())
            .arg("--ctx-size")
            .arg(context_size.to_string())
            .arg("--parallel")
            .arg("1")
            .arg("--no-ui")
            .arg("--jinja")
            .arg("--api-key-file")
            .arg(&key_path)
            .arg("--timeout")
            .arg("600")
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

            InferenceError::spawn_failed(format!(
                "The engine could not be started. Check that the runtime path is correct. ({error})"
            ))
        })?;

        let stderr = Arc::new(Mutex::new(String::new()));

        if let Some(mut pipe) = child.stderr.take() {
            let sink = Arc::clone(&stderr);

            // Drain continuously and retain only the tail. Stopping the read
            // early would let the child block forever once the OS pipe buffer
            // filled, which looks exactly like a hung model load.
            std::thread::spawn(move || {
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
            });
        }

        self.running = Some(RunningServer {
            child,
            port,
            key,
            key_path,
            stderr,
        });

        Ok(())
    }

    pub fn stop(&mut self) {
        if let Some(mut server) = self.running.take() {
            let _ = server.child.kill();
            let _ = server.child.wait();
            let _ = std::fs::remove_file(&server.key_path);
        }
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

    /// Last lines of engine stderr, for a readable failure message.
    ///
    /// The engine is never given prompt text on the command line, but the tail
    /// is still bounded and is not written to any log file by Nexora.
    fn stderr_tail(&self) -> String {
        let Some(server) = self.running.as_ref() else {
            return String::new();
        };

        let Ok(text) = server.stderr.lock() else {
            return String::new();
        };

        let tail: Vec<&str> = text
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
        self.stop();
    }
}

fn free_loopback_port() -> InferenceResult<u16> {
    let listener = TcpListener::bind(("127.0.0.1", 0)).map_err(|error| {
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
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| InferenceError::storage(error.to_string()))?;
    }

    std::fs::write(path, format!("{key}\n")).map_err(|error| {
        InferenceError::storage(format!("The session key file could not be written: {error}"))
    })?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
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

    let mut child = command.spawn().map_err(|error| {
        InferenceError::spawn_failed(format!("The runtime could not be started: {error}"))
    })?;

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

        if let Some(mut pipe) = pipe {
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

#[cfg(test)]
mod tests {
    use super::*;

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
            stderr: "warning: no usable GPU found\n".to_string()
                + OBSERVED_STDERR,
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
}

/// True when the engine answers `/health` right now.
pub async fn health_ok(port: u16) -> bool {
    let Ok(client) = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_millis(1200))
        .build()
    else {
        return false;
    };

    matches!(
        client
            .get(format!("http://127.0.0.1:{port}/health"))
            .send()
            .await,
        Ok(response) if response.status().is_success()
    )
}

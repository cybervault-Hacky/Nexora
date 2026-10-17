use std::io::Read;
use std::net::TcpListener;
use std::path::Path;
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
            .arg("--no-webui")
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
            stderr,
        });

        Ok(())
    }

    pub fn stop(&mut self) {
        if let Some(mut server) = self.running.take() {
            let _ = server.child.kill();
            let _ = server.child.wait();
        }
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
    let (sender, receiver) = mpsc::channel::<String>();

    std::thread::spawn(move || {
        let mut text = String::new();

        if let Some(mut pipe) = stdout {
            let _ = pipe.take(STDERR_CAP).read_to_string(&mut text);
        }

        let _ = sender.send(text);
    });

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

    let text = receiver
        .recv_timeout(Duration::from_secs(2))
        .unwrap_or_default();

    let version_line = text
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("unknown build")
        .chars()
        .take(200)
        .collect::<String>();

    Ok(RuntimeInfo {
        pinned_build_matched: reports_pinned_build(&version_line),
        path: runtime_path.to_string(),
        version_line,
    })
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

    #[test]
    fn rejects_other_or_partial_build_numbers() {
        assert!(!reports_pinned_build("version: 1171 (abc)"));
        assert!(!reports_pinned_build("version: 117140 (abc)"));
        assert!(!reports_pinned_build("unknown build"));
        assert!(!reports_pinned_build(""));
    }
}

/// Polls `GET /health` until the model reports loaded.
///
/// `/health` is unauthenticated in the runtime, so it is used only as a
/// liveness signal; `/props` and generation prove the engine really works.
pub async fn wait_for_health(port: u16, timeout: Duration) -> InferenceResult<()> {
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(5))
        .build()?;

    let url = format!("http://127.0.0.1:{port}/health");
    let deadline = Instant::now() + timeout;

    loop {
        match client.get(&url).send().await {
            Ok(response) if response.status().is_success() => return Ok(()),
            Ok(_) => {}
            Err(_) => {}
        }

        if Instant::now() >= deadline {
            return Err(InferenceError::load_failed(
                "The engine did not finish loading the model in time. The model may be too large for this machine.",
            ));
        }

        tokio::time::sleep(Duration::from_millis(300)).await;
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

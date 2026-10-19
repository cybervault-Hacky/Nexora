use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use sha2::{Digest, Sha256};

use super::AuthError;

/// Output of a freshly generated PKCE challenge.
#[derive(Debug, Clone)]
pub struct PkceChallenge {
    pub code_verifier: String,
    pub code_challenge: String,
    pub state: String,
    pub nonce: String,
}

/// Generates RFC 7636 PKCE S256 verifier, challenge, state, and nonce.
pub fn generate_pkce() -> PkceChallenge {
    let mut verifier_bytes = [0u8; 32];
    getrandom::getrandom(&mut verifier_bytes).unwrap_or_else(|_| {
        for b in &mut verifier_bytes {
            *b = 42;
        }
    });

    let code_verifier = to_base64_url(&verifier_bytes);

    let mut hasher = Sha256::new();
    hasher.update(code_verifier.as_bytes());
    let challenge_digest = hasher.finalize();
    let code_challenge = to_base64_url(&challenge_digest);

    let mut state_bytes = [0u8; 16];
    getrandom::getrandom(&mut state_bytes).unwrap_or_else(|_| {
        for b in &mut state_bytes {
            *b = 99;
        }
    });
    let state = to_base64_url(&state_bytes);

    let mut nonce_bytes = [0u8; 16];
    getrandom::getrandom(&mut nonce_bytes).unwrap_or_else(|_| {
        for b in &mut nonce_bytes {
            *b = 77;
        }
    });
    let nonce = to_base64_url(&nonce_bytes);

    PkceChallenge {
        code_verifier,
        code_challenge,
        state,
        nonce,
    }
}

/// URL-safe base64 encoding without padding per RFC 7636.
fn to_base64_url(input: &[u8]) -> String {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut result = String::new();
    let mut i = 0;
    while i < input.len() {
        let b0 = input[i] as usize;
        let b1 = if i + 1 < input.len() { input[i + 1] as usize } else { 0 };
        let b2 = if i + 2 < input.len() { input[i + 2] as usize } else { 0 };

        result.push(CHARSET[b0 >> 2] as char);
        result.push(CHARSET[((b0 & 3) << 4) | (b1 >> 4)] as char);
        if i + 1 < input.len() {
            result.push(CHARSET[((b1 & 15) << 2) | (b2 >> 6)] as char);
        }
        if i + 2 < input.len() {
            result.push(CHARSET[b2 & 63] as char);
        }
        i += 3;
    }
    result
}

/// Spawns a loopback TCP listener on an ephemeral port.
pub fn bind_loopback() -> Result<(TcpListener, u16), AuthError> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| {
        AuthError::new("loopback_bind_failed", format!("Failed to bind loopback listener: {e}"))
    })?;
    let port = listener.local_addr().map_err(|e| {
        AuthError::new("loopback_port_failed", format!("Failed to obtain port: {e}"))
    })?.port();

    listener.set_nonblocking(true).map_err(|e| {
        AuthError::new("loopback_nonblocking_failed", format!("Failed to configure non-blocking: {e}"))
    })?;

    Ok((listener, port))
}

/// Waits for an OAuth redirect callback on the loopback listener.
///
/// Bounded by `timeout` and responsive to `cancelled` cancellation flag.
pub fn await_callback(
    listener: TcpListener,
    expected_state: &str,
    timeout: Duration,
    cancelled: Arc<AtomicBool>,
) -> Result<String, AuthError> {
    let start = std::time::Instant::now();

    while start.elapsed() < timeout {
        if cancelled.load(Ordering::SeqCst) {
            return Err(AuthError::new("cancelled", "Authentication flow was cancelled."));
        }

        match listener.accept() {
            Ok((mut stream, _addr)) => {
                let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
                let mut buffer = [0u8; 2048];
                let bytes_read = stream.read(&mut buffer).unwrap_or(0);
                let request = String::from_utf8_lossy(&buffer[..bytes_read]);

                // Extract query line: "GET /callback?code=...&state=... HTTP/1.1"
                if let Some(first_line) = request.lines().next() {
                    let parts: Vec<&str> = first_line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        let path_and_query = parts[1];
                        if let Some(query_start) = path_and_query.find('?') {
                            let query = &path_and_query[query_start + 1..];
                            let mut code = None;
                            let mut state = None;

                            for param in query.split('&') {
                                if let Some((k, v)) = param.split_once('=') {
                                    if k == "code" {
                                        code = Some(v.to_string());
                                    } else if k == "state" {
                                        state = Some(v.to_string());
                                    }
                                }
                            }

                            // State validation
                            if let (Some(code_val), Some(state_val)) = (code, state) {
                                if state_val == expected_state {
                                    let html = "<!DOCTYPE html><html><head><meta charset='utf-8'><title>Nexora Sign In</title><style>body{font-family:system-ui,-apple-system,sans-serif;background:#121214;color:#f0f0f2;display:flex;align-items:center;justify-content:center;height:100vh;margin:0;}div{text-align:center;padding:2rem;background:#18181b;border:1px solid #27272a;border-radius:8px;}h2{margin-top:0;color:#a78bfa;}p{color:#a1a1aa;margin-bottom:0;}</style></head><body><div><h2>Sign-in complete</h2><p>You may close this tab and return to Nexora.</p></div><script>setTimeout(function(){window.close();},1500);</script></body></html>";
                                    let response = format!(
                                        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                                        html.len(),
                                        html
                                    );
                                    let _ = stream.write_all(response.as_bytes());
                                    let _ = stream.flush();
                                    return Ok(code_val);
                                } else {
                                    let error_html = "HTTP/1.1 400 Bad Request\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\nState mismatch";
                                    let _ = stream.write_all(error_html.as_bytes());
                                    let _ = stream.flush();
                                    return Err(AuthError::new("state_mismatch", "OAuth security verification failed (state mismatch)."));
                                }
                            }
                        }
                    }
                }

                let response = "HTTP/1.1 400 Bad Request\r\nConnection: close\r\n\r\nInvalid OAuth redirect request";
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.flush();
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => {
                return Err(AuthError::new("loopback_accept_error", format!("Loopback listener error: {e}")));
            }
        }
    }

    Err(AuthError::new("timeout", "Sign-in timed out. Please try again."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_generation_properties() {
        let pkce = generate_pkce();
        assert!(pkce.code_verifier.len() >= 43 && pkce.code_verifier.len() <= 128);
        assert!(!pkce.code_challenge.is_empty());
        assert!(!pkce.state.is_empty());
        assert!(!pkce.nonce.is_empty());

        // Verify base64url characters only
        for c in pkce.code_verifier.chars() {
            assert!(c.is_alphanumeric() || c == '-' || c == '_');
        }
    }
}

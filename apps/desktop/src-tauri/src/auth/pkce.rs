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
pub fn generate_pkce() -> Result<PkceChallenge, AuthError> {
    // A predictable verifier, state, or nonce defeats PKCE entirely, so a
    // missing random source aborts the flow instead of substituting constants.
    let mut verifier_bytes = [0u8; 32];
    getrandom::getrandom(&mut verifier_bytes).map_err(|_| no_random_source())?;

    let code_verifier = to_base64_url(&verifier_bytes);

    let mut hasher = Sha256::new();
    hasher.update(code_verifier.as_bytes());
    let challenge_digest = hasher.finalize();
    let code_challenge = to_base64_url(&challenge_digest);

    let mut state_bytes = [0u8; 16];
    getrandom::getrandom(&mut state_bytes).map_err(|_| no_random_source())?;
    let state = to_base64_url(&state_bytes);

    let mut nonce_bytes = [0u8; 16];
    getrandom::getrandom(&mut nonce_bytes).map_err(|_| no_random_source())?;
    let nonce = to_base64_url(&nonce_bytes);

    Ok(PkceChallenge {
        code_verifier,
        code_challenge,
        state,
        nonce,
    })
}

fn no_random_source() -> AuthError {
    AuthError::new(
        "no_random_source",
        "The operating system's random source is unavailable, so sign-in cannot start safely.",
    )
}

/// URL-safe base64 encoding without padding per RFC 7636.
fn to_base64_url(input: &[u8]) -> String {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut result = String::new();
    let mut i = 0;
    while i < input.len() {
        let b0 = input[i] as usize;
        let b1 = if i + 1 < input.len() {
            input[i + 1] as usize
        } else {
            0
        };
        let b2 = if i + 2 < input.len() {
            input[i + 2] as usize
        } else {
            0
        };

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
        AuthError::new(
            "loopback_bind_failed",
            format!("Failed to bind loopback listener: {e}"),
        )
    })?;
    let port = listener
        .local_addr()
        .map_err(|e| {
            AuthError::new(
                "loopback_port_failed",
                format!("Failed to obtain port: {e}"),
            )
        })?
        .port();

    listener.set_nonblocking(true).map_err(|e| {
        AuthError::new(
            "loopback_nonblocking_failed",
            format!("Failed to configure non-blocking: {e}"),
        )
    })?;

    Ok((listener, port))
}

#[derive(Debug, PartialEq, Eq)]
enum OAuthCallback {
    AuthorizationCode(String),
    Cancelled,
    ProviderError,
}

#[derive(Debug, PartialEq, Eq)]
enum CallbackParseError {
    InvalidRequest,
    StateMismatch,
}

const MAX_CALLBACK_REQUEST_LINE_BYTES: usize = 4_096;

fn decode_form_component(raw: &str) -> Option<String> {
    fn hex_value(byte: u8) -> Option<u8> {
        match byte {
            b'0'..=b'9' => Some(byte - b'0'),
            b'a'..=b'f' => Some(byte - b'a' + 10),
            b'A'..=b'F' => Some(byte - b'A' + 10),
            _ => None,
        }
    }

    let input = raw.as_bytes();
    let mut decoded = Vec::with_capacity(input.len());
    let mut index = 0;
    while index < input.len() {
        match input[index] {
            b'+' => {
                decoded.push(b' ');
                index += 1;
            }
            b'%' => {
                if index + 2 >= input.len() {
                    return None;
                }
                let high = hex_value(input[index + 1])?;
                let low = hex_value(input[index + 2])?;
                decoded.push((high << 4) | low);
                index += 3;
            }
            byte => {
                decoded.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8(decoded).ok()
}

fn set_unique_query_value(
    target: &mut Option<String>,
    value: String,
) -> Result<(), CallbackParseError> {
    if target.is_some() {
        return Err(CallbackParseError::InvalidRequest);
    }
    *target = Some(value);
    Ok(())
}

fn parse_callback_target(
    target: &str,
    expected_state: &str,
) -> Result<OAuthCallback, CallbackParseError> {
    if target.len() > MAX_CALLBACK_REQUEST_LINE_BYTES {
        return Err(CallbackParseError::InvalidRequest);
    }
    let (path, query) = target
        .split_once('?')
        .ok_or(CallbackParseError::InvalidRequest)?;
    if path != "/callback" || query.is_empty() {
        return Err(CallbackParseError::InvalidRequest);
    }

    let mut code = None;
    let mut state = None;
    let mut provider_error = None;
    for parameter in query.split('&') {
        if parameter.is_empty() {
            continue;
        }
        let (raw_key, raw_value) = parameter.split_once('=').unwrap_or((parameter, ""));
        let key = decode_form_component(raw_key).ok_or(CallbackParseError::InvalidRequest)?;
        let value = decode_form_component(raw_value).ok_or(CallbackParseError::InvalidRequest)?;
        match key.as_str() {
            "code" => set_unique_query_value(&mut code, value)?,
            "state" => set_unique_query_value(&mut state, value)?,
            "error" => set_unique_query_value(&mut provider_error, value)?,
            _ => {}
        }
    }

    let state = state.ok_or(CallbackParseError::InvalidRequest)?;
    if state != expected_state {
        return Err(CallbackParseError::StateMismatch);
    }

    match (code, provider_error) {
        (Some(code), None)
            if !code.trim().is_empty()
                && code.len() <= MAX_CALLBACK_REQUEST_LINE_BYTES
                && !code.chars().any(char::is_control) =>
        {
            Ok(OAuthCallback::AuthorizationCode(code))
        }
        (None, Some(error)) if !error.is_empty() => {
            if error == "access_denied" {
                Ok(OAuthCallback::Cancelled)
            } else {
                Ok(OAuthCallback::ProviderError)
            }
        }
        _ => Err(CallbackParseError::InvalidRequest),
    }
}

fn request_target_from_line(line: &str) -> Option<&str> {
    let mut parts = line.split_ascii_whitespace();
    if parts.next()? != "GET" {
        return None;
    }
    let target = parts.next()?;
    if !matches!(parts.next()?, "HTTP/1.0" | "HTTP/1.1") || parts.next().is_some() {
        return None;
    }
    Some(target)
}

fn read_request_line(stream: &mut std::net::TcpStream) -> Option<String> {
    let mut bytes = Vec::with_capacity(512);
    let mut chunk = [0u8; 512];
    loop {
        let remaining = MAX_CALLBACK_REQUEST_LINE_BYTES.saturating_sub(bytes.len());
        if remaining == 0 {
            return None;
        }
        let read_size = remaining.min(chunk.len());
        let count = stream.read(&mut chunk[..read_size]).ok()?;
        if count == 0 {
            return None;
        }
        bytes.extend_from_slice(&chunk[..count]);
        if let Some(end) = bytes.windows(2).position(|window| window == b"\r\n") {
            return String::from_utf8(bytes[..end].to_vec()).ok();
        }
        if bytes.contains(&b'\n') {
            return None;
        }
    }
}

fn write_callback_response(stream: &mut std::net::TcpStream, status: &str, body: &str) {
    let response = format!(
        "HTTP/1.1 {status}\r\n\
Content-Type: text/html; charset=utf-8\r\n\
Content-Length: {}\r\n\
Connection: close\r\n\
Cache-Control: no-store, max-age=0\r\n\
Pragma: no-cache\r\n\
Referrer-Policy: no-referrer\r\n\
X-Content-Type-Options: nosniff\r\n\
Content-Security-Policy: default-src 'none'; base-uri 'none'; frame-ancestors 'none'\r\n\
\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
}

const CALLBACK_INVALID_HTML: &str = "<!doctype html><html><head><meta charset='utf-8'><title>Nexora</title></head><body><h2>Invalid sign-in response</h2><p>Return to Nexora and try again.</p></body></html>";
const CALLBACK_SUCCESS_HTML: &str = "<!doctype html><html><head><meta charset='utf-8'><title>Nexora</title></head><body><h2>Authorization response received</h2><p>Return to Nexora. Sign-in will finish there.</p></body></html>";
const CALLBACK_CANCELLED_HTML: &str = "<!doctype html><html><head><meta charset='utf-8'><title>Nexora</title></head><body><h2>Sign-in cancelled</h2><p>You can close this tab and return to Nexora.</p></body></html>";
const CALLBACK_FAILED_HTML: &str = "<!doctype html><html><head><meta charset='utf-8'><title>Nexora</title></head><body><h2>Sign-in not completed</h2><p>Return to Nexora and try again.</p></body></html>";

/// Waits for an OAuth redirect callback on the loopback listener.
///
/// Bounded by `timeout` and responsive to `cancelled`. Only an exact GET to
/// `/callback` with a single matching state and one code/error is accepted;
/// malformed or mismatched local requests are rejected without ending the flow.
pub fn await_callback(
    listener: TcpListener,
    expected_state: &str,
    timeout: Duration,
    cancelled: Arc<AtomicBool>,
) -> Result<String, AuthError> {
    let start = std::time::Instant::now();

    while start.elapsed() < timeout {
        if cancelled.load(Ordering::SeqCst) {
            return Err(AuthError::new(
                "cancelled",
                "Authentication flow was cancelled.",
            ));
        }

        match listener.accept() {
            Ok((mut stream, _addr)) => {
                let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
                let request_line = read_request_line(&mut stream);
                let callback = request_line
                    .as_deref()
                    .and_then(request_target_from_line)
                    .ok_or(CallbackParseError::InvalidRequest)
                    .and_then(|target| parse_callback_target(target, expected_state));

                match callback {
                    Ok(OAuthCallback::AuthorizationCode(code)) => {
                        write_callback_response(&mut stream, "200 OK", CALLBACK_SUCCESS_HTML);
                        return Ok(code);
                    }
                    Ok(OAuthCallback::Cancelled) => {
                        write_callback_response(&mut stream, "200 OK", CALLBACK_CANCELLED_HTML);
                        return Err(AuthError::new(
                            "cancelled",
                            "Google sign-in was cancelled.",
                        ));
                    }
                    Ok(OAuthCallback::ProviderError) => {
                        write_callback_response(&mut stream, "200 OK", CALLBACK_FAILED_HTML);
                        return Err(AuthError::new(
                            "google_failed",
                            "Google sign-in could not be completed. Try again.",
                        ));
                    }
                    Err(CallbackParseError::InvalidRequest | CallbackParseError::StateMismatch) => {
                        write_callback_response(
                            &mut stream,
                            "400 Bad Request",
                            CALLBACK_INVALID_HTML,
                        );
                    }
                }
            }
            Err(ref error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(_) => {
                return Err(AuthError::new(
                    "loopback_accept_error",
                    "The local sign-in callback could not be received.",
                ));
            }
        }
    }

    Err(AuthError::new(
        "timeout",
        "Sign-in timed out. Please try again.",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpStream;

    fn request_loopback(port: u16, request_line: &str) -> String {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect to loopback");
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .expect("set response timeout");
        write!(stream, "{request_line}\r\nHost: 127.0.0.1:{port}\r\n\r\n")
            .expect("write callback request");
        let mut response = Vec::new();
        stream.read_to_end(&mut response).expect("read callback response");
        String::from_utf8(response).expect("HTTP response is UTF-8")
    }

    #[test]
    fn callback_requires_exact_http_get_request_line() {
        assert_eq!(
            request_target_from_line("GET /callback?code=valid HTTP/1.1"),
            Some("/callback?code=valid")
        );
        for line in [
            "POST /callback?code=ignored HTTP/1.1",
            "GET /callback?code=ignored HTTP/2",
            "GET /callback?code=ignored HTTP/1.1 extra",
            "GET /callback?code=ignored",
        ] {
            assert_eq!(request_target_from_line(line), None, "reject request line: {line}");
        }
    }

    #[test]
    fn pkce_generation_properties() {
        let pkce = generate_pkce().expect("the test host has a random source");
        assert!(pkce.code_verifier.len() >= 43 && pkce.code_verifier.len() <= 128);
        assert!(!pkce.code_challenge.is_empty());
        assert_eq!(pkce.state.len(), 22, "128-bit base64url state representation");
        assert_eq!(pkce.nonce.len(), 22, "128-bit base64url nonce representation");

        for value in [&pkce.code_verifier, &pkce.code_challenge, &pkce.state, &pkce.nonce] {
            assert!(value.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '-' | '_')
            }));
        }
    }

    #[test]
    fn callback_parser_decodes_query_components_and_requires_exact_path_and_state() {
        let state = "s".repeat(22);
        let parsed = parse_callback_target(
            &format!("/callback?code=4%2F0A%2BB&state={state}&scope=openid"),
            &state,
        )
        .unwrap();
        assert_eq!(parsed, OAuthCallback::AuthorizationCode("4/0A+B".to_string()));

        assert_eq!(
            parse_callback_target(
                &format!("/other?code=secret&state={state}"),
                &state
            ),
            Err(CallbackParseError::InvalidRequest)
        );
        assert_eq!(
            parse_callback_target(
                &format!("/callback?code=secret&state={}", "x".repeat(22)),
                &state
            ),
            Err(CallbackParseError::StateMismatch)
        );
    }

    #[test]
    fn callback_parser_rejects_duplicate_or_malformed_security_parameters() {
        let state = "s".repeat(22);
        for target in [
            format!("/callback?code=one&code=two&state={state}"),
            format!("/callback?code=one&state={state}&state={state}"),
            format!("/callback?code=bad%2&state={state}"),
            format!("/callback?code=one&error=access_denied&state={state}"),
        ] {
            assert_eq!(
                parse_callback_target(&target, &state),
                Err(CallbackParseError::InvalidRequest),
                "target must be refused: {target}"
            );
        }
    }

    #[test]
    fn callback_parser_maps_google_denial_to_safe_cancellation() {
        let state = "s".repeat(22);
        assert_eq!(
            parse_callback_target(&format!("/callback?error=access_denied&state={state}"), &state),
            Ok(OAuthCallback::Cancelled)
        );
        assert_eq!(
            parse_callback_target(&format!("/callback?error=server_error&state={state}"), &state),
            Ok(OAuthCallback::ProviderError)
        );
    }

    #[test]
    fn loopback_callback_rejects_bad_requests_then_accepts_only_valid_response() {
        let (listener, port) = bind_loopback().expect("bind loopback");
        let state = "s".repeat(22);
        let sender_state = state.clone();
        let sender = std::thread::spawn(move || {
            let invalid = request_loopback(
                port,
                &format!("GET /wrong?code=wrong&state={sender_state} HTTP/1.1"),
            );
            assert!(invalid.starts_with("HTTP/1.1 400 Bad Request"));

            let valid = request_loopback(
                port,
                &format!("GET /callback?code=4%2F0A%2BB&state={sender_state} HTTP/1.1"),
            );
            assert!(valid.contains("Authorization response received"));
            assert!(!valid.contains("4/0A+B"));
            assert!(!valid.contains(&sender_state));
        });

        let code = await_callback(
            listener,
            &state,
            Duration::from_secs(3),
            Arc::new(AtomicBool::new(false)),
        )
        .expect("valid callback after an invalid local request");
        sender.join().expect("callback sender");
        assert_eq!(code, "4/0A+B");
    }

    #[test]
    fn loopback_callback_reports_google_access_denial_as_cancelled() {
        let (listener, port) = bind_loopback().expect("bind loopback");
        let state = "s".repeat(22);
        let sender_state = state.clone();
        let sender = std::thread::spawn(move || {
            let response = request_loopback(
                port,
                &format!("GET /callback?error=access_denied&state={sender_state} HTTP/1.1"),
            );
            assert!(response.contains("Sign-in cancelled"));
            assert!(!response.contains("access_denied"));
            assert!(!response.contains(&sender_state));
        });

        let error = await_callback(
            listener,
            &state,
            Duration::from_secs(3),
            Arc::new(AtomicBool::new(false)),
        )
        .unwrap_err();
        sender.join().expect("callback sender");
        assert_eq!(error.code, "cancelled");
    }
}

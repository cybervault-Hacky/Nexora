use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tauri::State;

use super::client::AuthApiClient;
use super::pkce::{await_callback, bind_loopback, generate_pkce};
use super::storage::{delete_session_token, get_session_token, save_session_token};
use super::{
    AuthError, AuthResult, AuthSessionInfo, AuthStatusResponse, AuthUser, ChangePasswordPayload,
    DeveloperDiagnostics, LoginPayload, RecoveryPayload, RegisterPayload,
    ResendVerificationPayload, ResetPasswordPayload, VerifyEmailPayload,
};

pub struct AuthState {
    pub client: AuthApiClient,
    pub current_user: Mutex<Option<AuthUser>>,
    pub google_cancel: Arc<AtomicBool>,
}

impl AuthState {
    pub fn new() -> Self {
        Self {
            client: AuthApiClient::new(),
            current_user: Mutex::new(None),
            google_cancel: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl Default for AuthState {
    fn default() -> Self {
        Self::new()
    }
}

/// Retrieves the current authenticated user and session state.
#[tauri::command]
pub async fn auth_get_session(state: State<'_, AuthState>) -> AuthResult<AuthSessionInfo> {
    if let Ok(guard) = state.current_user.lock() {
        if let Some(user) = guard.as_ref() {
            return Ok(AuthSessionInfo {
                user: Some(user.clone()),
                authenticated: true,
            });
        }
    }

    // Attempt restoring session from secure storage (Windows Credential Manager)
    if let Ok(Some(token)) = get_session_token() {
        match state.client.get_me(&token).await {
            Ok(user) => {
                if let Ok(mut guard) = state.current_user.lock() {
                    *guard = Some(user.clone());
                }
                return Ok(AuthSessionInfo {
                    user: Some(user),
                    authenticated: true,
                });
            }
            Err(e) if e.code == "session_expired" || e.code == "invalid_credentials" => {
                let _ = delete_session_token();
            }
            Err(_) => {
                // Network failure or temporary error; keep local state intact
            }
        }
    }

    Ok(AuthSessionInfo {
        user: None,
        authenticated: false,
    })
}

/// Submits user credentials, acquires a session, and saves it to OS credential storage.
#[tauri::command]
pub async fn auth_login(
    state: State<'_, AuthState>,
    request: LoginPayload,
) -> AuthResult<AuthUser> {
    let (user, token) = state
        .client
        .login(&request.email, &request.password)
        .await?;

    // Store in OS credential manager (never returns raw token to frontend)
    let _ = save_session_token(&token);

    if let Ok(mut guard) = state.current_user.lock() {
        *guard = Some(user.clone());
    }

    Ok(user)
}

/// Registers a new account.
#[tauri::command]
pub async fn auth_register(
    state: State<'_, AuthState>,
    request: RegisterPayload,
) -> AuthResult<AuthStatusResponse> {
    state
        .client
        .register(&request.name, &request.email, &request.password)
        .await?;

    Ok(AuthStatusResponse {
        status: "verification_pending".to_string(),
        message: "Account created. Please check your email to verify your address.".to_string(),
    })
}

/// Verifies an email token.
#[tauri::command]
pub async fn auth_verify_email(
    state: State<'_, AuthState>,
    request: VerifyEmailPayload,
) -> AuthResult<AuthStatusResponse> {
    state.client.verify_email(&request.token).await?;

    Ok(AuthStatusResponse {
        status: "verified".to_string(),
        message: "Email verified successfully. You can now sign in.".to_string(),
    })
}

/// Requests a new email verification message.
#[tauri::command]
pub async fn auth_resend_verification(
    state: State<'_, AuthState>,
    request: ResendVerificationPayload,
) -> AuthResult<AuthStatusResponse> {
    state.client.resend_verification(&request.email).await?;

    Ok(AuthStatusResponse {
        status: "sent".to_string(),
        message: "If an account exists for that email, a verification link has been sent."
            .to_string(),
    })
}

/// Requests a password reset link.
#[tauri::command]
pub async fn auth_recover_password(
    state: State<'_, AuthState>,
    request: RecoveryPayload,
) -> AuthResult<AuthStatusResponse> {
    state.client.recovery_request(&request.email).await?;

    Ok(AuthStatusResponse {
        status: "sent".to_string(),
        message: "If an account exists for that email, password reset instructions have been sent."
            .to_string(),
    })
}

/// Resets a password using a verification token.
#[tauri::command]
pub async fn auth_reset_password(
    state: State<'_, AuthState>,
    request: ResetPasswordPayload,
) -> AuthResult<AuthStatusResponse> {
    state
        .client
        .reset_password(&request.token, &request.password)
        .await?;

    // Password reset revokes all sessions; clear local credential storage
    let _ = delete_session_token();
    if let Ok(mut guard) = state.current_user.lock() {
        *guard = None;
    }

    Ok(AuthStatusResponse {
        status: "reset".to_string(),
        message: "Password updated successfully. You can now sign in with your new password."
            .to_string(),
    })
}

/// Changes the authenticated user's password using their current password.
#[tauri::command]
pub async fn auth_change_password(
    state: State<'_, AuthState>,
    request: ChangePasswordPayload,
) -> AuthResult<AuthStatusResponse> {
    let token = get_session_token()?.ok_or_else(|| {
        AuthError::new(
            "unauthenticated",
            "You must be signed in to change your password.",
        )
    })?;

    state
        .client
        .password_change(&token, &request.current_password, &request.new_password)
        .await?;

    Ok(AuthStatusResponse {
        status: "success".to_string(),
        message: "Password changed successfully.".to_string(),
    })
}

/// Returns developer and system diagnostics for Founder Control Centre.
#[tauri::command]
pub async fn get_developer_diagnostics(
    state: State<'_, AuthState>,
) -> AuthResult<DeveloperDiagnostics> {
    let vault_status = if cfg!(windows) {
        "Windows Credential Manager (keyring)".to_string()
    } else {
        "Process-memory Secure Vault (non-Windows fallback)".to_string()
    };

    Ok(DeveloperDiagnostics {
        platform: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        credential_vault: vault_status,
        auth_service_url: state.client.base_url().to_string(),
        csp_enforced: true,
        offline_capable: true,
    })
}

/// Logs out the user, invalidating the session on the server and removing credentials.
#[tauri::command]
pub async fn auth_logout(state: State<'_, AuthState>) -> AuthResult<bool> {
    if let Ok(Some(token)) = get_session_token() {
        let _ = state.client.logout(&token).await;
    }

    let _ = delete_session_token();
    if let Ok(mut guard) = state.current_user.lock() {
        *guard = None;
    }

    Ok(true)
}

/// Begins Google OAuth PKCE flow using system browser and loopback redirect.
#[tauri::command]
pub async fn auth_google_begin(state: State<'_, AuthState>) -> AuthResult<AuthUser> {
    state.google_cancel.store(false, Ordering::SeqCst);

    let pkce = generate_pkce();
    let (listener, port) = bind_loopback()?;
    let redirect_uri = format!("http://127.0.0.1:{port}/callback");

    let client_id = std::env::var("GOOGLE_OAUTH_CLIENT_ID")
        .unwrap_or_else(|_| "nexora-desktop-client.apps.googleusercontent.com".to_string());

    let auth_url = format!(
        "https://accounts.google.com/o/oauth2/v2/auth?response_type=code&client_id={}&redirect_uri={}&scope=openid%20email%20profile&state={}&code_challenge={}&code_challenge_method=S256&nonce={}",
        urlencoding(&client_id),
        urlencoding(&redirect_uri),
        urlencoding(&pkce.state),
        urlencoding(&pkce.code_challenge),
        urlencoding(&pkce.nonce),
    );

    // Open system browser safely
    open_browser(&auth_url);

    // Wait for loopback redirect (120-second timeout).
    //
    // The expected `state` is cloned into the blocking task: `pkce` is still
    // needed for the server-side exchange below, so nothing may be moved out
    // of it here.
    let cancel_flag = state.google_cancel.clone();
    let expected_state = pkce.state.clone();
    let callback = tokio::task::spawn_blocking(move || {
        await_callback(
            listener,
            &expected_state,
            Duration::from_secs(120),
            cancel_flag,
        )
    })
    .await
    .map_err(|e| AuthError::new("task_error", format!("OAuth listener task error: {e}")))?;

    let code = callback?;

    // Perform server-side code exchange
    let (user, token) = state
        .client
        .google_exchange(
            &code,
            &pkce.code_verifier,
            &pkce.state,
            &redirect_uri,
            Some(&pkce.nonce),
        )
        .await?;

    // Store in OS credential manager
    let _ = save_session_token(&token);
    if let Ok(mut guard) = state.current_user.lock() {
        *guard = Some(user.clone());
    }

    Ok(user)
}

/// Cancels an in-progress Google OAuth loopback flow.
#[tauri::command]
pub async fn auth_google_cancel(state: State<'_, AuthState>) -> AuthResult<bool> {
    state.google_cancel.store(true, Ordering::SeqCst);
    Ok(true)
}

fn urlencoding(input: &str) -> String {
    let mut out = String::new();
    for b in input.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => {
                out.push_str(&format!("%{:02X}", b));
            }
        }
    }
    out
}

fn open_browser(url: &str) {
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn();
    }

    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(url).spawn();
    }

    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    }
}

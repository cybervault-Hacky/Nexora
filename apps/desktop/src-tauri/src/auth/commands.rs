use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tauri::State;

use super::client::AuthApiClient;
use super::pkce::{await_callback, bind_loopback, generate_pkce};
use super::storage::{delete_session_token, get_session_token, save_session_token};
use super::{
    AuthError, AuthResult, AuthSessionInfo, AuthStatusResponse, AuthUser, ChangePasswordPayload,
    DeveloperDiagnostics, LoginPayload, RecoveryPayload, RegisterPayload,
    ResendVerificationPayload, ResetPasswordPayload, VerifyEmailPayload,
};

const REFRESH_COALESCE_WINDOW: Duration = Duration::from_secs(2);

pub trait CredentialVault: Send + Sync {
    fn save_token(&self, token: &str) -> AuthResult<()>;
    fn get_token(&self) -> AuthResult<Option<String>>;
    fn delete_token(&self) -> AuthResult<()>;
}

struct OsCredentialVault;

impl CredentialVault for OsCredentialVault {
    fn save_token(&self, token: &str) -> AuthResult<()> {
        save_session_token(token)
    }

    fn get_token(&self) -> AuthResult<Option<String>> {
        get_session_token()
    }

    fn delete_token(&self) -> AuthResult<()> {
        delete_session_token()
    }
}

struct RecentRotation {
    token: String,
    rotated_at: Instant,
}

pub struct AuthState {
    pub client: AuthApiClient,
    pub current_user: Mutex<Option<AuthUser>>,
    pub google_cancel: Arc<AtomicBool>,
    vault: Arc<dyn CredentialVault>,
    refresh_state: tokio::sync::Mutex<Option<RecentRotation>>,
}

impl AuthState {
    pub fn new() -> Self {
        Self {
            client: AuthApiClient::new(),
            current_user: Mutex::new(None),
            google_cancel: Arc::new(AtomicBool::new(false)),
            vault: Arc::new(OsCredentialVault),
            refresh_state: tokio::sync::Mutex::new(None),
        }
    }

    #[cfg(test)]
    pub(crate) fn with_vault(client: AuthApiClient, vault: Arc<dyn CredentialVault>) -> Self {
        Self {
            client,
            current_user: Mutex::new(None),
            google_cancel: Arc::new(AtomicBool::new(false)),
            vault,
            refresh_state: tokio::sync::Mutex::new(None),
        }
    }

    pub fn read_session_token(&self) -> AuthResult<Option<String>> {
        self.vault.get_token()
    }

    pub fn require_session_token(&self) -> AuthResult<String> {
        self.vault.get_token()?.ok_or_else(|| {
            AuthError::new("unauthenticated", "Sign in to access your Nexora account.")
        })
    }

    pub fn persist_session_token(&self, token: &str) -> AuthResult<()> {
        self.vault.save_token(token)
    }

    pub fn purge_local_session(&self) {
        let _ = self.vault.delete_token();
        let mut user_guard = match self.current_user.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        *user_guard = None;
    }

    fn cache_user(&self, user: AuthUser) {
        let mut guard = match self.current_user.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        *guard = Some(user);
    }

    fn cached_user(&self) -> Option<AuthUser> {
        match self.current_user.lock() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }

    fn inspect_auth_result<T>(&self, result: AuthResult<T>) -> AuthResult<T> {
        if let Err(ref err) = result {
            if err.is_session_invalid() {
                self.purge_local_session();
            }
        }
        result
    }

    /// Coalesces concurrent bearer session rotation requests onto a single
    /// `POST /v1/auth/refresh` round-trip so parallel desktop callers never
    /// replay an already-rotated bearer token or trigger reuse revocation.
    pub async fn refresh_session(&self) -> AuthResult<AuthSessionInfo> {
        let token_before_lock = self.require_session_token()?;
        let mut rotation_guard = self.refresh_state.lock().await;

        let active_token = match self.read_session_token()? {
            Some(tok) => tok,
            None => return Err(AuthError::session_expired()),
        };

        let recently_coalesced = rotation_guard.as_ref().is_some_and(|recent| {
            recent.token == active_token && recent.rotated_at.elapsed() < REFRESH_COALESCE_WINDOW
        });

        let latest_token = if active_token != token_before_lock || recently_coalesced {
            active_token
        } else {
            match self.client.refresh(&active_token).await {
                Ok(rotated) => {
                    let next_token = rotated.token().to_string();
                    self.persist_session_token(&next_token)?;
                    *rotation_guard = Some(RecentRotation {
                        token: next_token.clone(),
                        rotated_at: Instant::now(),
                    });
                    next_token
                }
                Err(err) => {
                    if err.is_session_invalid() {
                        *rotation_guard = None;
                        self.purge_local_session();
                    }
                    return Err(err);
                }
            }
        };

        let resolved_user = match self.cached_user() {
            Some(user) => user,
            None => match self.client.get_me(&latest_token).await {
                Ok(user) => {
                    self.cache_user(user.clone());
                    user
                }
                Err(err) => {
                    if err.is_session_invalid() {
                        *rotation_guard = None;
                        self.purge_local_session();
                    }
                    return Err(err);
                }
            },
        };

        Ok(AuthSessionInfo {
            user: Some(resolved_user),
            authenticated: true,
        })
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
    if let Some(user) = state.cached_user() {
        return Ok(AuthSessionInfo {
            user: Some(user),
            authenticated: true,
        });
    }

    // Attempt restoring session from secure storage (Windows Credential Manager)
    if let Ok(Some(token)) = state.read_session_token() {
        match state.client.get_me(&token).await {
            Ok(user) => {
                state.cache_user(user.clone());
                return Ok(AuthSessionInfo {
                    user: Some(user),
                    authenticated: true,
                });
            }
            Err(e) if e.is_session_invalid() || e.code == "invalid_credentials" => {
                state.purge_local_session();
            }
            Err(_) => {
                // Network failure or offline unconfigured build; preserve local state intact.
            }
        }
    }

    Ok(AuthSessionInfo {
        user: None,
        authenticated: false,
    })
}

/// Rotates the active bearer session using single-flight coordination and updates
/// OS credential storage without exposing raw tokens to the webview.
#[tauri::command]
pub async fn auth_refresh_session(state: State<'_, AuthState>) -> AuthResult<AuthSessionInfo> {
    state.refresh_session().await
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

    state.persist_session_token(&token)?;
    state.cache_user(user.clone());

    Ok(user)
}

/// Registers a new account and surfaces the server's honest verification status.
#[tauri::command]
pub async fn auth_register(
    state: State<'_, AuthState>,
    request: RegisterPayload,
) -> AuthResult<AuthStatusResponse> {
    state
        .client
        .register(&request.name, &request.email, &request.password)
        .await
}

/// Verifies an email token.
#[tauri::command]
pub async fn auth_verify_email(
    state: State<'_, AuthState>,
    request: VerifyEmailPayload,
) -> AuthResult<AuthStatusResponse> {
    state.client.verify_email(&request.token).await?;

    Ok(AuthStatusResponse::new(
        "verified",
        "Email verified successfully. You can now sign in.",
    ))
}

/// Requests a new email verification message.
#[tauri::command]
pub async fn auth_resend_verification(
    state: State<'_, AuthState>,
    request: ResendVerificationPayload,
) -> AuthResult<AuthStatusResponse> {
    state.client.resend_verification(&request.email).await
}

/// Requests a password reset link.
#[tauri::command]
pub async fn auth_recover_password(
    state: State<'_, AuthState>,
    request: RecoveryPayload,
) -> AuthResult<AuthStatusResponse> {
    state.client.recovery_request(&request.email).await
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
    state.purge_local_session();

    Ok(AuthStatusResponse::new(
        "reset",
        "Password updated successfully. You can now sign in with your new password.",
    ))
}

/// Changes the authenticated user's password using their current password.
#[tauri::command]
pub async fn auth_change_password(
    state: State<'_, AuthState>,
    request: ChangePasswordPayload,
) -> AuthResult<AuthStatusResponse> {
    let token = state.read_session_token()?.ok_or_else(|| {
        AuthError::new(
            "unauthenticated",
            "You must be signed in to change your password.",
        )
    })?;

    state.inspect_auth_result(
        state
            .client
            .password_change(&token, &request.current_password, &request.new_password)
            .await,
    )?;

    Ok(AuthStatusResponse::new(
        "success",
        "Password changed successfully.",
    ))
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
    if let Ok(Some(token)) = state.read_session_token() {
        let _ = state.client.logout(&token).await;
    }

    state.purge_local_session();

    Ok(true)
}

/// Begins Google OAuth PKCE flow using system browser and loopback redirect.
#[tauri::command]
pub async fn auth_google_begin(state: State<'_, AuthState>) -> AuthResult<AuthUser> {
    state.google_cancel.store(false, Ordering::SeqCst);

    let pkce = generate_pkce()?;
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
    .map_err(|_| AuthError::new("task_error", "OAuth listener task ended unexpectedly."))?;

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

    state.persist_session_token(&token)?;
    state.cache_user(user.clone());

    Ok(user)
}

/// Cancels an in-progress Google OAuth loopback flow.
#[tauri::command]
pub async fn auth_google_cancel(state: State<'_, AuthState>) -> AuthResult<bool> {
    state.google_cancel.store(true, Ordering::SeqCst);
    Ok(true)
}

/// Fetches the enriched account profile (providers, password flag, active plan).
#[tauri::command]
pub async fn account_get_profile(state: State<'_, AuthState>) -> AuthResult<super::AccountProfile> {
    let token = state.require_session_token()?;
    state.inspect_auth_result(state.client.get_account_profile(&token).await)
}

/// Updates the signed-in user's display name and refreshes cached session identity.
#[tauri::command]
pub async fn account_update_profile(
    state: State<'_, AuthState>,
    request: super::UpdateProfilePayload,
) -> AuthResult<super::AccountProfile> {
    let token = state.require_session_token()?;
    let profile =
        state.inspect_auth_result(state.client.update_account_profile(&token, &request).await)?;
    state.cache_user(AuthUser {
        id: profile.id.clone(),
        name: profile.name.clone(),
        email: profile.email.clone(),
        email_verified: profile.email_verified,
    });
    Ok(profile)
}

/// Links a verified Google OAuth identity to the currently authenticated Nexora account.
#[tauri::command]
pub async fn account_link_google(state: State<'_, AuthState>) -> AuthResult<super::AccountProfile> {
    let token = state.require_session_token()?;
    state.google_cancel.store(false, Ordering::SeqCst);

    let pkce = generate_pkce()?;
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
    open_browser(&auth_url);

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
    .map_err(|_| AuthError::new("task_error", "OAuth listener task ended unexpectedly."))?;

    let code = callback?;
    state.inspect_auth_result(
        state
            .client
            .link_google(
                &token,
                &code,
                &pkce.code_verifier,
                &pkce.state,
                &redirect_uri,
                Some(&pkce.nonce),
            )
            .await,
    )
}

/// Exports all server-held user data as structured JSON for privacy/GDPR portability.
#[tauri::command]
pub async fn account_export_data(state: State<'_, AuthState>) -> AuthResult<serde_json::Value> {
    let token = state.require_session_token()?;
    state.inspect_auth_result(state.client.export_account_data(&token).await)
}

/// Permanently deletes the user's server account and clears local session credentials.
#[tauri::command]
pub async fn account_delete(
    state: State<'_, AuthState>,
    request: super::DeleteAccountPayload,
) -> AuthResult<bool> {
    let token = state.require_session_token()?;
    state.inspect_auth_result(state.client.delete_account(&token, &request).await)?;
    state.purge_local_session();
    Ok(true)
}

/// Lists server-configured Nexora subscription tiers, prices, and per-model daily quotas.
#[tauri::command]
pub async fn billing_list_plans(state: State<'_, AuthState>) -> AuthResult<super::PlanCatalogDto> {
    state.client.list_plans().await
}

/// Fetches the signed-in user's active subscription, effective plan, and recent orders.
#[tauri::command]
pub async fn billing_get_status(
    state: State<'_, AuthState>,
) -> AuthResult<super::BillingStatusDto> {
    let token = state.require_session_token()?;
    state.inspect_auth_result(state.client.get_billing_status(&token).await)
}

/// Creates a server-priced checkout session and opens it in the user's system browser.
#[tauri::command]
pub async fn billing_create_checkout(
    state: State<'_, AuthState>,
    request: super::CheckoutPayload,
) -> AuthResult<super::CheckoutSessionDto> {
    let token = state.require_session_token()?;
    let session =
        state.inspect_auth_result(state.client.create_checkout(&token, &request).await)?;
    validate_external_https_url(&session.checkout_url)?;
    open_browser(&session.checkout_url);
    Ok(session)
}

/// Reconciles an order directly against the payment provider and returns refreshed entitlements.
#[tauri::command]
pub async fn billing_verify_order(
    state: State<'_, AuthState>,
    request: super::VerifyOrderPayload,
) -> AuthResult<super::BillingStatusDto> {
    let token = state.require_session_token()?;
    state.inspect_auth_result(state.client.verify_order(&token, &request).await)
}

/// Schedules cancellation at period end or cancels immediately.
#[tauri::command]
pub async fn billing_cancel_subscription(
    state: State<'_, AuthState>,
    request: super::CancelSubscriptionPayload,
) -> AuthResult<super::BillingStatusDto> {
    let token = state.require_session_token()?;
    state.inspect_auth_result(state.client.cancel_subscription(&token, &request).await)
}

/// Upgrades or downgrades the user's active subscription plan.
#[tauri::command]
pub async fn billing_change_plan(
    state: State<'_, AuthState>,
    request: super::ChangePlanPayload,
) -> AuthResult<super::BillingStatusDto> {
    let token = state.require_session_token()?;
    state.inspect_auth_result(state.client.change_plan(&token, &request).await)
}

/// Creates a billing management portal session and launches it safely in the system browser.
#[tauri::command]
pub async fn billing_open_portal(
    state: State<'_, AuthState>,
) -> AuthResult<super::PortalSessionDto> {
    let token = state.require_session_token()?;
    let portal = state.inspect_auth_result(state.client.create_portal(&token).await)?;
    validate_external_https_url(&portal.portal_url)?;
    open_browser(&portal.portal_url);
    Ok(portal)
}

/// Returns backend-authoritative daily per-model quota usage, falling back honestly when offline.
#[tauri::command]
pub async fn usage_get_server_summary(
    state: State<'_, AuthState>,
    local_usage: State<'_, crate::usage::UsageState>,
) -> AuthResult<super::ServerUsageSummaryDto> {
    let token = state.require_session_token()?;
    match state.inspect_auth_result(state.client.get_usage_summary(&token).await) {
        Ok(summary) => Ok(summary),
        Err(err) if err.code == "network_error" || err.code == "backend_unconfigured" => {
            let snap = local_usage.status(std::time::SystemTime::now());
            Ok(super::ServerUsageSummaryDto {
                plan_id: "free".to_string(),
                plan_name: "Free (Offline Fallback)".to_string(),
                period_day_utc: snap.day,
                resets_at_utc: format!(
                    "{} (offline local allowance)",
                    crate::usage::format_utc_instant(snap.reset_at_ms)
                ),
                models: vec![
                    super::ModelUsageItemDto {
                        model_id: "nexora-mini".to_string(),
                        daily_limit: snap.allowance as i32,
                        used: snap.used as i32,
                        reserved: 0,
                        remaining: snap.remaining as i32,
                    },
                    super::ModelUsageItemDto {
                        model_id: "nexora-sonnet".to_string(),
                        daily_limit: snap.allowance as i32,
                        used: snap.used as i32,
                        reserved: 0,
                        remaining: snap.remaining as i32,
                    },
                ],
                offline_fallback: true,
            })
        }
        Err(err) => Err(err),
    }
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

pub(crate) fn validate_external_https_url(url: &str) -> AuthResult<&str> {
    let trimmed = url.trim();
    if !trimmed.starts_with("https://") {
        return Err(AuthError::new(
            "unsafe_url",
            "Refusing to open non-HTTPS external URL.",
        ));
    }
    if trimmed.len() > 2048
        || trimmed
            .chars()
            .any(|c| c.is_control() || c.is_whitespace() || c == '"')
    {
        return Err(AuthError::new(
            "unsafe_url",
            "External URL contains forbidden control or whitespace characters.",
        ));
    }
    Ok(trimmed)
}

fn open_browser(url: &str) {
    let Ok(safe_url) = validate_external_https_url(url) else {
        return;
    };

    #[cfg(target_os = "windows")]
    {
        // Use rundll32 FileProtocolHandler rather than `cmd /C start` so `&` in OAuth/checkout
        // query strings is NEVER interpreted as a shell command separator on Windows 10/11.
        let _ = std::process::Command::new("rundll32.exe")
            .args(["url.dll,FileProtocolHandler", safe_url])
            .spawn();
    }

    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(safe_url).spawn();
    }

    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("xdg-open").arg(safe_url).spawn();
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    use super::{validate_external_https_url, AuthState, CredentialVault};
    use crate::auth::client::AuthApiClient;
    use crate::auth::{AuthResult, AuthUser};

    #[derive(Default)]
    pub(crate) struct TestVault {
        token: Mutex<Option<String>>,
    }

    impl TestVault {
        pub(crate) fn with_token(token: &str) -> Arc<Self> {
            Arc::new(Self {
                token: Mutex::new(Some(token.to_string())),
            })
        }
    }

    impl CredentialVault for TestVault {
        fn save_token(&self, token: &str) -> AuthResult<()> {
            *self.token.lock().unwrap() = Some(token.to_string());
            Ok(())
        }

        fn get_token(&self) -> AuthResult<Option<String>> {
            Ok(self.token.lock().unwrap().clone())
        }

        fn delete_token(&self) -> AuthResult<()> {
            *self.token.lock().unwrap() = None;
            Ok(())
        }
    }

    #[test]
    fn allows_valid_https_urls_with_query_ampersands() {
        let url = "https://accounts.google.com/o/oauth2/v2/auth?response_type=code&client_id=abc&state=123";
        assert_eq!(validate_external_https_url(url).unwrap(), url);
    }

    #[test]
    fn rejects_non_https_or_whitespace_or_control_urls() {
        assert!(validate_external_https_url("http://insecure.example/pay").is_err());
        assert!(validate_external_https_url("file:///C:/Windows/System32/calc.exe").is_err());
        assert!(validate_external_https_url("javascript:alert(1)").is_err());
        assert!(validate_external_https_url("https://ok.example/a b").is_err());
        assert!(validate_external_https_url("https://ok.example/a\r\nb").is_err());
    }

    #[tokio::test]
    async fn concurrent_refresh_coalesces_to_single_network_rotation_without_token_reuse() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock auth server");
        let addr = listener.local_addr().expect("mock addr");

        let refresh_calls = Arc::new(AtomicUsize::new(0));
        let family_revoked = Arc::new(AtomicBool::new(false));
        let stop = Arc::new(AtomicBool::new(false));

        let refresh_calls_srv = Arc::clone(&refresh_calls);
        let family_revoked_srv = Arc::clone(&family_revoked);
        let stop_srv = Arc::clone(&stop);

        std::thread::spawn(move || {
            listener
                .set_nonblocking(false)
                .expect("blocking mock accept");
            let active_token = Mutex::new("tok-initial-generation-1".to_string());

            while !stop_srv.load(Ordering::SeqCst) {
                let Ok((mut stream, _)) = listener.accept() else {
                    break;
                };
                if stop_srv.load(Ordering::SeqCst) {
                    break;
                }

                let mut buf = [0u8; 4096];
                let n = stream.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]);

                if req.starts_with("POST /v1/auth/refresh ") {
                    let count = refresh_calls_srv.fetch_add(1, Ordering::SeqCst) + 1;
                    let mut current = active_token.lock().unwrap();
                    let expected_header = format!("authorization: bearer {}", *current);
                    if !req.to_ascii_lowercase().contains(&expected_header) {
                        family_revoked_srv.store(true, Ordering::SeqCst);
                        let body = r#"{"error":{"code":"invalid_or_expired_token","message":"Reuse detected; session family revoked"}}"#;
                        let resp = format!(
                            "HTTP/1.1 401 Unauthorized\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                            body.len()
                        );
                        let _ = stream.write_all(resp.as_bytes());
                    } else {
                        let next = format!("tok-rotated-generation-{}", count + 1);
                        *current = next.clone();
                        let body = format!(
                            r#"{{"session":{{"token":"{next}","expiresAt":"2026-11-21T00:00:00Z"}}}}"#
                        );
                        let resp = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                            body.len()
                        );
                        let _ = stream.write_all(resp.as_bytes());
                    }
                } else if req.starts_with("GET /v1/auth/me ") {
                    let current = active_token.lock().unwrap();
                    let expected_header = format!("authorization: bearer {}", *current);
                    if !req.to_ascii_lowercase().contains(&expected_header) {
                        let body = r#"{"error":{"code":"invalid_or_expired_token","message":"Stale token"}}"#;
                        let resp = format!(
                            "HTTP/1.1 401 Unauthorized\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                            body.len()
                        );
                        let _ = stream.write_all(resp.as_bytes());
                    } else {
                        let body = r#"{"user":{"id":"usr_01","name":"Ada Lovelace","email":"ada@example.org","emailVerified":true}}"#;
                        let resp = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                            body.len()
                        );
                        let _ = stream.write_all(resp.as_bytes());
                    }
                }
            }
        });

        let vault = TestVault::with_token("tok-initial-generation-1");
        let state = Arc::new(AuthState::with_vault(
            AuthApiClient::with_base_url(format!("http://{addr}")),
            Arc::clone(&vault) as Arc<dyn CredentialVault>,
        ));

        let mut handles = Vec::new();
        for _ in 0..8 {
            let st = Arc::clone(&state);
            handles.push(tokio::spawn(async move { st.refresh_session().await }));
        }

        for handle in handles {
            let info = handle
                .await
                .expect("join refresh task")
                .expect("refresh ok");
            assert!(info.authenticated);
            assert_eq!(info.user.unwrap().email, "ada@example.org");
        }

        stop.store(true, Ordering::SeqCst);
        let _ = std::net::TcpStream::connect(addr);

        assert_eq!(refresh_calls.load(Ordering::SeqCst), 1);
        assert!(!family_revoked.load(Ordering::SeqCst));
        assert_eq!(
            vault.get_token().unwrap().as_deref(),
            Some("tok-rotated-generation-2")
        );
    }

    #[tokio::test]
    async fn refresh_401_purges_stored_token_and_cached_user_immediately() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind 401 mock");
        let addr = listener.local_addr().expect("addr");
        let calls = Arc::new(AtomicUsize::new(0));
        let calls_srv = Arc::clone(&calls);

        std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                calls_srv.fetch_add(1, Ordering::SeqCst);
                let mut buf = [0u8; 2048];
                let _ = stream.read(&mut buf);
                let body = r#"{"error":{"code":"invalid_or_expired_token","message":"Expired"}}"#;
                let resp = format!(
                    "HTTP/1.1 401 Unauthorized\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(resp.as_bytes());
            }
        });

        let vault = TestVault::with_token("dead-token-123");
        let state = AuthState::with_vault(
            AuthApiClient::with_base_url(format!("http://{addr}")),
            Arc::clone(&vault) as Arc<dyn CredentialVault>,
        );
        state.cache_user(AuthUser {
            id: "u1".to_string(),
            name: "Cached User".to_string(),
            email: "cached@example.org".to_string(),
            email_verified: true,
        });

        let err = state.refresh_session().await.unwrap_err();
        assert_eq!(err.code, "session_expired");
        assert!(vault.get_token().unwrap().is_none());
        assert!(state.cached_user().is_none());
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
}

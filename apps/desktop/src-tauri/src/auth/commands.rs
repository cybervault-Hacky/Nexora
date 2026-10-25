use std::collections::BTreeMap;
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
    google_flow_status: Mutex<GoogleFlowStatus>,
    vault: Arc<dyn CredentialVault>,
    refresh_state: tokio::sync::Mutex<Option<RecentRotation>>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum GoogleFlowStatus {
    Idle,
    Active,
    Cancelled,
    Completed,
}

struct GoogleFlowGuard<'a> {
    status: &'a Mutex<GoogleFlowStatus>,
}

impl Drop for GoogleFlowGuard<'_> {
    fn drop(&mut self) {
        let mut status = self
            .status
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *status = GoogleFlowStatus::Idle;
    }
}

impl AuthState {
    pub fn new() -> Self {
        Self {
            client: AuthApiClient::new(),
            current_user: Mutex::new(None),
            google_cancel: Arc::new(AtomicBool::new(false)),
            google_flow_status: Mutex::new(GoogleFlowStatus::Idle),
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
            google_flow_status: Mutex::new(GoogleFlowStatus::Idle),
            vault,
            refresh_state: tokio::sync::Mutex::new(None),
        }
    }

    fn begin_google_flow(&self) -> AuthResult<GoogleFlowGuard<'_>> {
        let mut status = self
            .google_flow_status
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if *status != GoogleFlowStatus::Idle {
            return Err(AuthError::new(
                "google_flow_in_progress",
                "A Google authentication flow is already in progress.",
            ));
        }
        self.google_cancel.store(false, Ordering::SeqCst);
        *status = GoogleFlowStatus::Active;
        drop(status);
        Ok(GoogleFlowGuard {
            status: &self.google_flow_status,
        })
    }

    fn cancel_google_flow(&self) -> bool {
        let mut status = self
            .google_flow_status
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        match *status {
            GoogleFlowStatus::Active => {
                *status = GoogleFlowStatus::Cancelled;
                self.google_cancel.store(true, Ordering::SeqCst);
                true
            }
            GoogleFlowStatus::Cancelled => true,
            GoogleFlowStatus::Idle | GoogleFlowStatus::Completed => false,
        }
    }

    fn complete_google_flow(&self) -> bool {
        let mut status = self
            .google_flow_status
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if *status == GoogleFlowStatus::Active {
            *status = GoogleFlowStatus::Completed;
            true
        } else {
            false
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
        let _ = self.try_purge_local_session();
    }

    /// Clears cached identity even if the OS credential vault refuses deletion.
    /// Account operations that promise a completed logout/deletion can surface
    /// the vault failure; best-effort invalid-session cleanup stays non-blocking.
    pub fn try_purge_local_session(&self) -> AuthResult<()> {
        let delete_result = self.vault.delete_token();
        let mut user_guard = match self.current_user.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        *user_guard = None;
        delete_result
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

    // A vault failure is an account-status error, not a fabricated signed-out
    // state. Local chat separately treats missing/unreadable account state as
    // an optional warning and continues offline.
    if let Some(token) = state.read_session_token()? {
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

    if let Err(error) = state.persist_session_token(&token) {
        let _ = state.client.logout(&token).await;
        state.purge_local_session();
        return Err(error);
    }
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

    // Password reset revokes all sessions; report if persistent local cleanup failed.
    state.try_purge_local_session()?;

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

fn configured_google_client_id(raw: Option<&str>) -> Option<&str> {
    let value = raw?.trim();
    let lower = value.to_ascii_lowercase();
    if value.is_empty()
        || value.len() > 512
        || !value.ends_with(".apps.googleusercontent.com")
        || value
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
        || [
            "replace_with",
            "change_me",
            "changeme",
            "placeholder",
            "your_",
            "your-",
            "example.invalid",
            "example.test",
        ]
        .iter()
        .any(|marker| lower.contains(marker))
    {
        None
    } else {
        Some(value)
    }
}

fn select_google_client_id(runtime: Option<&str>, packaged: Option<&str>) -> Option<String> {
    configured_google_client_id(runtime.or(packaged)).map(str::to_owned)
}

fn desktop_google_client_id() -> Option<String> {
    let runtime = std::env::var("GOOGLE_OAUTH_CLIENT_ID").ok();
    select_google_client_id(
        runtime.as_deref(),
        option_env!("NEXORA_GOOGLE_OAUTH_CLIENT_ID"),
    )
}

fn require_google_client_id() -> AuthResult<String> {
    desktop_google_client_id().ok_or_else(|| {
        AuthError::new(
            "dependency_unavailable",
            "Google sign-in is not configured in this desktop build.",
        )
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
        google_oauth_configured: desktop_google_client_id().is_some(),
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

    state.try_purge_local_session()?;

    Ok(true)
}

/// Begins Google OAuth PKCE flow using system browser and loopback redirect.
#[tauri::command]
pub async fn auth_google_begin(state: State<'_, AuthState>) -> AuthResult<AuthUser> {
    let _flow_guard = state.begin_google_flow()?;
    let client_id = require_google_client_id()?;

    let pkce = generate_pkce()?;
    let (listener, port) = bind_loopback()?;
    let redirect_uri = format!("http://127.0.0.1:{port}/callback");

    let auth_url = format!(
        "https://accounts.google.com/o/oauth2/v2/auth?response_type=code&client_id={}&redirect_uri={}&scope=openid%20email%20profile&state={}&code_challenge={}&code_challenge_method=S256&nonce={}",
        urlencoding(&client_id),
        urlencoding(&redirect_uri),
        urlencoding(&pkce.state),
        urlencoding(&pkce.code_challenge),
        urlencoding(&pkce.nonce),
    );

    // Open system browser without ever sending the authorization URL through a shell.
    open_browser_result(&auth_url)?;

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
    if state.google_cancel.load(Ordering::SeqCst) {
        return Err(AuthError::new(
            "cancelled",
            "Authentication flow was cancelled.",
        ));
    }

    // Only the trusted auth service exchanges the code and issues the Nexora session.
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

    if !state.complete_google_flow() {
        let _ = state.client.logout(&token).await;
        return Err(AuthError::new(
            "cancelled",
            "Authentication flow was cancelled.",
        ));
    }
    if let Err(error) = state.persist_session_token(&token) {
        let _ = state.client.logout(&token).await;
        state.purge_local_session();
        return Err(error);
    }
    state.cache_user(user.clone());

    Ok(user)
}

/// Cancels an in-progress Google OAuth loopback flow.
#[tauri::command]
pub async fn auth_google_cancel(state: State<'_, AuthState>) -> AuthResult<bool> {
    Ok(state.cancel_google_flow())
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
    let _flow_guard = state.begin_google_flow()?;
    let client_id = require_google_client_id()?;

    let pkce = generate_pkce()?;
    let (listener, port) = bind_loopback()?;
    let redirect_uri = format!("http://127.0.0.1:{port}/callback");

    let auth_url = format!(
        "https://accounts.google.com/o/oauth2/v2/auth?response_type=code&client_id={}&redirect_uri={}&scope=openid%20email%20profile&state={}&code_challenge={}&code_challenge_method=S256&nonce={}",
        urlencoding(&client_id),
        urlencoding(&redirect_uri),
        urlencoding(&pkce.state),
        urlencoding(&pkce.code_challenge),
        urlencoding(&pkce.nonce),
    );
    open_browser_result(&auth_url)?;

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
    if state.google_cancel.load(Ordering::SeqCst) {
        return Err(AuthError::new(
            "cancelled",
            "Authentication flow was cancelled.",
        ));
    }
    if !state.complete_google_flow() {
        return Err(AuthError::new(
            "cancelled",
            "Authentication flow was cancelled.",
        ));
    }
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
    state.try_purge_local_session()?;
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

/// Requests cancellation; the backend currently fails closed until a verified
/// provider lifecycle operation is supported.
#[tauri::command]
pub async fn billing_cancel_subscription(
    state: State<'_, AuthState>,
    request: super::CancelSubscriptionPayload,
) -> AuthResult<super::BillingStatusDto> {
    let token = state.require_session_token()?;
    state.inspect_auth_result(state.client.cancel_subscription(&token, &request).await)
}

/// Requests a plan change; the backend currently fails closed without
/// provider-verified lifecycle support.
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

/// Returns a usage summary only when the authenticated backend can report one.
/// Network/backend failures are not converted into a fabricated Free plan or
/// account usage; they do not affect the separate local inference path.
#[tauri::command]
pub async fn usage_get_server_summary(
    state: State<'_, AuthState>,
) -> AuthResult<super::ServerUsageSummaryDto> {
    let token = state.require_session_token()?;
    state.inspect_auth_result(state.client.get_usage_summary(&token).await)
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

fn valid_loopback_oauth_redirect(raw: &str) -> bool {
    let Ok(parsed) = reqwest::Url::parse(raw) else {
        return false;
    };
    parsed.scheme() == "http"
        && parsed.host_str() == Some("127.0.0.1")
        && parsed.port().is_some_and(|port| port > 0)
        && parsed.path() == "/callback"
        && parsed.query().is_none()
        && parsed.fragment().is_none()
        && parsed.username().is_empty()
        && parsed.password().is_none()
}

fn is_base64url(value: &str, expected_len: usize) -> bool {
    value.len() == expected_len
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

/// Only the fixed Google OAuth authorization destination may be opened by the
/// desktop. Billing URLs remain unavailable until an approved provider exists.
pub(crate) fn validate_external_https_url(url: &str) -> AuthResult<&str> {
    let trimmed = url.trim();
    if trimmed.is_empty()
        || trimmed.len() > 2_048
        || trimmed
            .chars()
            .any(|character| {
                character.is_control()
                    || character.is_whitespace()
                    || matches!(character, '"' | '\'' | '\\')
            })
    {
        return Err(AuthError::new(
            "unsafe_url",
            "External browser URL contains invalid characters.",
        ));
    }

    let parsed = reqwest::Url::parse(trimmed).map_err(|_| {
        AuthError::new(
            "unsafe_url",
            "Only the fixed Google OAuth authorization destination may be opened.",
        )
    })?;
    let raw_authority = trimmed
        .split_once("://")
        .map(|(_, rest)| rest.split('/').next().unwrap_or(""))
        .unwrap_or("");
    if parsed.scheme() != "https"
        || parsed.host_str() != Some("accounts.google.com")
        || parsed.port().is_some()
        || parsed.path() != "/o/oauth2/v2/auth"
        || parsed.fragment().is_some()
        || raw_authority.contains('@')
        || !parsed.username().is_empty()
        || parsed.password().is_some()
    {
        return Err(AuthError::new(
            "unsafe_url",
            "Only the fixed Google OAuth authorization destination may be opened.",
        ));
    }

    let mut parameters = BTreeMap::new();
    for (key, value) in parsed.query_pairs() {
        if parameters.insert(key.into_owned(), value.into_owned()).is_some() {
            return Err(AuthError::new(
                "unsafe_url",
                "Google authorization URL contains duplicate parameters.",
            ));
        }
    }
    const ALLOWED_PARAMETERS: [&str; 8] = [
        "response_type",
        "client_id",
        "redirect_uri",
        "scope",
        "state",
        "code_challenge",
        "code_challenge_method",
        "nonce",
    ];
    if parameters.len() != ALLOWED_PARAMETERS.len()
        || parameters
            .keys()
            .any(|key| !ALLOWED_PARAMETERS.contains(&key.as_str()))
    {
        return Err(AuthError::new(
            "unsafe_url",
            "Google authorization URL contains missing or unsupported parameters.",
        ));
    }

    let client_id = parameters.get("client_id").map(String::as_str);
    let redirect_uri = parameters.get("redirect_uri").map(String::as_str);
    if parameters.get("response_type").map(String::as_str) != Some("code")
        || configured_google_client_id(client_id).is_none()
        || !redirect_uri.is_some_and(valid_loopback_oauth_redirect)
        || parameters.get("scope").map(String::as_str) != Some("openid email profile")
        || !parameters
            .get("state")
            .is_some_and(|value| is_base64url(value, 22))
        || !parameters
            .get("code_challenge")
            .is_some_and(|value| is_base64url(value, 43))
        || parameters.get("code_challenge_method").map(String::as_str) != Some("S256")
        || !parameters
            .get("nonce")
            .is_some_and(|value| is_base64url(value, 22))
    {
        return Err(AuthError::new(
            "unsafe_url",
            "Google authorization URL failed local PKCE validation.",
        ));
    }

    Ok(trimmed)
}

fn open_browser_result(url: &str) -> AuthResult<()> {
    let safe_url = validate_external_https_url(url)?;
    let spawn_error = || {
        AuthError::new(
            "browser_unavailable",
            "Could not open the system browser. Please open it and try again.",
        )
    };

    #[cfg(target_os = "windows")]
    {
        // Use rundll32 rather than a shell so OAuth query delimiters are never
        // interpreted as command separators.
        std::process::Command::new("rundll32.exe")
            .args(["url.dll,FileProtocolHandler", safe_url])
            .spawn()
            .map_err(|_| spawn_error())?;
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(safe_url)
            .spawn()
            .map_err(|_| spawn_error())?;
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(safe_url)
            .spawn()
            .map_err(|_| spawn_error())?;
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        let _ = safe_url;
        return Err(spawn_error());
    }

    Ok(())
}

fn open_browser(url: &str) {
    let _ = open_browser_result(url);
}

#[cfg(test)]
pub(crate) mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    use super::{
        configured_google_client_id, select_google_client_id, validate_external_https_url,
        AuthState, CredentialVault,
    };
    use crate::auth::client::AuthApiClient;
    use crate::auth::{AuthError, AuthResult, AuthUser};

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

    struct FailingDeleteVault;

    impl CredentialVault for FailingDeleteVault {
        fn save_token(&self, _token: &str) -> AuthResult<()> {
            Ok(())
        }

        fn get_token(&self) -> AuthResult<Option<String>> {
            Ok(None)
        }

        fn delete_token(&self) -> AuthResult<()> {
            Err(AuthError::storage("The credential vault could not remove the saved session."))
        }
    }

    #[test]
    fn strict_vault_cleanup_reports_failure_but_always_clears_cached_identity() {
        let state = AuthState::with_vault(
            AuthApiClient::unconfigured(),
            Arc::new(FailingDeleteVault),
        );
        state.cache_user(AuthUser {
            id: "SENTINEL_user_id".to_string(),
            name: "SENTINEL_user".to_string(),
            email: "user@example.test".to_string(),
            email_verified: true,
        });

        let error = state
            .try_purge_local_session()
            .expect_err("persistent cleanup failure must be visible to logout");
        assert_eq!(error.code, "storage_error");
        assert!(state.cached_user().is_none());
        assert!(!error.message.contains("SENTINEL"));
    }

    #[test]
    fn allows_only_the_fixed_google_oauth_destination_with_pkce_parameters() {
        let url = "https://accounts.google.com/o/oauth2/v2/auth?response_type=code&client_id=public-client.apps.googleusercontent.com&redirect_uri=http%3A%2F%2F127.0.0.1%3A49152%2Fcallback&scope=openid%20email%20profile&state=aaaaaaaaaaaaaaaaaaaaaa&code_challenge=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa&code_challenge_method=S256&nonce=bbbbbbbbbbbbbbbbbbbbbb";
        assert_eq!(validate_external_https_url(url).unwrap(), url);
    }

    #[test]
    fn browser_url_validation_rejects_non_google_hosts_and_malformed_pkce() {
        assert!(validate_external_https_url("http://insecure.example/pay").is_err());
        assert!(validate_external_https_url("file:///C:/Windows/System32/calc.exe").is_err());
        assert!(validate_external_https_url("javascript:alert(1)").is_err());
        assert!(validate_external_https_url("https://checkout.stripe.com/pay/session").is_err());
        assert!(validate_external_https_url("https://accounts.google.com.evil.example/o/oauth2/v2/auth").is_err());
        assert!(validate_external_https_url("https://accounts.google.com/o/oauth2/v2/auth?client_id=abc").is_err());
        assert!(validate_external_https_url("https://accounts.google.com/o/oauth2/v2/auth?response_type=code&client_id=public-client.apps.googleusercontent.com&redirect_uri=http%3A%2F%2F127.0.0.1%3A49152%2Fcallback&scope=openid%20email%20profile&state=short&code_challenge=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa&code_challenge_method=S256&nonce=bbbbbbbbbbbbbbbbbbbbbb").is_err());
        assert!(validate_external_https_url("https://accounts.google.com/o/oauth2/v2/auth?x=1&x=2").is_err());
    }

    #[test]
    fn rejects_whitespace_or_control_urls() {
        assert!(validate_external_https_url("https://accounts.google.com/a b").is_err());
        assert!(validate_external_https_url("https://accounts.google.com/a\r\nb").is_err());
    }

    #[test]
    fn google_client_id_configuration_fails_closed_for_missing_or_placeholders() {
        assert_eq!(configured_google_client_id(None), None);
        assert_eq!(configured_google_client_id(Some("  ")), None);
        assert_eq!(
            configured_google_client_id(Some(" REPLACE_WITH_GOOGLE_OAUTH_CLIENT_ID ")),
            None
        );
        assert_eq!(configured_google_client_id(Some("change_me")), None);
        assert_eq!(
            configured_google_client_id(Some(" test-client-id.apps.googleusercontent.com ")),
            Some("test-client-id.apps.googleusercontent.com")
        );
        assert_eq!(configured_google_client_id(Some("client id")), None);
    }

    #[test]
    fn packaged_google_client_id_works_without_a_parent_shell_environment() {
        let packaged = "public-client.apps.googleusercontent.com";
        assert_eq!(
            select_google_client_id(None, Some(packaged)).as_deref(),
            Some(packaged)
        );
        assert_eq!(
            select_google_client_id(
                Some("runtime-client.apps.googleusercontent.com"),
                Some(packaged),
            )
            .as_deref(),
            Some("runtime-client.apps.googleusercontent.com")
        );
        assert_eq!(
            select_google_client_id(None, Some("REPLACE_WITH_GOOGLE_OAUTH_CLIENT_ID")),
            None
        );
        assert_eq!(select_google_client_id(Some(""), Some(packaged)), None);
    }

    #[test]
    fn only_one_google_browser_flow_can_be_active_at_a_time() {
        let state = AuthState::with_vault(
            AuthApiClient::with_base_url("http://127.0.0.1:8787"),
            Arc::new(TestVault::default()),
        );
        let guard = state.begin_google_flow().expect("first flow starts");
        assert!(state.begin_google_flow().is_err());
        assert!(state.cancel_google_flow());
        assert!(!state.complete_google_flow());
        assert!(
            state.begin_google_flow().is_err(),
            "cancelled flow stays guarded until it exits"
        );
        drop(guard);

        let completed = state
            .begin_google_flow()
            .expect("next flow starts after cleanup");
        assert!(state.complete_google_flow());
        assert!(!state.cancel_google_flow(), "completed flow is no longer cancellable");
        drop(completed);
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

    #[tokio::test]
    async fn refresh_service_failure_preserves_stored_token_and_cached_user() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind 503 mock");
        let addr = listener.local_addr().expect("addr");
        std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 2048];
                let _ = stream.read(&mut buf);
                let body = r#"{"error":{"code":"dependency_unavailable","message":"Sessions are temporarily unavailable."}}"#;
                let resp = format!(
                    "HTTP/1.1 503 Service Unavailable\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(resp.as_bytes());
            }
        });

        let vault = TestVault::with_token("stored-session-token");
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
        assert_eq!(err.code, "dependency_unavailable");
        assert!(!err.is_session_invalid());
        assert_eq!(vault.get_token().unwrap().as_deref(), Some("stored-session-token"));
        assert_eq!(state.cached_user().unwrap().email, "cached@example.org");
    }
}

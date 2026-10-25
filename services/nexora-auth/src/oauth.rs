//! Google OAuth exchange (Phase 23F) — plan §2.5 (planned behaviour) and §6
//! `POST /v1/auth/google/exchange {code, codeVerifier, state, redirectUri}`
//! → `200 {user, session}` | `409 email_conflict`.
//!
//! Security invariants (plan §2.5/§4):
//! - **PKCE S256 + loopback redirect:** the desktop runs the system-browser
//!   flow with a `http://127.0.0.1:<port>/callback` loopback redirect and a
//!   PKCE verifier; this service performs the code exchange and **refuses any
//!   `redirectUri` that is not an exact match** for the configured one.
//! - **State:** 128-bit random, generated with the PKCE verifier in the
//!   desktop process, validated on the loopback callback, and mirrored here
//!   **hashed** — single-use (replays are refused), 10-minute window.
//! - **Nonce:** required on every exchange and compared to the verified OIDC
//!   ID token claim; the desktop generates it with the authorization request.
//! - **Code exchange runs on the server** (plan §2.5: client secret lives only
//!   in the server secret store; even secret-less desktop clients route the
//!   exchange through the server for uniform auditing and linking). The
//!   provider seam ([`GoogleProvider`]) is the same honesty pattern as
//!   [`crate::email::EmailProvider`]: [`UnconfiguredGoogleProvider`] never
//!   invents identities, and tests inject a real-logic fake at this boundary.
//! - **Identity/linking rules:** sign-in is keyed by the immutable Google
//!   `sub` (`oauth_accounts (provider, provider_subject)`), never by email.
//!   A verified Google email may create a new account only when no Nexora
//!   account already uses it. Existing-email matches require the user to
//!   authenticate to Nexora and link explicitly; email equality alone never
//!   links accounts. Unverified provider emails are refused outright.
//! - Tokens (authorization codes, access/ID tokens, PKCE verifiers, states)
//!   are never stored, never logged, and never returned in API responses.

use serde::Serialize;
use sqlx::PgPool;

use crate::login::LoginOutcome;
use crate::registration::{self, validate_email};
use crate::session;

/// Plan §2.5: state is single-use with a 10-minute window.
pub const STATE_WINDOW_SECS: u64 = 600;

/// Identity claims taken from Google's ID token (OIDC). Access/refresh tokens
/// are deliberately NOT part of this struct and must never be stored.
#[derive(Clone, PartialEq, Eq)]
pub struct GoogleIdentity {
    /// Google `sub` — immutable provider subject (the linking key).
    pub subject: String,
    /// Google-reported email (compared only after normalization).
    pub email: String,
    /// Google's `email_verified` claim — the ONLY basis for trusting the email.
    pub email_verified: bool,
    /// OIDC `nonce` claim, when the response type yields one.
    pub nonce: Option<String>,
    /// Optional display name claim.
    pub name: Option<String>,
}

impl std::fmt::Debug for GoogleIdentity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GoogleIdentity")
            .field("subject", &crate::scrub::REDACTED)
            .field("email", &crate::scrub::REDACTED)
            .field("email_verified", &self.email_verified)
            .field(
                "nonce",
                &self.nonce.as_ref().map(|_| crate::scrub::REDACTED),
            )
            .field("name", &self.name.as_ref().map(|_| crate::scrub::REDACTED))
            .finish()
    }
}

#[derive(Debug)]
pub enum GoogleExchangeError {
    /// The provider is not configured in this deployment (honest 503).
    NotConfigured,
    /// Google refused the exchange (bad/used/expired code, PKCE failure, …).
    /// Deliberately carries no provider detail.
    Refused,
}

/// The one network edge of the flow. Implementations must never log or return
/// raw codes/tokens.
pub trait GoogleProvider: Send + Sync {
    fn exchange_code<'a>(
        &'a self,
        code: &'a str,
        code_verifier: &'a str,
        redirect_uri: &'a str,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<GoogleIdentity, GoogleExchangeError>>
                + Send
                + 'a,
        >,
    >;
}

/// Honest default: no Google client is configured in this build, so the
/// exchange answers `dependency_unavailable` — **no identity is ever invented**.
pub struct UnconfiguredGoogleProvider;

impl GoogleProvider for UnconfiguredGoogleProvider {
    fn exchange_code<'a>(
        &'a self,
        _code: &'a str,
        _code_verifier: &'a str,
        _redirect_uri: &'a str,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<GoogleIdentity, GoogleExchangeError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async { Err(GoogleExchangeError::NotConfigured) })
    }
}

/// Real server-side exchange against Google's token endpoint
/// (`https://oauth2.googleapis.com/token` — Google's documented OAuth 2.0
/// endpoint, not an invented value). The response is received over TLS, and
/// the returned ID token is verified locally against Google's JWKS plus the
/// expected issuer, audience, expiry, nonce, and identity claims.
pub struct HttpGoogleProvider {
    client_id: String,
    /// Client secret — **server secret store only** (plan §8). Absent for the
    /// secret-less desktop client type (unresolved decision #3: this provider
    /// supports both shapes).
    client_secret: Option<String>,
    client: reqwest::Client,
    /// Cached Google JWKS (key rotation: refetched when stale or unknown kid).
    jwks_cache: std::sync::Mutex<Option<(std::time::Instant, JwksMaterial)>>,
}

/// JWKS cache lifetime (Google rotates keys; unknown kids force a refetch).
const JWKS_TTL: std::time::Duration = std::time::Duration::from_secs(3_600);
/// Google's documented JWKS endpoint for its OIDC ID tokens.
const GOOGLE_JWKS_URI: &str = "https://www.googleapis.com/oauth2/v3/certs";

impl HttpGoogleProvider {
    pub fn new(
        client_id: String,
        client_secret: Option<String>,
    ) -> Result<Self, reqwest::Error> {
        let client = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(5))
            .timeout(std::time::Duration::from_secs(15))
            // OAuth tokens and authorization codes must never be forwarded to
            // a redirect target if Google's endpoint responds unexpectedly.
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        Ok(Self {
            client_id,
            client_secret,
            client,
            jwks_cache: std::sync::Mutex::new(None),
        })
    }

    /// Fetch (or reuse) the JWKS document. `force` bypasses the cache — used
    /// when a token references a key we have not seen (rotation).
    async fn jwks_material(&self, force: bool) -> Result<JwksMaterial, GoogleExchangeError> {
        {
            let cache = self.jwks_cache.lock().unwrap();
            if let Some((fetched_at, material)) = cache.as_ref() {
                if !force && fetched_at.elapsed() < JWKS_TTL {
                    return Ok(material.clone());
                }
            }
        }
        let response = self
            .client
            .get(GOOGLE_JWKS_URI)
            .send()
            .await
            .map_err(|_| GoogleExchangeError::Refused)?;
        if !response.status().is_success() {
            return Err(GoogleExchangeError::Refused);
        }
        let material: JwksMaterial = response
            .json()
            .await
            .map_err(|_| GoogleExchangeError::Refused)?;
        *self.jwks_cache.lock().unwrap() = Some((std::time::Instant::now(), material.clone()));
        Ok(material)
    }
}

/// OIDC permits a single audience string or a list of audiences.
#[derive(serde::Deserialize)]
#[serde(untagged)]
enum AudienceClaim {
    Single(String),
    Multiple(Vec<String>),
}

impl AudienceClaim {
    fn contains(&self, expected: &str) -> bool {
        match self {
            Self::Single(audience) => audience == expected,
            Self::Multiple(audiences) => audiences.iter().any(|audience| audience == expected),
        }
    }

    fn len(&self) -> usize {
        match self {
            Self::Single(_) => 1,
            Self::Multiple(audiences) => audiences.len(),
        }
    }
}

/// ID-token claims payload (deserialized only AFTER the RS256 signature and
/// registered claims — iss/aud/exp — are verified).
#[derive(serde::Deserialize)]
struct IdTokenClaims {
    sub: String,
    iss: String,
    aud: AudienceClaim,
    exp: u64,
    #[serde(default)]
    azp: Option<String>,
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    email_verified: Option<bool>,
    #[serde(default)]
    nonce: Option<String>,
    #[serde(default)]
    name: Option<String>,
}

/// Redacting `Debug`: nonce is CSRF material, while sub/email/name are
/// personal identity data and should not leak through diagnostics either.
impl std::fmt::Debug for IdTokenClaims {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IdTokenClaims")
            .field("sub", &crate::scrub::REDACTED)
            .field("email", &crate::scrub::REDACTED)
            .field("email_verified", &self.email_verified)
            .field(
                "nonce",
                &self.nonce.as_ref().map(|_| crate::scrub::REDACTED),
            )
            .field("name", &self.name.as_ref().map(|_| crate::scrub::REDACTED))
            .finish()
    }
}

/// One Google verification key from the JWKS document (`kid`, RSA modulus,
/// exponent). Standard `serde` parsing — unknown fields ignored.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct JwkKey {
    pub kid: String,
    pub n: String,
    pub e: String,
}

/// JWKS document (Google's `oauth2/v3/certs`).
#[derive(Debug, Clone, serde::Deserialize)]
pub struct JwksMaterial {
    pub keys: Vec<JwkKey>,
}

/// **Proper OIDC ID-token verification (Phase 23G, plan step 6):** RS256
/// signature against JWKS key material + registered-claim checks
/// (`iss` ∈ Google issuers, `aud` == this client, `exp` valid). Unverified
/// token payloads are never trusted — any failure is the same generic
/// refusal. Testable against injected key material (see unit tests).
pub fn verify_id_token_with(
    id_token: &str,
    client_id: &str,
    jwks: &JwksMaterial,
) -> Result<GoogleIdentity, GoogleExchangeError> {
    use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};

    let header = decode_header(id_token).map_err(|_| GoogleExchangeError::Refused)?;
    // Never accept "none" or symmetric algs: Google signs with RS256 only.
    if header.alg != Algorithm::RS256 {
        return Err(GoogleExchangeError::Refused);
    }
    let kid = header.kid.ok_or(GoogleExchangeError::Refused)?;
    let key = jwks
        .keys
        .iter()
        .find(|key| key.kid == kid)
        .ok_or(GoogleExchangeError::Refused)?;
    let decoding = DecodingKey::from_rsa_components(&key.n, &key.e)
        .map_err(|_| GoogleExchangeError::Refused)?;

    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_issuer(&["https://accounts.google.com", "accounts.google.com"]);
    validation.set_audience(&[client_id]);
    validation.leeway = 0;
    let data = decode::<IdTokenClaims>(id_token, &decoding, &validation)
        .map_err(|_| GoogleExchangeError::Refused)?;

    let claims = data.claims;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| GoogleExchangeError::Refused)?
        .as_secs();
    if !matches!(claims.iss.as_str(), "https://accounts.google.com" | "accounts.google.com")
        || !claims.aud.contains(client_id)
        || claims.azp.as_deref().is_some_and(|azp| azp != client_id)
        || (claims.aud.len() > 1 && claims.azp.as_deref() != Some(client_id))
        || claims.exp <= now
    {
        return Err(GoogleExchangeError::Refused);
    }
    let email = claims.email.ok_or(GoogleExchangeError::Refused)?;
    Ok(GoogleIdentity {
        subject: claims.sub,
        email,
        email_verified: claims.email_verified.unwrap_or(false),
        nonce: claims.nonce,
        name: claims.name,
    })
}

impl GoogleProvider for HttpGoogleProvider {
    fn exchange_code<'a>(
        &'a self,
        code: &'a str,
        code_verifier: &'a str,
        redirect_uri: &'a str,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<GoogleIdentity, GoogleExchangeError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            #[derive(Serialize)]
            struct TokenForm<'a> {
                code: &'a str,
                client_id: &'a str,
                #[serde(skip_serializing_if = "Option::is_none")]
                client_secret: Option<&'a str>,
                redirect_uri: &'a str,
                grant_type: &'a str,
                code_verifier: &'a str,
            }
            #[derive(serde::Deserialize)]
            struct TokenResponse {
                id_token: String,
            }

            let form = TokenForm {
                code,
                client_id: &self.client_id,
                client_secret: self.client_secret.as_deref(),
                redirect_uri,
                grant_type: "authorization_code",
                code_verifier,
            };
            let response = self
                .client
                .post("https://oauth2.googleapis.com/token")
                .form(&form)
                .send()
                .await
                .map_err(|_| GoogleExchangeError::Refused)?;
            if !response.status().is_success() {
                return Err(GoogleExchangeError::Refused);
            }
            let body: TokenResponse = response
                .json()
                .await
                .map_err(|_| GoogleExchangeError::Refused)?;

            // Proper signature + claim verification (step 6). On an unknown
            // kid (key rotation) refetch the JWKS once and retry.
            let verify = |material: &JwksMaterial| -> Result<GoogleIdentity, GoogleExchangeError> {
                verify_id_token_with(&body.id_token, &self.client_id, material)
            };
            match verify(&self.jwks_material(false).await?) {
                Ok(identity) => Ok(identity),
                Err(GoogleExchangeError::Refused) => verify(&self.jwks_material(true).await?),
                Err(other) => Err(other),
            }
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OAuthError {
    /// 400 `validation_failed`.
    Validation(String),
    /// 400 `invalid_or_expired_token` — one generic failure for state
    /// mismatch/replay, redirect-URI mismatch, nonce mismatch, and any
    /// provider refusal (bad/reused code included).
    InvalidOrExpiredToken,
    /// 403 `email_unverified` — Google has not verified the provider email.
    EmailUnverified,
    /// 401 `invalid_credentials` — the linked/created account is not active.
    InvalidCredentials,
    /// 409 `email_conflict` — existing account match; no email-only auto-link.
    EmailConflict,
    /// 503 `dependency_unavailable`.
    DependencyUnavailable,
    /// 500 `internal`.
    Internal,
}

impl OAuthError {
    pub fn code(&self) -> &'static str {
        match self {
            OAuthError::Validation(_) => "validation_failed",
            OAuthError::InvalidOrExpiredToken => "invalid_or_expired_token",
            OAuthError::EmailUnverified => "email_unverified",
            OAuthError::InvalidCredentials => "invalid_credentials",
            OAuthError::EmailConflict => "email_conflict",
            OAuthError::DependencyUnavailable => "dependency_unavailable",
            OAuthError::Internal => "internal",
        }
    }
    pub fn message(&self) -> &str {
        match self {
            OAuthError::Validation(message) => message,
            OAuthError::InvalidOrExpiredToken => {
                "Google sign-in could not be completed. Try again."
            }
            OAuthError::EmailUnverified => {
                "Your Google account's email address is not verified by Google. \
                 Verify it with Google and try again."
            }
            OAuthError::InvalidCredentials => "This account cannot sign in.",
            OAuthError::EmailConflict => {
                "A Nexora account with this email already exists. Sign in to that account before linking Google."
            }
            OAuthError::DependencyUnavailable => {
                "Google sign-in is unavailable: it is not configured in this build."
            }
            OAuthError::Internal => "The request could not be completed.",
        }
    }
    pub fn status_code(&self) -> u16 {
        match self {
            OAuthError::Validation(_) => 400,
            OAuthError::InvalidOrExpiredToken => 400,
            OAuthError::EmailUnverified => 403,
            OAuthError::InvalidCredentials => 401,
            OAuthError::EmailConflict => 409,
            OAuthError::DependencyUnavailable => 503,
            OAuthError::Internal => 500,
        }
    }
}

#[derive(Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GoogleExchangeRequest {
    pub code: String,
    pub code_verifier: String,
    pub state: String,
    pub redirect_uri: String,
    /// Required OIDC nonce generated for this authorization request and
    /// validated against the verified ID token claim.
    #[serde(default)]
    pub nonce: Option<String>,
}

/// Redacting `Debug` (Phase 23G): codes, PKCE verifiers, state, and nonce
/// must never reach logs/traces.
impl std::fmt::Debug for GoogleExchangeRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GoogleExchangeRequest")
            .field("code", &crate::scrub::REDACTED)
            .field("code_verifier", &crate::scrub::REDACTED)
            .field("state", &crate::scrub::REDACTED)
            .field("redirect_uri", &self.redirect_uri)
            .field(
                "nonce",
                &self.nonce.as_ref().map(|_| crate::scrub::REDACTED),
            )
            .finish()
    }
}

/// PKCE `code_verifier`: RFC 7636 charset, 43–128 chars.
pub fn is_valid_code_verifier(raw: &str) -> bool {
    (43..=128).contains(&raw.len())
        && raw
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_' | '~'))
}

/// State and nonce values generated from 128 random bits encode to 22
/// unpadded base64url characters. Accept that representation plus longer
/// base64url values, while rejecting whitespace and query delimiters.
pub fn is_valid_state_shape(raw: &str) -> bool {
    (22..=128).contains(&raw.len())
        && raw
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
}

pub fn is_valid_nonce_shape(raw: &str) -> bool {
    is_valid_state_shape(raw)
}

/// Exact loopback callback shape from RFC 8252 §7.3:
/// `http://127.0.0.1:<non-zero-u16-port>/callback`.
pub fn is_loopback_redirect_uri(raw: &str) -> bool {
    let Some(rest) = raw.strip_prefix("http://127.0.0.1:") else {
        return false;
    };
    let Some((port_text, path)) = rest.split_once('/') else {
        return false;
    };
    let Ok(port) = port_text.parse::<u16>() else {
        return false;
    };
    port > 0 && port.to_string() == port_text && path == "callback"
}

fn validate_nonce(request: &GoogleExchangeRequest) -> Result<(), OAuthError> {
    match request.nonce.as_deref() {
        Some(nonce) if is_valid_nonce_shape(nonce) => Ok(()),
        _ => Err(OAuthError::Validation(
            "OIDC nonce is missing or malformed.".to_string(),
        )),
    }
}

/// Validate request field shapes independently of server configuration so
/// malformed input always answers 400/401 before dependency checks (503).
pub fn validate_exchange_shapes_only(request: &GoogleExchangeRequest) -> Result<(), OAuthError> {
    if request.code.trim().is_empty() || request.code.len() > 4_096 {
        return Err(OAuthError::Validation(
            "Authorization code is missing or malformed.".to_string(),
        ));
    }
    if !is_valid_code_verifier(request.code_verifier.trim()) {
        return Err(OAuthError::Validation(
            "PKCE code verifier is malformed.".to_string(),
        ));
    }
    if !is_valid_state_shape(request.state.trim()) {
        return Err(OAuthError::Validation(
            "OAuth state is malformed.".to_string(),
        ));
    }
    if !is_loopback_redirect_uri(&request.redirect_uri) {
        return Err(OAuthError::InvalidOrExpiredToken);
    }
    validate_nonce(request)?;
    Ok(())
}

fn matches_expected_redirect_uri(actual: &str, expected: &str) -> bool {
    if !is_loopback_redirect_uri(actual) {
        return false;
    }
    if actual == expected {
        return true;
    }
    // RFC 8252 §7.3: native desktop apps listen on an ephemeral loopback port
    // (`http://127.0.0.1:{port}/callback`) when the registered URI omits a fixed port
    // (`http://127.0.0.1/callback`). If `expected` specifies a port explicitly, require
    // exact equality above.
    if let Some(expected_path) = expected.strip_prefix("http://127.0.0.1/") {
        if let Some(rest) = actual.strip_prefix("http://127.0.0.1:") {
            if let Some((port_str, actual_path)) = rest.split_once('/') {
                if let Ok(port) = port_str.parse::<u16>() {
                    return port > 0 && actual_path == expected_path;
                }
            }
        }
    }
    false
}

/// Validate request shapes WITHOUT touching the state store or provider
/// (invalid input must not burn state or reach Google).
pub fn validate_exchange_request(
    request: &GoogleExchangeRequest,
    expected_redirect_uri: &str,
) -> Result<(), OAuthError> {
    if request.code.trim().is_empty() || request.code.len() > 4_096 {
        return Err(OAuthError::Validation(
            "Authorization code is missing or malformed.".to_string(),
        ));
    }
    if !is_valid_code_verifier(request.code_verifier.trim()) {
        return Err(OAuthError::Validation(
            "PKCE code verifier is malformed.".to_string(),
        ));
    }
    if !is_valid_state_shape(request.state.trim()) {
        return Err(OAuthError::Validation(
            "OAuth state is malformed.".to_string(),
        ));
    }
    // Plan §2.5 + RFC 8252 §7.3 loopback matching.
    if !matches_expected_redirect_uri(&request.redirect_uri, expected_redirect_uri) {
        return Err(OAuthError::InvalidOrExpiredToken);
    }
    validate_nonce(request)?;
    Ok(())
}

/// Shared single-use state + Google OIDC code exchange & claim verification
/// used by both sign-in (`google_exchange`) and safe account linking.
pub async fn verify_code_with_provider(
    pool: &PgPool,
    google: &dyn GoogleProvider,
    request: &GoogleExchangeRequest,
) -> Result<GoogleIdentity, OAuthError> {
    validate_exchange_shapes_only(request)?;
    consume_state(pool, &request.state).await?;

    let identity = match google
        .exchange_code(
            request.code.trim(),
            request.code_verifier.trim(),
            &request.redirect_uri,
        )
        .await
    {
        Ok(identity) => identity,
        Err(GoogleExchangeError::NotConfigured) => return Err(OAuthError::DependencyUnavailable),
        Err(GoogleExchangeError::Refused) => return Err(OAuthError::InvalidOrExpiredToken),
    };

    let expected_nonce = request
        .nonce
        .as_deref()
        .ok_or(OAuthError::InvalidOrExpiredToken)?;
    if identity.nonce.as_deref() != Some(expected_nonce) {
        return Err(OAuthError::InvalidOrExpiredToken);
    }
    if identity.subject.trim().is_empty() || identity.subject.len() > 255 {
        return Err(OAuthError::InvalidOrExpiredToken);
    }
    if !identity.email_verified {
        return Err(OAuthError::EmailUnverified);
    }
    let _ = validate_email(&identity.email).map_err(OAuthError::Validation)?;
    Ok(identity)
}

/// Mirror the state hash server-side as single-use (plan §2.5). First use
/// records the sha256 digest; any replay within the window is refused. Old
/// rows are pruned after [`STATE_WINDOW_SECS`].
async fn consume_state(pool: &PgPool, raw_state: &str) -> Result<(), OAuthError> {
    let digest = session::hash_session_token(raw_state.trim());
    let mut tx = pool.begin().await.map_err(|_| OAuthError::Internal)?;
    sqlx::query(
        "DELETE FROM google_oauth_states WHERE seen_at < now() - make_interval(secs => $1)",
    )
    .bind(STATE_WINDOW_SECS as f64)
    .execute(&mut *tx)
    .await
    .map_err(|_| OAuthError::Internal)?;
    let inserted = sqlx::query("INSERT INTO google_oauth_states (state_hash) VALUES ($1)")
        .bind(&digest)
        .execute(&mut *tx)
        .await;
    match inserted {
        Ok(_) => {
            tx.commit().await.map_err(|_| OAuthError::Internal)?;
            Ok(())
        }
        Err(error) => {
            // Unique violation = state replay (hard generic failure); anything
            // else is a database failure.
            let replay = error
                .as_database_error()
                .and_then(|db| db.code())
                .is_some_and(|code| code == "23505");
            if replay {
                Err(OAuthError::InvalidOrExpiredToken)
            } else {
                Err(OAuthError::Internal)
            }
        }
    }
}

fn display_name_for(identity: &GoogleIdentity) -> String {
    identity
        .name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty() && name.len() <= 128)
        .map(str::to_string)
        .unwrap_or_else(|| {
            let local = identity.email.split('@').next().unwrap_or("Nexora user");
            if local.is_empty() || local.len() > 128 {
                "Nexora user".to_string()
            } else {
                local.to_string()
            }
        })
}

/// Full exchange flow: validate → state one-time-ness → provider exchange →
/// nonce/claims checks → linking rules → session. Success returns the same
/// outcome shape as password login.
pub async fn google_exchange(
    pool: Option<&PgPool>,
    google: &dyn GoogleProvider,
    expected_redirect_uri: &str,
    request: &GoogleExchangeRequest,
) -> Result<LoginOutcome, OAuthError> {
    validate_exchange_request(request, expected_redirect_uri)?;
    let pool = pool.ok_or(OAuthError::DependencyUnavailable)?;
    let identity = verify_code_with_provider(pool, google, request).await?;
    let email = validate_email(&identity.email).map_err(OAuthError::Validation)?;

    let mut tx = pool.begin().await.map_err(|_| OAuthError::Internal)?;

    // Identity is keyed by immutable `sub`, never by email (plan §2.5).
    let linked: Option<(String,)> = sqlx::query_as(
        "SELECT user_id::text FROM oauth_accounts \
         WHERE provider = 'google' AND provider_subject = $1",
    )
    .bind(identity.subject.trim())
    .fetch_optional(&mut *tx)
    .await
    .map_err(|_| OAuthError::Internal)?;

    let user_id = if let Some((user_id,)) = linked {
        // Existing Google identity: sign in (account lock serializes against
        // password changes — Phase 23E discipline).
        let locked: Option<(String,)> = sqlx::query_as(
            "SELECT id::text FROM users \
             WHERE id = CAST($1 AS uuid) AND status = 'active' FOR UPDATE",
        )
        .bind(&user_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|_| OAuthError::Internal)?;
        let Some((user_id,)) = locked else {
            return Err(OAuthError::InvalidCredentials);
        };
        user_id
    } else {
        // Email may create a brand-new account, but must never select or link
        // an existing Nexora account. Linking is a separate authenticated flow.
        let existing: Option<(String,)> =
            sqlx::query_as("SELECT id::text FROM users WHERE email = $1 FOR UPDATE")
                .bind(&email)
                .fetch_optional(&mut *tx)
                .await
                .map_err(|_| OAuthError::Internal)?;
        if existing.is_some() {
            return Err(OAuthError::EmailConflict);
        }

        let display_name = display_name_for(&identity);
        let created: Option<(String,)> = sqlx::query_as(
            "INSERT INTO users (display_name, email, password_hash, email_verified_at) \
             VALUES ($1, $2, NULL, now()) \
             ON CONFLICT (email) DO NOTHING \
             RETURNING id::text",
        )
        .bind(&display_name)
        .bind(&email)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|_| OAuthError::Internal)?;

        match created {
            Some((user_id,)) => user_id,
            // A concurrent first login can reuse the account only when it is
            // already bound to this exact immutable Google subject. A race on
            // email alone remains a conflict; it is never an auto-link path.
            None => {
                let raced_identity: Option<(String,)> = sqlx::query_as(
                    "SELECT user_id::text FROM oauth_accounts \
                     WHERE provider = 'google' AND provider_subject = $1 FOR UPDATE",
                )
                .bind(identity.subject.trim())
                .fetch_optional(&mut *tx)
                .await
                .map_err(|_| OAuthError::Internal)?;
                let Some((raced_user_id,)) = raced_identity else {
                    return Err(OAuthError::EmailConflict);
                };

                let locked: Option<(String,)> = sqlx::query_as(
                    "SELECT id::text FROM users \
                     WHERE id = CAST($1 AS uuid) AND status = 'active' FOR UPDATE",
                )
                .bind(&raced_user_id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(|_| OAuthError::Internal)?;
                let Some((user_id,)) = locked else {
                    return Err(OAuthError::InvalidCredentials);
                };
                user_id
            }
        }
    };

    // The UNIQUE(provider, provider_subject) constraint is the identity map.
    // Recheck its owner after an insert conflict so a concurrent first sign-in
    // cannot receive a session for an unrelated account created by email.
    let inserted: Option<(String,)> = sqlx::query_as(
        "INSERT INTO oauth_accounts (user_id, provider, provider_subject, provider_email) \
         VALUES (CAST($1 AS uuid), 'google', $2, $3) \
         ON CONFLICT (provider, provider_subject) DO NOTHING \
         RETURNING user_id::text",
    )
    .bind(&user_id)
    .bind(identity.subject.trim())
    .bind(&email)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|_| OAuthError::Internal)?;

    if inserted.is_none() {
        let owner: Option<(String,)> = sqlx::query_as(
            "SELECT user_id::text FROM oauth_accounts \
             WHERE provider = 'google' AND provider_subject = $1 FOR UPDATE",
        )
        .bind(identity.subject.trim())
        .fetch_optional(&mut *tx)
        .await
        .map_err(|_| OAuthError::Internal)?;
        match owner {
            Some((owner_id,)) if owner_id == user_id => {}
            Some(_) => return Err(OAuthError::EmailConflict),
            None => return Err(OAuthError::Internal),
        }
    }

    let session = session::insert_session(
        &mut *tx,
        &user_id,
        session::DEFAULT_SESSION_TTL_SECS,
        None,
        None,
    )
    .await
    .map_err(|_| OAuthError::Internal)?;
    tx.commit().await.map_err(|_| OAuthError::Internal)?;

    // Profile DTO only — no tokens, no provider detail.
    let (name, email_out): (String, String) =
        sqlx::query_as("SELECT display_name, email FROM users WHERE id = CAST($1 AS uuid)")
            .bind(&user_id)
            .fetch_one(pool)
            .await
            .map_err(|_| OAuthError::Internal)?;

    Ok(LoginOutcome {
        user_id,
        name,
        email: email_out,
        session,
    })
}

// Re-export for handlers/tests.
pub use registration::normalize_email;

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> GoogleExchangeRequest {
        GoogleExchangeRequest {
            code: "4/0AeanS0b-secret-code".to_string(),
            code_verifier: "a".repeat(43),
            state: "b".repeat(22),
            redirect_uri: "http://127.0.0.1:49152/callback".to_string(),
            nonce: Some("n".repeat(22)),
        }
    }

    #[test]
    fn oauth_identity_and_claim_debug_redact_personal_data_and_nonce() {
        let identity = GoogleIdentity {
            subject: "SENTINEL_subject".to_string(),
            email: "SENTINEL_email@example.test".to_string(),
            email_verified: true,
            nonce: Some("SENTINEL_nonce".to_string()),
            name: Some("SENTINEL_name".to_string()),
        };
        let claims = IdTokenClaims {
            sub: "SENTINEL_subject".to_string(),
            iss: "https://accounts.google.com".to_string(),
            aud: AudienceClaim::Single("client-id".to_string()),
            exp: 1,
            azp: None,
            email: Some("SENTINEL_email@example.test".to_string()),
            email_verified: Some(true),
            nonce: Some("SENTINEL_nonce".to_string()),
            name: Some("SENTINEL_name".to_string()),
        };
        assert!(!format!("{identity:?}").contains("SENTINEL"));
        assert!(!format!("{claims:?}").contains("SENTINEL"));
    }

    #[test]
    fn code_verifier_follows_rfc7636() {
        assert!(is_valid_code_verifier(&"a".repeat(43)));
        assert!(is_valid_code_verifier(&"aZ09-._~".repeat(16))); // 128
        assert!(!is_valid_code_verifier(&"a".repeat(42)));
        assert!(!is_valid_code_verifier(&"a".repeat(129)));
        assert!(!is_valid_code_verifier(&("a".repeat(42) + "!")));
    }

    #[test]
    fn state_shape_accepts_the_desktop_generator_representation() {
        assert!(is_valid_state_shape(&"a".repeat(22))); // 128 random bits, unpadded base64url
        assert!(is_valid_state_shape(&"0123456789abcdef".repeat(2)));
        assert!(!is_valid_state_shape(&"a".repeat(21)));
        assert!(!is_valid_state_shape("short"));
        assert!(!is_valid_state_shape(&("a".repeat(32) + "+")));
        assert!(is_valid_nonce_shape(&"n".repeat(22)));
        assert!(!is_valid_nonce_shape(&"n".repeat(21)));
    }

    #[test]
    fn loopback_redirect_requires_exact_valid_127_0_0_1_callback() {
        assert!(is_loopback_redirect_uri("http://127.0.0.1:1/callback"));
        assert!(is_loopback_redirect_uri("http://127.0.0.1:49152/callback"));
        assert!(is_loopback_redirect_uri("http://127.0.0.1:65535/callback"));
        assert!(!is_loopback_redirect_uri("http://127.0.0.1:0/callback"));
        assert!(!is_loopback_redirect_uri("http://127.0.0.1:65536/callback"));
        assert!(!is_loopback_redirect_uri("http://127.0.0.1:049152/callback"));
        assert!(!is_loopback_redirect_uri("http://127.0.0.1:49152/callback/"));
        assert!(!is_loopback_redirect_uri("http://127.0.0.1:49152/callback?code=x"));
        assert!(!is_loopback_redirect_uri("http://localhost:1/callback"));
        assert!(!is_loopback_redirect_uri("https://127.0.0.1:1/callback"));
        assert!(!is_loopback_redirect_uri("http://10.0.0.1:1/callback"));
    }

    #[test]
    fn redirect_uri_mismatch_is_a_hard_generic_failure() {
        let mut r = request();
        r.redirect_uri = "http://127.0.0.1:49152/callback".to_string();
        assert!(validate_exchange_request(&r, "http://127.0.0.1:49152/callback").is_ok());
        let error = validate_exchange_request(&r, "http://127.0.0.1:5555/callback").unwrap_err();
        assert_eq!(error.code(), "invalid_or_expired_token");
    }

    #[test]
    fn malformed_requests_validate_before_any_state_or_provider_work() {
        let mut r = request();
        r.nonce = None;
        assert_eq!(
            validate_exchange_request(&r, &r.redirect_uri)
                .unwrap_err()
                .code(),
            "validation_failed"
        );
        let mut r = request();
        r.nonce = Some("bad nonce".to_string());
        assert_eq!(
            validate_exchange_shapes_only(&r).unwrap_err().code(),
            "validation_failed"
        );
        let mut r = request();
        r.code = " ".to_string();
        assert_eq!(
            validate_exchange_request(&r, &r.redirect_uri)
                .unwrap_err()
                .code(),
            "validation_failed"
        );
        let mut r = request();
        r.code_verifier = "x".repeat(42);
        assert_eq!(
            validate_exchange_request(&r, &r.redirect_uri)
                .unwrap_err()
                .code(),
            "validation_failed"
        );
        let mut r = request();
        r.state = "zzz".to_string();
        assert_eq!(
            validate_exchange_request(&r, &r.redirect_uri)
                .unwrap_err()
                .code(),
            "validation_failed"
        );
    }

    #[test]
    fn error_codes_and_messages_leak_nothing() {
        assert_eq!(OAuthError::EmailConflict.status_code(), 409);
        assert_eq!(OAuthError::EmailConflict.code(), "email_conflict");
        assert_eq!(OAuthError::EmailUnverified.status_code(), 403);
        assert_eq!(OAuthError::InvalidOrExpiredToken.status_code(), 400);
        for error in [
            OAuthError::InvalidOrExpiredToken,
            OAuthError::EmailUnverified,
            OAuthError::EmailConflict,
            OAuthError::DependencyUnavailable,
        ] {
            let message = error.message();
            assert!(!message.contains('@'));
            assert!(!message.contains("token="));
            assert!(!message.to_lowercase().contains("secret"));
        }
    }

    // ---- OIDC ID-token signature verification (Phase 23G step 6) ----------
    //
    // TEST-ONLY RSA keypair: generated in-sandbox purely to SIGN synthetic ID
    // tokens so the real RS256 verification path is exercised end to end. It
    // is not a credential and has no access to any system (synthetic fixture,
    // same standing as the `example.test` accounts).

    const TEST_JWK_N: &str = "oD051OXkPqHe3zavB9GOFMaxfs1RQ84dZBWCCTlh7OMbJU5RBg-qkkxd9iyzo2mm32t03UDJH9J-p4Sih2p0aR7m7OVT6R_PS3PPU5r48I9WEDatlzv5vzRyzqQDIz3I5Eavokl3aWrPI0NYDyJvwHmEdKXKrr4ZF0Bns9Cw9WiwrbrkmeO6wGvUw1Lo6MDYNScFUSnKdQgLL4-NeY6W2Z_1T8-D5tdu5xp28G4z8_Vn7zOhmSz_GIlsXLdWdFPY1U_UkvChB9nmBOdKf-QFAcIxgadw69oJtDH1tTvlAmtzSZHaimTIRJ39SkjZLiDPBLHlzuhQOulG5Gm5C-r95Q";
    const TEST_JWK_E: &str = "AQAB";
    const TEST_RSA_PRIVATE_PEM: &str = "-----BEGIN RSA PRIVATE KEY-----\nMIIEowIBAAKCAQEAoD051OXkPqHe3zavB9GOFMaxfs1RQ84dZBWCCTlh7OMbJU5R\nBg+qkkxd9iyzo2mm32t03UDJH9J+p4Sih2p0aR7m7OVT6R/PS3PPU5r48I9WEDat\nlzv5vzRyzqQDIz3I5Eavokl3aWrPI0NYDyJvwHmEdKXKrr4ZF0Bns9Cw9Wiwrbrk\nmeO6wGvUw1Lo6MDYNScFUSnKdQgLL4+NeY6W2Z/1T8+D5tdu5xp28G4z8/Vn7zOh\nmSz/GIlsXLdWdFPY1U/UkvChB9nmBOdKf+QFAcIxgadw69oJtDH1tTvlAmtzSZHa\nimTIRJ39SkjZLiDPBLHlzuhQOulG5Gm5C+r95QIDAQABAoIBAAyzvcb08FDP53xq\nd8zjw8kKCFDxn2/QdQwvF+J5gRwHoFCNP3P+piCE9QY9YrVHlaa5STrIVqK/2DIy\ntYJZiHlt+mB5PChLN4hjv+G0DU7FQs1abQyDgLEzZaegYO7lIdMiqh9WhosKbSzk\n7j3I44xU6N1cTrXnNAYsh1JXbs4NPbR1P70Lp3PntbVFeyv+1P9HhDwer5XHL4fU\nHmamaLUULqgimPPdCkBeAkFTjjEJO5ZHlGveOdNQP7AlUCVzVNs3EIEWuGFVG+Y2\noXPnd8Cgw9y/flZBa7byAL3gW7+3Xmsjhgzaobo+42haTkpeSyN2bej2AhvMgC9o\nWB36ANMCgYEA3ulrW/8HenGvwtFiP30usq8KsmI7WQAZ8Dnaj1xN0jL3fCdtF8oH\nB8AH7cs1/gcfI/emdC/rGb9HX8L21By+ALAz+czJPhNUvD+b7AJFPk22PKPiU8hV\n+f9Jon+uVbXsYrEKP45laCG5rF9zjQj9GCcNxWJpVQ7A6R1ZTeMKTw8CgYEAuAZD\n+7OzjF5yMWydwDKAtS5fNQt/E5Na9At9ciddO5eM20hXzb4S90Iuh2yDaCJUw4zM\nibhEIiZdo7MV2QmNEVVy30uJQzA/5cbidGXLLpg6loeEcDAoVVPBs8OTJYRFuB+H\nB4dCwM82pVO/pdh8E+7Q2NliCYbOvJ77CTcc48sCgYAsFZuu4Pv4oj0xWTmF9tri\naa3C45x7GIMIuFhaG9YkKQI64o03tDO5u6eVxqeR6pSlB1g7CDMFr4eND8DC6EBH\nYJGW6IIwLaCHbkkG3tgU30eYLI0ZJriiLXPBmGDBAK6UQhEVyrKRaoIO7f2rOcjc\nGvUlQTY1pSpPsAc2WR1zzwKBgQC2PRYz/CjR2HITufGa9Sh6xHpqaIEm3KWUypAn\nbQ4qRpGqUsEJMbNMg5Dacb7McDsrEJM5y25Ft2LCC0aWkZvOz6Ubto0vKWgN3+BA\nztHgycYzbe7cvwQiQ/iuUf61h4EXrrkk3nFfyZTfMsm2rmVpw/W02cn5jzPKnVIT\nNS358wKBgDfi1CTkpIDBb9PzxUDmE8pmYs7+03GnUIVmkjvNurE/2DCyOCSaJpcG\nvgPPdggvM8sudgGz5E6ESgx1BlWwCEkWJMscZU4USN9zrR9paRZXYkCX5hDypSyY\nReeMm5yNT5NBKES9Njx2kW0jZtfrTV0HWZBX1wEwzKb2CbUSRMig\n-----END RSA PRIVATE KEY-----";

    fn test_jwks(kid: &str) -> JwksMaterial {
        JwksMaterial {
            keys: vec![JwkKey {
                kid: kid.to_string(),
                n: TEST_JWK_N.to_string(),
                e: TEST_JWK_E.to_string(),
            }],
        }
    }

    fn sign_test_token(
        kid: Option<&str>,
        client_id: &str,
        mutate: impl FnOnce(&mut serde_json::Value),
    ) -> String {
        use jsonwebtoken::{encode, EncodingKey, Header};
        let mut header = Header::new(jsonwebtoken::Algorithm::RS256);
        header.kid = kid.map(str::to_string);
        let exp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + 600;
        let mut claims = serde_json::json!({
            "sub": "google-sub-1",
            "email": "ada@example.test",
            "email_verified": true,
            "nonce": "n-abc",
            "name": "Ada",
            "iss": "https://accounts.google.com",
            "aud": client_id,
            "exp": exp,
        });
        mutate(&mut claims);
        encode(
            &header,
            &claims,
            &EncodingKey::from_rsa_pem(TEST_RSA_PRIVATE_PEM.as_bytes()).unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn valid_signed_id_token_verifies_and_yields_identity() {
        let token = sign_test_token(Some("test-kid"), "client-123", |_| {});
        let identity = verify_id_token_with(&token, "client-123", &test_jwks("test-kid")).unwrap();
        assert_eq!(identity.subject, "google-sub-1");
        assert_eq!(identity.email, "ada@example.test");
        assert!(identity.email_verified);
        assert_eq!(identity.nonce.as_deref(), Some("n-abc"));
    }

    #[test]
    fn id_token_signature_and_claim_checks_reject_forgeries() {
        let good = sign_test_token(Some("test-kid"), "client-123", |_| {});

        // Tampered payload (signature no longer matches).
        let mut segments: Vec<String> = good.split('.').map(str::to_string).collect();
        let flipped = if segments[1].ends_with('A') { 'B' } else { 'A' };
        segments[1].pop();
        segments[1].push(flipped);
        let tampered = segments.join(".");
        assert!(verify_id_token_with(&tampered, "client-123", &test_jwks("test-kid")).is_err());

        // Wrong audience (token not for this client).
        let wrong_aud = sign_test_token(Some("test-kid"), "someone-else", |_| {});
        assert!(verify_id_token_with(&wrong_aud, "client-123", &test_jwks("test-kid")).is_err());

        // Multiple audiences require an authorized-party claim naming this client.
        let multiple_audiences_without_azp = sign_test_token(Some("test-kid"), "client-123", |c| {
            c["aud"] = serde_json::json!(["client-123", "other-client"]);
        });
        assert!(verify_id_token_with(
            &multiple_audiences_without_azp,
            "client-123",
            &test_jwks("test-kid")
        )
        .is_err());
        let multiple_audiences_with_azp = sign_test_token(Some("test-kid"), "client-123", |c| {
            c["aud"] = serde_json::json!(["client-123", "other-client"]);
            c["azp"] = serde_json::json!("client-123");
        });
        assert!(verify_id_token_with(
            &multiple_audiences_with_azp,
            "client-123",
            &test_jwks("test-kid")
        )
        .is_ok());
        let wrong_azp = sign_test_token(Some("test-kid"), "client-123", |c| {
            c["azp"] = serde_json::json!("someone-else");
        });
        assert!(verify_id_token_with(&wrong_azp, "client-123", &test_jwks("test-kid")).is_err());

        // Wrong issuer.
        let wrong_iss = sign_test_token(Some("test-kid"), "client-123", |c| {
            c["iss"] = serde_json::json!("https://evil.example.test");
        });
        assert!(verify_id_token_with(&wrong_iss, "client-123", &test_jwks("test-kid")).is_err());

        // Expired.
        let expired = sign_test_token(Some("test-kid"), "client-123", |c| {
            c["exp"] = serde_json::json!(1);
        });
        assert!(verify_id_token_with(&expired, "client-123", &test_jwks("test-kid")).is_err());
        let missing_exp = sign_test_token(Some("test-kid"), "client-123", |c| {
            c.as_object_mut().unwrap().remove("exp");
        });
        assert!(verify_id_token_with(&missing_exp, "client-123", &test_jwks("test-kid")).is_err());

        // Unknown kid (not in the JWKS material).
        assert!(verify_id_token_with(&good, "client-123", &test_jwks("other-kid")).is_err());

        // Missing kid in header.
        let no_kid = sign_test_token(None, "client-123", |_| {});
        assert!(verify_id_token_with(&no_kid, "client-123", &test_jwks("test-kid")).is_err());

        // Alg confusion: an "alg":"none" token is refused outright.
        let unsigned = "eyJhbGciOiJub25lIn0.eyJzdWIiOiJoYWNrZXIifQ.";
        assert!(verify_id_token_with(unsigned, "client-123", &test_jwks("test-kid")).is_err());

        // Malformed inputs never panic.
        for junk in ["", "a", "a.b", "a.b.c", "!!!.???.$$$"] {
            assert!(verify_id_token_with(junk, "client-123", &test_jwks("test-kid")).is_err());
        }
    }

    #[test]
    fn display_name_falls_back_to_the_email_local_part() {
        let identity = GoogleIdentity {
            subject: "s".to_string(),
            email: "ada@example.test".to_string(),
            email_verified: true,
            nonce: None,
            name: Some("  ".to_string()),
        };
        assert_eq!(display_name_for(&identity), "ada");
        let named = GoogleIdentity {
            name: Some("Ada Lovelace".to_string()),
            ..identity
        };
        assert_eq!(display_name_for(&named), "Ada Lovelace");
    }
}

//! nexora-auth — standalone authentication service.
//!
//! Phase 23A foundation (health-only scaffold) + Phase 23B (Argon2id password
//! hashing, users/oauth schema, DB configuration) + Phase 23C (registration,
//! email verification, resend) + Phase 23D (login, sessions, logout, refresh
//! with family reuse detection, DB-backed rate limiting). Password recovery,
//! Google OAuth, and desktop IPC wiring are later plan steps and are not
//! implemented. See `docs/phase-23a/README.md` … `docs/phase-23d/README.md`.

pub mod account;
pub mod app;
pub mod audit;
pub mod billing;
pub mod config;
pub mod email;
pub mod login;
pub mod oauth;
pub mod password;
pub mod quota;
pub mod rate_limit;
pub mod recovery;
pub mod registration;
pub mod scrub;
pub mod session;

/// Apply all SQL migrations in `migrations/` idempotently against `pool`.
pub async fn run_migrations(pool: &sqlx::PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    let dir = [
        std::path::Path::new("./migrations"),
        std::path::Path::new("services/nexora-auth/migrations"),
        std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/migrations")),
    ]
    .into_iter()
    .find(|candidate| candidate.is_dir())
    .unwrap_or_else(|| std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/migrations")));
    let migrator = sqlx::migrate::Migrator::new(dir).await?;
    migrator.run(pool).await
}

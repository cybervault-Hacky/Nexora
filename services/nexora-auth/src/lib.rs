//! nexora-auth — standalone authentication service.
//!
//! Phase 23A foundation (health-only scaffold) + Phase 23B (Argon2id password
//! hashing, users/oauth schema, DB configuration) + Phase 23C (registration,
//! email verification, resend) + Phase 23D (login, sessions, logout, refresh
//! with family reuse detection, DB-backed rate limiting). Password recovery,
//! Google OAuth, and desktop IPC wiring are later plan steps and are not
//! implemented. See `docs/phase-23a/README.md` … `docs/phase-23d/README.md`.

pub mod app;
pub mod config;
pub mod email;
pub mod login;
pub mod oauth;
pub mod password;
pub mod rate_limit;
pub mod recovery;
pub mod scrub;
pub mod registration;
pub mod session;

-- Phase 13 — enforce the digest width already required by all auth token writers.
--
-- Session, verification, reset, and Google OAuth state hashes are SHA-256
-- bytea values (32 bytes). This migration does not rewrite or delete rows. It
-- validates existing values as constraints are added; if any digest is not 32
-- bytes, the migration must fail and leave the database unchanged so the
-- operator can inspect the affected rows before retrying.
--
-- Forward-only: never edit an already-applied migration to add these checks.

ALTER TABLE email_verification_tokens
    ADD CONSTRAINT email_verification_tokens_token_hash_len_ck
    CHECK (octet_length(token_hash) = 32);

ALTER TABLE password_reset_tokens
    ADD CONSTRAINT password_reset_tokens_token_hash_len_ck
    CHECK (octet_length(token_hash) = 32);

ALTER TABLE sessions
    ADD CONSTRAINT sessions_token_hash_len_ck
    CHECK (octet_length(token_hash) = 32);

ALTER TABLE google_oauth_states
    ADD CONSTRAINT google_oauth_states_state_hash_len_ck
    CHECK (octet_length(state_hash) = 32);

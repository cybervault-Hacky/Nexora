-- Phase 23F — Google OAuth state replay store (plan §2.5 state validation).
-- Forward-only. Does not modify any previous migration.
--
-- The plan holds the OAuth `state` (CSRF) in the desktop process and mirrors
-- it **hashed** server-side as "single-use, 10-min TTL". The exchange contract
-- (plan §6) carries the state only at exchange time, so the server enforces
-- one-time-ness here: a state hash is recorded on first use and any replay is
-- refused. Rows are pruned after 10 minutes (a replay past that window would
-- already fail at Google: authorization codes are single-use and short-lived).
-- Raw states are NEVER stored — sha256 digests only, like every other token
-- in this schema.
CREATE TABLE IF NOT EXISTS google_oauth_states (
    state_hash  bytea PRIMARY KEY,
    seen_at     timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS google_oauth_states_seen_at_idx
    ON google_oauth_states (seen_at);

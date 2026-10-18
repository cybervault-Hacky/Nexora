-- Phase 23D — sessions + login_attempts (plan §5 entity outlines).
--
-- sessions: opaque 32-byte tokens, stored ONLY as sha256(raw). Family_id
-- groups a rotation chain: refresh rotates the token within the family and a
-- request presenting an already-revoked token is treated as reuse → the whole
-- family is revoked (plan §4: rotation + family reuse detection). Absolute
-- expiry (expires_at) and idle expiry (last_used_at) are both enforced on
-- every authenticated request; revoked_at is checked every time (plan §4:
-- "revocation gap" control).
--
-- login_attempts: the plan's v1 rate-limiting/abuse store ("can move to Redis
-- without contract changes"). Rows record auth-endpoint outcomes keyed by
-- hashed identifiers (never raw emails); sliding-window counts across all
-- backend instances because the store is shared PostgreSQL. Window-based only
-- — no permanent account lockout (which would hand attackers a DoS).
--
-- Applied with sqlx in order after 20261018000003. Forward-recovery reversal:
-- ship a NEW migration containing the DROP statements below. Never edit this
-- applied file in place. Never run against production from a dev machine.
--
--   DROP TABLE IF EXISTS login_attempts;
--   DROP TABLE IF EXISTS sessions;

CREATE TABLE sessions (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    -- sha256(raw token) — 32 bytes. UNIQUE gives the authenticate-by-hash lookup.
    token_hash bytea NOT NULL UNIQUE,
    -- Rotation/reuse grouping. A fresh uuid per login; refresh keeps it.
    family_id uuid NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    -- Absolute lifetime end.
    expires_at timestamptz NOT NULL,
    -- Idle-expiry anchor; updated (throttled) on authenticated requests.
    last_used_at timestamptz NOT NULL DEFAULT now(),
    -- Logout, rotation, or family revocation.
    revoked_at timestamptz NULL,
    app_version text NULL,
    platform text NULL,
    CONSTRAINT sessions_expiry_ck CHECK (expires_at > created_at)
);

CREATE INDEX sessions_user_id_idx ON sessions (user_id);
CREATE INDEX sessions_family_id_idx ON sessions (family_id);

CREATE TABLE login_attempts (
    id bigserial PRIMARY KEY,
    -- 'login' | 'register' | 'resend' | 'recovery' | 'google' (latter two
    -- reserved for the plan's later steps).
    scope text NOT NULL,
    -- Hashed key (email-hash or ip-hash) — never a raw email or credential.
    key text NOT NULL,
    outcome text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT login_attempts_scope_ck
        CHECK (scope IN ('login', 'register', 'resend', 'recovery', 'google'))
);

CREATE INDEX login_attempts_scope_key_created_idx
    ON login_attempts (scope, key, created_at);

-- OPAQUE sign-in (docs/superpowers/specs/2026-10-13-pake-login-design.md).

-- The account's OPAQUE registration record. NULL until the account enrols.
-- auth_verifier (Argon2id of the legacy auth key) stays during the
-- migration window and is NULL for accounts created with OPAQUE.
ALTER TABLE accounts ADD COLUMN opaque_record BYTEA;

-- Set by `admin allow-legacy-sign-in`: until then this one account may use
-- the legacy login once more, after the cutoff, to enrol. Cleared on enrol.
ALTER TABLE accounts ADD COLUMN legacy_grace_until TIMESTAMPTZ;

-- The server's long-term OPAQUE secret, one row. Losing it locks every
-- enrolled account out, so it lives in the database and its backups.
CREATE TABLE pake_server_setup (
  id     SMALLINT PRIMARY KEY CHECK (id = 1),
  setup  BYTEA NOT NULL
);

-- Half-finished exchanges: 60 seconds, deleted on first read.
CREATE TABLE pake_pending (
  id_hash     BYTEA PRIMARY KEY,
  purpose     TEXT NOT NULL CHECK (purpose IN ('login', 'reauth')),
  account_id  UUID REFERENCES accounts(id) ON DELETE CASCADE,
  state       BYTEA NOT NULL,
  expires_at  TIMESTAMPTZ NOT NULL
);
CREATE INDEX pake_pending_by_expiry ON pake_pending (expires_at);

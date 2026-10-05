-- Session tokens of deleted accounts, so the account's other devices learn
-- of the deletion (docs/superpowers/specs/2026-10-05-account-deletion-design.md §4).
-- Nothing here identifies a person: no account id, no email, no device id.
CREATE TABLE deleted_sessions (
  token_hash BYTEA PRIMARY KEY CHECK (octet_length(token_hash) = 32),
  expires_at TIMESTAMPTZ NOT NULL
);
CREATE INDEX deleted_sessions_by_expiry ON deleted_sessions (expires_at);

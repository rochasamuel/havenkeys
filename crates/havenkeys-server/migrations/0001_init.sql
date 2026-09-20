-- The whole server schema (design 2026-09-20 §7.1).
--
-- The server holds ciphertext it cannot open: `vaults.header` and the two
-- `items` blobs are opaque bytes. Everything else is the bookkeeping that
-- makes the server the single writer: a per-vault `revision` that is the sync
-- cursor, and a per-item `revision` that is the optimistic concurrency token.

CREATE TABLE accounts (
  id                UUID PRIMARY KEY,
  email_normalized  TEXT NOT NULL UNIQUE,          -- NFC + lowercase
  status            TEXT NOT NULL CHECK (status IN ('invited', 'active', 'disabled')),
  kdf_algorithm     TEXT,                          -- set at activation
  kdf_memory_kib    INTEGER,
  kdf_iterations    INTEGER,
  kdf_parallelism   INTEGER,
  kdf_salt          BYTEA,                         -- 16 bytes, client-chosen
  auth_verifier     TEXT,                          -- PHC string (Argon2id of the auth key)
  invite_hash       BYTEA,                         -- SHA-256, cleared on activation
  invite_expires_at TIMESTAMPTZ,
  created_at        TIMESTAMPTZ NOT NULL,
  activated_at      TIMESTAMPTZ
);

CREATE TABLE vaults (
  id              UUID PRIMARY KEY,
  account_id      UUID NOT NULL UNIQUE REFERENCES accounts(id) ON DELETE CASCADE,
  header          BYTEA NOT NULL,                  -- header.json + attestation, never parsed
  header_revision BIGINT NOT NULL,
  key_scheme      SMALLINT NOT NULL,
  revision        BIGINT NOT NULL DEFAULT 0,       -- the sync cursor
  created_at      TIMESTAMPTZ NOT NULL
);

-- A deleted item keeps its row with both blobs NULL: that row is the
-- tombstone, and it is what lets a device that was away learn about the
-- deletion. Clients keep no tombstones of their own.
CREATE TABLE items (
  vault_id   UUID NOT NULL REFERENCES vaults(id) ON DELETE CASCADE,
  item_id    UUID NOT NULL,
  overview   BYTEA,
  details    BYTEA,
  deleted_at TIMESTAMPTZ,
  revision   BIGINT NOT NULL,
  PRIMARY KEY (vault_id, item_id)
);
CREATE INDEX items_by_revision ON items (vault_id, revision, item_id);

CREATE TABLE devices (
  id           UUID PRIMARY KEY,
  account_id   UUID NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
  name         TEXT NOT NULL,
  created_at   TIMESTAMPTZ NOT NULL,
  last_seen_at TIMESTAMPTZ,
  revoked_at   TIMESTAMPTZ
);
CREATE INDEX devices_by_account ON devices (account_id);

CREATE TABLE sessions (
  token_hash BYTEA PRIMARY KEY,                    -- SHA-256 of the bearer token
  account_id UUID NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
  device_id  UUID NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
  created_at TIMESTAMPTZ NOT NULL,
  expires_at TIMESTAMPTZ NOT NULL
);
CREATE INDEX sessions_by_device ON sessions (device_id);

CREATE TABLE login_attempts (
  key           TEXT PRIMARY KEY,                  -- "acct:<uuid>" or "ip:<addr>"
  failures      INTEGER NOT NULL,
  window_start  TIMESTAMPTZ NOT NULL,
  blocked_until TIMESTAMPTZ
);

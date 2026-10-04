-- Signing in a new device from an approving one
-- (docs/superpowers/specs/2026-10-03-phone-approved-sign-in-design.md §5).
CREATE TABLE pairings (
  id               TEXT PRIMARY KEY,
  state            TEXT NOT NULL CHECK (state IN ('pending', 'approved', 'denied', 'claimed')),
  device_id        UUID NOT NULL,
  device_name      TEXT NOT NULL,
  claim_hash       BYTEA NOT NULL CHECK (octet_length(claim_hash) = 32),
  ip               TEXT NOT NULL,
  location         TEXT,
  created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
  expires_at       TIMESTAMPTZ NOT NULL,
  account_id       UUID REFERENCES accounts(id) ON DELETE CASCADE,
  envelope         BYTEA
);
CREATE INDEX pairings_by_ip ON pairings (ip);
CREATE INDEX pairings_by_created ON pairings (created_at);

ALTER TABLE devices ADD COLUMN approved_by UUID;

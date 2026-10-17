-- Encrypted file attachments (design 2026-10-13-encrypted-attachments §4.3).
--
-- `blob` is a sealed attachment the server cannot open. A row is uploaded with
-- `committed = false` and counts towards the vault's quotas; an item write
-- that lists it makes it readable.

CREATE TABLE attachments (
  vault_id      UUID NOT NULL REFERENCES vaults(id) ON DELETE CASCADE,
  attachment_id UUID NOT NULL,
  item_id       UUID NOT NULL,
  blob          BYTEA NOT NULL,
  committed     BOOLEAN NOT NULL DEFAULT false,
  uploaded_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (vault_id, attachment_id)
);
CREATE INDEX attachments_by_item ON attachments (vault_id, item_id);

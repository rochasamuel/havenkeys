-- Plans, trials and self-service signup
-- (docs/superpowers/specs/2026-10-07-self-signup-and-plans-design.md §4.3, §5.1, §5.2).

-- One row per account. The stored status is what an operator or a payment
-- provider last set; whether the account may write is computed from the row
-- and the clock (billing::entitlement), never flipped by a scheduled job.
CREATE TABLE subscriptions (
  account_id         UUID PRIMARY KEY REFERENCES accounts(id) ON DELETE CASCADE,
  plan               TEXT NOT NULL,
  status             TEXT NOT NULL CHECK (status IN
                       ('trialing', 'active', 'past_due', 'frozen', 'complimentary')),
  trial_ends_at      TIMESTAMPTZ,                  -- set at activation: now() + 14 days
  current_period_end TIMESTAMPTZ,                  -- paid until
  grace_ends_at      TIMESTAMPTZ,                  -- past_due: 7 days of grace
  provider           TEXT,                         -- NULL | 'stripe' | 'mercadopago'
  provider_customer  TEXT,
  provider_ref       TEXT,                         -- the gateway's subscription id
  updated_at         TIMESTAMPTZ NOT NULL
);

-- Append-only audit trail of status changes and notices. No card data ever
-- reaches the server. Erased with the account (ON DELETE CASCADE).
CREATE TABLE billing_events (
  id          BIGSERIAL PRIMARY KEY,
  account_id  UUID NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
  old_status  TEXT,
  new_status  TEXT NOT NULL,
  actor       TEXT NOT NULL CHECK (actor IN ('admin', 'system', 'provider')),
  reason      TEXT NOT NULL,
  created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX billing_events_by_account ON billing_events (account_id, created_at);

-- Every account that exists today keeps what it has: a complimentary plan
-- with no end date (§5.5).
INSERT INTO subscriptions (account_id, plan, status, updated_at)
SELECT id, 'personal', 'complimentary', now() FROM accounts;

-- Self-service signup (§4.3). The code is stored keyed by the server secret:
-- six digits alone would fall to an offline brute force of a dumped table.
-- The locale and terms version travel with the code so `verify`, which only
-- carries email and code, can record them on the account.
CREATE TABLE signup_codes (
  email_normalized TEXT PRIMARY KEY,
  code_hash        BYTEA NOT NULL CHECK (octet_length(code_hash) = 32),
  locale           TEXT NOT NULL,
  terms_version    TEXT NOT NULL,
  expires_at       TIMESTAMPTZ NOT NULL,
  attempts         INTEGER NOT NULL DEFAULT 0,
  created_at       TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX signup_codes_by_expiry ON signup_codes (expires_at);

-- Who created the account (the abandoned-signup sweep only touches
-- 'signup'), the LGPD consent record, and the language for account notices.
ALTER TABLE accounts
  ADD COLUMN created_by        TEXT NOT NULL DEFAULT 'admin'
                               CHECK (created_by IN ('admin', 'signup')),
  ADD COLUMN terms_version     TEXT,
  ADD COLUMN terms_accepted_at TIMESTAMPTZ,
  ADD COLUMN locale            TEXT;

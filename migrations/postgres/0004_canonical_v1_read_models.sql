-- Canonical V1 read models.
--
-- Identity mapping (do not duplicate wasi-auth tables):
--   sketch accounts          → auth_users
--   sketch refresh_tokens    → auth_refresh_tokens
--   sketch devices            → buwiz_server.oauth_device_codes + auth_sessions
-- tax_profiles.account_id    → auth_users.user_id
--
-- UNIQUE (tin_hash) is V1 global exclusivity. Drop that unique later when
-- tax_profile_members exists; keep UNIQUE (account_id, tin_hash).
-- Never store PIN, TOTP, IMAP passwords, or mailbox OAuth tokens.

-- Drop 0003 stub tables that used different keys (safe to re-run).
DROP TABLE IF EXISTS buwiz_server.filing_job_events;
DROP TABLE IF EXISTS buwiz_server.filing_jobs;
DROP TABLE IF EXISTS buwiz_server.per_year_forms_sets;

-- Rewrite form_drafts only if the stub draft_id PK is still present.
DO $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM information_schema.columns
        WHERE table_schema = 'buwiz_server'
          AND table_name = 'form_drafts'
          AND column_name = 'draft_id'
    ) THEN
        DROP TABLE buwiz_server.form_drafts CASCADE;
    END IF;
END $$;

ALTER TABLE buwiz_server.tax_profiles
    ADD COLUMN IF NOT EXISTS account_id UUID REFERENCES auth_users (user_id) ON DELETE RESTRICT,
    ADD COLUMN IF NOT EXISTS tin_hash BYTEA,
    ADD COLUMN IF NOT EXISTS full_name TEXT,
    ADD COLUMN IF NOT EXISTS eopt_tier TEXT,
    ADD COLUMN IF NOT EXISTS business_start_date TEXT,
    ADD COLUMN IF NOT EXISTS birth_date TEXT,
    ADD COLUMN IF NOT EXISTS atc_codes JSONB NOT NULL DEFAULT '[]'::jsonb,
    ADD COLUMN IF NOT EXISTS is_archived BOOLEAN NOT NULL DEFAULT FALSE;

UPDATE buwiz_server.tax_profiles
SET account_id = COALESCE(account_id, holder_user_id, owner_user_id, verified_owner_user_id)
WHERE account_id IS NULL;

UPDATE buwiz_server.tax_profiles
SET full_name = COALESCE(full_name, registered_name, display_name)
WHERE full_name IS NULL;

UPDATE buwiz_server.tax_profiles
SET branch_code = COALESCE(NULLIF(TRIM(branch_code), ''), '00000');

ALTER TABLE buwiz_server.tax_profiles
    ALTER COLUMN branch_code SET DEFAULT '00000';

ALTER TABLE buwiz_server.tax_profiles
    ALTER COLUMN branch_code SET NOT NULL;

DO $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM information_schema.columns
        WHERE table_schema = 'buwiz_server'
          AND table_name = 'tax_profiles'
          AND column_name = 'tin_identity_hash'
    ) THEN
        UPDATE buwiz_server.tax_profiles
        SET tin_hash = decode(tin_identity_hash, 'hex')
        WHERE tin_hash IS NULL
          AND tin_identity_hash IS NOT NULL
          AND length(tin_identity_hash) = 64
          AND tin_identity_hash ~ '^[0-9a-fA-F]+$';
    END IF;
END $$;

CREATE UNIQUE INDEX IF NOT EXISTS tax_profiles_tin_hash_uidx
    ON buwiz_server.tax_profiles (tin_hash)
    WHERE tin_hash IS NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS tax_profiles_account_tin_hash_uidx
    ON buwiz_server.tax_profiles (account_id, tin_hash)
    WHERE account_id IS NOT NULL AND tin_hash IS NOT NULL;

DROP INDEX IF EXISTS buwiz_server.tax_profiles_tin_identity_hash_uidx;

CREATE INDEX IF NOT EXISTS tax_profiles_account_idx
    ON buwiz_server.tax_profiles (account_id)
    WHERE account_id IS NOT NULL;

CREATE TABLE IF NOT EXISTS buwiz_server.profile_years (
    id UUID PRIMARY KEY,
    tax_profile_id UUID NOT NULL REFERENCES buwiz_server.tax_profiles (profile_id) ON DELETE CASCADE,
    tax_year SMALLINT NOT NULL,
    registered_name TEXT,
    rdo_code TEXT,
    line_of_business TEXT,
    registered_address TEXT,
    zip_code TEXT,
    phone TEXT,
    email TEXT,
    tax_classification TEXT,
    is_vat_registered BOOLEAN,
    eopt_tier TEXT,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tax_profile_id, tax_year)
);

CREATE TABLE IF NOT EXISTS buwiz_server.per_year_forms (
    id UUID PRIMARY KEY,
    profile_year_id UUID NOT NULL REFERENCES buwiz_server.profile_years (id) ON DELETE CASCADE,
    form_code TEXT NOT NULL,
    frequency TEXT NOT NULL,
    active BOOLEAN NOT NULL DEFAULT TRUE,
    UNIQUE (profile_year_id, form_code)
);

CREATE TABLE IF NOT EXISTS buwiz_server.form_drafts (
    id UUID PRIMARY KEY,
    tax_profile_id UUID NOT NULL REFERENCES buwiz_server.tax_profiles (profile_id) ON DELETE CASCADE,
    form_code TEXT NOT NULL,
    tax_year SMALLINT NOT NULL,
    period_key TEXT NOT NULL,
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    rev BIGINT NOT NULL DEFAULT 1,
    client_rev BIGINT,
    saved_once BOOLEAN NOT NULL DEFAULT FALSE,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tax_profile_id, form_code, tax_year, period_key)
);

CREATE INDEX IF NOT EXISTS form_drafts_year_idx
    ON buwiz_server.form_drafts (tax_profile_id, tax_year);

CREATE TABLE IF NOT EXISTS buwiz_server.filings (
    id UUID PRIMARY KEY,
    tax_profile_id UUID NOT NULL REFERENCES buwiz_server.tax_profiles (profile_id) ON DELETE CASCADE,
    form_draft_id UUID REFERENCES buwiz_server.form_drafts (id) ON DELETE SET NULL,
    form_code TEXT NOT NULL,
    tax_year SMALLINT NOT NULL,
    period_key TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'queued',
    xml_sha256 TEXT,
    submitted_at TIMESTAMPTZ,
    receipt_ref TEXT,
    error_summary TEXT,
    paid_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS filings_profile_period_idx
    ON buwiz_server.filings (tax_profile_id, form_code, period_key);

INSERT INTO buwiz_server.schema_migrations (version)
VALUES ('0004_canonical_v1_read_models')
ON CONFLICT (version) DO NOTHING;

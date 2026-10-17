-- Who changed a draft or filing, and draft reviews, for the year / draft /
-- filing endpoints (docs/sync-api.md). One open filing per form and period.
--
-- wasi-auth-migrate re-runs every SQL file on apply: keep this idempotent.

ALTER TABLE buwiz_server.form_drafts
    ADD COLUMN IF NOT EXISTS updated_by UUID REFERENCES auth_users (user_id) ON DELETE SET NULL,
    ADD COLUMN IF NOT EXISTS reviewed_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS reviewed_by UUID REFERENCES auth_users (user_id) ON DELETE SET NULL;

ALTER TABLE buwiz_server.filings
    ADD COLUMN IF NOT EXISTS created_by UUID REFERENCES auth_users (user_id) ON DELETE SET NULL,
    ADD COLUMN IF NOT EXISTS updated_by UUID REFERENCES auth_users (user_id) ON DELETE SET NULL;

-- Filings were never written before this migration (no endpoint existed).
CREATE UNIQUE INDEX IF NOT EXISTS filings_open_uidx
    ON buwiz_server.filings (tax_profile_id, form_code, period_key)
    WHERE status IN ('queued', 'submitted');

INSERT INTO buwiz_server.schema_migrations (version)
VALUES ('0010_record_authors')
ON CONFLICT (version) DO NOTHING;

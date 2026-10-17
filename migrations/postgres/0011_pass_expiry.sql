-- A pass that runs past its end date is recorded as ended
-- (`tax_profile.pass_expired`), so the read model gains the `expired` status.
--
-- wasi-auth-migrate re-runs every SQL file on apply: keep this idempotent.

ALTER TABLE buwiz_server.tax_profile_grants
    DROP CONSTRAINT IF EXISTS tax_profile_grants_status_chk;
ALTER TABLE buwiz_server.tax_profile_grants
    ADD CONSTRAINT tax_profile_grants_status_chk CHECK (
        status IN ('requested', 'offered', 'active', 'declined', 'revoked', 'expired')
    );

INSERT INTO buwiz_server.schema_migrations (version)
VALUES ('0011_pass_expiry')
ON CONFLICT (version) DO NOTHING;

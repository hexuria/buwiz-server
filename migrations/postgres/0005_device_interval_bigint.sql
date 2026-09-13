-- Spin Postgres binds every JSON number as int8. INTEGER (int4) columns
-- fail at bind time with WrongType { postgres: Int4, rust: "i64" }.
ALTER TABLE buwiz_server.oauth_device_codes
    ALTER COLUMN interval_seconds TYPE BIGINT;

INSERT INTO buwiz_server.schema_migrations (version)
VALUES ('0005_device_interval_bigint')
ON CONFLICT (version) DO NOTHING;

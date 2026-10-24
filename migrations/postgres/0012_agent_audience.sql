-- Audience of an agent connection (RFC 8707 resource indicator).
--
-- NULL is a connection from before this column: those tokens were only ever
-- issued for this server's own MCP resource. New rows store the canonical
-- audience, which is `{issuer}/mcp` or a loopback bir-mcp URL.
--
-- wasi-auth-migrate re-runs every SQL file on apply: keep this idempotent.

ALTER TABLE buwiz_server.agent_connections
    ADD COLUMN IF NOT EXISTS resource TEXT;

INSERT INTO buwiz_server.schema_migrations (version)
VALUES ('0012_agent_audience')
ON CONFLICT (version) DO NOTHING;

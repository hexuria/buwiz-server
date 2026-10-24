//! AI agent clients, connections and tokens (`0007_agent_connections`).
//!
//! Codes and tokens are stored as SHA-256 hashes only. Every token lookup
//! re-checks the connection, the taxpayer's account status and its security
//! revision, so ending a connection or changing a password stops the agent
//! on its next call.

use serde_json::{json, Value};

use crate::contracts::{AgentConnectionView, AgentProfileRef};
use crate::domain::PassPermissions;
use crate::error::{AuthStackError, AuthStackResult};

use super::{
    AtomicSqlStatement, execute_sql, execute_sql_atomic, initialize_schema_async, required_string,
    row_bool, row_i64, row_string, row_string_list,
};

/// How agent codes and tokens are stored: SHA-256, base64url without
/// padding. The OAuth endpoints and the MCP check must hash the same way.
pub(crate) fn sha256_b64url(value: &str) -> String {
    use base64::Engine as _;
    use sha2::Digest as _;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(sha2::Sha256::digest(value.as_bytes()))
}

const TIMESTAMP: &str = "'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"'";

#[derive(Clone, Debug)]
pub(crate) struct StoredAgentClient {
    pub client_id: String,
    pub client_name: String,
    pub redirect_uris: Vec<String>,
}

/// A profile a user may connect an agent to: their own, or one a working
/// pass of their workspace reaches.
#[derive(Clone, Debug)]
pub(crate) struct ConnectableProfile {
    pub profile_id: String,
    pub profile_name: String,
    pub tin_last4: String,
    /// The workspace whose pass reaches it; `None` for the user's own.
    pub via: Option<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct ConsumedAgentCode {
    pub connection_id: String,
    pub client_id: String,
    pub redirect_uri: String,
    pub code_challenge: String,
    /// Canonical audience. Empty on a connection from before audiences.
    pub resource: String,
}

/// The agent behind a valid access token.
#[derive(Clone, Debug)]
pub(crate) struct AgentActor {
    pub connection_id: String,
    pub user_id: String,
    pub client_id: String,
    pub client_name: String,
    pub permissions: PassPermissions,
    pub profile_ids: Vec<String>,
    /// Canonical audience. Empty on a connection from before audiences.
    pub resource: String,
    /// Access-token expiry, unix seconds. Zero when the column was unreadable.
    pub expires_at: i64,
}

/// Drops apps that registered over a week ago and were never allowed.
pub(crate) async fn sweep_unused_agent_clients() -> AuthStackResult<()> {
    initialize_schema_async().await?;
    execute_sql(
        "DELETE FROM buwiz_server.agent_clients c \
         WHERE c.created_at < CURRENT_TIMESTAMP - INTERVAL '7 days' \
           AND NOT EXISTS (SELECT 1 FROM buwiz_server.agent_connections x \
                           WHERE x.client_id = c.client_id)",
        vec![],
    )
    .await?;
    Ok(())
}

/// Registrations in the last hour, for the server-wide cap. Known downside:
/// the cap is global, so a flood of anonymous registrations can hold off new
/// apps for up to an hour; existing connections keep working.
pub(crate) async fn agent_registrations_last_hour() -> AuthStackResult<i64> {
    initialize_schema_async().await?;
    let rows = execute_sql(
        "SELECT count(*)::bigint AS n FROM buwiz_server.agent_clients \
         WHERE created_at > CURRENT_TIMESTAMP - INTERVAL '1 hour'",
        vec![],
    )
    .await?;
    Ok(rows.first().and_then(|row| row_i64(row, "n")).unwrap_or(0))
}

pub(crate) async fn insert_agent_client(
    client_id: &str,
    client_name: &str,
    redirect_uris: &[String],
) -> AuthStackResult<()> {
    initialize_schema_async().await?;
    execute_sql(
        "INSERT INTO buwiz_server.agent_clients (client_id, client_name, redirect_uris) \
         VALUES (?1, ?2, ?3::jsonb)",
        vec![json!(client_id), json!(client_name), json!(redirect_uris)],
    )
    .await?;
    Ok(())
}

pub(crate) async fn load_agent_client(client_id: &str) -> AuthStackResult<Option<StoredAgentClient>> {
    initialize_schema_async().await?;
    let rows = execute_sql(
        "SELECT client_id, client_name, redirect_uris \
         FROM buwiz_server.agent_clients WHERE client_id = ?1",
        vec![json!(client_id)],
    )
    .await?;
    let Some(row) = rows.first() else {
        return Ok(None);
    };
    Ok(Some(StoredAgentClient {
        client_id: required_string(row, "client_id")?,
        client_name: required_string(row, "client_name")?,
        redirect_uris: row_string_list(row, "redirect_uris"),
    }))
}

/// `FROM ... WHERE ...` over the working passes on profile `p` held by an
/// active workspace the user in parameter `?{user}` is an active member of
/// (aliases `g` pass, `o` workspace).
fn workspace_pass_sql(user: u8) -> String {
    format!(
        "FROM buwiz_server.tax_profile_grants g \
         JOIN auth_memberships m ON m.organization_id::text = g.grantee_id \
                                 AND m.user_id = ?{user}::uuid AND m.status = 'active' \
         JOIN auth_organizations o ON o.organization_id = m.organization_id \
                                  AND o.status = 'active' \
         WHERE g.profile_id = p.profile_id AND g.grantee_kind = 'organization' \
           AND g.status = 'active' \
           AND (g.expires_at IS NULL OR g.expires_at > CURRENT_TIMESTAMP)"
    )
}

/// Whether the user in parameter `?{user}` owns profile `p`
/// (`verified_owner_user_id` also counts, so a profile moved to a firm
/// under the old rules is reachable before the projection is rebuilt; on
/// replay its owner keeps it).
fn owns_sql(user: u8) -> String {
    format!("(p.holder_user_id = ?{user}::uuid OR p.verified_owner_user_id = ?{user}::uuid)")
}

/// When that user may connect an agent to profile `p`: they own it, or a
/// workspace of theirs holds a working pass on it. Never when archived.
fn connectable_sql(user: u8) -> String {
    format!(
        "COALESCE(p.is_archived, false) = FALSE AND ({} OR EXISTS (SELECT 1 {}))",
        owns_sql(user),
        workspace_pass_sql(user)
    )
}

/// Profiles `user_id` may connect an agent to, own ones first.
pub(crate) async fn list_connectable_profiles(user_id: &str) -> AuthStackResult<Vec<ConnectableProfile>> {
    initialize_schema_async().await?;
    let rows = execute_sql(
        &format!(
            "SELECT p.profile_id::text AS profile_id, \
                    COALESCE(p.display_name, p.registered_name) AS profile_name, \
                    COALESCE(p.tin_last4, '') AS tin_last4, \
                    COALESCE({}, false) AS mine, (SELECT min(o.name) {}) AS via \
             FROM buwiz_server.tax_profiles p \
             WHERE {} \
             ORDER BY mine DESC, profile_name",
            owns_sql(1),
            workspace_pass_sql(1),
            connectable_sql(1)
        ),
        vec![json!(user_id)],
    )
    .await?;
    rows.iter().map(connectable_from_row).collect()
}

fn connectable_from_row(row: &Value) -> AuthStackResult<ConnectableProfile> {
    let mine = row_bool(row, "mine").unwrap_or(false);
    Ok(ConnectableProfile {
        profile_id: required_string(row, "profile_id")?,
        profile_name: row_string(row, "profile_name").unwrap_or_default(),
        tin_last4: row_string(row, "tin_last4").unwrap_or_default(),
        via: if mine { None } else { row_string(row, "via") },
    })
}

/// The active workspaces `user_id` is an active member of. Agents have no
/// browser session, so their firm standing is read from here.
pub(crate) async fn active_organization_ids_for(user_id: &str) -> AuthStackResult<Vec<String>> {
    initialize_schema_async().await?;
    let rows = execute_sql(
        "SELECT m.organization_id::text AS organization_id \
         FROM auth_memberships m \
         JOIN auth_organizations o ON o.organization_id = m.organization_id AND o.status = 'active' \
         WHERE m.user_id = ?1::uuid AND m.status = 'active'",
        vec![json!(user_id)],
    )
    .await?;
    Ok(rows.iter().filter_map(|row| row_string(row, "organization_id")).collect())
}

/// Records the taxpayer's Allow: a new connection on the chosen profiles
/// they own, and the one-time code for the app. An earlier connection of the
/// same app for the same taxpayer ends, so re-allowing replaces it.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn create_agent_connection(
    connection_id: &str,
    user_id: &str,
    client_id: &str,
    permissions: PassPermissions,
    profile_ids: &[String],
    code_hash: &str,
    redirect_uri: &str,
    code_challenge: &str,
    code_ttl_seconds: i64,
    resource: &str,
) -> AuthStackResult<()> {
    initialize_schema_async().await?;
    execute_sql_atomic(vec![
        AtomicSqlStatement::execute(
            "UPDATE buwiz_server.agent_tokens SET revoked_at = CURRENT_TIMESTAMP \
             WHERE revoked_at IS NULL AND connection_id IN ( \
                 SELECT connection_id FROM buwiz_server.agent_connections \
                 WHERE user_id = ?1::uuid AND client_id = ?2 AND revoked_at IS NULL)",
            vec![json!(user_id), json!(client_id)],
        ),
        AtomicSqlStatement::execute(
            "UPDATE buwiz_server.agent_connections SET revoked_at = CURRENT_TIMESTAMP \
             WHERE user_id = ?1::uuid AND client_id = ?2 AND revoked_at IS NULL",
            vec![json!(user_id), json!(client_id)],
        ),
        AtomicSqlStatement::guard(
            "INSERT INTO buwiz_server.agent_connections \
                 (connection_id, client_id, user_id, security_revision, permissions, resource) \
             SELECT ?1::uuid, ?2, u.user_id, u.security_revision, ?4::jsonb, ?5 \
             FROM auth_users u WHERE u.user_id = ?3::uuid AND u.status = 'active' \
             RETURNING connection_id",
            vec![
                json!(connection_id),
                json!(client_id),
                json!(user_id),
                json!(permissions.ids()),
                json!(resource),
            ],
        ),
        // Re-checks reach here so a forged form cannot add a profile the
        // user may not hand on.
        AtomicSqlStatement::guard(
            format!(
                "INSERT INTO buwiz_server.agent_connection_profiles (connection_id, profile_id) \
                 SELECT ?1::uuid, p.profile_id FROM buwiz_server.tax_profiles p \
                 WHERE p.profile_id::text IN (SELECT jsonb_array_elements_text(?2::jsonb)) \
                   AND {} \
                 RETURNING profile_id",
                connectable_sql(3)
            ),
            vec![json!(connection_id), json!(profile_ids), json!(user_id)],
        ),
        AtomicSqlStatement::execute(
            "INSERT INTO buwiz_server.agent_auth_codes \
                 (code_hash, connection_id, redirect_uri, code_challenge, expires_at) \
             VALUES (?1, ?2::uuid, ?3, ?4, CURRENT_TIMESTAMP + (?5::bigint * INTERVAL '1 second'))",
            vec![
                json!(code_hash),
                json!(connection_id),
                json!(redirect_uri),
                json!(code_challenge),
                json!(code_ttl_seconds),
            ],
        ),
    ])
    .await?;
    Ok(())
}

pub(crate) async fn consume_agent_code(code_hash: &str) -> AuthStackResult<Option<ConsumedAgentCode>> {
    initialize_schema_async().await?;
    let rows = execute_sql(
        "UPDATE buwiz_server.agent_auth_codes a SET consumed_at = CURRENT_TIMESTAMP \
         FROM buwiz_server.agent_connections c \
         WHERE a.code_hash = ?1 AND a.consumed_at IS NULL AND a.expires_at > CURRENT_TIMESTAMP \
           AND c.connection_id = a.connection_id AND c.revoked_at IS NULL \
         RETURNING a.connection_id::text AS connection_id, c.client_id, a.redirect_uri, \
                   a.code_challenge, COALESCE(c.resource, '') AS resource",
        vec![json!(code_hash)],
    )
    .await?;
    let Some(row) = rows.first() else {
        return Ok(None);
    };
    Ok(Some(ConsumedAgentCode {
        connection_id: required_string(row, "connection_id")?,
        client_id: required_string(row, "client_id")?,
        redirect_uri: required_string(row, "redirect_uri")?,
        code_challenge: required_string(row, "code_challenge")?,
        resource: row_string(row, "resource").unwrap_or_default(),
    }))
}

fn insert_token_statement(
    token_hash: &str,
    connection_id: &str,
    kind: &str,
    ttl_seconds: i64,
) -> AtomicSqlStatement {
    AtomicSqlStatement::execute(
        "INSERT INTO buwiz_server.agent_tokens (token_hash, connection_id, kind, expires_at) \
         VALUES (?1, ?2::uuid, ?3, CURRENT_TIMESTAMP + (?4::bigint * INTERVAL '1 second'))",
        vec![
            json!(token_hash),
            json!(connection_id),
            json!(kind),
            json!(ttl_seconds),
        ],
    )
}

/// Stores a fresh access + refresh pair for a connection.
pub(crate) async fn insert_agent_tokens(
    connection_id: &str,
    access_hash: &str,
    access_ttl_seconds: i64,
    refresh_hash: &str,
    refresh_ttl_seconds: i64,
) -> AuthStackResult<()> {
    initialize_schema_async().await?;
    execute_sql_atomic(vec![
        AtomicSqlStatement::execute(
            "DELETE FROM buwiz_server.agent_tokens \
             WHERE connection_id = ?1::uuid AND expires_at < CURRENT_TIMESTAMP - INTERVAL '1 day'",
            vec![json!(connection_id)],
        ),
        insert_token_statement(access_hash, connection_id, "access", access_ttl_seconds),
        insert_token_statement(refresh_hash, connection_id, "refresh", refresh_ttl_seconds),
    ])
    .await?;
    Ok(())
}

/// Audience of a refresh token that is still good for `client_id`.
/// `Ok(None)` when that refresh token cannot be used.
pub(crate) async fn live_agent_refresh_audience(
    refresh_hash: &str,
    client_id: &str,
) -> AuthStackResult<Option<String>> {
    initialize_schema_async().await?;
    let rows = execute_sql(
        &format!(
            "SELECT COALESCE(c.resource, '') AS resource \
             FROM buwiz_server.agent_tokens t \
             JOIN buwiz_server.agent_connections c ON c.connection_id = t.connection_id \
             JOIN auth_users u ON u.user_id = c.user_id \
             WHERE {LIVE_REFRESH}"
        ),
        vec![json!(refresh_hash), json!(client_id)],
    )
    .await?;
    Ok(rows
        .first()
        .map(|row| row_string(row, "resource").unwrap_or_default()))
}

/// A refresh token still good for `client_id`: unexpired, not revoked, its
/// connection open, and the taxpayer's account active and unchanged since
/// the Allow.
const LIVE_REFRESH: &str = "t.token_hash = ?1 AND t.kind = 'refresh' AND t.revoked_at IS NULL \
     AND t.expires_at > CURRENT_TIMESTAMP \
     AND c.connection_id = t.connection_id AND c.revoked_at IS NULL AND c.client_id = ?2 \
     AND u.user_id = c.user_id AND u.status = 'active' \
     AND u.security_revision = c.security_revision";

fn insert_rotated_statement(
    old_refresh_hash: &str,
    token_hash: &str,
    kind: &str,
    ttl_seconds: i64,
) -> AtomicSqlStatement {
    AtomicSqlStatement::execute(
        "INSERT INTO buwiz_server.agent_tokens (token_hash, connection_id, kind, expires_at) \
         SELECT ?2, connection_id, ?3, CURRENT_TIMESTAMP + (?4::bigint * INTERVAL '1 second') \
         FROM buwiz_server.agent_tokens WHERE token_hash = ?1",
        vec![
            json!(old_refresh_hash),
            json!(token_hash),
            json!(kind),
            json!(ttl_seconds),
        ],
    )
}

/// Swaps a refresh token issued to `client_id` for a new pair, in one
/// transaction: the old refresh token is revoked only if the new pair is
/// stored. `Ok(None)` when the refresh token is unknown, expired, revoked,
/// for another app, or its connection ended.
///
/// A refresh token that was already used ends its whole connection (RFC
/// 9700 §4.14.2): one copy is in the wrong hands, and the taxpayer allows
/// the app again to recover.
pub(crate) async fn rotate_agent_refresh(
    refresh_hash: &str,
    client_id: &str,
    access_hash: &str,
    access_ttl_seconds: i64,
    new_refresh_hash: &str,
    refresh_ttl_seconds: i64,
) -> AuthStackResult<Option<String>> {
    initialize_schema_async().await?;
    let rotated = execute_sql_atomic(vec![
        AtomicSqlStatement::guard(
            format!(
                "UPDATE buwiz_server.agent_tokens t SET revoked_at = CURRENT_TIMESTAMP \
                 FROM buwiz_server.agent_connections c, auth_users u \
                 WHERE {LIVE_REFRESH} \
                 RETURNING t.connection_id::text AS connection_id"
            ),
            vec![json!(refresh_hash), json!(client_id)],
        ),
        insert_rotated_statement(refresh_hash, access_hash, "access", access_ttl_seconds),
        insert_rotated_statement(refresh_hash, new_refresh_hash, "refresh", refresh_ttl_seconds),
    ])
    .await;
    match rotated {
        Ok(results) => Ok(results
            .first()
            .and_then(|rows| rows.first())
            .and_then(|row| row_string(row, "connection_id"))),
        Err(error) => refresh_refused_or(error, refresh_hash, client_id).await,
    }
}

/// After a failed rotation: `Ok(None)` when the refresh token was simply not
/// usable (and a reused one ends its connection); the original error when
/// the token was fine, so a database failure is not reported as a bad token.
async fn refresh_refused_or(
    error: AuthStackError,
    refresh_hash: &str,
    client_id: &str,
) -> AuthStackResult<Option<String>> {
    let reused = execute_sql(
        "SELECT connection_id::text AS connection_id FROM buwiz_server.agent_tokens \
         WHERE token_hash = ?1 AND kind = 'refresh' AND revoked_at IS NOT NULL",
        vec![json!(refresh_hash)],
    )
    .await?;
    if let Some(connection_id) = reused.first().and_then(|row| row_string(row, "connection_id")) {
        end_connection(&connection_id).await?;
        return Ok(None);
    }
    let live = execute_sql(
        &format!(
            "SELECT 1 AS live FROM buwiz_server.agent_tokens t, \
                    buwiz_server.agent_connections c, auth_users u \
             WHERE {LIVE_REFRESH}"
        ),
        vec![json!(refresh_hash), json!(client_id)],
    )
    .await?;
    if live.is_empty() { Ok(None) } else { Err(error) }
}

/// Ends a connection and every token it holds.
async fn end_connection(connection_id: &str) -> AuthStackResult<()> {
    execute_sql_atomic(vec![
        AtomicSqlStatement::execute(
            "UPDATE buwiz_server.agent_connections SET revoked_at = CURRENT_TIMESTAMP \
             WHERE connection_id::text = ?1 AND revoked_at IS NULL",
            vec![json!(connection_id)],
        ),
        AtomicSqlStatement::execute(
            "UPDATE buwiz_server.agent_tokens SET revoked_at = CURRENT_TIMESTAMP \
             WHERE connection_id::text = ?1 AND revoked_at IS NULL",
            vec![json!(connection_id)],
        ),
    ])
    .await?;
    Ok(())
}

/// The agent behind an access token, or `None` when the token is unknown,
/// expired or revoked, its connection ended, or the taxpayer's account is
/// disabled or had a security change since the Allow.
pub(crate) async fn resolve_agent_access_token(
    access_hash: &str,
) -> AuthStackResult<Option<AgentActor>> {
    initialize_schema_async().await?;
    let rows = execute_sql(
        "SELECT c.connection_id::text AS connection_id, c.user_id::text AS user_id, \
                c.client_id, ac.client_name, c.permissions, \
                COALESCE(c.resource, '') AS resource, \
                EXTRACT(EPOCH FROM t.expires_at)::bigint AS expires_at, \
                COALESCE((SELECT jsonb_agg(cp.profile_id::text) \
                          FROM buwiz_server.agent_connection_profiles cp \
                          WHERE cp.connection_id = c.connection_id), '[]'::jsonb) AS profile_ids \
         FROM buwiz_server.agent_tokens t \
         JOIN buwiz_server.agent_connections c ON c.connection_id = t.connection_id \
         JOIN buwiz_server.agent_clients ac ON ac.client_id = c.client_id \
         JOIN auth_users u ON u.user_id = c.user_id \
         WHERE t.token_hash = ?1 AND t.kind = 'access' AND t.revoked_at IS NULL \
           AND t.expires_at > CURRENT_TIMESTAMP AND c.revoked_at IS NULL \
           AND u.status = 'active' AND u.security_revision = c.security_revision",
        vec![json!(access_hash)],
    )
    .await?;
    let Some(row) = rows.first() else {
        return Ok(None);
    };
    let actor = AgentActor {
        connection_id: required_string(row, "connection_id")?,
        user_id: required_string(row, "user_id")?,
        client_id: required_string(row, "client_id")?,
        client_name: required_string(row, "client_name")?,
        permissions: PassPermissions::from_stored(&row_string_list(row, "permissions")),
        profile_ids: row_string_list(row, "profile_ids"),
        resource: row_string(row, "resource").unwrap_or_default(),
        expires_at: row
            .get("expires_at")
            .and_then(|value| match value {
                Value::Number(number) => number.as_i64().or_else(|| number.as_f64().map(|n| n as i64)),
                Value::String(text) => text.parse().ok(),
                _ => None,
            })
            .unwrap_or(0),
    };
    // Best effort: a failed timestamp must not fail the agent's call.
    let _ = execute_sql(
        "UPDATE buwiz_server.agent_connections SET last_used_at = CURRENT_TIMESTAMP \
         WHERE connection_id = ?1::uuid \
           AND (last_used_at IS NULL OR last_used_at < CURRENT_TIMESTAMP - INTERVAL '1 minute')",
        vec![json!(actor.connection_id)],
    )
    .await;
    Ok(Some(actor))
}

pub(crate) async fn list_agent_connections(
    user_id: &str,
) -> AuthStackResult<Vec<AgentConnectionView>> {
    initialize_schema_async().await?;
    let sql = format!(
        "SELECT c.connection_id::text AS connection_id, c.client_id, ac.client_name, c.may_queue, c.permissions, \
                to_char(c.created_at AT TIME ZONE 'UTC', {TIMESTAMP}) AS created_at, \
                to_char(c.last_used_at AT TIME ZONE 'UTC', {TIMESTAMP}) AS last_used_at, \
                COALESCE((SELECT jsonb_agg(jsonb_build_object( \
                              'profile_id', p.profile_id::text, \
                              'profile_name', COALESCE(p.display_name, p.registered_name), \
                              'tin_last4', COALESCE(p.tin_last4, '')) \
                              ORDER BY COALESCE(p.display_name, p.registered_name)) \
                          FROM buwiz_server.agent_connection_profiles cp \
                          JOIN buwiz_server.tax_profiles p ON p.profile_id = cp.profile_id \
                          WHERE cp.connection_id = c.connection_id), '[]'::jsonb) AS profiles \
         FROM buwiz_server.agent_connections c \
         JOIN buwiz_server.agent_clients ac ON ac.client_id = c.client_id \
         JOIN auth_users u ON u.user_id = c.user_id \
         WHERE c.user_id = ?1::uuid AND c.revoked_at IS NULL \
           AND u.status = 'active' AND u.security_revision = c.security_revision \
         ORDER BY c.created_at DESC"
    );
    let rows = execute_sql(&sql, vec![json!(user_id)]).await?;
    rows.iter()
        .map(|row| {
            let permissions = PassPermissions::from_stored(&row_string_list(row, "permissions"));
            Ok(AgentConnectionView {
                connection_id: required_string(row, "connection_id")?,
                client_id: required_string(row, "client_id")?,
                client_name: required_string(row, "client_name")?,
                permissions: permissions.ids(),
                preset: permissions.preset().map(|preset| preset.as_str().to_owned()),
                profiles: profile_refs(row.get("profiles")),
                may_queue: row_bool(row, "may_queue").unwrap_or(false),
                created_at: required_string(row, "created_at")?,
                last_used_at: row_string(row, "last_used_at"),
            })
        })
        .collect()
}

fn profile_refs(value: Option<&Value>) -> Vec<AgentProfileRef> {
    let parsed;
    let items = match value {
        Some(Value::Array(items)) => items,
        Some(Value::String(raw)) => {
            parsed = serde_json::from_str::<Vec<Value>>(raw).unwrap_or_default();
            &parsed
        }
        _ => return Vec::new(),
    };
    items
        .iter()
        .map(|item| AgentProfileRef {
            profile_id: row_string(item, "profile_id").unwrap_or_default(),
            profile_name: row_string(item, "profile_name").unwrap_or_default(),
            tin_last4: row_string(item, "tin_last4").unwrap_or_default(),
        })
        .collect()
}

/// Ends one of the caller's agent connections and every token it holds.
pub(crate) async fn revoke_agent_connection(
    user_id: &str,
    connection_id: &str,
) -> AuthStackResult<()> {
    initialize_schema_async().await?;
    let rows = execute_sql(
        "SELECT 1 AS mine FROM buwiz_server.agent_connections \
         WHERE connection_id::text = ?1 AND user_id = ?2::uuid AND revoked_at IS NULL",
        vec![json!(connection_id), json!(user_id)],
    )
    .await?;
    if rows.is_empty() {
        return Err(AuthStackError::not_found("agent connection not found"));
    }
    end_connection(connection_id).await
}

/// The owner turns queue on or off for one of their agent connections.
pub(crate) async fn set_agent_may_queue(
    user_id: &str,
    connection_id: &str,
    may_queue: bool,
) -> AuthStackResult<()> {
    initialize_schema_async().await?;
    let rows = execute_sql(
        "UPDATE buwiz_server.agent_connections SET may_queue = ?3 \
         WHERE connection_id = ?1::uuid AND user_id = ?2::uuid AND revoked_at IS NULL \
         RETURNING connection_id::text AS connection_id",
        vec![json!(connection_id), json!(user_id), json!(may_queue)],
    )
    .await?;
    if rows.is_empty() {
        return Err(AuthStackError::not_found("agent connection not found"));
    }
    Ok(())
}

/// Whether this live connection may leave filings for the owner's computer.
pub(crate) async fn agent_may_queue(connection_id: &str) -> AuthStackResult<bool> {
    initialize_schema_async().await?;
    let rows = execute_sql(
        "SELECT may_queue FROM buwiz_server.agent_connections \
         WHERE connection_id = ?1::uuid AND revoked_at IS NULL",
        vec![json!(connection_id)],
    )
    .await?;
    Ok(rows
        .first()
        .and_then(|row| row_bool(row, "may_queue"))
        .unwrap_or(false))
}

pub(crate) struct NewDeviceFiling {
    pub user_id: String,
    pub connection_id: String,
    pub profile_id: String,
    pub tin: String,
    pub form_code: String,
    pub tax_year: i32,
    pub period: i32,
    pub fields: Value,
}

/// Insert or reuse a queued device filing. A retry for the same period
/// replaces the fields and returns the existing id.
pub(crate) async fn enqueue_device_filing(filing: &NewDeviceFiling) -> AuthStackResult<String> {
    initialize_schema_async().await?;
    let existing = execute_sql(
        "SELECT id::text AS id FROM buwiz_server.device_filings \
         WHERE agent_connection_id = ?1::uuid AND profile_id = ?2::uuid \
           AND form_code = ?3 AND tax_year = ?4::bigint AND period = ?5::bigint AND status = 'queued' \
         ORDER BY created_at DESC LIMIT 1",
        vec![
            json!(filing.connection_id),
            json!(filing.profile_id),
            json!(filing.form_code),
            json!(filing.tax_year),
            json!(filing.period),
        ],
    )
    .await?;
    if let Some(id) = existing.first().and_then(|row| row_string(row, "id")) {
        execute_sql(
            "UPDATE buwiz_server.device_filings \
             SET tin = ?2, fields = ?3::jsonb, updated_at = CURRENT_TIMESTAMP \
             WHERE id = ?1::uuid AND status = 'queued'",
            vec![json!(id), json!(filing.tin), json!(filing.fields)],
        )
        .await?;
        return Ok(id);
    }
    let rows = execute_sql(
        "INSERT INTO buwiz_server.device_filings \
            (user_id, agent_connection_id, profile_id, tin, form_code, tax_year, period, fields) \
         VALUES (?1::uuid, ?2::uuid, ?3::uuid, ?4, ?5, ?6::bigint, ?7::bigint, ?8::jsonb) \
         RETURNING id::text AS id",
        vec![
            json!(filing.user_id),
            json!(filing.connection_id),
            json!(filing.profile_id),
            json!(filing.tin),
            json!(filing.form_code),
            json!(filing.tax_year),
            json!(filing.period),
            json!(filing.fields),
        ],
    )
    .await?;
    rows.first()
        .and_then(|row| row_string(row, "id"))
        .ok_or_else(|| AuthStackError::store("device filing was not stored"))
}

pub(crate) async fn list_queued_device_filings(user_id: &str) -> AuthStackResult<Vec<Value>> {
    initialize_schema_async().await?;
    let rows = execute_sql(
        "SELECT id::text AS id, tin, form_code, tax_year, period, fields \
         FROM buwiz_server.device_filings \
         WHERE user_id = ?1::uuid AND status = 'queued' \
         ORDER BY created_at ASC LIMIT 20",
        vec![json!(user_id)],
    )
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| {
            json!({
                "id": row_string(&row, "id").unwrap_or_default(),
                "tin": row_string(&row, "tin").unwrap_or_default(),
                "form_code": row_string(&row, "form_code").unwrap_or_default(),
                "tax_year": row_number(&row, "tax_year"),
                "period": row_number(&row, "period"),
                "fields": json_cell(&row, "fields"),
            })
        })
        .collect())
}

pub(crate) async fn device_filing_for_agent(
    connection_id: &str,
    filing_id: &str,
) -> AuthStackResult<Option<Value>> {
    initialize_schema_async().await?;
    let rows = execute_sql(
        "SELECT id::text AS id, form_code, tax_year, period, status, detail, report \
         FROM buwiz_server.device_filings \
         WHERE id = ?1::uuid AND agent_connection_id = ?2::uuid",
        vec![json!(filing_id), json!(connection_id)],
    )
    .await?;
    let Some(row) = rows.first() else {
        return Ok(None);
    };
    Ok(Some(json!({
        "id": row_string(row, "id").unwrap_or_default(),
        "form_code": row_string(row, "form_code").unwrap_or_default(),
        "tax_year": row_number(row, "tax_year"),
        "period": row_number(row, "period"),
        "status": row_string(row, "status").unwrap_or_default(),
        "message": row_string(row, "detail").unwrap_or_default(),
        "field_errors": json_cell(row, "report").get("field_errors").cloned().unwrap_or(json!([])),
    })))
}

pub(crate) async fn report_device_filing(
    user_id: &str,
    filing_id: &str,
    status: &str,
    detail: &str,
    report: &Value,
) -> AuthStackResult<()> {
    initialize_schema_async().await?;
    let rows = execute_sql(
        "UPDATE buwiz_server.device_filings \
         SET status = ?3, detail = ?4, report = ?5::jsonb, updated_at = CURRENT_TIMESTAMP \
         WHERE id = ?1::uuid AND user_id = ?2::uuid AND status = 'queued' \
         RETURNING id::text AS id",
        vec![
            json!(filing_id),
            json!(user_id),
            json!(status),
            json!(detail),
            json!(report),
        ],
    )
    .await?;
    if rows.is_empty() {
        return Err(AuthStackError::not_found("queued device filing not found"));
    }
    Ok(())
}

pub(crate) async fn profile_tin_last4(profile_id: &str) -> AuthStackResult<Option<String>> {
    initialize_schema_async().await?;
    let rows = execute_sql(
        "SELECT tin_last4 FROM buwiz_server.tax_profiles WHERE profile_id = ?1::uuid",
        vec![json!(profile_id)],
    )
    .await?;
    Ok(rows.first().and_then(|row| row_string(row, "tin_last4")))
}

fn row_number(row: &Value, key: &str) -> i64 {
    row_i64(row, key)
        .or_else(|| {
            row.get(key)
                .and_then(Value::as_str)
                .and_then(|text| text.parse().ok())
        })
        .unwrap_or(0)
}

fn json_cell(row: &Value, key: &str) -> Value {
    match row.get(key) {
        Some(Value::String(raw)) => serde_json::from_str(raw).unwrap_or(Value::Null),
        Some(value) => value.clone(),
        None => Value::Null,
    }
}

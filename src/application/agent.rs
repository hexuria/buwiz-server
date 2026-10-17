//! AI agent connections: the taxpayer's list and off switch, and the check
//! the MCP endpoint runs on every call.

use crate::contracts::AgentConnectionListResponse;
use crate::domain::{agent_effective_permissions, is_agent_token, PassPermissions};
use crate::error::{AuthStackError, AuthStackResult};
use crate::store::AgentActor;

use super::{RequestAuth, authenticated_session_view};

/// The caller's connected agents.
pub async fn list_agent_connections(
    auth: RequestAuth,
) -> AuthStackResult<AgentConnectionListResponse> {
    let user_id = caller(auth).await?;
    Ok(AgentConnectionListResponse {
        connections: crate::store::list_agent_connections(&user_id).await?,
    })
}

/// Ends one of the caller's agent connections. Its tokens stop working on
/// the next call; the caller's own sign-in is untouched.
pub async fn revoke_agent_connection(
    connection_id: String,
    auth: RequestAuth,
) -> AuthStackResult<AgentConnectionListResponse> {
    let user_id = caller(auth).await?;
    crate::store::revoke_agent_connection(&user_id, &connection_id).await?;
    Ok(AgentConnectionListResponse {
        connections: crate::store::list_agent_connections(&user_id).await?,
    })
}

async fn caller(auth: RequestAuth) -> AuthStackResult<String> {
    authenticated_session_view(auth)
        .await?
        .user_id
        .ok_or(AuthStackError::AuthRequired)
}

/// The agent behind a bearer token, for the MCP endpoint. Anything but a
/// live agent access token is `InvalidToken`.
#[allow(dead_code)] // called by the MCP endpoint
pub(crate) async fn agent_from_bearer(token: &str) -> AuthStackResult<AgentActor> {
    let token = token.trim();
    if !is_agent_token(token) {
        return Err(AuthStackError::InvalidToken);
    }
    crate::store::resolve_agent_access_token(&crate::store::sha256_b64url(token))
        .await?
        .ok_or(AuthStackError::InvalidToken)
}

/// What `agent` may do on `profile_id` right now: the connection's
/// permissions narrowed by the taxpayer's live access. `Forbidden` when the
/// profile is not connected or nothing is left.
#[allow(dead_code)] // called by the MCP endpoint
pub(crate) async fn agent_profile_permissions(
    agent: &AgentActor,
    profile_id: &str,
) -> AuthStackResult<PassPermissions> {
    if !agent.profile_ids.iter().any(|id| id == profile_id) {
        return Err(AuthStackError::Forbidden);
    }
    let loaded = crate::store::load_tax_profile_by_id(profile_id).await?;
    if !loaded.state.exists {
        return Err(AuthStackError::Forbidden);
    }
    let now = crate::store::rfc3339_now();
    // No organizations: an agent acts only on what its taxpayer owns.
    let access = loaded.state.access_for(&agent.user_id, &[], &now);
    let permissions = agent_effective_permissions(agent.permissions, access);
    if permissions.is_empty() {
        return Err(AuthStackError::Forbidden);
    }
    Ok(permissions)
}

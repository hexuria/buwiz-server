//! AI agent connections: the taxpayer's list and off switch, and the check
//! the MCP endpoint runs on every call.

use crate::contracts::AgentConnectionListResponse;
use crate::domain::{PassPermission, PassPermissions, agent_effective_permissions, is_agent_token};
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

/// The owner allows or stops one agent leaving filings for their computer.
/// Turning this on does not send anything to BIR.
pub async fn set_agent_may_queue(
    connection_id: String,
    may_queue: bool,
    auth: RequestAuth,
) -> AuthStackResult<AgentConnectionListResponse> {
    let user_id = caller(auth).await?;
    crate::store::set_agent_may_queue(&user_id, &connection_id, may_queue).await?;
    Ok(AgentConnectionListResponse {
        connections: crate::store::list_agent_connections(&user_id).await?,
    })
}

/// Filings an agent left for this account's computer. The caller's app
/// sends them. This server does not.
pub async fn list_device_filings(auth: RequestAuth) -> AuthStackResult<serde_json::Value> {
    let user_id = caller(auth).await?;
    let filings = crate::store::list_queued_device_filings(&user_id).await?;
    Ok(serde_json::json!({ "filings": filings }))
}

/// The owner's app reports what happened to a filing it picked up.
pub async fn report_device_filing(
    filing_id: String,
    report: crate::contracts::DeviceFilingReportRequest,
    auth: RequestAuth,
) -> AuthStackResult<serde_json::Value> {
    let user_id = caller(auth).await?;
    let status = match report.status.trim() {
        "picked" | "invalid" | "failed" => report.status.trim(),
        _ => {
            return Err(AuthStackError::validation(
                "status must be picked, invalid, or failed",
            ));
        }
    };
    let detail: String = report.error_summary.chars().take(500).collect();
    let body = serde_json::json!({
        "error_summary": detail,
        "field_errors": report.field_errors,
    });
    crate::store::report_device_filing(&user_id, &filing_id, status, &detail, &body).await?;
    Ok(serde_json::json!({ "id": filing_id, "status": status }))
}

const QUEUE_REFUSED: &str = "The account owner has not allowed this agent to queue filings. \
They turn that on under Connected agents. Nothing was queued, and this server does not contact BIR.";

/// Leave a filing for the owner's computer. Refuses unless the owner turned
/// queue on for this connection. Does not contact BIR.
pub(crate) async fn agent_queue_on_device(
    agent: &AgentActor,
    profile_id: &str,
    tin: &str,
    form_code: &str,
    tax_year: i32,
    period: i32,
    fields: &serde_json::Value,
) -> AuthStackResult<serde_json::Value> {
    if !agent.profile_ids.iter().any(|id| id == profile_id) {
        return Err(AuthStackError::Forbidden);
    }
    if !crate::store::agent_may_queue(&agent.connection_id).await? {
        return Err(AuthStackError::validation(QUEUE_REFUSED));
    }
    let last4 = crate::store::profile_tin_last4(profile_id)
        .await?
        .unwrap_or_default();
    let digits: String = tin.chars().filter(|ch| ch.is_ascii_digit()).collect();
    if digits.len() < 9
        || digits.len() > 14
        || !digits.ends_with(last4.as_str())
        || last4.len() != 4
    {
        return Err(AuthStackError::validation(
            "tin does not match this profile",
        ));
    }
    let id = crate::store::enqueue_device_filing(&crate::store::NewDeviceFiling {
        user_id: agent.user_id.clone(),
        connection_id: agent.connection_id.clone(),
        profile_id: profile_id.to_owned(),
        tin: digits,
        form_code: form_code.to_owned(),
        tax_year,
        period,
        fields: fields.clone(),
    })
    .await?;
    Ok(serde_json::json!({
        "id": id,
        "status": "queued",
        "message": "Queued for the account owner's computer. This server does not contact BIR. \
    Read device_filing_status for the result. The owner's app runs the form's Validate before upload.",
    }))
}

pub(crate) async fn agent_device_filing_status(
    agent: &AgentActor,
    filing_id: &str,
) -> AuthStackResult<serde_json::Value> {
    crate::store::device_filing_for_agent(&agent.connection_id, filing_id)
        .await?
        .ok_or_else(|| AuthStackError::not_found("device filing not found"))
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
/// permissions narrowed by its user's live access (as owner, or through a
/// working pass of one of their workspaces). `Forbidden` when the
/// profile is not connected or nothing is left.
pub(crate) async fn agent_profile_permissions(
    agent: &AgentActor,
    profile_id: &str,
) -> AuthStackResult<PassPermissions> {
    if !agent.profile_ids.iter().any(|id| id == profile_id) {
        return Err(AuthStackError::Forbidden);
    }
    let loaded = crate::store::load_tax_profile_by_id(profile_id).await?;
    // An archived profile is out of an agent's reach until it is restored.
    if !loaded.state.exists || loaded.state.is_archived {
        return Err(AuthStackError::Forbidden);
    }
    let now = crate::store::rfc3339_now();
    // An agent has no browser session, so its user's workspaces come from
    // their active memberships. Only workspaces holding a working pass on
    // this profile matter (the owner needs none).
    let pass_orgs: Vec<&str> = loaded
        .state
        .passes
        .iter()
        .filter(|pass| pass.is_usable(&now))
        .map(|pass| pass.grantee.id())
        .collect();
    let actor_orgs = if pass_orgs.is_empty() {
        Vec::new()
    } else {
        crate::store::active_organization_ids_for(&agent.user_id)
            .await?
            .into_iter()
            .filter(|org| pass_orgs.contains(&org.as_str()))
            .collect()
    };
    let access = loaded.state.access_for(&agent.user_id, &actor_orgs, &now);
    let permissions = agent_effective_permissions(agent.permissions, access);
    if permissions.is_empty() {
        return Err(AuthStackError::Forbidden);
    }
    Ok(permissions)
}

/// Refuses unless `agent` may use `permission` on `profile_id` right now.
pub(crate) async fn agent_require(
    agent: &AgentActor,
    profile_id: &str,
    permission: PassPermission,
) -> AuthStackResult<()> {
    let permissions = agent_profile_permissions(agent, profile_id).await?;
    if permissions.contains(permission) {
        Ok(())
    } else {
        Err(AuthStackError::Forbidden)
    }
}

/// One connected profile as an agent sees it, with what it may do now.
pub(crate) struct AgentProfile {
    pub view: crate::contracts::TaxProfileView,
    pub permissions: PassPermissions,
}

/// Every active profile the agent is connected to. A profile it cannot
/// reach right now is kept with empty permissions so the agent can say why.
pub(crate) async fn agent_tax_profiles(agent: &AgentActor) -> AuthStackResult<Vec<AgentProfile>> {
    let mut out = Vec::new();
    for profile_id in &agent.profile_ids {
        // Gone (deleted) or archived profiles drop out; any other failure is
        // the caller's to see, not a silently shorter list.
        let view = match crate::store::fetch_tax_profile_view(profile_id, &agent.user_id).await {
            Ok(view) if !view.is_archived => view,
            Ok(_) | Err(AuthStackError::NotFound { .. }) => continue,
            Err(error) => return Err(error),
        };
        let permissions = match agent_profile_permissions(agent, profile_id).await {
            Ok(permissions) => permissions,
            Err(AuthStackError::Forbidden) => PassPermissions::empty(),
            Err(error) => return Err(error),
        };
        out.push(AgentProfile { view, permissions });
    }
    Ok(out)
}

/// One profile, when the agent may see it (`profile.view`).
pub(crate) async fn agent_tax_profile(
    agent: &AgentActor,
    profile_id: &str,
) -> AuthStackResult<AgentProfile> {
    let permissions = agent_profile_permissions(agent, profile_id).await?;
    if !permissions.contains(PassPermission::ProfileView) {
        return Err(AuthStackError::Forbidden);
    }
    let view = crate::store::fetch_tax_profile_view(profile_id, &agent.user_id).await?;
    Ok(AgentProfile { view, permissions })
}

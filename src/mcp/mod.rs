//! `POST /mcp`: the MCP endpoint for AI agents (Streamable HTTP, JSON
//! answers only, stateless).
//!
//! Every request carries an agent access token from the OAuth flow in
//! `desktop_oauth::agent`. A missing or dead token gets 401 with a
//! `WWW-Authenticate` header pointing at the RFC 9728 metadata, which is how
//! MCP clients find out where to sign in. Tools are read-only and check the
//! agent's live permissions on every call.

mod protocol;

use bytes::Bytes;
use http::{Method, StatusCode};
use http_body_util::{BodyExt, StreamBody, combinators::UnsyncBoxBody};
use serde_json::{json, Value};

use crate::domain::{PassPermission, PassPermissions};
use crate::error::{AuthStackError, AuthStackResult};
use crate::store::AgentActor;

use protocol::{Routed, TOOL_GET_PROFILE, TOOL_LIST_PROFILES};

type McpBody = UnsyncBoxBody<Bytes, std::io::Error>;
type McpResponse = http::Response<McpBody>;
type McpRequest = http::Request<wasip3::http_compat::IncomingRequestBody>;

const MAX_BODY_BYTES: usize = 256 * 1024;

pub fn is_mcp_request(path: &str) -> bool {
    path == "/mcp"
}

pub async fn serve(req: McpRequest) -> AuthStackResult<McpResponse> {
    if req.method() != Method::POST {
        // No server-sent event stream and no sessions to delete.
        return respond(
            StatusCode::METHOD_NOT_ALLOWED,
            Some(json!({"error": "use POST"})),
            &[("allow", "POST".to_owned())],
        );
    }
    // Read the body before any database call, as `server.rs` does for
    // `/api/ui`: the request body is never touched once Postgres work starts.
    let headers = req.headers().clone();
    let body = read_body(req).await?;
    let base = crate::application::public_base_url().await;
    // DNS-rebinding guard (MCP transport spec): a browser page from another
    // site must not reach this endpoint. Native clients send no Origin.
    if let Some(origin) = header(&headers, "origin")
        && origin != base
    {
        return respond(StatusCode::FORBIDDEN, Some(json!({"error": "origin not allowed"})), &[]);
    }
    if !protocol::header_version_supported(header(&headers, "mcp-protocol-version").as_deref()) {
        return respond(
            StatusCode::BAD_REQUEST,
            Some(json!({"error": "unsupported MCP-Protocol-Version", "supported": protocol::SUPPORTED_VERSIONS})),
            &[],
        );
    }
    let token = header(&headers, "authorization")
        .and_then(|value| value.strip_prefix("Bearer ").map(|t| t.trim().to_owned()));
    let agent = match token.as_deref() {
        None => return unauthorized(&base, None),
        Some(token) => match crate::application::agent_from_bearer(token).await {
            Ok(agent) => agent,
            Err(AuthStackError::InvalidToken) => return unauthorized(&base, Some("invalid_token")),
            Err(error) => return Err(error),
        },
    };
    let (messages, batch) = match protocol::parse_body(&body) {
        Ok(parsed) => parsed,
        Err(reply) => return respond(StatusCode::OK, Some(reply), &[]),
    };
    let version = env!("CARGO_PKG_VERSION");
    let mut replies = Vec::new();
    for message in &messages {
        match protocol::route(message, version) {
            Routed::Reply(reply) => replies.push(reply),
            Routed::Silent => {}
            Routed::CallTool { id, name, arguments } => {
                replies.push(call_tool(&agent, id, &name, &arguments).await);
            }
        }
    }
    match (replies.len(), batch) {
        (0, _) => respond(StatusCode::ACCEPTED, None, &[]),
        (_, true) => respond(StatusCode::OK, Some(Value::Array(replies)), &[]),
        (_, false) => respond(StatusCode::OK, replies.pop(), &[]),
    }
}

async fn call_tool(agent: &AgentActor, id: Value, name: &str, arguments: &Value) -> Value {
    let outcome = match name {
        TOOL_LIST_PROFILES => list_profiles(agent).await,
        TOOL_GET_PROFILE => match arguments.get("profile_id").and_then(Value::as_str) {
            Some(profile_id) if !profile_id.trim().is_empty() => {
                get_profile(agent, profile_id.trim()).await
            }
            _ => Err(AuthStackError::validation("profile_id is required")),
        },
        _ => Err(AuthStackError::not_found("unknown tool")),
    };
    match outcome {
        Ok(data) => protocol::tool_result(id, data),
        Err(AuthStackError::Forbidden) => protocol::tool_error(
            id,
            "Not allowed: this profile is not connected to you, or the taxpayer's access no longer covers it.",
        ),
        Err(error @ (AuthStackError::Validation { .. } | AuthStackError::NotFound { .. })) => {
            protocol::tool_error(id, &error.to_string())
        }
        Err(error) => {
            tracing::warn!(error = %error, tool = name, "MCP tool failed");
            protocol::error(id, protocol::INTERNAL_ERROR, "the tool failed; try again later")
        }
    }
}

async fn list_profiles(agent: &AgentActor) -> AuthStackResult<Value> {
    let profiles = crate::application::agent_tax_profiles(agent).await?;
    let profiles: Vec<Value> = profiles
        .into_iter()
        .map(|profile| {
            if profile.permissions.contains(PassPermission::ProfileView) {
                json!({
                    "profile_id": profile.view.profile_id,
                    "name": display_name(&profile.view),
                    "tin_last4": profile.view.tin_last4,
                    "branch_code": profile.view.branch_code,
                    "taxpayer_type": profile.view.taxpayer_type,
                    "permissions": profile.permissions.ids(),
                })
            } else {
                // Out of reach now: say so without showing whose it is.
                json!({
                    "profile_id": profile.view.profile_id,
                    "name": "(no access right now)",
                    "permissions": PassPermissions::empty().ids(),
                })
            }
        })
        .collect();
    Ok(json!({ "app_name": agent.client_name, "profiles": profiles }))
}

async fn get_profile(agent: &AgentActor, profile_id: &str) -> AuthStackResult<Value> {
    let profile = crate::application::agent_tax_profile(agent, profile_id).await?;
    let view = profile.view;
    // Internal user and organization ids stay out of the agent's view.
    Ok(json!({
        "profile_id": view.profile_id,
        "name": display_name(&view),
        "registered_name": view.full_name,
        "tin_last4": view.tin_last4,
        "branch_code": view.branch_code,
        "rdo_code": view.rdo_code,
        "address": view.address,
        "zip_code": view.zip_code,
        "line_of_business": view.line_of_business,
        "phone": view.phone,
        "email": view.email,
        "taxpayer_type": view.taxpayer_type,
        "tax_classification": view.tax_classification,
        "is_vat_registered": view.is_vat_registered,
        "held_by": if view.holder_kind == "organization" { "a firm" } else { "the taxpayer" },
        "verification_status": view.verification_status,
        "updated_at": view.updated_at,
        "permissions": profile.permissions.ids(),
    }))
}

fn display_name(view: &crate::contracts::TaxProfileView) -> String {
    if view.display_name.trim().is_empty() {
        view.full_name.clone()
    } else {
        view.display_name.clone()
    }
}

fn unauthorized(base: &str, error: Option<&str>) -> AuthStackResult<McpResponse> {
    let mut challenge = format!(
        "Bearer resource_metadata=\"{base}/.well-known/oauth-protected-resource/mcp\", scope=\"{}\"",
        crate::domain::AGENT_SCOPE
    );
    if let Some(error) = error {
        challenge.push_str(&format!(", error=\"{error}\""));
    }
    respond(
        StatusCode::UNAUTHORIZED,
        Some(json!({"error": error.unwrap_or("unauthorized")})),
        &[("www-authenticate", challenge)],
    )
}

fn header(headers: &http::HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

async fn read_body(req: McpRequest) -> AuthStackResult<Vec<u8>> {
    let (parts, incoming) = req.into_parts();
    let too_large = |status| match status {
        StatusCode::PAYLOAD_TOO_LARGE => AuthStackError::validation("request body is too large"),
        _ => AuthStackError::transport("failed to read request body"),
    };
    let body = if let Some(body) =
        crate::server_fn_http::payload_from_headers(&parts.headers).map_err(too_large)?
    {
        crate::server_fn_http::discard_incoming_body(incoming);
        body
    } else {
        crate::server_fn_http::collect_wasi_request_body(incoming, MAX_BODY_BYTES)
            .await
            .map_err(too_large)?
    };
    Ok(body.to_vec())
}

fn respond(
    status: StatusCode,
    body: Option<Value>,
    headers: &[(&str, String)],
) -> AuthStackResult<McpResponse> {
    let bytes = match &body {
        Some(value) => Bytes::from(
            serde_json::to_vec(value)
                .map_err(|error| AuthStackError::serialization(error.to_string()))?,
        ),
        None => Bytes::new(),
    };
    let stream =
        futures::stream::once(async move { Ok::<_, std::io::Error>(http_body::Frame::data(bytes)) });
    let mut builder = http::Response::builder()
        .status(status)
        .header(http::header::CACHE_CONTROL, "no-store");
    if body.is_some() {
        builder = builder.header(http::header::CONTENT_TYPE, "application/json");
    }
    for (name, value) in headers {
        builder = builder.header(*name, value);
    }
    builder
        .body(StreamBody::new(stream).boxed_unsync())
        .map_err(|error| AuthStackError::transport(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_mcp_path_is_claimed() {
        assert!(is_mcp_request("/mcp"));
        assert!(!is_mcp_request("/mcp/x"));
        assert!(!is_mcp_request("/api/mcp"));
    }
}

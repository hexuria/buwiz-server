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

use protocol::{
    Routed, TOOL_GET_DRAFT, TOOL_GET_PROFILE, TOOL_LIST_DRAFTS, TOOL_LIST_FILINGS,
    TOOL_LIST_PROFILES, TOOL_LIST_YEARS,
};

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
    let token = header(&headers, "authorization").and_then(|value| bearer(&value));
    let agent = match token.as_deref() {
        None => return unauthorized(&base, None),
        Some(token) => match crate::application::agent_from_bearer(token).await {
            Ok(agent) => agent,
            Err(AuthStackError::InvalidToken) => return unauthorized(&base, Some("invalid_token")),
            Err(error) => return Err(error),
        },
    };
    let header_version = header(&headers, "mcp-protocol-version");
    let (messages, batch) = match protocol::parse_body(&body, header_version.as_deref()) {
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
    let outcome = run_tool(agent, name, arguments).await;
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

async fn run_tool(agent: &AgentActor, name: &str, arguments: &Value) -> AuthStackResult<Value> {
    use crate::application as app;
    if name == TOOL_LIST_PROFILES {
        return list_profiles(agent).await;
    }
    let profile_id = text_arg(arguments, "profile_id")?;
    match name {
        TOOL_GET_PROFILE => get_profile(agent, profile_id).await,
        TOOL_LIST_YEARS => {
            let list = app::agent_list_tax_years(agent, profile_id).await?;
            Ok(json!({ "years": list.years }))
        }
        TOOL_LIST_DRAFTS => {
            let tax_year = year_arg(arguments)?;
            let list = app::agent_list_drafts(agent, profile_id, tax_year).await?;
            Ok(json!({ "drafts": list.drafts.iter().map(draft_summary).collect::<Vec<_>>() }))
        }
        TOOL_GET_DRAFT => {
            let draft_id = text_arg(arguments, "draft_id")?;
            let draft = app::agent_get_draft(agent, profile_id, draft_id).await?;
            let mut out = draft_summary(&draft.summary);
            out["payload"] = draft.payload;
            Ok(out)
        }
        TOOL_LIST_FILINGS => {
            let list = app::agent_list_filings(agent, profile_id).await?;
            Ok(json!({ "filings": list.filings }))
        }
        _ => Err(AuthStackError::not_found("unknown tool")),
    }
}

/// A required, non-blank string argument, trimmed.
fn text_arg<'a>(arguments: &'a Value, name: &str) -> AuthStackResult<&'a str> {
    arguments
        .get(name)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AuthStackError::validation(format!("{name} is required")))
}

/// The optional `tax_year` argument: absent, null, or a whole number.
fn year_arg(arguments: &Value) -> AuthStackResult<Option<i32>> {
    match arguments.get("tax_year") {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_i64()
            .and_then(|year| i32::try_from(year).ok())
            .map(Some)
            .ok_or_else(|| AuthStackError::validation("tax_year must be a whole number such as 2026")),
    }
}

/// A draft as an agent sees it: who saved or reviewed it (internal user
/// ids) stays out.
fn draft_summary(summary: &crate::contracts::DraftSummary) -> Value {
    json!({
        "draft_id": summary.draft_id,
        "form_code": summary.form_code,
        "tax_year": summary.tax_year,
        "period_key": summary.period_key,
        "rev": summary.rev,
        "reviewed": summary.reviewed_at.is_some(),
        "reviewed_at": summary.reviewed_at,
        "updated_at": summary.updated_at,
    })
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

/// The token of an `Authorization: Bearer <token>` value; the scheme name is
/// case-insensitive (RFC 9110 §11.1).
fn bearer(value: &str) -> Option<String> {
    let (scheme, token) = value.split_once(' ')?;
    let token = token.trim();
    (scheme.eq_ignore_ascii_case("bearer") && !token.is_empty()).then(|| token.to_owned())
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
    fn bearer_scheme_is_case_insensitive() {
        assert_eq!(bearer("Bearer bwa_x").as_deref(), Some("bwa_x"));
        assert_eq!(bearer("bearer  bwa_x ").as_deref(), Some("bwa_x"));
        assert_eq!(bearer("Basic abc"), None);
        assert_eq!(bearer("Bearer "), None);
    }

    #[test]
    fn text_arguments_are_required_and_trimmed() {
        let args = json!({"profile_id": "  p1 ", "blank": "  ", "number": 7});
        assert_eq!(text_arg(&args, "profile_id").unwrap(), "p1");
        for name in ["blank", "number", "missing"] {
            let error = text_arg(&args, name).unwrap_err();
            assert!(matches!(error, AuthStackError::Validation { .. }), "{name}");
            assert!(error.to_string().contains(name), "{error}");
        }
    }

    #[test]
    fn tax_year_is_optional_but_whole() {
        assert_eq!(year_arg(&json!({})).unwrap(), None);
        assert_eq!(year_arg(&json!({"tax_year": null})).unwrap(), None);
        assert_eq!(year_arg(&json!({"tax_year": 2026})).unwrap(), Some(2026));
        for bad in [json!("2026"), json!(2026.5), json!(i64::from(i32::MAX) + 1), json!(true)] {
            assert!(year_arg(&json!({"tax_year": bad})).is_err(), "{bad}");
        }
    }

    #[test]
    fn agents_never_see_who_saved_or_reviewed_a_draft() {
        let mut summary = crate::contracts::DraftSummary {
            draft_id: "d1".into(),
            form_code: "1701Q".into(),
            tax_year: 2026,
            period_key: "2026-Q1".into(),
            rev: 3,
            saved_once: true,
            reviewed_at: Some("2026-04-01T00:00:00.000Z".into()),
            reviewed_by: Some("11111111-1111-4111-8111-111111111111".into()),
            updated_by: Some("22222222-2222-4222-8222-222222222222".into()),
            updated_at: "2026-04-01T00:00:00.000Z".into(),
        };
        let shown = draft_summary(&summary);
        let text = shown.to_string();
        assert!(!text.contains("1111-4111") && !text.contains("2222-4222"), "{text}");
        assert_eq!(shown["reviewed"], true);
        assert_eq!(shown["rev"], 3);
        assert_eq!(shown["period_key"], "2026-Q1");
        summary.reviewed_at = None;
        assert_eq!(draft_summary(&summary)["reviewed"], false);
    }

    #[test]
    fn only_the_mcp_path_is_claimed() {
        assert!(is_mcp_request("/mcp"));
        assert!(!is_mcp_request("/mcp/x"));
        assert!(!is_mcp_request("/api/mcp"));
    }
}

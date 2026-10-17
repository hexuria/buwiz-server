//! OAuth for AI agents (Claude Code, Codex, Grok and the like) reaching
//! Buwiz through MCP.
//!
//! - `POST /oauth/register`: Dynamic Client Registration (RFC 7591), public
//!   clients only. Registering grants nothing.
//! - `GET|POST /oauth/authorize` with an `agent_…` client id: the Allow
//!   screen. The signed-in user picks which profiles the agent may reach
//!   (their own, or ones a working pass of their workspace reaches) and
//!   what it may do there.
//! - `POST /oauth/token`: PKCE code exchange and refresh. Tokens belong to
//!   the agent connection, not to the taxpayer's browser session, so ending
//!   one never signs the other out.
//! - `GET /.well-known/oauth-protected-resource[/mcp]`: RFC 9728 metadata
//!   for the MCP endpoint.

use std::collections::HashMap;

use http::StatusCode;

use crate::domain::{
    agent_permissions_from_choice, redirect_uri_matches, validate_registration, PassPermission,
    PassPreset, AGENT_ACCESS_PREFIX, AGENT_ACCESS_TTL_SECONDS, AGENT_CLIENT_PREFIX,
    AGENT_CODE_TTL_SECONDS, AGENT_REFRESH_PREFIX, AGENT_REFRESH_TTL_SECONDS, AGENT_SCOPE,
};
use crate::error::{AuthStackError, AuthStackResult};

use super::{
    html, html_escape, json_response, oauth_error, oauth_page, pkce_s256, pkce_verifier_valid,
    query_map, random_token, read_body, redirect, session_from_headers, session_from_request,
    urlencoding_encode, RestRequest, RestResponse,
};

/// Registrations allowed per hour across the server, so an open endpoint
/// cannot fill the table.
const REGISTRATIONS_PER_HOUR: i64 = 500;

pub(super) fn is_agent_client(client_id: &str) -> bool {
    client_id.starts_with(AGENT_CLIENT_PREFIX)
}

pub(super) fn is_agent_refresh_token(token: &str) -> bool {
    token.starts_with(AGENT_REFRESH_PREFIX)
}

pub(super) async fn protected_resource_metadata() -> AuthStackResult<RestResponse> {
    let issuer = crate::application::public_base_url().await;
    json_response(
        StatusCode::OK,
        &serde_json::json!({
            "resource": format!("{issuer}/mcp"),
            "resource_name": "Buwiz tax profiles",
            "authorization_servers": [issuer],
            "bearer_methods_supported": ["header"],
            "scopes_supported": [AGENT_SCOPE],
        }),
    )
}

#[derive(serde::Deserialize)]
struct RegistrationRequest {
    #[serde(default)]
    client_name: Option<String>,
    #[serde(default)]
    redirect_uris: Vec<String>,
    #[serde(default)]
    token_endpoint_auth_method: Option<String>,
    #[serde(default)]
    grant_types: Option<Vec<String>>,
    #[serde(default)]
    response_types: Option<Vec<String>>,
}

pub(super) async fn register(req: RestRequest) -> AuthStackResult<RestResponse> {
    let body = read_body(req).await?;
    let Ok(request) = serde_json::from_slice::<RegistrationRequest>(&body) else {
        return oauth_error(
            StatusCode::BAD_REQUEST,
            "invalid_client_metadata",
            "body must be a JSON client registration",
        );
    };
    let unsupported_grant = request.grant_types.as_ref().is_some_and(|grants| {
        grants
            .iter()
            .any(|grant| !matches!(grant.as_str(), "authorization_code" | "refresh_token"))
    });
    let unsupported_response = request
        .response_types
        .as_ref()
        .is_some_and(|types| types.iter().any(|kind| kind != "code"));
    if unsupported_grant || unsupported_response {
        return oauth_error(
            StatusCode::BAD_REQUEST,
            "invalid_client_metadata",
            "only the authorization_code and refresh_token grants with response_type code are supported",
        );
    }
    let registration = match validate_registration(
        request.client_name.as_deref(),
        &request.redirect_uris,
        request.token_endpoint_auth_method.as_deref(),
    ) {
        Ok(registration) => registration,
        Err(message) => {
            let code = if message.contains("redirect_uri") {
                "invalid_redirect_uri"
            } else {
                "invalid_client_metadata"
            };
            return oauth_error(StatusCode::BAD_REQUEST, code, &message);
        }
    };
    crate::store::sweep_unused_agent_clients().await?;
    if crate::store::agent_registrations_last_hour().await? >= REGISTRATIONS_PER_HOUR {
        return oauth_error(
            StatusCode::TOO_MANY_REQUESTS,
            "temporarily_unavailable",
            "too many registrations, try again later",
        );
    }
    let client_id = format!("{AGENT_CLIENT_PREFIX}{}", random_token(18)?);
    crate::store::insert_agent_client(
        &client_id,
        &registration.client_name,
        &registration.redirect_uris,
    )
    .await?;
    json_response(
        StatusCode::CREATED,
        &serde_json::json!({
            "client_id": client_id,
            "client_id_issued_at": unix_now(),
            "client_name": registration.client_name,
            "redirect_uris": registration.redirect_uris,
            "token_endpoint_auth_method": "none",
            "grant_types": ["authorization_code", "refresh_token"],
            "response_types": ["code"],
            "scope": AGENT_SCOPE,
        }),
    )
}

/// The checked parts of an agent's authorize request.
struct AgentAuthorize {
    client: crate::store::StoredAgentClient,
    redirect_uri: String,
    state: String,
    challenge: String,
}

async fn check_authorize(params: &HashMap<String, String>) -> Result<AgentAuthorize, String> {
    let get = |key: &str| params.get(key).cloned().unwrap_or_default();
    let client_id = get("client_id");
    let client = crate::store::load_agent_client(&client_id)
        .await
        .map_err(|_| "could not look up the app".to_owned())?
        .ok_or_else(|| "this app is not registered".to_owned())?;
    let redirect_uri = get("redirect_uri");
    if !redirect_uri_matches(&client.redirect_uris, &redirect_uri) {
        return Err("redirect_uri was not registered by this app".to_owned());
    }
    if get("response_type") != "code" {
        return Err("response_type must be code".to_owned());
    }
    if get("code_challenge_method") != "S256" {
        return Err("code_challenge_method must be S256".to_owned());
    }
    let challenge = get("code_challenge");
    if challenge.len() < 43 {
        return Err("code_challenge is required".to_owned());
    }
    let resource = get("resource");
    if !resource.is_empty() {
        let issuer = crate::application::public_base_url().await;
        if resource != format!("{issuer}/mcp") && resource != issuer {
            return Err("resource is not this server's MCP endpoint".to_owned());
        }
    }
    Ok(AgentAuthorize {
        client,
        redirect_uri,
        state: get("state"),
        challenge,
    })
}

pub(super) async fn authorize_get(
    req: &RestRequest,
    uri: &http::Uri,
) -> AuthStackResult<RestResponse> {
    let params = query_map(uri.query().unwrap_or_default());
    let checked = match check_authorize(&params).await {
        Ok(checked) => checked,
        Err(message) => {
            return html(
                StatusCode::BAD_REQUEST,
                &oauth_page("Cannot continue", &message, ""),
            );
        }
    };
    let Some(session) = session_from_request(req).await? else {
        let next = format!("/oauth/authorize?{}", uri.query().unwrap_or_default());
        return redirect(&format!("/login?next={}", urlencoding_encode(&next)));
    };
    let user_id = session.user_id.clone().ok_or(AuthStackError::AuthRequired)?;
    let profiles = crate::store::list_connectable_profiles(&user_id).await?;
    if profiles.is_empty() {
        return html(
            StatusCode::OK,
            &oauth_page(
                "No profiles to connect",
                "You can connect an agent to tax profiles you own, or that a workspace of yours has a working pass on. Add or claim one first.",
                "<p><a href=\"/tax-profiles\" style=\"color:#fff\">Go to Tax profiles</a></p>",
            ),
        );
    }
    let session_id = session.session_id.as_deref().ok_or(AuthStackError::AuthRequired)?;
    let csrf = crate::store::csrf_token_for_session(session_id).await?;
    let only_one = profiles.len() == 1;
    let profile_boxes: String = profiles
        .iter()
        .map(|profile| {
            format!(
                r#"<label class="row"><input type="checkbox" name="profile" value="{id}"{checked} /> {name} <span class="dim">TIN ···{last4}{via}</span></label>"#,
                id = html_escape(&profile.profile_id),
                checked = if only_one { " checked" } else { "" },
                name = html_escape(&profile.profile_name),
                last4 = html_escape(&profile.tin_last4),
                via = profile
                    .via
                    .as_deref()
                    .map(|firm| format!(" · through {}'s pass", html_escape(firm)))
                    .unwrap_or_default(),
            )
        })
        .collect();
    let presets: String = PassPreset::ALL
        .into_iter()
        .map(|preset| {
            let list = preset
                .permissions()
                .iter()
                .map(PassPermission::label)
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                r#"<label class="row"><input type="radio" name="preset" value="{value}"{checked} /> <strong>{label}</strong> <span class="dim">{list}</span></label>"#,
                value = preset.as_str(),
                checked = if preset == PassPreset::ReadOnly { " checked" } else { "" },
                label = html_escape(preset.label()),
                list = html_escape(&list),
            )
        })
        .collect();
    let hidden = |name: &str, value: &str| {
        format!(
            r#"<input type="hidden" name="{name}" value="{value}" />"#,
            value = html_escape(value)
        )
    };
    let body = format!(
        r#"<style>.row{{display:flex;gap:.5rem;align-items:baseline;margin:.45rem 0;font-size:.95rem}} .row input{{width:auto;margin:0}} .dim{{color:#9a9a9a;font-size:.85rem}} fieldset{{border:1px solid #333;border-radius:.5rem;margin:0 0 1rem;padding:.6rem .8rem}} legend{{font-size:.85rem;color:#c4c4c4}} .deny{{background:transparent;color:#fff;border:1px solid #444;margin-left:.5rem}}</style>
<form method="post" action="/oauth/authorize">
{client}{redirect}{state}{challenge}<input type="hidden" name="code_challenge_method" value="S256" />{csrf}
<fieldset><legend>Profiles it may reach</legend>{profile_boxes}</fieldset>
<fieldset><legend>What it may do</legend>{presets}<p class="dim">An agent can never publish to BIR, share profiles, archive or delete them. It never gets more than you have (on a firm's pass, no more than that pass), and it stops the moment you end it, the owner ends the pass, or you leave the workspace.</p></fieldset>
<p class="dim">Signed in as {user}. You go back to {target} after you choose. You can end this any time under Tax profiles → Connected agents.</p>
<button type="submit" name="decision" value="allow">Allow</button><button class="deny" type="submit" name="decision" value="deny">Deny</button>
</form>"#,
        client = hidden("client_id", &checked.client.client_id),
        redirect = hidden("redirect_uri", &checked.redirect_uri),
        state = hidden("state", &checked.state),
        challenge = hidden("code_challenge", &checked.challenge),
        csrf = hidden("csrf", &csrf),
        user = html_escape(session.primary_email.as_deref().unwrap_or("your account")),
        target = html_escape(&redirect_target(&checked.redirect_uri)),
    );
    html(
        StatusCode::OK,
        &oauth_page(
            &format!("Allow {} to work on tax profiles for you?", checked.client.client_name),
            &format!(
                "\u{201c}{}\u{201d} named itself; Buwiz cannot confirm who made it. Allow only an app you just started yourself.",
                checked.client.client_name
            ),
            &body,
        ),
    )
}

pub(super) async fn authorize_post(
    headers: &http::HeaderMap,
    body: Vec<u8>,
) -> AuthStackResult<RestResponse> {
    let session = session_from_headers(headers)
        .await?
        .ok_or(AuthStackError::AuthRequired)?;
    let session_id = session
        .session_id
        .clone()
        .ok_or(AuthStackError::AuthRequired)?;
    let user_id = session.user_id.clone().ok_or(AuthStackError::AuthRequired)?;
    let pairs = form_pairs(body);
    // First value wins, as in the dispatcher that routed this form here.
    let mut single: HashMap<String, String> = HashMap::new();
    for (key, value) in &pairs {
        single.entry(key.clone()).or_insert_with(|| value.clone());
    }
    crate::application::validate_csrf_token_for_session(
        Some(session_id),
        single.get("csrf").cloned(),
    )
    .await?;
    let mut params = single.clone();
    params.insert("response_type".to_owned(), "code".to_owned());
    let checked = check_authorize(&params)
        .await
        .map_err(AuthStackError::validation)?;
    if single.get("decision").map(String::as_str) != Some("allow") {
        return redirect(&with_query(
            &checked.redirect_uri,
            &[("error", "access_denied"), ("state", &checked.state)],
        ));
    }
    // Only profiles the user may hand on can be connected. The store checks
    // again inside the transaction; filtering here keeps a real database
    // error from being reported as "nothing to connect".
    let reachable = crate::store::list_connectable_profiles(&user_id).await?;
    let profile_ids: Vec<String> = pairs
        .iter()
        .filter(|(key, _)| key == "profile")
        .map(|(_, value)| value.clone())
        .filter(|id| reachable.iter().any(|profile| &profile.profile_id == id))
        .collect();
    if profile_ids.is_empty() {
        return html(
            StatusCode::BAD_REQUEST,
            &oauth_page(
                "Pick at least one profile",
                "Go back and tick the profiles the agent may reach.",
                "",
            ),
        );
    }
    let preset = single
        .get("preset")
        .and_then(|raw| PassPreset::parse(raw))
        .unwrap_or(PassPreset::ReadOnly);
    let permissions =
        agent_permissions_from_choice(&preset.permissions().ids()).map_err(AuthStackError::validation)?;
    let code = random_token(32)?;
    let connection_id = crate::store::new_uuid_v4()?;
    crate::store::create_agent_connection(
        &connection_id,
        &user_id,
        &checked.client.client_id,
        permissions,
        &profile_ids,
        &crate::store::sha256_b64url(&code),
        &checked.redirect_uri,
        &checked.challenge,
        AGENT_CODE_TTL_SECONDS,
    )
    .await?;
    redirect(&with_query(
        &checked.redirect_uri,
        &[("code", &code), ("state", &checked.state)],
    ))
}

pub(super) async fn authorization_code_grant(
    form: &HashMap<String, String>,
) -> AuthStackResult<RestResponse> {
    let get = |key: &str| form.get(key).cloned().unwrap_or_default();
    let verifier = get("code_verifier");
    if !pkce_verifier_valid(&verifier) {
        return oauth_error(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "code_verifier is invalid",
        );
    }
    let Some(stored) = crate::store::consume_agent_code(&crate::store::sha256_b64url(&get("code"))).await? else {
        return oauth_error(
            StatusCode::BAD_REQUEST,
            "invalid_grant",
            "authorization code is invalid or expired",
        );
    };
    if stored.client_id != get("client_id") || stored.redirect_uri != get("redirect_uri") {
        return oauth_error(
            StatusCode::BAD_REQUEST,
            "invalid_grant",
            "client_id or redirect_uri does not match",
        );
    }
    if pkce_s256(&verifier) != stored.code_challenge {
        return oauth_error(
            StatusCode::BAD_REQUEST,
            "invalid_grant",
            "PKCE verification failed",
        );
    }
    let (access, refresh) = new_token_pair()?;
    crate::store::insert_agent_tokens(
        &stored.connection_id,
        &crate::store::sha256_b64url(&access),
        AGENT_ACCESS_TTL_SECONDS,
        &crate::store::sha256_b64url(&refresh),
        AGENT_REFRESH_TTL_SECONDS,
    )
    .await?;
    token_response(&access, &refresh)
}

pub(super) async fn refresh_grant(
    refresh: &str,
    form: &HashMap<String, String>,
) -> AuthStackResult<RestResponse> {
    // Public clients must name themselves (RFC 6749 §3.2.1); the refresh
    // token only works for the app it was issued to.
    let client_id = form.get("client_id").map(String::as_str).unwrap_or_default();
    if !is_agent_client(client_id) {
        return oauth_error(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "client_id is required",
        );
    }
    let (access, new_refresh) = new_token_pair()?;
    let rotated = crate::store::rotate_agent_refresh(
        &crate::store::sha256_b64url(refresh),
        client_id,
        &crate::store::sha256_b64url(&access),
        AGENT_ACCESS_TTL_SECONDS,
        &crate::store::sha256_b64url(&new_refresh),
        AGENT_REFRESH_TTL_SECONDS,
    )
    .await?;
    if rotated.is_none() {
        return oauth_error(
            StatusCode::BAD_REQUEST,
            "invalid_grant",
            "refresh token is invalid, expired or the connection ended",
        );
    }
    token_response(&access, &new_refresh)
}

fn new_token_pair() -> AuthStackResult<(String, String)> {
    Ok((
        format!("{AGENT_ACCESS_PREFIX}{}", random_token(32)?),
        format!("{AGENT_REFRESH_PREFIX}{}", random_token(32)?),
    ))
}

fn token_response(access: &str, refresh: &str) -> AuthStackResult<RestResponse> {
    // Never log token values.
    json_response(
        StatusCode::OK,
        &serde_json::json!({
            "access_token": access,
            "refresh_token": refresh,
            "token_type": "Bearer",
            "expires_in": AGENT_ACCESS_TTL_SECONDS,
            "scope": AGENT_SCOPE,
        }),
    )
}

fn form_pairs(body: Vec<u8>) -> Vec<(String, String)> {
    form_urlencoded::parse(&body).into_owned().collect()
}

fn with_query(base: &str, pairs: &[(&str, &str)]) -> String {
    let mut out = base.to_owned();
    let mut separator = if base.contains('?') { '&' } else { '?' };
    for (key, value) in pairs {
        if value.is_empty() {
            continue;
        }
        out.push(separator);
        out.push_str(key);
        out.push('=');
        out.push_str(&urlencoding_encode(value));
        separator = '&';
    }
    out
}

/// Where the browser goes after the choice, for the Allow screen: the host
/// for web callbacks, "this computer" for loopback ones.
fn redirect_target(uri: &str) -> String {
    let rest = uri.split_once("://").map_or(uri, |(_, rest)| rest);
    let host = rest.split(['/', '?']).next().unwrap_or(rest);
    let bare = host.rsplit_once(':').map_or(host, |(h, port)| {
        if port.chars().all(|ch| ch.is_ascii_digit()) { h } else { host }
    });
    if matches!(bare, "127.0.0.1" | "localhost" | "[::1]") {
        "the app on this computer".to_owned()
    } else if uri.starts_with("https://") {
        bare.to_owned()
    } else {
        uri.split(':').next().map_or_else(String::new, |scheme| format!("the {scheme}: app"))
    }
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_is_appended_and_empty_state_skipped() {
        assert_eq!(
            with_query("http://127.0.0.1:9/cb", &[("code", "a b"), ("state", "")]),
            "http://127.0.0.1:9/cb?code=a+b"
        );
        assert_eq!(
            with_query("https://x.test/cb?k=1", &[("error", "access_denied"), ("state", "s")]),
            "https://x.test/cb?k=1&error=access_denied&state=s"
        );
    }

    #[test]
    fn redirect_target_names_where_the_code_goes() {
        assert_eq!(redirect_target("http://127.0.0.1:5555/cb"), "the app on this computer");
        assert_eq!(
            redirect_target("https://claude.ai/api/mcp/auth_callback"),
            "claude.ai"
        );
        assert_eq!(redirect_target("cursor://x/cb"), "the cursor: app");
    }

    #[test]
    fn agent_ids_and_tokens_are_recognized() {
        assert!(is_agent_client("agent_abc"));
        assert!(!is_agent_client("buwiz-desktop"));
        assert!(is_agent_refresh_token("bwr_abc"));
        assert!(!is_agent_refresh_token("eyJhbGciOi"));
    }
}

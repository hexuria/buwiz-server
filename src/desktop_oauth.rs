//! First-party OAuth 2.1 (PKCE) + RFC 8628 device-code for the Buwiz desktop app.

use bytes::Bytes;
use http::{Method, StatusCode};
use http_body_util::{BodyExt, StreamBody, combinators::UnsyncBoxBody};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::domain::{
    desktop_redirect_uri_allowed, DESKTOP_CLIENT_ID, DESKTOP_SCOPE,
};
use crate::error::{AuthStackError, AuthStackResult};

type RestBody = UnsyncBoxBody<Bytes, std::io::Error>;
type RestResponse = http::Response<RestBody>;
type RestRequest = http::Request<wasip3::http_compat::IncomingRequestBody>;

const AUTH_CODE_TTL_SECONDS: i64 = 600;
const DEVICE_TTL_SECONDS: i64 = 900;
const DEVICE_INTERVAL_SECONDS: i64 = 5;

pub fn is_desktop_oauth_request(path: &str) -> bool {
    matches!(
        path,
        "/.well-known/oauth-authorization-server"
            | "/oauth/authorize"
            | "/oauth/token"
            | "/oauth/device/code"
            | "/login/device"
    )
}

pub async fn serve(req: RestRequest) -> AuthStackResult<RestResponse> {
    let method = req.method().clone();
    let uri = req.uri().clone();
    let path = uri.path().to_string();
    match (method, path.as_str()) {
        (Method::GET, "/.well-known/oauth-authorization-server") => {
            json_response(StatusCode::OK, &authorization_server_metadata().await)
        }
        (Method::GET, "/oauth/authorize") => authorize_get(&req, &uri).await,
        (Method::POST, "/oauth/authorize") => authorize_post(req).await,
        (Method::POST, "/oauth/token") => token(req).await,
        (Method::POST, "/oauth/device/code") => device_code(req).await,
        (Method::GET, "/login/device") => device_get(&req, &uri).await,
        (Method::POST, "/login/device") => device_post(req).await,
        (Method::OPTIONS, _) => empty(StatusCode::NO_CONTENT),
        _ => json_response(
            StatusCode::METHOD_NOT_ALLOWED,
            &serde_json::json!({"error": "invalid_request", "error_description": "method is not allowed"}),
        ),
    }
}

async fn authorization_server_metadata() -> serde_json::Value {
    let issuer = crate::application::public_base_url().await;
    serde_json::json!({
        "issuer": issuer,
        "authorization_endpoint": format!("{issuer}/oauth/authorize"),
        "token_endpoint": format!("{issuer}/oauth/token"),
        "device_authorization_endpoint": format!("{issuer}/oauth/device/code"),
        "revocation_endpoint": format!("{issuer}/api/auth/logout"),
        "jwks_uri": format!("{issuer}/api/auth/.well-known/jwks.json"),
        "response_types_supported": ["code"],
        "grant_types_supported": [
            "authorization_code",
            "refresh_token",
            "urn:ietf:params:oauth:grant-type:device_code"
        ],
        "code_challenge_methods_supported": ["S256"],
        "token_endpoint_auth_methods_supported": ["none"],
        "scopes_supported": DESKTOP_SCOPE.split_whitespace().collect::<Vec<_>>(),
        "client_id_issued": DESKTOP_CLIENT_ID,
    })
}

async fn authorize_get(req: &RestRequest, uri: &http::Uri) -> AuthStackResult<RestResponse> {
    let params = query_map(uri.query().unwrap_or_default());
    let client_id = params.get("client_id").cloned().unwrap_or_default();
    let redirect_uri = params.get("redirect_uri").cloned().unwrap_or_default();
    let state = params.get("state").cloned().unwrap_or_default();
    let challenge = params.get("code_challenge").cloned().unwrap_or_default();
    let method = params
        .get("code_challenge_method")
        .cloned()
        .unwrap_or_default();
    let response_type = params.get("response_type").cloned().unwrap_or_default();
    if let Err(message) = validate_authorize_params(
        &client_id,
        &redirect_uri,
        &challenge,
        &method,
        &response_type,
    )
    .await
    {
        return html(
            StatusCode::BAD_REQUEST,
            &oauth_page("Cannot continue", &html_escape(&message), ""),
        );
    }
    let Some(session) = session_from_request(req).await? else {
        let next = format!(
            "/oauth/authorize?{}",
            uri.query().unwrap_or_default()
        );
        return redirect(&format!(
            "/login?next={}",
            urlencoding_encode(&next)
        ));
    };
    let user_label = session
        .primary_email
        .unwrap_or_else(|| "signed-in user".to_owned());
    let csrf = crate::store::csrf_token_for_session(
        session.session_id.as_deref().unwrap_or_default(),
    )
    .await
    .unwrap_or_default();
    let body = format!(
        r#"<form method="post" action="/oauth/authorize">
<input type="hidden" name="client_id" value="{client_id}" />
<input type="hidden" name="redirect_uri" value="{redirect}" />
<input type="hidden" name="state" value="{state}" />
<input type="hidden" name="code_challenge" value="{challenge}" />
<input type="hidden" name="code_challenge_method" value="S256" />
<input type="hidden" name="csrf" value="{csrf}" />
<p>Authorize <strong>Buwiz desktop</strong> to sign in as {user} and access tax profiles.</p>
<button type="submit">Authorize desktop app</button>
</form>"#,
        client_id = html_escape(&client_id),
        redirect = html_escape(&redirect_uri),
        state = html_escape(&state),
        challenge = html_escape(&challenge),
        csrf = html_escape(&csrf),
        user = html_escape(&user_label),
    );
    html(
        StatusCode::OK,
        &oauth_page("Authorize Buwiz desktop", "This native client uses PKCE. No client secret is issued.", &body),
    )
}

async fn authorize_post(req: RestRequest) -> AuthStackResult<RestResponse> {
    let session = session_from_request(&req)
        .await?
        .ok_or(AuthStackError::AuthRequired)?;
    let session_id = session
        .session_id
        .clone()
        .ok_or(AuthStackError::AuthRequired)?;
    let user_id = session.user_id.ok_or(AuthStackError::AuthRequired)?;
    let form = parse_form(req).await?;
    let csrf = form.get("csrf").cloned().unwrap_or_default();
    crate::application::validate_csrf_token_for_session(Some(session_id.clone()), Some(csrf))
        .await?;
    let client_id = form.get("client_id").cloned().unwrap_or_default();
    let redirect_uri = form.get("redirect_uri").cloned().unwrap_or_default();
    let state = form.get("state").cloned().unwrap_or_default();
    let challenge = form.get("code_challenge").cloned().unwrap_or_default();
    let method = form
        .get("code_challenge_method")
        .cloned()
        .unwrap_or_else(|| "S256".to_owned());
    validate_authorize_params(&client_id, &redirect_uri, &challenge, &method, "code")
        .await
        .map_err(AuthStackError::validation)?;
    let code = random_token(32)?;
    crate::store::insert_auth_code(
        &sha256_b64(&code),
        &client_id,
        &user_id,
        &session_id,
        &redirect_uri,
        &challenge,
        DESKTOP_SCOPE,
        AUTH_CODE_TTL_SECONDS,
    )
    .await?;
    let mut location = format!(
        "{}{}code={}",
        redirect_uri,
        if redirect_uri.contains('?') { "&" } else { "?" },
        urlencoding_encode(&code)
    );
    if !state.is_empty() {
        location.push_str("&state=");
        location.push_str(&urlencoding_encode(&state));
    }
    redirect(&location)
}

async fn token(req: RestRequest) -> AuthStackResult<RestResponse> {
    let form = parse_form_or_json(req).await?;
    let grant = form.get("grant_type").cloned().unwrap_or_default();
    match grant.as_str() {
        "authorization_code" => authorization_code_grant(&form).await,
        "refresh_token" => refresh_grant(&form).await,
        "urn:ietf:params:oauth:grant-type:device_code" => device_grant(&form).await,
        _ => oauth_error(
            StatusCode::BAD_REQUEST,
            "unsupported_grant_type",
            "grant_type is not supported",
        ),
    }
}

async fn authorization_code_grant(
    form: &std::collections::HashMap<String, String>,
) -> AuthStackResult<RestResponse> {
    let client_id = form.get("client_id").cloned().unwrap_or_default();
    let code = form.get("code").cloned().unwrap_or_default();
    let redirect_uri = form.get("redirect_uri").cloned().unwrap_or_default();
    let verifier = form.get("code_verifier").cloned().unwrap_or_default();
    if client_id != DESKTOP_CLIENT_ID {
        return oauth_error(
            StatusCode::BAD_REQUEST,
            "invalid_client",
            "client_id is unknown",
        );
    }
    if !pkce_verifier_valid(&verifier) {
        return oauth_error(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "code_verifier is invalid",
        );
    }
    let stored = match crate::store::consume_auth_code(&sha256_b64(&code)).await {
        Ok(stored) => stored,
        Err(_) => {
            return oauth_error(
                StatusCode::BAD_REQUEST,
                "invalid_grant",
                "authorization code is invalid or expired",
            );
        }
    };
    if stored.client_id != client_id || stored.redirect_uri != redirect_uri {
        return oauth_error(
            StatusCode::BAD_REQUEST,
            "invalid_grant",
            "redirect_uri does not match",
        );
    }
    if pkce_s256(&verifier) != stored.code_challenge {
        return oauth_error(
            StatusCode::BAD_REQUEST,
            "invalid_grant",
            "PKCE verification failed",
        );
    }
    token_from_session(&stored.session_id).await
}

async fn refresh_grant(
    form: &std::collections::HashMap<String, String>,
) -> AuthStackResult<RestResponse> {
    let refresh = form.get("refresh_token").cloned().unwrap_or_default();
    if refresh.is_empty() {
        return oauth_error(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "refresh_token is required",
        );
    }
    let tokens = crate::application::refresh_token_for(None, Some(refresh)).await?;
    json_response(
        StatusCode::OK,
        &serde_json::json!({
            "access_token": tokens.access_token,
            "refresh_token": tokens.refresh_token,
            "token_type": "Bearer",
            "expires_in": tokens.expires_in_seconds,
            "scope": DESKTOP_SCOPE,
        }),
    )
}

async fn device_grant(
    form: &std::collections::HashMap<String, String>,
) -> AuthStackResult<RestResponse> {
    let client_id = form.get("client_id").cloned().unwrap_or_default();
    let device_code = form.get("device_code").cloned().unwrap_or_default();
    if client_id != DESKTOP_CLIENT_ID {
        return oauth_error(
            StatusCode::BAD_REQUEST,
            "invalid_client",
            "client_id is unknown",
        );
    }
    let hashed = sha256_b64(&device_code);
    let pending = match crate::store::poll_device_code(&hashed).await {
        Ok(pending) => pending,
        Err(_) => {
            return oauth_error(
                StatusCode::BAD_REQUEST,
                "expired_token",
                "device code is unknown or expired",
            );
        }
    };
    if pending.client_id != client_id {
        return oauth_error(
            StatusCode::BAD_REQUEST,
            "invalid_grant",
            "client_id does not match",
        );
    }
    if !pending.authorized {
        return oauth_error(
            StatusCode::BAD_REQUEST,
            "authorization_pending",
            "the user has not authorized the device yet",
        );
    }
    let consumed = crate::store::consume_device_code(&hashed).await?;
    let session_id = consumed
        .session_id
        .ok_or(AuthStackError::InvalidToken)?;
    token_from_session(&session_id).await
}

async fn device_code(req: RestRequest) -> AuthStackResult<RestResponse> {
    let form = parse_form_or_json(req).await?;
    let client_id = form
        .get("client_id")
        .cloned()
        .unwrap_or_else(|| DESKTOP_CLIENT_ID.to_owned());
    if client_id != DESKTOP_CLIENT_ID {
        return oauth_error(
            StatusCode::BAD_REQUEST,
            "invalid_client",
            "client_id is unknown",
        );
    }
    let device_code = random_token(32)?;
    let user_code = random_user_code()?;
    let issuer = crate::application::public_base_url().await;
    let verification_uri = format!("{issuer}/login/device");
    crate::store::insert_device_code(
        &sha256_b64(&device_code),
        &user_code,
        &client_id,
        &verification_uri,
        DEVICE_INTERVAL_SECONDS,
        DEVICE_TTL_SECONDS,
    )
    .await?;
    json_response(
        StatusCode::OK,
        &serde_json::json!({
            "device_code": device_code,
            "user_code": user_code,
            "verification_uri": verification_uri,
            "verification_uri_complete": format!("{verification_uri}?user_code={user_code}"),
            "expires_in": DEVICE_TTL_SECONDS,
            "interval": DEVICE_INTERVAL_SECONDS,
        }),
    )
}

async fn device_get(req: &RestRequest, uri: &http::Uri) -> AuthStackResult<RestResponse> {
    let Some(session) = session_from_request(req).await? else {
        let next = format!("/login/device?{}", uri.query().unwrap_or_default());
        return redirect(&format!(
            "/login?next={}",
            urlencoding_encode(&next)
        ));
    };
    let params = query_map(uri.query().unwrap_or_default());
    let user_code = params.get("user_code").cloned().unwrap_or_default();
    let csrf = crate::store::csrf_token_for_session(
        session.session_id.as_deref().unwrap_or_default(),
    )
    .await
    .unwrap_or_default();
    let body = format!(
        r#"<form method="post" action="/login/device">
<input type="hidden" name="csrf" value="{csrf}" />
<label>User code
<input name="user_code" value="{code}" autocomplete="one-time-code" required />
</label>
<button type="submit">Authorize this device</button>
</form>"#,
        csrf = html_escape(&csrf),
        code = html_escape(&user_code),
    );
    html(
        StatusCode::OK,
        &oauth_page(
            "Authorize a device",
            "Enter the code shown in the Buwiz desktop app. You stay in control of the submit.",
            &body,
        ),
    )
}

async fn device_post(req: RestRequest) -> AuthStackResult<RestResponse> {
    let session = session_from_request(&req)
        .await?
        .ok_or(AuthStackError::AuthRequired)?;
    let session_id = session
        .session_id
        .clone()
        .ok_or(AuthStackError::AuthRequired)?;
    let user_id = session.user_id.ok_or(AuthStackError::AuthRequired)?;
    let form = parse_form(req).await?;
    crate::application::validate_csrf_token_for_session(
        Some(session_id.clone()),
        form.get("csrf").cloned(),
    )
    .await?;
    let user_code = form.get("user_code").cloned().unwrap_or_default();
    crate::store::authorize_device_code(&user_code, &user_id, &session_id).await?;
    html(
        StatusCode::OK,
        &oauth_page(
            "Device authorized",
            "Return to the Buwiz desktop app. It will finish signing in automatically.",
            "<p>You can close this tab.</p>",
        ),
    )
}

async fn token_from_session(session_id: &str) -> AuthStackResult<RestResponse> {
    let session_id = wasi_auth::context::SessionId::new(session_id.to_owned())
        .map_err(|_| AuthStackError::AuthRequired)?;
    let (access_token, refresh_token, expires_in) =
        crate::auth_product::issue_tokens(&session_id).await?;
    json_response(
        StatusCode::OK,
        &serde_json::json!({
            "access_token": access_token,
            "refresh_token": refresh_token,
            "token_type": "Bearer",
            "expires_in": expires_in,
            "scope": DESKTOP_SCOPE,
        }),
    )
}

async fn validate_authorize_params(
    client_id: &str,
    redirect_uri: &str,
    challenge: &str,
    method: &str,
    response_type: &str,
) -> Result<(), String> {
    if client_id != DESKTOP_CLIENT_ID {
        return Err("unknown client_id (use buwiz-desktop)".to_owned());
    }
    if response_type != "code" {
        return Err("response_type must be code".to_owned());
    }
    if method != "S256" {
        return Err("code_challenge_method must be S256".to_owned());
    }
    if challenge.len() < 43 {
        return Err("code_challenge is required".to_owned());
    }
    let extra = extra_redirects().await;
    if !desktop_redirect_uri_allowed(redirect_uri, &extra) {
        return Err("redirect_uri is not allowed for the desktop client".to_owned());
    }
    Ok(())
}

async fn extra_redirects() -> Vec<String> {
    crate::application::config_value("DESKTOP_OAUTH_REDIRECT_URIS")
        .await
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

async fn session_from_request(
    req: &RestRequest,
) -> AuthStackResult<Option<crate::contracts::SessionView>> {
    let Some(session_id) = cookie_session_id(req) else {
        return Ok(None);
    };
    let session = crate::application::get_current_session_for(Some(session_id)).await?;
    if session.authenticated {
        Ok(Some(session))
    } else {
        Ok(None)
    }
}

fn cookie_session_id(req: &RestRequest) -> Option<String> {
    req.headers()
        .get(http::header::COOKIE)
        .and_then(|value| value.to_str().ok())
        .and_then(|header| {
            header.split(';').find_map(|part| {
                let (name, value) = part.trim().split_once('=')?;
                matches!(name, "__Host-session" | "wasi_auth_dev_session")
                    .then(|| value.trim().to_string())
                    .filter(|value| !value.is_empty())
            })
        })
}

async fn parse_form(req: RestRequest) -> AuthStackResult<std::collections::HashMap<String, String>> {
    let body = read_body(req).await?;
    Ok(form_urlencoded::parse(&body)
        .into_owned()
        .collect())
}

async fn parse_form_or_json(
    req: RestRequest,
) -> AuthStackResult<std::collections::HashMap<String, String>> {
    let content_type = req
        .headers()
        .get(http::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase();
    if content_type.contains("application/json") {
        let body = read_body(req).await?;
        let value: serde_json::Value = serde_json::from_slice(&body)
            .map_err(|error| AuthStackError::validation(format!("invalid JSON: {error}")))?;
        let mut map = std::collections::HashMap::new();
        if let Some(object) = value.as_object() {
            for (key, value) in object {
                if let Some(text) = value.as_str() {
                    map.insert(key.clone(), text.to_owned());
                } else if !value.is_null() {
                    map.insert(key.clone(), value.to_string());
                }
            }
        }
        return Ok(map);
    }
    parse_form(req).await
}

async fn read_body(req: RestRequest) -> AuthStackResult<Vec<u8>> {
    let mut incoming = req.into_body();
    let mut body = Vec::new();
    while let Some(frame) = incoming.frame().await {
        let frame = frame.map_err(|error| {
            AuthStackError::transport(format!("failed to read request body: {error:?}"))
        })?;
        let Ok(data) = frame.into_data() else {
            continue;
        };
        if body.len().saturating_add(data.len()) > 64 * 1024 {
            return Err(AuthStackError::validation("request body is too large"));
        }
        body.extend_from_slice(&data);
    }
    Ok(body)
}

fn query_map(query: &str) -> std::collections::HashMap<String, String> {
    form_urlencoded::parse(query.as_bytes())
        .into_owned()
        .collect()
}

fn pkce_s256(verifier: &str) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

fn pkce_verifier_valid(verifier: &str) -> bool {
    (43..=128).contains(&verifier.len())
        && verifier
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '.' | '_' | '~'))
}

fn sha256_b64(value: &str) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(Sha256::digest(value.as_bytes()))
}

fn random_token(len: usize) -> AuthStackResult<String> {
    use base64::Engine as _;
    let mut bytes = vec![0u8; len];
    getrandom::getrandom(&mut bytes)
        .map_err(|error| AuthStackError::store(format!("entropy unavailable: {error}")))?;
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes))
}

fn random_user_code() -> AuthStackResult<String> {
    const ALPH: &[u8] = b"BCDFGHJKLMNPQRSTVWXZ23456789";
    let mut bytes = [0u8; 8];
    getrandom::getrandom(&mut bytes)
        .map_err(|error| AuthStackError::store(format!("entropy unavailable: {error}")))?;
    let chars: String = bytes
        .iter()
        .map(|byte| ALPH[*byte as usize % ALPH.len()] as char)
        .collect();
    Ok(format!("{}-{}", &chars[..4], &chars[4..]))
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn urlencoding_encode(value: &str) -> String {
    form_urlencoded::byte_serialize(value.as_bytes()).collect()
}

fn oauth_page(title: &str, copy: &str, body: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en"><head><meta charset="utf-8" /><meta name="viewport" content="width=device-width, initial-scale=1" />
<title>{title}</title>
<style>
body {{ font-family: ui-sans-serif, system-ui, sans-serif; background:#0d0d0d; color:#f5f5f5; margin:0; }}
main {{ max-width: 28rem; margin: 12vh auto; padding: 1.5rem; }}
h1 {{ font-size: 1.4rem; }} p {{ color:#c4c4c4; }}
button, input {{ font: inherit; padding: .55rem .8rem; border-radius: .5rem; }}
button {{ background:#fff; color:#111; border:0; cursor:pointer; }}
input {{ width:100%; box-sizing:border-box; margin:.35rem 0 1rem; background:#1a1a1a; color:#fff; border:1px solid #333; }}
label {{ display:block; font-size:.85rem; }}
</style></head>
<body><main><p>Buwiz</p><h1>{title}</h1><p>{copy}</p>{body}</main></body></html>"#,
        title = html_escape(title),
        copy = html_escape(copy),
        body = body,
    )
}

fn json_response<T: Serialize>(status: StatusCode, value: &T) -> AuthStackResult<RestResponse> {
    let bytes = serde_json::to_vec(value)
        .map_err(|error| AuthStackError::serialization(error.to_string()))?;
    response(status, "application/json", Bytes::from(bytes))
}

fn html(status: StatusCode, body: &str) -> AuthStackResult<RestResponse> {
    response(status, "text/html; charset=utf-8", Bytes::from(body.to_owned()))
}

fn empty(status: StatusCode) -> AuthStackResult<RestResponse> {
    response(status, "text/plain", Bytes::new())
}

fn redirect(location: &str) -> AuthStackResult<RestResponse> {
    let stream = futures::stream::once(async move {
        Ok::<_, std::io::Error>(http_body::Frame::data(Bytes::new()))
    });
    let body = StreamBody::new(stream).boxed_unsync();
    http::Response::builder()
        .status(StatusCode::FOUND)
        .header(http::header::LOCATION, location)
        .body(body)
        .map_err(|error| AuthStackError::transport(error.to_string()))
}

fn oauth_error(
    status: StatusCode,
    error: &str,
    description: &str,
) -> AuthStackResult<RestResponse> {
    json_response(
        status,
        &serde_json::json!({
            "error": error,
            "error_description": description,
        }),
    )
}

fn response(
    status: StatusCode,
    content_type: &'static str,
    bytes: Bytes,
) -> AuthStackResult<RestResponse> {
    let stream =
        futures::stream::once(async move { Ok::<_, std::io::Error>(http_body::Frame::data(bytes)) });
    let body = StreamBody::new(stream).boxed_unsync();
    http::Response::builder()
        .status(status)
        .header(http::header::CONTENT_TYPE, content_type)
        .body(body)
        .map_err(|error| AuthStackError::transport(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_verifier_length_and_charset() {
        assert!(pkce_verifier_valid(&"a".repeat(43)));
        assert!(!pkce_verifier_valid("short"));
        assert!(!pkce_verifier_valid(&format!("{} ", "a".repeat(43))));
    }

    #[test]
    fn desktop_paths_are_recognized() {
        assert!(is_desktop_oauth_request(
            "/.well-known/oauth-authorization-server"
        ));
        assert!(is_desktop_oauth_request("/oauth/device/code"));
        assert!(!is_desktop_oauth_request("/api/auth/password/login"));
    }
}

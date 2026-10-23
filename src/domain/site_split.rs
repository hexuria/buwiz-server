//! How the public hostnames share one buwiz-server deployment.
//!
//! `account.buwiz.com` is the browser origin: Leptos pages, the session
//! cookie, passkeys, and OAuth redirect URIs. `api.buwiz.com` is the same
//! process for bearer clients (the desktop app, agents, `/mcp`). The session
//! cookie stays host-only — `__Host-` forbids a `Domain` attribute — so it
//! is never sent to `buwiz.com` or `api.buwiz.com`. A browser that opens a
//! page on the API host is sent to the account host. API calls stay put.

pub const MARKETING_ORIGIN: &str = "https://buwiz.com";
pub const ACCOUNT_ORIGIN: &str = "https://account.buwiz.com";
pub const API_ORIGIN: &str = "https://api.buwiz.com";

/// Origins allowed to call the HTTP API from a browser.
///
/// The account origin and, when configured, the API origin are always
/// included. `extra_csv` is `AUTH_CORS_ORIGINS` (the marketing site).
pub fn cors_origins(public_base: &str, api_base: &str, extra_csv: &str) -> Vec<String> {
    let mut origins = Vec::new();
    push_origin(&mut origins, public_base);
    push_origin(&mut origins, api_base);
    for part in extra_csv.split(',') {
        push_origin(&mut origins, part);
    }
    origins
}

/// Redirect a document navigation that arrived on the API host to the account
/// host. Bearer calls, JSON reads, and `/api`, `/auth`, `/oauth`, `/mcp`
/// stay on the host that received them.
pub fn account_document_redirect(
    api_base: &str,
    account_base: &str,
    host_header: &str,
    path: &str,
    query: Option<&str>,
    has_authorization: bool,
    accept: Option<&str>,
) -> Option<String> {
    let api_base = api_base.trim().trim_end_matches('/');
    let account_base = account_base.trim().trim_end_matches('/');
    if api_base.is_empty() || account_base.is_empty() || api_base.eq_ignore_ascii_case(account_base)
    {
        return None;
    }
    if has_authorization || prefers_json(accept) || is_api_path(path) {
        return None;
    }
    if !path.starts_with('/') || path.starts_with("//") || path.contains('\\') {
        return None;
    }
    let api_host = origin_host(api_base)?;
    let request_host = host_name(host_header)?;
    if !request_host.eq_ignore_ascii_case(&api_host) {
        return None;
    }
    let mut location = format!("{account_base}{path}");
    if let Some(query) = query.filter(|query| {
        !query.is_empty()
            && !query.contains('\n')
            && !query.contains('\r')
            && !query.contains("://")
    }) {
        location.push('?');
        location.push_str(query);
    }
    Some(location)
}

fn prefers_json(accept: Option<&str>) -> bool {
    accept.is_some_and(|value| {
        let lower = value.to_ascii_lowercase();
        lower.contains("application/json") && !lower.contains("text/html")
    })
}

fn is_api_path(path: &str) -> bool {
    path.starts_with("/api/")
        || path.starts_with("/oauth/")
        || path.starts_with("/auth/")
        || path == "/mcp"
        || path.starts_with("/mcp/")
        || path.starts_with("/.well-known/")
        || path.starts_with("/pkg/")
        || path == "/me"
        || path == "/health"
        || path.starts_with("/health/")
}

fn push_origin(origins: &mut Vec<String>, raw: &str) {
    let raw = raw.trim().trim_end_matches('/');
    if raw.is_empty() {
        return;
    }
    let Some((scheme, rest)) = raw.split_once("://") else {
        return;
    };
    if !matches!(scheme, "https" | "http") || rest.is_empty() || rest.contains('/') {
        return;
    }
    if origins
        .iter()
        .any(|origin| origin.eq_ignore_ascii_case(raw))
    {
        return;
    }
    origins.push(raw.to_owned());
}

fn origin_host(base: &str) -> Option<String> {
    let rest = base.split_once("://")?.1;
    host_name(rest.split('/').next()?)
}

fn host_name(host: &str) -> Option<String> {
    let host = host.trim();
    if host.is_empty() {
        return None;
    }
    if let Some(rest) = host.strip_prefix('[') {
        return Some(rest.split_once(']')?.0.to_owned());
    }
    if let Some((name, port)) = host.rsplit_once(':') {
        if !name.is_empty() && port.chars().all(|character| character.is_ascii_digit()) {
            return Some(name.to_owned());
        }
    }
    Some(host.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cors_keeps_the_account_origin_and_the_marketing_site() {
        let origins = cors_origins(
            ACCOUNT_ORIGIN,
            API_ORIGIN,
            &format!("{MARKETING_ORIGIN}, https://www.buwiz.com, {ACCOUNT_ORIGIN}"),
        );
        assert_eq!(
            origins,
            vec![
                "https://account.buwiz.com",
                "https://api.buwiz.com",
                "https://buwiz.com",
                "https://www.buwiz.com",
            ]
        );
        assert!(cors_origins("http://localhost:3008", "", "javascript:alert(1),").len() == 1);
    }

    #[test]
    fn api_host_pages_go_to_the_account_host_and_api_calls_stay() {
        let login = account_document_redirect(
            API_ORIGIN,
            ACCOUNT_ORIGIN,
            "api.buwiz.com",
            "/login",
            Some("next=%2Fdashboard"),
            false,
            Some("text/html"),
        );
        assert_eq!(
            login.as_deref(),
            Some("https://account.buwiz.com/login?next=%2Fdashboard")
        );
        assert!(
            account_document_redirect(
                API_ORIGIN,
                ACCOUNT_ORIGIN,
                "api.buwiz.com",
                "/auth/device/start",
                None,
                false,
                None,
            )
            .is_none()
        );
        assert!(
            account_document_redirect(
                API_ORIGIN,
                ACCOUNT_ORIGIN,
                "api.buwiz.com",
                "/oauth/token",
                None,
                false,
                None,
            )
            .is_none()
        );
        assert!(
            account_document_redirect(
                API_ORIGIN,
                ACCOUNT_ORIGIN,
                "api.buwiz.com",
                "/me",
                None,
                true,
                Some("application/json"),
            )
            .is_none()
        );
        assert!(
            account_document_redirect(
                API_ORIGIN,
                ACCOUNT_ORIGIN,
                "account.buwiz.com",
                "/login",
                None,
                false,
                None,
            )
            .is_none()
        );
        assert!(
            account_document_redirect(
                "",
                ACCOUNT_ORIGIN,
                "api.buwiz.com",
                "/login",
                None,
                false,
                None
            )
            .is_none()
        );
        assert!(
            account_document_redirect(
                API_ORIGIN,
                ACCOUNT_ORIGIN,
                "api.buwiz.com",
                "//evil.example",
                None,
                false,
                None,
            )
            .is_none()
        );
    }
}

//! First-party desktop OAuth client policy (no identity provider).
//!
//! wasi-auth is an OAuth *client* for Google/Apple/Facebook. Buwiz desktop needs
//! this app to act as an authorization server (PKCE + device code).

/// Public native client for the Buwiz desktop app. No client secret.
pub const DESKTOP_CLIENT_ID: &str = "buwiz-desktop";

pub const DESKTOP_SCOPE: &str = "openid profile email tax_profiles";

/// Custom-scheme callback used by the native app.
pub const DESKTOP_CUSTOM_SCHEME_REDIRECT: &str = "buwiz://auth/callback";

/// Whether `redirect_uri` is allowed for the public desktop client.
///
/// Loopback HTTP(S) on any port is allowed so a native helper can bind an
/// ephemeral port. Extra URIs come from `DESKTOP_OAUTH_REDIRECT_URIS`.
pub fn desktop_redirect_uri_allowed(uri: &str, extra: &[String]) -> bool {
    let uri = uri.trim();
    if uri.is_empty() {
        return false;
    }
    if extra.iter().any(|candidate| candidate.trim() == uri) {
        return true;
    }
    if uri == DESKTOP_CUSTOM_SCHEME_REDIRECT {
        return true;
    }
    loopback_http_redirect_allowed(uri)
}

fn loopback_http_redirect_allowed(uri: &str) -> bool {
    let lower = uri.to_ascii_lowercase();
    let rest = if let Some(rest) = lower.strip_prefix("http://") {
        rest
    } else if let Some(rest) = lower.strip_prefix("https://") {
        rest
    } else {
        return false;
    };
    let (host_port, path) = match rest.split_once('/') {
        Some((host, path)) => (host, path),
        None => (rest, ""),
    };
    if path.contains("..") || path.contains("//") {
        return false;
    }
    let host = host_port
        .rsplit_once(':')
        .and_then(|(host, port)| {
            if port.chars().all(|ch| ch.is_ascii_digit()) {
                Some(host)
            } else {
                None
            }
        })
        .unwrap_or(host_port);
    matches!(host, "127.0.0.1" | "localhost" | "[::1]" | "::1")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_custom_scheme_and_loopback() {
        assert!(desktop_redirect_uri_allowed(
            "buwiz://auth/callback",
            &[]
        ));
        assert!(desktop_redirect_uri_allowed(
            "http://127.0.0.1:9876/callback",
            &[]
        ));
        assert!(desktop_redirect_uri_allowed(
            "http://localhost:43110/auth",
            &[]
        ));
        assert!(desktop_redirect_uri_allowed("http://[::1]:8080/", &[]));
    }

    #[test]
    fn rejects_public_http_hosts() {
        assert!(!desktop_redirect_uri_allowed(
            "https://evil.example/callback",
            &[]
        ));
        assert!(!desktop_redirect_uri_allowed("http://example.com/", &[]));
    }

    #[test]
    fn extra_allowlist_is_honored() {
        assert!(desktop_redirect_uri_allowed(
            "buwiz://dev/callback",
            &["buwiz://dev/callback".to_owned()]
        ));
        assert!(!desktop_redirect_uri_allowed("buwiz://dev/callback", &[]));
    }
}

//! AI agents (Claude Code, Codex, Grok and the like) working on a taxpayer's
//! profiles through MCP.
//!
//! An agent app registers itself (OAuth Dynamic Client Registration), then a
//! signed-in taxpayer allows it on the Allow screen: which of their own
//! profiles and what it may do there. That makes an *agent connection*, with
//! its own tokens. Ending the connection stops the agent at once and leaves
//! the taxpayer's own sign-in alone.
//!
//! An agent never does more than its taxpayer can do right now: what it may
//! do is the connection's permissions narrowed by the taxpayer's live access.
//! Only profiles the taxpayer owns can be connected (sharing stays with the
//! owner), and an agent can never publish to BIR.

use super::pass::{PassPermission, PassPermissions};
use super::tax_profile::ProfileAccess;

/// Most redirect URIs one agent app may register.
pub const MAX_REDIRECT_URIS: usize = 10;
/// Longest agent app name kept.
pub const MAX_CLIENT_NAME_CHARS: usize = 80;
/// Access token lifetime.
pub const AGENT_ACCESS_TTL_SECONDS: i64 = 3_600;
/// Refresh token lifetime. Every refresh replaces the refresh token.
pub const AGENT_REFRESH_TTL_SECONDS: i64 = 30 * 24 * 3_600;
/// Authorization code lifetime.
pub const AGENT_CODE_TTL_SECONDS: i64 = 600;
/// Prefix of every agent client id, so the token endpoint can tell agent
/// apps from the first-party desktop app without a lookup.
pub const AGENT_CLIENT_PREFIX: &str = "agent_";
/// Prefixes of agent tokens. wasi-auth tokens never start with these.
pub const AGENT_ACCESS_PREFIX: &str = "bwa_";
pub const AGENT_REFRESH_PREFIX: &str = "bwr_";
/// The only OAuth scope agents ask for. What the agent may do comes from the
/// Allow screen, not from the scope.
pub const AGENT_SCOPE: &str = "tax_profiles";

/// Whether `token` is an agent access or refresh token. Those are good only
/// at the MCP endpoint; anywhere a user token is expected they are refused,
/// so an agent can never act with its taxpayer's full rights.
pub fn is_agent_token(token: &str) -> bool {
    let token = token.trim();
    token.starts_with(AGENT_ACCESS_PREFIX) || token.starts_with(AGENT_REFRESH_PREFIX)
}

/// A registration request that passed the checks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentRegistration {
    pub client_name: String,
    pub redirect_uris: Vec<String>,
}

/// Checks an RFC 7591 registration: a name to show on the Allow screen and
/// redirect URIs that can receive a code safely. Public clients only (PKCE,
/// no secret).
pub fn validate_registration(
    client_name: Option<&str>,
    redirect_uris: &[String],
    token_endpoint_auth_method: Option<&str>,
) -> Result<AgentRegistration, String> {
    if let Some(method) = token_endpoint_auth_method
        && method != "none"
    {
        return Err("only public clients are supported (token_endpoint_auth_method none)".to_owned());
    }
    let client_name: String = client_name
        .unwrap_or("")
        .chars()
        .filter(|ch| !ch.is_control())
        .collect::<String>()
        .trim()
        .chars()
        .take(MAX_CLIENT_NAME_CHARS)
        .collect();
    let client_name = if client_name.is_empty() {
        "Unnamed agent".to_owned()
    } else {
        client_name
    };
    if redirect_uris.is_empty() {
        return Err("redirect_uris is required".to_owned());
    }
    if redirect_uris.len() > MAX_REDIRECT_URIS {
        return Err(format!("at most {MAX_REDIRECT_URIS} redirect_uris"));
    }
    let mut kept = Vec::new();
    for uri in redirect_uris {
        let uri = uri.trim();
        redirect_uri_registrable(uri)?;
        if !kept.iter().any(|k: &String| k == uri) {
            kept.push(uri.to_owned());
        }
    }
    Ok(AgentRegistration {
        client_name,
        redirect_uris: kept,
    })
}

/// A redirect URI an agent may register: `https://`, loopback `http://`
/// (RFC 8252 §7.3), or an app's own scheme (RFC 8252 §7.1). No fragments, no
/// credentials, and nothing a browser would run.
pub fn redirect_uri_registrable(uri: &str) -> Result<(), String> {
    if uri.is_empty() || uri.len() > 512 {
        return Err("redirect_uri must be 1 to 512 characters".to_owned());
    }
    if uri.chars().any(|ch| ch.is_whitespace() || ch.is_control()) {
        return Err("redirect_uri must not contain spaces".to_owned());
    }
    if uri.contains('#') {
        return Err("redirect_uri must not have a fragment".to_owned());
    }
    let Some((scheme, rest)) = uri.split_once(':') else {
        return Err("redirect_uri needs a scheme".to_owned());
    };
    let scheme = scheme.to_ascii_lowercase();
    let scheme_ok = scheme
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_alphabetic())
        && scheme
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '+' | '-' | '.'));
    if !scheme_ok {
        return Err("redirect_uri has an invalid scheme".to_owned());
    }
    match scheme.as_str() {
        "https" => {
            let host = authority_host(rest).ok_or("redirect_uri needs a host")?;
            if host.is_empty() {
                return Err("redirect_uri needs a host".to_owned());
            }
            Ok(())
        }
        "http" => match authority_host(rest) {
            Some(host) if is_loopback(&host) => Ok(()),
            _ => Err("http redirect_uri must be a loopback address (127.0.0.1, [::1] or localhost)".to_owned()),
        },
        "javascript" | "data" | "file" | "vbscript" | "about" | "blob" | "ftp" | "ws" | "wss" => {
            Err(format!("redirect_uri scheme `{scheme}` is not allowed"))
        }
        _ => Ok(()),
    }
}

/// Whether `presented` matches one of `registered`. Exact match, except a
/// loopback URI may use any port (native apps pick a free one each time).
pub fn redirect_uri_matches(registered: &[String], presented: &str) -> bool {
    registered.iter().any(|candidate| {
        candidate == presented
            || (loopback_parts(candidate).is_some()
                && loopback_parts(candidate) == loopback_parts(presented))
    })
}

/// `(scheme, host, path-and-query)` of a loopback http URI, port dropped.
fn loopback_parts(uri: &str) -> Option<(String, String, String)> {
    let rest = uri.strip_prefix("http://")?;
    let (authority, tail) = match rest.find(['/', '?']) {
        Some(index) => (&rest[..index], &rest[index..]),
        None => (rest, ""),
    };
    let host = host_without_port(authority);
    is_loopback(&host).then(|| ("http".to_owned(), host, tail.to_owned()))
}

fn authority_host(rest: &str) -> Option<String> {
    let rest = rest.strip_prefix("//")?;
    let authority = rest.split(['/', '?']).next().unwrap_or("");
    if authority.contains('@') {
        return None;
    }
    Some(host_without_port(authority))
}

fn host_without_port(authority: &str) -> String {
    let host = if authority.starts_with('[') {
        authority
            .split_once(']')
            .map(|(host, _)| format!("{host}]"))
            .unwrap_or_else(|| authority.to_owned())
    } else {
        authority
            .rsplit_once(':')
            .filter(|(_, port)| port.chars().all(|ch| ch.is_ascii_digit()))
            .map(|(host, _)| host.to_owned())
            .unwrap_or_else(|| authority.to_owned())
    };
    host.to_ascii_lowercase()
}

fn is_loopback(host: &str) -> bool {
    matches!(host, "127.0.0.1" | "localhost" | "[::1]")
}

/// Permissions as chosen on the Allow screen. Publishing to BIR is never
/// available to an agent.
pub fn agent_permissions_from_choice(ids: &[String]) -> Result<PassPermissions, String> {
    let set = PassPermissions::from_choice(ids, false)?;
    if set.contains(PassPermission::FilingsPublish) {
        return Err("an agent can never publish to BIR".to_owned());
    }
    Ok(set)
}

/// What an agent may do on one profile right now: its connection's
/// permissions narrowed by the taxpayer's live access. Owner-only rights
/// (sharing, archiving, deleting) and publishing are never included.
pub fn agent_effective_permissions(
    granted: PassPermissions,
    taxpayer_access: ProfileAccess,
) -> PassPermissions {
    let live = match taxpayer_access {
        // Only profiles the taxpayer owns can be connected; anything else
        // (holding org, firm pass) is not theirs to hand on.
        ProfileAccess::Owner | ProfileAccess::OwnerReadOnly => taxpayer_access.permissions(),
        ProfileAccess::HoldingOrg | ProfileAccess::Pass(_) | ProfileAccess::None => {
            PassPermissions::empty()
        }
    };
    granted
        .intersect(live)
        .without(PassPermission::FilingsPublish)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::pass::PassPreset;

    fn uris(values: &[&str]) -> Vec<String> {
        values.iter().map(|v| (*v).to_owned()).collect()
    }

    #[test]
    fn registration_accepts_loopback_https_and_app_schemes() {
        let reg = validate_registration(
            Some("Claude Code"),
            &uris(&[
                "http://localhost:33418/callback",
                "http://127.0.0.1/callback",
                "https://claude.ai/api/mcp/auth_callback",
                "cursor://anysphere.cursor-retrieval/oauth/callback",
            ]),
            Some("none"),
        )
        .unwrap();
        assert_eq!(reg.client_name, "Claude Code");
        assert_eq!(reg.redirect_uris.len(), 4);
    }

    #[test]
    fn registration_refuses_unsafe_redirects_and_secrets() {
        for bad in [
            "http://example.com/callback",
            "javascript:alert(1)",
            "data:text/html,hi",
            "https://claude.ai/cb#frag",
            "https://user@evil.example/cb",
            "no-scheme",
            "",
        ] {
            assert!(
                validate_registration(Some("x"), &uris(&[bad]), None).is_err(),
                "{bad} should be refused"
            );
        }
        assert!(validate_registration(Some("x"), &[], None).is_err());
        assert!(
            validate_registration(
                Some("x"),
                &uris(&["http://localhost/cb"]),
                Some("client_secret_basic")
            )
            .is_err()
        );
    }

    #[test]
    fn registration_cleans_the_name() {
        let reg =
            validate_registration(Some("  Grok\u{7}Bot  "), &uris(&["http://[::1]:9/cb"]), None)
                .unwrap();
        assert_eq!(reg.client_name, "GrokBot");
        let reg = validate_registration(None, &uris(&["http://[::1]:9/cb"]), None).unwrap();
        assert_eq!(reg.client_name, "Unnamed agent");
        let long = "a".repeat(200);
        let reg = validate_registration(Some(&long), &uris(&["http://[::1]/cb"]), None).unwrap();
        assert_eq!(reg.client_name.chars().count(), MAX_CLIENT_NAME_CHARS);
    }

    #[test]
    fn loopback_redirect_matches_any_port_but_nothing_else() {
        let registered = uris(&[
            "http://127.0.0.1:5000/callback",
            "https://claude.ai/api/mcp/auth_callback",
        ]);
        assert!(redirect_uri_matches(&registered, "http://127.0.0.1:61234/callback"));
        assert!(redirect_uri_matches(&registered, "http://127.0.0.1/callback"));
        assert!(!redirect_uri_matches(&registered, "http://127.0.0.1:61234/other"));
        assert!(!redirect_uri_matches(&registered, "http://localhost:5000/callback"));
        assert!(redirect_uri_matches(
            &registered,
            "https://claude.ai/api/mcp/auth_callback"
        ));
        assert!(!redirect_uri_matches(
            &registered,
            "https://claude.ai:444/api/mcp/auth_callback"
        ));
    }

    #[test]
    fn agent_tokens_are_told_apart_from_user_tokens() {
        assert!(is_agent_token("bwa_abc"));
        assert!(is_agent_token(" bwr_abc"));
        assert!(!is_agent_token("eyJhbGciOiJFUzI1NiJ9.x.y"));
    }

    #[test]
    fn agents_never_get_publish() {
        assert!(agent_permissions_from_choice(&uris(&["filings.publish"])).is_err());
        let full = agent_effective_permissions(PassPermissions::all(), ProfileAccess::Owner);
        assert!(!full.contains(PassPermission::FilingsPublish));
    }

    #[test]
    fn agent_is_bounded_by_the_taxpayers_live_access() {
        let safe = PassPreset::Safe.permissions();
        assert_eq!(agent_effective_permissions(safe, ProfileAccess::Owner), safe);
        // A firm holds the profile now: the owner can only read, so can the agent.
        let narrowed = agent_effective_permissions(safe, ProfileAccess::OwnerReadOnly);
        assert_eq!(narrowed, PassPreset::ReadOnly.permissions());
        // Not the taxpayer's own profile any more: nothing.
        for access in [
            ProfileAccess::HoldingOrg,
            ProfileAccess::Pass(PassPermissions::all()),
            ProfileAccess::None,
        ] {
            assert!(agent_effective_permissions(safe, access).is_empty());
        }
    }
}

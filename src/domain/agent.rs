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
        .filter(|ch| !ch.is_control() && !is_invisible_format(*ch))
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

/// Schemes a browser would run or hand to another app in ways a redirect
/// must never reach.
const DENIED_SCHEMES: [&str; 17] = [
    "javascript", "data", "file", "vbscript", "about", "blob", "ftp", "ws", "wss",
    "mailto", "tel", "sms", "intent", "chrome", "view-source", "jar", "filesystem",
];

/// A redirect URI an agent may register: `https://`, loopback `http://`
/// (RFC 8252 §7.3), or an app's own scheme (RFC 8252 §7.1). ASCII only, no
/// fragment, no credentials, no backslash or percent-escape in the
/// authority, and nothing a browser would run.
///
/// The same check runs on the URI presented at authorize time, so a URI
/// that parses differently in a browser can never match.
pub fn redirect_uri_registrable(uri: &str) -> Result<(), String> {
    if uri.is_empty() || uri.len() > 512 {
        return Err("redirect_uri must be 1 to 512 characters".to_owned());
    }
    if !uri.is_ascii() {
        return Err("redirect_uri must be ASCII (percent-encode anything else)".to_owned());
    }
    if uri.chars().any(|ch| ch.is_ascii_whitespace() || ch.is_ascii_control()) {
        return Err("redirect_uri must not contain spaces".to_owned());
    }
    if uri.contains('#') {
        return Err("redirect_uri must not have a fragment".to_owned());
    }
    if uri.contains('\\') {
        return Err("redirect_uri must not contain a backslash".to_owned());
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
            let authority = authority(rest).ok_or("redirect_uri needs a host")?;
            parse_authority(authority)
                .filter(|(host, _)| !host.is_empty())
                .map(|_| ())
                .ok_or_else(|| "redirect_uri has an invalid host".to_owned())
        }
        "http" => loopback_parts(uri).map(|_| ()).ok_or_else(|| {
            "http redirect_uri must be a loopback address (127.0.0.1, [::1] or localhost)"
                .to_owned()
        }),
        denied if DENIED_SCHEMES.contains(&denied) => {
            Err(format!("redirect_uri scheme `{scheme}` is not allowed"))
        }
        _ => Ok(()),
    }
}

/// Whether `presented` matches one of `registered`. `presented` must itself
/// pass [`redirect_uri_registrable`]; then it matches exactly, or, for a
/// loopback URI, with any port (native apps pick a free one each time).
pub fn redirect_uri_matches(registered: &[String], presented: &str) -> bool {
    if redirect_uri_registrable(presented).is_err() {
        return false;
    }
    let presented_loopback = loopback_parts(presented);
    registered.iter().any(|candidate| {
        candidate == presented
            || (presented_loopback.is_some() && loopback_parts(candidate) == presented_loopback)
    })
}

/// `(host, path-and-query)` of a loopback `http` URI, port dropped and host
/// lowercased. `None` for anything else, including an authority with
/// userinfo, a percent-escape, or junk after the port.
fn loopback_parts(uri: &str) -> Option<(String, String)> {
    let (scheme, rest) = uri.split_once(':')?;
    if !scheme.eq_ignore_ascii_case("http") {
        return None;
    }
    let authority = authority(rest)?;
    let (host, _) = parse_authority(authority)?;
    let tail = &rest[2 + authority.len()..];
    is_loopback(&host).then(|| (host, tail.to_owned()))
}

/// The authority of `//authority/path?query` (the part after `scheme:`).
fn authority(rest: &str) -> Option<&str> {
    let rest = rest.strip_prefix("//")?;
    Some(rest.split(['/', '?']).next().unwrap_or(""))
}

/// `(lowercased host, port)` when `authority` is exactly a host and an
/// optional numeric port: no userinfo, no percent-escapes, an IPv6 literal
/// only in brackets.
fn parse_authority(authority: &str) -> Option<(String, Option<u16>)> {
    if authority.contains(['@', '%', '\\']) {
        return None;
    }
    let (host, port) = if let Some(inner) = authority.strip_prefix('[') {
        let (literal, after) = inner.split_once(']')?;
        let port = match after {
            "" => None,
            _ => Some(after.strip_prefix(':')?),
        };
        (format!("[{literal}]"), port)
    } else {
        match authority.split_once(':') {
            Some((host, port)) => (host.to_owned(), Some(port)),
            None => (authority.to_owned(), None),
        }
    };
    let port = match port {
        None => None,
        Some(digits) if (1..=5).contains(&digits.len()) && digits.bytes().all(|b| b.is_ascii_digit()) => {
            Some(digits.parse::<u16>().ok()?)
        }
        Some(_) => return None,
    };
    let host_ok = host.starts_with('[')
        || host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'));
    host_ok.then(|| (host.to_ascii_lowercase(), port))
}

fn is_loopback(host: &str) -> bool {
    matches!(host, "127.0.0.1" | "localhost" | "[::1]")
}

/// Bidi controls, zero-width and other invisible format characters (Unicode
/// category Cf) that could make a self-chosen app name read as another
/// (`"Claude\u{202e}edoC"`).
fn is_invisible_format(ch: char) -> bool {
    matches!(
        ch,
        '\u{00ad}'
            | '\u{061c}'
            | '\u{180e}'
            | '\u{200b}'..='\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2060}'..='\u{2064}'
            | '\u{2066}'..='\u{206f}'
            | '\u{feff}'
            | '\u{fff9}'..='\u{fffb}'
    )
}

/// Permissions as chosen on the Allow screen. Publishing to BIR is never
/// available to an agent: it is not in the list `from_choice` accepts and
/// its switch is always off here.
pub fn agent_permissions_from_choice(ids: &[String]) -> Result<PassPermissions, String> {
    let set = PassPermissions::from_choice(ids, false)?;
    debug_assert!(!set.contains(PassPermission::FilingsPublish));
    Ok(set)
}

/// What an agent may do on one profile right now: its connection's
/// permissions narrowed by its user's live access. Owner-only rights
/// (sharing, archiving, deleting) and publishing are never included.
///
/// The user may hand on what they own, or what a working pass gives their
/// workspace (a firm member's agent works within the firm's pass, and the
/// owner ending that pass ends it too). A firm's own draft is not handed on:
/// nobody has agreed to an agent there yet.
pub fn agent_effective_permissions(
    granted: PassPermissions,
    user_access: ProfileAccess,
) -> PassPermissions {
    let live = match user_access {
        ProfileAccess::Owner | ProfileAccess::Pass(_) => user_access.permissions(),
        ProfileAccess::HoldingOrg | ProfileAccess::None => PassPermissions::empty(),
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
    fn invisible_characters_cannot_disguise_the_name() {
        let reg = validate_registration(
            Some("Claude\u{202e}edoC\u{200b} Code\u{feff}"),
            &uris(&["http://[::1]/cb"]),
            None,
        )
        .unwrap();
        assert_eq!(reg.client_name, "ClaudeedoC Code");
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
    fn userinfo_and_junk_after_a_loopback_host_never_match() {
        // Each of these sends the browser (and the code) somewhere else.
        let registered = uris(&["http://[::1]:5000/cb", "http://127.0.0.1/cb", "http://localhost/cb"]);
        for attack in [
            "http://[::1]@evil.com/cb",
            "http://[::1]:5000@evil.com/cb",
            "http://127.0.0.1@evil.com/cb",
            "http://localhost:80@evil.com/cb",
            "http://127.0.0.1:5000\\@evil.com/cb",
            "http://[::1].evil.com/cb",
            "http://[::1]x/cb",
            "http://[::1]:5000:6000/cb",
            "http://127.0.0.1%2f@evil.com/cb",
            "http://127.0.0.1:99999/cb",
            "http://evil.com/cb",
        ] {
            assert!(!redirect_uri_matches(&registered, attack), "{attack} must not match");
        }
    }

    #[test]
    fn boundaries_of_the_registration_rules() {
        assert_eq!(AGENT_REFRESH_TTL_SECONDS, 2_592_000, "30 days");
        let ten: Vec<String> = (0..10).map(|i| format!("http://localhost/cb{i}")).collect();
        assert!(validate_registration(Some("x"), &ten, None).is_ok());
        let eleven: Vec<String> = (0..11).map(|i| format!("http://localhost/cb{i}")).collect();
        assert!(validate_registration(Some("x"), &eleven, None).is_err());
        let at_limit = format!("https://a.example/{}", "p".repeat(512 - 18));
        assert_eq!(at_limit.len(), 512);
        assert!(redirect_uri_registrable(&at_limit).is_ok());
        assert!(redirect_uri_registrable(&format!("{at_limit}p")).is_err());
        for bad in [
            "https://a.example/x y",      // space
            "1app://callback",            // scheme starts with a digit
            "my_app://callback",          // underscore not allowed in a scheme
            "http://127.0.0.1:+80/cb",    // a sign is not a port
            "http://127.0.0.1:/cb",       // empty port
            "http://127.0.0.1:123456/cb", // six digits
        ] {
            assert!(redirect_uri_registrable(bad).is_err(), "{bad} should be refused");
        }
        assert!(redirect_uri_registrable("com.example.app:/cb").is_ok());
    }

    #[test]
    fn loopback_scheme_and_host_ignore_case() {
        let registered = uris(&["http://localhost/cb"]);
        assert!(redirect_uri_matches(&registered, "HTTP://LOCALHOST:9/cb"));
        assert!(!redirect_uri_matches(&registered, "http://localhost:9/CB"), "path stays exact");
    }

    #[test]
    fn non_ascii_and_extra_schemes_are_refused() {
        for bad in ["https://exämple.com/cb", "mailto:x@example.com", "intent://x", "tel:123"] {
            assert!(redirect_uri_registrable(bad).is_err(), "{bad} should be refused");
        }
    }

    /// Oracle: the WHATWG URL parser (what browsers follow). Whatever this
    /// module lets match a loopback registration must, to a browser, be that
    /// loopback host with no credentials and the registered path.
    #[cfg(feature = "ssr")]
    mod browser_agrees {
        use super::super::*;
        use proptest::prelude::*;

        const HOSTS: [&str; 6] = ["[::1]", "127.0.0.1", "localhost", "LocalHost", "evil.com", ""];
        const SUFFIXES: [&str; 14] = [
            "", ":5000", ":0", ":65535", ":99999", "@evil.com", ":5000@evil.com", ".evil.com",
            "x", "%2f@evil.com", "\\\\@evil.com", ":5000:1", "]", ":",
        ];
        const TAILS: [&str; 5] = ["/cb", "/cb?x=1", "/CB", "", "/cb/.."];

        fn registered() -> Vec<String> {
            vec![
                "http://[::1]:5000/cb".to_owned(),
                "http://127.0.0.1/cb".to_owned(),
                "http://localhost/cb".to_owned(),
            ]
        }

        fn browser_agrees(presented: &str) -> Result<(), String> {
            let parsed = url::Url::parse(presented).map_err(|e| format!("{presented}: {e}"))?;
            let host = parsed.host_str().unwrap_or_default().to_owned();
            let ok = parsed.scheme() == "http"
                && parsed.username().is_empty()
                && parsed.password().is_none()
                && matches!(host.as_str(), "127.0.0.1" | "localhost" | "[::1]")
                && parsed.path() == "/cb";
            if ok { Ok(()) } else { Err(format!("{presented}: browser goes to {parsed}")) }
        }

        /// Exhaustive over hosts × suffixes × tails × schemes (6·14·5·3 = 1,260
        /// URIs): every match is the loopback host to a browser, and the
        /// legitimate shapes do match (so the check is not vacuous).
        #[test]
        fn every_match_in_the_grid_is_loopback_to_a_browser() {
            let mut matches = 0;
            for scheme in ["http://", "HTTP://", "https://"] {
                for host in HOSTS {
                    for suffix in SUFFIXES {
                        for tail in TAILS {
                            let presented = format!("{scheme}{host}{suffix}{tail}");
                            if redirect_uri_matches(&registered(), &presented) {
                                matches += 1;
                                browser_agrees(&presented).unwrap();
                            }
                        }
                    }
                }
            }
            assert!(matches >= 12, "only {matches} legitimate matches; grid too weak");
        }

        fn uri() -> impl Strategy<Value = String> {
            (
                prop_oneof![Just("http://"), Just("HTTP://"), Just("https://"), Just("http:/")],
                prop::sample::select(HOSTS.to_vec()),
                "[a-zA-Z0-9.:@%\\[\\]\\\\_-]{0,12}",
                prop::sample::select(TAILS.to_vec()),
            )
                .prop_map(|(scheme, host, junk, tail)| format!("{scheme}{host}{junk}{tail}"))
        }

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(4096))]
            #[test]
            fn a_match_is_the_loopback_host_to_a_browser(presented in uri()) {
                let registered = registered();
                if redirect_uri_matches(&registered, &presented) {
                    prop_assert!(browser_agrees(&presented).is_ok(), "{:?}", browser_agrees(&presented));
                }
            }

            #[test]
            fn validation_never_panics(raw in "\\PC{0,64}") {
                let _ = redirect_uri_registrable(&raw);
                let _ = redirect_uri_matches(&["http://localhost/cb".to_owned()], &raw);
            }
        }
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
    fn agent_is_bounded_by_its_users_live_access() {
        let safe = PassPreset::Safe.permissions();
        assert_eq!(agent_effective_permissions(safe, ProfileAccess::Owner), safe);
        // A firm member's agent gets no more than the firm's pass...
        let read_only = PassPreset::ReadOnly.permissions();
        assert_eq!(agent_effective_permissions(safe, ProfileAccess::Pass(read_only)), read_only);
        // ...nor more than its own connection, and never publishing.
        let full_pass = ProfileAccess::Pass(PassPreset::Full.permissions().with(PassPermission::FilingsPublish));
        assert_eq!(agent_effective_permissions(read_only, full_pass), read_only);
        assert!(!agent_effective_permissions(PassPermissions::all(), full_pass).contains(PassPermission::FilingsPublish));
        // A firm's own draft, or no standing: nothing.
        for access in [ProfileAccess::HoldingOrg, ProfileAccess::None] {
            assert!(agent_effective_permissions(safe, access).is_empty());
        }
    }
}

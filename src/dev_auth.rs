//! Pure helpers for development authentication (capture mail + gated bypass).
//!
//! These stay free of Spin/Leptos so `cargo test --lib` can lock the product
//! rules: capture/skip controls are visible only when the mail-capture feature
//! is compiled in, production mode is off, AUTH_DEV_TOOLS is on, and transport
//! is capture.

/// Public login banner when a pending account is recognized in development.
pub const EMAIL_UNVERIFIED_PUBLIC_MESSAGE: &str =
    "This account is waiting for email verification.";

/// Whether the register/login UI may show capture / skip-verify controls.
pub fn development_verification_controls_visible(
    mail_capture_compiled: bool,
    production_mode: bool,
    dev_tools: bool,
    mail_transport_capture: bool,
) -> bool {
    mail_capture_compiled && !production_mode && dev_tools && mail_transport_capture
}

/// Rewrite a captured action URL to a same-origin path + query.
///
/// Capture / Resend templates embed `AUTH_PUBLIC_BASE_URL` (often localhost).
/// Opening that URL from a real inbox or a remote browser never reaches this
/// process. Same-origin paths stay on the host the user is already using.
pub fn same_origin_action_path(action_url: &str) -> Option<String> {
    let trimmed = action_url.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.starts_with('/') && !trimmed.starts_with("//") {
        return Some(trimmed.to_owned());
    }
    let Some(scheme_end) = trimmed.find("://") else {
        return None;
    };
    let after_scheme = &trimmed[scheme_end + 3..];
    let path_start = after_scheme.find('/')?;
    let path = &after_scheme[path_start..];
    if path.is_empty() {
        return None;
    }
    Some(path.to_owned())
}

/// Extract the one-time `token` query parameter from a captured action URL.
pub fn token_from_action_url(action_url: &str) -> Option<String> {
    let candidate = same_origin_action_path(action_url).unwrap_or_else(|| action_url.to_owned());
    let query = candidate.split_once('?')?.1;
    for pair in query.split('&') {
        let Some((key, value)) = pair.split_once('=') else {
            continue;
        };
        if key == "token" {
            let token = value.trim();
            if !token.is_empty() {
                return Some(token.to_owned());
            }
        }
    }
    None
}

/// True when a login/register error should open the pending-verify panel.
pub fn login_error_is_unverified(message: &str) -> bool {
    message.contains(EMAIL_UNVERIFIED_PUBLIC_MESSAGE)
        || message.to_ascii_lowercase().contains("waiting for email verification")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_controls_require_dev_capture_not_production() {
        assert!(development_verification_controls_visible(
            true, false, true, true
        ));
        assert!(!development_verification_controls_visible(
            true, true, true, true
        ));
        assert!(!development_verification_controls_visible(
            true, false, false, true
        ));
        assert!(!development_verification_controls_visible(
            true, false, true, false
        ));
        assert!(!development_verification_controls_visible(
            false, false, true, true
        ));
    }

    #[test]
    fn same_origin_rewrites_localhost_public_base() {
        assert_eq!(
            same_origin_action_path("http://localhost:3008/verify-email?token=abc"),
            Some("/verify-email?token=abc".to_owned())
        );
        assert_eq!(
            same_origin_action_path("http://127.0.0.1:3008/verify-email?token=abc"),
            Some("/verify-email?token=abc".to_owned())
        );
        assert_eq!(
            same_origin_action_path("/verify-email?token=abc"),
            Some("/verify-email?token=abc".to_owned())
        );
        assert_eq!(same_origin_action_path("mailto:nobody@example.test"), None);
    }

    #[test]
    fn token_is_taken_from_query() {
        assert_eq!(
            token_from_action_url("http://localhost:3008/verify-email?token=one-time"),
            Some("one-time".to_owned())
        );
        assert_eq!(token_from_action_url("/verify-email"), None);
    }

    #[test]
    fn unverified_login_copy_is_recognized() {
        assert!(login_error_is_unverified(EMAIL_UNVERIFIED_PUBLIC_MESSAGE));
        assert!(!login_error_is_unverified("Email or password is incorrect"));
    }
}

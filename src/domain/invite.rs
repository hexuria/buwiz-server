//! Owner invites: a firm that created a draft profile invites the taxpayer
//! to own it.
//!
//! The invite is bound to one email address. Only a signed-in account whose
//! *verified* email is that address, holding the link's secret, can accept.
//! The firm shares the link itself (no new email kinds are sent). Accepting
//! makes the taxpayer the owner, and the firm keeps working under a Full pass
//! without publishing, which the owner can change or end like any other.

use serde::{Deserialize, Serialize};

/// How long an invite link works when the firm names no shorter time.
pub const INVITE_LIFETIME_DAYS: i64 = 14;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InviteStatus {
    Open,
    Accepted,
    /// The firm withdrew it, sent a newer one, or the profile got an owner
    /// another way.
    Withdrawn,
}

impl InviteStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Accepted => "accepted",
            Self::Withdrawn => "withdrawn",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OwnerInvite {
    pub invite_id: String,
    /// The firm that holds the draft and sent the invite.
    pub organization_id: String,
    /// Normalized (see [`normalize_invite_email`]).
    pub email: String,
    /// SHA-256 (base64url) of the link's secret; the secret is never stored.
    pub token_hash: String,
    pub status: InviteStatus,
    pub expires_at: String,
    pub created_by: String,
    pub created_at: String,
    pub updated_at: String,
    pub decided_by: Option<String>,
}

/// The email as wasi-auth stores it (`auth_users.normalized_email`): trimmed
/// and lowercased, with a local part and a dotted domain.
pub fn normalize_invite_email(value: &str) -> Result<String, String> {
    let email = value.trim().to_ascii_lowercase();
    let valid = email.len() <= 320
        && !email.chars().any(char::is_whitespace)
        && email.split_once('@').is_some_and(|(local, domain)| {
            !local.is_empty()
                && !domain.is_empty()
                && !domain.contains('@')
                && domain.contains('.')
                && !domain.starts_with('.')
                && !domain.ends_with('.')
        });
    if valid {
        Ok(email)
    } else {
        Err("enter the taxpayer's email address".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emails_normalize_like_wasi_auth() {
        assert_eq!(normalize_invite_email("  Ana@Example.PH ").unwrap(), "ana@example.ph");
    }

    #[test]
    fn bad_emails_are_refused() {
        for bad in [
            "",
            "ana",
            "@example.ph",
            "ana@",
            "ana@example",
            "ana@.ph",
            "ana@example.",
            "ana@b@example.ph",
            "an a@example.ph",
        ] {
            assert!(normalize_invite_email(bad).is_err(), "{bad}");
        }
        let long = format!("{}@example.ph", "a".repeat(309));
        assert_eq!(long.len(), 320);
        assert!(normalize_invite_email(&long).is_ok());
        assert!(normalize_invite_email(&format!("a{long}")).is_err());
    }

    #[test]
    fn statuses_have_stable_names() {
        assert_eq!(InviteStatus::Open.as_str(), "open");
        assert_eq!(InviteStatus::Accepted.as_str(), "accepted");
        assert_eq!(InviteStatus::Withdrawn.as_str(), "withdrawn");
    }
}

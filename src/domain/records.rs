//! Rules for a tax profile's synced records: tax years and their forms,
//! form drafts, and filings.
//!
//! The server stores these for backup and multi-device sync. It never reads a
//! draft's contents and never files with BIR: the installed buwiz-forms app
//! publishes and then reports what happened here. Who may do what is the
//! profile's standing (`ProfileAccess`); each step names the one permission
//! it needs (`docs/sync-api.md`).

use serde_json::Value;

use crate::domain::pass::PassPermission;
use crate::domain::tax_profile::ProfileAccess;

/// Largest draft payload, as serialized JSON.
pub const MAX_PAYLOAD_BYTES: usize = 200 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RecordError {
    /// The caller's standing lacks this permission.
    NotAllowed { needs: PassPermission },
    Invalid { reason: String },
    /// Someone else saved the draft first; `current` is its revision now.
    StaleDraft { current: u64 },
    /// The filing is not in a state that allows this step.
    FilingState { reason: &'static str },
}

impl std::fmt::Display for RecordError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotAllowed { needs } => write!(f, "this needs the `{}` permission", needs.as_str()),
            Self::Invalid { reason } => write!(f, "{reason}"),
            Self::StaleDraft { current } => write!(
                f,
                "this draft changed on another device (now revision {current}); load it again"
            ),
            Self::FilingState { reason } => write!(f, "{reason}"),
        }
    }
}

fn invalid(reason: impl Into<String>) -> RecordError {
    RecordError::Invalid {
        reason: reason.into(),
    }
}

pub fn require(access: ProfileAccess, needs: PassPermission) -> Result<(), RecordError> {
    if access.has(needs) {
        Ok(())
    } else {
        Err(RecordError::NotAllowed { needs })
    }
}

/// A BIR form code such as `1701Q`, `2550Q` or `0619E`: 3 to 12 uppercase
/// letters and digits. Returned uppercased.
pub fn form_code(raw: &str) -> Result<String, RecordError> {
    let code = raw.trim().to_ascii_uppercase();
    if (3..=12).contains(&code.len()) && code.bytes().all(|b| b.is_ascii_alphanumeric()) {
        Ok(code)
    } else {
        Err(invalid("a form code is 3 to 12 letters and digits, like 1701Q"))
    }
}

pub fn tax_year(year: i32) -> Result<i16, RecordError> {
    if (2000..=2100).contains(&year) {
        Ok(year as i16)
    } else {
        Err(invalid("the tax year must be between 2000 and 2100"))
    }
}

/// The period within the year a draft or filing covers, as the desktop app
/// names it (`2026`, `2026-Q1`, `2026-03`, `2026-03-15`): 4 to 16 uppercase
/// letters, digits and dashes, starting with the year.
pub fn period_key(raw: &str, year: i16) -> Result<String, RecordError> {
    let key = raw.trim().to_ascii_uppercase();
    let shaped = (4..=16).contains(&key.len())
        && key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        && key.starts_with(&year.to_string());
    if shaped {
        Ok(key)
    } else {
        Err(invalid(format!(
            "the period must start with the tax year ({year}), like {year}-Q1 or {year}-03"
        )))
    }
}

/// How often a form is filed.
pub fn frequency(raw: &str) -> Result<String, RecordError> {
    let value = raw.trim().to_ascii_lowercase();
    match value.as_str() {
        "monthly" | "quarterly" | "annual" | "one_time" => Ok(value),
        _ => Err(invalid("frequency is monthly, quarterly, annual or one_time")),
    }
}

/// Checks a draft's contents without reading them: a JSON object, small
/// enough, and carrying no secret the desktop must keep to itself.
pub fn draft_payload(payload: &Value) -> Result<(), RecordError> {
    if !payload.is_object() {
        return Err(invalid("a draft is a JSON object"));
    }
    let size = serde_json::to_vec(payload).map(|bytes| bytes.len()).unwrap_or(usize::MAX);
    if size > MAX_PAYLOAD_BYTES {
        return Err(invalid("a draft is at most 200 KB"));
    }
    if let Some(key) = secret_key(payload) {
        return Err(invalid(format!(
            "drafts never carry secrets; remove `{key}` (PINs, TOTP, mailbox and BIR credentials stay on the device)"
        )));
    }
    Ok(())
}

/// The first object key, at any depth, that names something only the device
/// may hold.
pub fn secret_key(value: &Value) -> Option<String> {
    match value {
        Value::Object(map) => map.iter().find_map(|(key, inner)| {
            if is_secret_key(key) {
                Some(key.clone())
            } else {
                secret_key(inner)
            }
        }),
        Value::Array(items) => items.iter().find_map(secret_key),
        _ => None,
    }
}

fn is_secret_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    key == "pin"
        || key.starts_with("imap_")
        || key.starts_with("totp_")
        || key.ends_with("_pin")
        || key.contains("pin_hash")
        || key.contains("password")
        || key.contains("access_token")
        || key.contains("refresh_token")
        || key.contains("oauth_token")
        || key.contains("secret")
}

/// The revision a save produces. A new draft starts at 1 and may name no
/// base (or 0). Changing a draft must name the revision it started from.
pub fn next_draft_rev(existing: Option<u64>, base: Option<u64>) -> Result<u64, RecordError> {
    match (existing, base) {
        (None, None | Some(0)) => Ok(1),
        (None, Some(_)) => Err(RecordError::StaleDraft { current: 0 }),
        (Some(current), Some(base)) if base == current => Ok(current + 1),
        (Some(current), _) => Err(RecordError::StaleDraft { current }),
    }
}

/// The permission a save needs: a new draft `drafts.save`, a change
/// `drafts.update`.
pub fn save_permission(existing: bool) -> PassPermission {
    if existing {
        PassPermission::DraftsUpdate
    } else {
        PassPermission::DraftsSave
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilingStatus {
    /// Ready for the app to send.
    Queued,
    /// The app sent it to BIR.
    Submitted,
    /// BIR's confirmation came back.
    Confirmed,
    /// Sending failed; queue a new filing to try again.
    Failed,
    /// The tax due is paid.
    Paid,
}

impl FilingStatus {
    pub const ALL: [Self; 5] = [
        Self::Queued,
        Self::Submitted,
        Self::Confirmed,
        Self::Failed,
        Self::Paid,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Submitted => "submitted",
            Self::Confirmed => "confirmed",
            Self::Failed => "failed",
            Self::Paid => "paid",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|status| status.as_str() == raw)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilingStep {
    Submitted,
    Confirmed,
    Failed,
    Paid,
}

impl FilingStep {
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "submitted" => Some(Self::Submitted),
            "confirmed" => Some(Self::Confirmed),
            "failed" => Some(Self::Failed),
            "paid" => Some(Self::Paid),
            _ => None,
        }
    }
}

/// queued → submitted → confirmed → paid; queued or submitted → failed.
pub fn next_filing_status(current: FilingStatus, step: FilingStep) -> Result<FilingStatus, RecordError> {
    use FilingStatus::{Confirmed, Failed, Paid, Queued, Submitted};
    match (current, step) {
        (Queued, FilingStep::Submitted) => Ok(Submitted),
        (Submitted, FilingStep::Confirmed) => Ok(Confirmed),
        (Confirmed, FilingStep::Paid) => Ok(Paid),
        (Queued | Submitted, FilingStep::Failed) => Ok(Failed),
        (Failed, _) => Err(RecordError::FilingState {
            reason: "this filing failed; queue a new one",
        }),
        (Paid, _) => Err(RecordError::FilingState {
            reason: "this filing is already paid",
        }),
        _ => Err(RecordError::FilingState {
            reason: "filings go queued, submitted, confirmed, then paid",
        }),
    }
}

/// A SHA-256 in hex, as the app reports for the XML it sent.
pub fn sha256_hex(raw: &str) -> Result<String, RecordError> {
    let value = raw.trim().to_ascii_lowercase();
    if value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit()) {
        Ok(value)
    } else {
        Err(invalid("xml_sha256 is 64 hex characters"))
    }
}

/// Free text the app reports (a receipt reference, an error summary).
pub fn short_text(raw: &str, what: &str, max: usize) -> Result<String, RecordError> {
    let value = raw.trim();
    if value.is_empty() || value.chars().count() > max {
        Err(invalid(format!("{what} is 1 to {max} characters")))
    } else {
        Ok(value.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::pass::PassPreset;
    use proptest::prelude::*;
    use serde_json::json;

    #[test]
    fn payload_limit_is_200_kib() {
        assert_eq!(MAX_PAYLOAD_BYTES, 204_800);
    }

    #[test]
    fn filing_steps_parse_by_name() {
        assert_eq!(FilingStep::parse("submitted"), Some(FilingStep::Submitted));
        assert_eq!(FilingStep::parse("confirmed"), Some(FilingStep::Confirmed));
        assert_eq!(FilingStep::parse("failed"), Some(FilingStep::Failed));
        assert_eq!(FilingStep::parse("paid"), Some(FilingStep::Paid));
        assert_eq!(FilingStep::parse("Paid"), None);
        assert_eq!(FilingStep::parse("queued"), None);
    }

    #[test]
    fn a_paid_filing_says_so() {
        for step in [FilingStep::Submitted, FilingStep::Confirmed, FilingStep::Failed, FilingStep::Paid] {
            assert_eq!(
                next_filing_status(FilingStatus::Paid, step),
                Err(RecordError::FilingState {
                    reason: "this filing is already paid"
                })
            );
        }
    }

    #[test]
    fn form_codes_years_and_periods() {
        assert_eq!(form_code(" 1701q ").unwrap(), "1701Q");
        assert_eq!(form_code("0619E").unwrap(), "0619E");
        assert_eq!(form_code("ABC").unwrap(), "ABC");
        assert_eq!(form_code("ABCDEFGHIJKL").unwrap(), "ABCDEFGHIJKL");
        for bad in ["", "17", "ABCDEFGHIJKLM", "1701-Q", "17 01"] {
            assert!(form_code(bad).is_err(), "{bad}");
        }
        assert_eq!(tax_year(2000).unwrap(), 2000);
        assert_eq!(tax_year(2100).unwrap(), 2100);
        assert!(tax_year(1999).is_err());
        assert!(tax_year(2101).is_err());
        assert_eq!(period_key("2026-q1", 2026).unwrap(), "2026-Q1");
        assert_eq!(period_key("2026", 2026).unwrap(), "2026");
        assert_eq!(period_key("2026-03-15", 2026).unwrap(), "2026-03-15");
        assert_eq!(period_key("2026-0123456789A", 2026).unwrap().len(), 16);
        assert!(period_key("2026-0123456789AB", 2026).is_err(), "17 characters");
        for bad in ["2025-Q1", "Q1", "2026_Q1", "2026-Q1 ", "2026-01234567890AB"] {
            let result = period_key(bad, 2026);
            if bad == "2026-Q1 " {
                assert!(result.is_ok(), "trimmed");
            } else {
                assert!(result.is_err(), "{bad}");
            }
        }
    }

    #[test]
    fn frequencies() {
        for ok in ["monthly", "Quarterly", " annual ", "one_time"] {
            assert!(frequency(ok).is_ok(), "{ok}");
        }
        assert_eq!(frequency("Quarterly").unwrap(), "quarterly");
        assert!(frequency("weekly").is_err());
    }

    #[test]
    fn drafts_are_objects_without_secrets() {
        assert!(draft_payload(&json!({"line_15": "1000.00", "items": [{"atc": "WI010"}]})).is_ok());
        assert!(draft_payload(&json!([1, 2])).is_err());
        assert!(draft_payload(&json!("text")).is_err());
        for key in [
            "pin",
            "PIN",
            "profile_pin_hash",
            "imap_password",
            "imap_host",
            "totp_secret",
            "bir_password",
            "mailbox_refresh_token",
            "oauth_token",
            "access_token",
            "client_secret",
            "owner_pin",
        ] {
            let nested = json!({"form": {"rows": [{ key: "x" }]}});
            let err = draft_payload(&nested).unwrap_err();
            assert!(err.to_string().contains(key), "{key}: {err}");
        }
        // Ordinary tax words are fine.
        assert!(draft_payload(&json!({"pinned": true, "spin_count": 1, "tin": "123"})).is_ok());
    }

    #[test]
    fn drafts_have_a_size_limit() {
        let fits = json!({ "x": "a".repeat(MAX_PAYLOAD_BYTES - 8) });
        assert_eq!(serde_json::to_vec(&fits).unwrap().len(), MAX_PAYLOAD_BYTES);
        assert!(draft_payload(&fits).is_ok());
        let over = json!({ "x": "a".repeat(MAX_PAYLOAD_BYTES - 7) });
        assert!(draft_payload(&over).is_err());
    }

    #[test]
    fn draft_revisions_are_optimistic() {
        assert_eq!(next_draft_rev(None, None), Ok(1));
        assert_eq!(next_draft_rev(None, Some(0)), Ok(1));
        assert_eq!(next_draft_rev(None, Some(3)), Err(RecordError::StaleDraft { current: 0 }));
        assert_eq!(next_draft_rev(Some(4), Some(4)), Ok(5));
        assert_eq!(next_draft_rev(Some(4), Some(3)), Err(RecordError::StaleDraft { current: 4 }));
        assert_eq!(next_draft_rev(Some(4), None), Err(RecordError::StaleDraft { current: 4 }));
        assert_eq!(save_permission(false), PassPermission::DraftsSave);
        assert_eq!(save_permission(true), PassPermission::DraftsUpdate);
    }

    #[test]
    fn the_filing_path() {
        use FilingStatus::*;
        let all_steps = [FilingStep::Submitted, FilingStep::Confirmed, FilingStep::Failed, FilingStep::Paid];
        let allowed = [
            (Queued, FilingStep::Submitted, Submitted),
            (Submitted, FilingStep::Confirmed, Confirmed),
            (Confirmed, FilingStep::Paid, Paid),
            (Queued, FilingStep::Failed, Failed),
            (Submitted, FilingStep::Failed, Failed),
        ];
        for status in FilingStatus::ALL {
            for step in all_steps {
                let expected = allowed
                    .iter()
                    .find(|(from, by, _)| *from == status && *by == step)
                    .map(|(_, _, to)| *to);
                assert_eq!(next_filing_status(status, step).ok(), expected, "{status:?} {step:?}");
            }
        }
        for status in FilingStatus::ALL {
            assert_eq!(FilingStatus::parse(status.as_str()), Some(status));
        }
        assert_eq!(FilingStep::parse("paid"), Some(FilingStep::Paid));
        assert_eq!(FilingStep::parse("queued"), None);
        assert!(next_filing_status(Failed, FilingStep::Submitted)
            .unwrap_err()
            .to_string()
            .contains("queue a new one"));
    }

    #[test]
    fn reported_values() {
        let hash = "A".repeat(64);
        assert_eq!(sha256_hex(&hash).unwrap(), "a".repeat(64));
        assert!(sha256_hex(&"a".repeat(63)).is_err());
        assert!(sha256_hex(&"g".repeat(64)).is_err());
        assert_eq!(short_text(" R-1 ", "receipt", 5).unwrap(), "R-1");
        assert!(short_text("", "receipt", 5).is_err());
        assert!(short_text("123456", "receipt", 5).is_err());
        assert!(short_text("12345", "receipt", 5).is_ok());
    }

    #[test]
    fn each_step_needs_its_permission() {
        let safe = ProfileAccess::Pass(PassPreset::Safe.permissions());
        assert!(require(safe, PassPermission::DraftsSave).is_ok());
        assert_eq!(
            require(safe, PassPermission::FilingsPublish),
            Err(RecordError::NotAllowed { needs: PassPermission::FilingsPublish })
        );
        assert!(require(ProfileAccess::Owner, PassPermission::FilingsPublish).is_ok());
        assert!(require(ProfileAccess::None, PassPermission::YearsView).is_err());
    }

    proptest! {
        /// A secret key is found wherever it is nested.
        #[test]
        fn secrets_are_found_at_any_depth(depth in 0usize..6, in_array in any::<bool>()) {
            let mut value = json!({ "totp_secret": "x" });
            for level in 0..depth {
                value = if in_array && level % 2 == 0 {
                    json!([1, value])
                } else {
                    json!({ format!("k{level}"): value })
                };
            }
            let wrapped = json!({ "root": value });
            prop_assert_eq!(secret_key(&wrapped), Some("totp_secret".to_owned()));
        }

        /// Revisions only ever move forward by one.
        #[test]
        fn revisions_move_forward_by_one(current in 1u64..u64::MAX / 2, base in 0u64..u64::MAX / 2) {
            match next_draft_rev(Some(current), Some(base)) {
                Ok(next) => prop_assert!(base == current && next == current + 1),
                Err(error) => prop_assert_eq!(error, RecordError::StaleDraft { current }),
            }
        }
    }
}

//! TIN root + branch code — the BIR registration-unit uniqueness key.

use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

/// Nine-digit taxpayer identity (TIN root), independent of branch suffix.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TinRoot(String);

impl TinRoot {
    pub fn parse(raw: &str) -> Result<Self, String> {
        let digits: String = raw.chars().filter(|ch| ch.is_ascii_digit()).collect();
        if digits.len() != 9 {
            return Err("tin_root must be exactly 9 digits".to_owned());
        }
        Ok(Self(digits))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for TinRoot {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}-{}-{}",
            &self.0[0..3],
            &self.0[3..6],
            &self.0[6..9]
        )
    }
}

/// Five-digit BIR branch code. Head office is `00000`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BranchCode(String);

impl BranchCode {
    pub const HEAD_OFFICE: &'static str = "00000";

    pub fn parse(raw: &str) -> Result<Self, String> {
        let digits: String = raw.chars().filter(|ch| ch.is_ascii_digit()).collect();
        let normalized = match digits.len() {
            5 => digits,
            3 => format!("00{digits}"),
            _ => return Err("branch_code must be 5 digits (head office 00000)".to_owned()),
        };
        Ok(Self(normalized))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_head_office(&self) -> bool {
        self.0 == Self::HEAD_OFFICE
    }
}

/// Registration unit: TIN root + branch code.
///
/// Used to *attest* identity at create/claim time. Cloud exclusive ownership
/// is enforced on [`crate::domain::TinIdentityHash`], never on this value as a
/// primary key.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RegistrationKey {
    pub tin_root: TinRoot,
    pub branch_code: BranchCode,
}

impl RegistrationKey {
    pub fn new(tin_root: TinRoot, branch_code: BranchCode) -> Self {
        Self {
            tin_root,
            branch_code,
        }
    }

    pub fn parse(tin_root: &str, branch_code: &str) -> Result<Self, String> {
        Ok(Self::new(
            TinRoot::parse(tin_root)?,
            BranchCode::parse(branch_code)?,
        ))
    }

    /// Compact 14-digit filing identifier (`00000000000000`).
    pub fn compact(&self) -> String {
        format!("{}{}", self.tin_root.as_str(), self.branch_code.as_str())
    }

    /// Compact display id. **Not** the cloud event-stream key (that is the profile UUID).
    pub fn stream_id(&self) -> String {
        format!("{}-{}", self.tin_root.as_str(), self.branch_code.as_str())
    }

    pub fn dashed_display(&self) -> String {
        format!("{}-{}", self.tin_root, self.branch_code.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_head_office_and_legacy_three_digit_branch() {
        let key = RegistrationKey::parse("010558054", "00000").unwrap();
        assert_eq!(key.compact(), "01055805400000");
        assert_eq!(key.stream_id(), "010558054-00000");
        assert!(key.branch_code.is_head_office());

        let legacy = RegistrationKey::parse("010-558-054", "000").unwrap();
        assert_eq!(legacy.branch_code.as_str(), "00000");
    }

    #[test]
    fn rejects_wrong_lengths() {
        assert!(TinRoot::parse("12345678").is_err());
        assert!(BranchCode::parse("12").is_err());
    }
}

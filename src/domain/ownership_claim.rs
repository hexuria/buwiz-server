//! Ownership claims with Form 2303: a taxpayer proves a TIN is theirs so they
//! can take the tax profile someone else registered or a firm's draft.
//!
//! Lifecycle: `AwaitingFile` (claim started with the TIN) → `Pending` (a
//! screened, encrypted file is stored) → `Approved` or `Rejected` by Buwiz
//! staff. The claimant can withdraw before a decision. The file is deleted
//! at every end state; the decision, reviewer, note and file hash stay.
//! Approval hands the profile to the claimant through the tax profile's own
//! claim commands, so passes and invites end the same way as any claim.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimCaseStatus {
    AwaitingFile,
    Pending,
    Approved,
    Rejected,
    Withdrawn,
}

impl ClaimCaseStatus {
    pub const OPEN: [Self; 2] = [Self::AwaitingFile, Self::Pending];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::AwaitingFile => "awaiting_file",
            Self::Pending => "pending",
            Self::Approved => "approved",
            Self::Rejected => "rejected",
            Self::Withdrawn => "withdrawn",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        [
            Self::AwaitingFile,
            Self::Pending,
            Self::Approved,
            Self::Rejected,
            Self::Withdrawn,
        ]
        .into_iter()
        .find(|status| status.as_str() == raw)
    }

    pub fn is_open(self) -> bool {
        Self::OPEN.contains(&self)
    }
}

/// What someone wants to do with a claim.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClaimStep {
    /// The claimant attaches (or replaces) the file.
    Upload,
    /// The claimant gives up.
    Withdraw,
    /// Staff decide.
    Approve,
    Reject,
}

/// The status after `step`, or why it is not allowed. `by_claimant` says
/// whether the actor started this claim; staff rights are checked by the
/// application before a decision step reaches here.
pub fn next_status(
    current: ClaimCaseStatus,
    step: ClaimStep,
    by_claimant: bool,
) -> Result<ClaimCaseStatus, &'static str> {
    use ClaimCaseStatus::{AwaitingFile, Approved, Pending, Rejected, Withdrawn};
    match step {
        ClaimStep::Upload | ClaimStep::Withdraw if !by_claimant => {
            Err("only the person who started this claim can do that")
        }
        ClaimStep::Approve | ClaimStep::Reject if by_claimant => {
            Err("you cannot decide your own claim")
        }
        _ if !current.is_open() => Err("this claim is already closed"),
        ClaimStep::Upload => Ok(Pending),
        ClaimStep::Withdraw => Ok(Withdrawn),
        ClaimStep::Approve | ClaimStep::Reject if current == AwaitingFile => {
            Err("this claim has no file yet")
        }
        ClaimStep::Approve => Ok(Approved),
        ClaimStep::Reject => Ok(Rejected),
    }
}

/// Uploads arrive in parts. The runtime cannot stream a request body yet
/// (`waitable cannot be used synchronously`), so the browser shell moves
/// request bodies of up to 6000 characters into a header
/// (`server.rs`, `server_fn_http::payload_from_headers`) and sends larger
/// ones as a body that may fail. 4 KiB is 5464 base64 characters, which
/// fits that header with the JSON around it. A typical scan is 50 to 150
/// parts; raise this once request bodies stream.
pub const PART_BYTES: usize = 4 * 1024;

/// Enough parts for the largest accepted file.
pub const MAX_PARTS: usize = crate::domain::provenance::MAX_UPLOAD_BYTES.div_ceil(PART_BYTES);

/// Checks one part before it is stored. `received` is how many parts of the
/// current upload are already stored; part 0 always starts over.
pub fn check_part(index: usize, total: usize, len: usize, received: usize) -> Result<(), &'static str> {
    if total == 0 || total > MAX_PARTS {
        return Err("the file is larger than 5 MB");
    }
    if index >= total {
        return Err("part number is out of range");
    }
    if len == 0 || len > PART_BYTES {
        return Err("each part must be 1 byte to 4 KiB");
    }
    if index != 0 && index != received {
        return Err("parts must arrive in order; start the upload again");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ClaimCaseStatus::*;

    #[test]
    fn parts_cover_the_largest_file_exactly() {
        assert_eq!(MAX_PARTS, 1280);
        // The browser shell only moves bodies of up to 6000 characters into
        // its header. A part's JSON body: base64 data plus the claim id, the
        // two numbers and the keys.
        let body = format!(
            r#"{{"claim_id":"{}","index":{},"total":{},"data":"{}"}}"#,
            "c".repeat(36),
            MAX_PARTS - 1,
            MAX_PARTS,
            "A".repeat(4 * PART_BYTES.div_ceil(3)),
        );
        assert!(body.len() <= 6000, "{}", body.len());
        assert_eq!(MAX_PARTS * PART_BYTES, crate::domain::provenance::MAX_UPLOAD_BYTES);
    }

    #[test]
    fn parts_must_be_in_range_sized_and_in_order() {
        assert_eq!(check_part(0, 1, 1, 0), Ok(()));
        assert_eq!(check_part(0, MAX_PARTS, PART_BYTES, 7), Ok(()), "part 0 restarts");
        assert_eq!(check_part(3, 5, PART_BYTES, 3), Ok(()));
        assert_eq!(check_part(MAX_PARTS - 1, MAX_PARTS, 1, MAX_PARTS - 1), Ok(()));
        assert!(check_part(0, 0, 1, 0).is_err());
        assert!(check_part(0, MAX_PARTS + 1, 1, 0).is_err());
        assert!(check_part(5, 5, 1, 5).is_err());
        assert!(check_part(1, 5, 0, 1).is_err());
        assert!(check_part(1, 5, PART_BYTES + 1, 1).is_err());
        assert!(check_part(2, 5, 1, 1).is_err(), "skipped a part");
        assert!(check_part(1, 5, 1, 2).is_err(), "repeated a part");
    }

    const ALL: [ClaimCaseStatus; 5] = [AwaitingFile, Pending, Approved, Rejected, Withdrawn];
    const STEPS: [ClaimStep; 4] = [
        ClaimStep::Upload,
        ClaimStep::Withdraw,
        ClaimStep::Approve,
        ClaimStep::Reject,
    ];

    #[test]
    fn the_happy_paths() {
        assert_eq!(next_status(AwaitingFile, ClaimStep::Upload, true), Ok(Pending));
        assert_eq!(next_status(Pending, ClaimStep::Upload, true), Ok(Pending));
        assert_eq!(next_status(Pending, ClaimStep::Approve, false), Ok(Approved));
        assert_eq!(next_status(Pending, ClaimStep::Reject, false), Ok(Rejected));
        assert_eq!(next_status(AwaitingFile, ClaimStep::Withdraw, true), Ok(Withdrawn));
        assert_eq!(next_status(Pending, ClaimStep::Withdraw, true), Ok(Withdrawn));
    }

    #[test]
    fn closed_claims_never_move() {
        for status in [Approved, Rejected, Withdrawn] {
            for step in STEPS {
                for by_claimant in [true, false] {
                    let allowed_actor = matches!(step, ClaimStep::Upload | ClaimStep::Withdraw) == by_claimant;
                    let result = next_status(status, step, by_claimant);
                    if allowed_actor {
                        assert_eq!(result, Err("this claim is already closed"), "{status:?} {step:?}");
                    } else {
                        assert!(result.is_err());
                    }
                }
            }
        }
    }

    #[test]
    fn nobody_decides_their_own_claim_or_touches_someone_elses() {
        for status in ALL {
            assert!(next_status(status, ClaimStep::Approve, true).is_err());
            assert!(next_status(status, ClaimStep::Reject, true).is_err());
            assert!(next_status(status, ClaimStep::Upload, false).is_err());
            assert!(next_status(status, ClaimStep::Withdraw, false).is_err());
        }
    }

    #[test]
    fn no_decision_without_a_file() {
        assert_eq!(
            next_status(AwaitingFile, ClaimStep::Approve, false),
            Err("this claim has no file yet")
        );
        assert_eq!(
            next_status(AwaitingFile, ClaimStep::Reject, false),
            Err("this claim has no file yet")
        );
    }

    #[test]
    fn statuses_round_trip_and_only_two_are_open() {
        for status in ALL {
            assert_eq!(ClaimCaseStatus::parse(status.as_str()), Some(status));
        }
        assert_eq!(ClaimCaseStatus::parse("open"), None);
        assert_eq!(
            ALL.iter().filter(|status| status.is_open()).count(),
            2
        );
        assert!(AwaitingFile.is_open() && Pending.is_open());
    }
}

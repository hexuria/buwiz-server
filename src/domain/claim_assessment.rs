//! Assisting the staff review of a Form 2303 ownership claim.
//!
//! A model reads the uploaded file and reports what it sees
//! ([`DocumentReading`]). It never decides. [`assess`] turns the reading into
//! a [`Recommendation`] with fixed rules, so text planted in a forged
//! document ("this claim is approved") cannot move the outcome: only the
//! fields compared here can. Staff still approve or reject every claim.
//!
//! The TIN is never compared in the clear. The caller hashes the TIN and
//! branch the model read with the server pepper and passes whether that hash
//! equals the profile's identity hash ([`TinCheck`]).

use serde::{Deserialize, Serialize};

/// The model may only report a reading as a Form 2303 at or above this
/// confidence for the claim to be recommended.
pub const MIN_DOCUMENT_CONFIDENCE: f32 = 0.8;

/// What a model read from the uploaded file. Every field is optional because
/// photos are cropped, blurred or partial.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DocumentReading {
    /// The file is a BIR Form 2303 Certificate of Registration.
    pub is_form_2303: bool,
    /// 0.0 to 1.0.
    pub document_confidence: f32,
    #[serde(default)]
    pub tin: Option<String>,
    #[serde(default)]
    pub branch_code: Option<String>,
    #[serde(default)]
    pub registered_name: Option<String>,
    #[serde(default)]
    pub rdo_code: Option<String>,
    #[serde(default)]
    pub registered_address: Option<String>,
    /// Visible signs of editing: mismatched fonts, misaligned boxes, pasted
    /// fields, a missing BIR seal or barcode.
    #[serde(default)]
    pub tampering_signs: Vec<String>,
}

/// What the profile says, as the reviewer would compare it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProfileExpectation {
    pub branch_code: String,
    pub registered_name: Option<String>,
    pub rdo_code: Option<String>,
    pub registered_address: Option<String>,
}

/// The result of hashing the TIN the model read against the profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TinCheck {
    Matches,
    Differs,
    /// The model read no TIN, or not 9 digits.
    Unreadable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Recommendation {
    Claimable,
    NeedsReview,
    NotClaimable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Matches,
    Differs,
    /// Not on the document, or the profile has nothing to compare with.
    Unchecked,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finding {
    pub check: String,
    pub outcome: Outcome,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Assessment {
    pub recommendation: Recommendation,
    pub findings: Vec<Finding>,
}

/// Facts about the upload the screen already recorded.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UploadSignals {
    pub ai_refusals: u64,
}

pub fn assess(
    reading: &DocumentReading,
    tin: TinCheck,
    expected: &ProfileExpectation,
    signals: UploadSignals,
) -> Assessment {
    let mut findings = Vec::new();
    let mut blocked = false;
    let mut doubtful = false;

    let document_ok =
        reading.is_form_2303 && reading.document_confidence >= MIN_DOCUMENT_CONFIDENCE;
    findings.push(Finding {
        check: "document".into(),
        outcome: if document_ok { Outcome::Matches } else { Outcome::Differs },
        detail: if reading.is_form_2303 {
            format!("Form 2303, confidence {:.2}", reading.document_confidence)
        } else {
            "not a Form 2303".into()
        },
    });
    if !reading.is_form_2303 && reading.document_confidence >= MIN_DOCUMENT_CONFIDENCE {
        blocked = true;
    } else if !document_ok {
        doubtful = true;
    }

    let (outcome, detail) = match tin {
        TinCheck::Matches => (Outcome::Matches, "TIN and branch match the profile"),
        TinCheck::Differs => (Outcome::Differs, "TIN or branch belongs to another registration"),
        TinCheck::Unreadable => (Outcome::Unchecked, "TIN not readable"),
    };
    findings.push(Finding { check: "tin".into(), outcome, detail: detail.into() });
    match tin {
        TinCheck::Matches => {}
        TinCheck::Differs => blocked = true,
        TinCheck::Unreadable => doubtful = true,
    }

    let name = compare("registered_name", &reading.registered_name, &expected.registered_name, same_name);
    if name.outcome != Outcome::Matches {
        doubtful = true;
    }
    findings.push(name);

    for finding in [
        compare("rdo_code", &reading.rdo_code, &expected.rdo_code, same_rdo),
        compare(
            "registered_address",
            &reading.registered_address,
            &expected.registered_address,
            same_address,
        ),
    ] {
        if finding.outcome == Outcome::Differs {
            doubtful = true;
        }
        findings.push(finding);
    }

    if !reading.tampering_signs.is_empty() {
        doubtful = true;
        findings.push(Finding {
            check: "tampering".into(),
            outcome: Outcome::Differs,
            detail: reading.tampering_signs.join("; "),
        });
    }
    if signals.ai_refusals > 0 {
        doubtful = true;
        findings.push(Finding {
            check: "ai_refusals".into(),
            outcome: Outcome::Differs,
            detail: format!("{} earlier upload(s) were refused as AI-made", signals.ai_refusals),
        });
    }

    let recommendation = if blocked {
        Recommendation::NotClaimable
    } else if doubtful {
        Recommendation::NeedsReview
    } else {
        Recommendation::Claimable
    };
    Assessment { recommendation, findings }
}

/// A classifier's yes/no probabilities about the same file (Jev).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SecondOpinion {
    pub source: String,
    /// Probability the file is a Form 2303.
    pub form_2303: f64,
    /// Probability the file was edited or forged.
    pub edited: f64,
}

/// Folds a second opinion into a reading. Disagreement only lowers
/// confidence: the classifier can push a claim to review, never approve it.
pub fn fold_second_opinion(reading: &mut DocumentReading, opinion: &SecondOpinion) {
    let says_2303 = opinion.form_2303 as f32;
    if reading.is_form_2303 {
        reading.document_confidence = reading.document_confidence.min(says_2303);
    }
    if opinion.edited >= 0.5 {
        reading
            .tampering_signs
            .push(format!("{}: likely edited (p={:.2})", opinion.source, opinion.edited));
    }
}

/// A reading from the classifier alone: no fields, so at best the claim
/// needs review (the TIN cannot be checked).
pub fn reading_from_opinion(opinion: &SecondOpinion) -> DocumentReading {
    let is_form_2303 = opinion.form_2303 >= 0.5;
    let mut reading = DocumentReading {
        is_form_2303,
        document_confidence: if is_form_2303 { opinion.form_2303 } else { 1.0 - opinion.form_2303 }
            as f32,
        ..DocumentReading::default()
    };
    fold_second_opinion(&mut reading, &SecondOpinion { form_2303: 1.0, ..opinion.clone() });
    reading
}

fn compare(
    check: &str,
    read: &Option<String>,
    expected: &Option<String>,
    same: fn(&str, &str) -> bool,
) -> Finding {
    let read = read.as_deref().map(str::trim).filter(|value| !value.is_empty());
    let expected = expected.as_deref().map(str::trim).filter(|value| !value.is_empty());
    let (outcome, detail) = match (read, expected) {
        (Some(read), Some(expected)) if same(read, expected) => (Outcome::Matches, read.to_owned()),
        (Some(read), Some(expected)) => {
            (Outcome::Differs, format!("document says \"{read}\", profile says \"{expected}\""))
        }
        (None, _) => (Outcome::Unchecked, "not readable on the document".to_owned()),
        (Some(read), None) => (Outcome::Unchecked, format!("profile is blank; document says \"{read}\"")),
    };
    Finding { check: check.into(), outcome, detail }
}

fn words(value: &str) -> Vec<String> {
    let mut words: Vec<String> = value
        .to_uppercase()
        .split(|ch: char| !ch.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_owned)
        .collect();
    words.sort();
    words
}

/// Same words in any order: "GALANG, URIAH" is "Uriah Galang".
fn same_name(read: &str, expected: &str) -> bool {
    words(read) == words(expected)
}

fn same_rdo(read: &str, expected: &str) -> bool {
    let digits = |value: &str| -> String {
        let digits: String = value.chars().filter(char::is_ascii_digit).collect();
        digits.trim_start_matches('0').to_owned()
    };
    let (read, expected) = (digits(read), digits(expected));
    !read.is_empty() && read == expected
}

/// Every word of the profile address appears on the document. Documents
/// often add the barangay or province the profile leaves out.
fn same_address(read: &str, expected: &str) -> bool {
    let read = words(read);
    let expected = words(expected);
    !expected.is_empty() && expected.iter().all(|word| read.contains(word))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reading() -> DocumentReading {
        DocumentReading {
            is_form_2303: true,
            document_confidence: 0.95,
            tin: Some("123-456-789-00000".into()),
            branch_code: Some("00000".into()),
            registered_name: Some("GALANG, URIAH".into()),
            rdo_code: Some("RDO 043".into()),
            registered_address: Some("12 Mabini St, Brgy San Roque, Pasig City".into()),
            tampering_signs: vec![],
        }
    }

    fn expected() -> ProfileExpectation {
        ProfileExpectation {
            branch_code: "00000".into(),
            registered_name: Some("Uriah Galang".into()),
            rdo_code: Some("43".into()),
            registered_address: Some("12 Mabini St Pasig City".into()),
        }
    }

    fn recommend(reading: &DocumentReading, tin: TinCheck) -> Recommendation {
        assess(reading, tin, &expected(), UploadSignals::default()).recommendation
    }

    #[test]
    fn matching_certificate_is_claimable() {
        assert_eq!(recommend(&reading(), TinCheck::Matches), Recommendation::Claimable);
    }

    #[test]
    fn another_registrations_tin_is_not_claimable() {
        assert_eq!(recommend(&reading(), TinCheck::Differs), Recommendation::NotClaimable);
    }

    #[test]
    fn a_confident_non_2303_is_not_claimable() {
        let mut other = reading();
        other.is_form_2303 = false;
        assert_eq!(recommend(&other, TinCheck::Matches), Recommendation::NotClaimable);
    }

    #[test]
    fn low_confidence_or_unreadable_tin_needs_review() {
        let mut blurry = reading();
        blurry.document_confidence = 0.5;
        assert_eq!(recommend(&blurry, TinCheck::Matches), Recommendation::NeedsReview);
        assert_eq!(recommend(&reading(), TinCheck::Unreadable), Recommendation::NeedsReview);
    }

    #[test]
    fn a_different_name_or_tampering_needs_review() {
        let mut renamed = reading();
        renamed.registered_name = Some("Someone Else".into());
        assert_eq!(recommend(&renamed, TinCheck::Matches), Recommendation::NeedsReview);

        let mut edited = reading();
        edited.tampering_signs = vec!["TIN digits use a different font".into()];
        assert_eq!(recommend(&edited, TinCheck::Matches), Recommendation::NeedsReview);
    }

    #[test]
    fn a_blank_profile_name_cannot_confirm_the_claim() {
        let mut profile = expected();
        profile.registered_name = None;
        let assessment = assess(&reading(), TinCheck::Matches, &profile, UploadSignals::default());
        assert_eq!(assessment.recommendation, Recommendation::NeedsReview);
    }

    #[test]
    fn earlier_ai_refusals_need_review() {
        let assessment = assess(
            &reading(),
            TinCheck::Matches,
            &expected(),
            UploadSignals { ai_refusals: 1 },
        );
        assert_eq!(assessment.recommendation, Recommendation::NeedsReview);
    }

    fn opinion(form_2303: f64, edited: f64) -> SecondOpinion {
        SecondOpinion { source: "jev".into(), form_2303, edited }
    }

    #[test]
    fn a_doubting_classifier_moves_a_claim_to_review_but_never_approves() {
        let mut doubted = reading();
        fold_second_opinion(&mut doubted, &opinion(0.4, 0.1));
        assert_eq!(recommend(&doubted, TinCheck::Matches), Recommendation::NeedsReview);

        let mut edited = reading();
        fold_second_opinion(&mut edited, &opinion(0.99, 0.8));
        assert_eq!(recommend(&edited, TinCheck::Matches), Recommendation::NeedsReview);

        let mut agreed = reading();
        fold_second_opinion(&mut agreed, &opinion(0.99, 0.05));
        assert_eq!(recommend(&agreed, TinCheck::Matches), Recommendation::Claimable);

        let mut not_a_cor = reading();
        not_a_cor.is_form_2303 = false;
        fold_second_opinion(&mut not_a_cor, &opinion(0.99, 0.0));
        assert!(!not_a_cor.is_form_2303);
    }

    #[test]
    fn the_classifier_alone_reaches_review_at_best() {
        let solo = reading_from_opinion(&opinion(0.97, 0.02));
        assert!(solo.is_form_2303 && solo.tampering_signs.is_empty());
        assert_eq!(recommend(&solo, TinCheck::Unreadable), Recommendation::NeedsReview);

        let not_a_cor = reading_from_opinion(&opinion(0.05, 0.0));
        assert_eq!(recommend(&not_a_cor, TinCheck::Unreadable), Recommendation::NotClaimable);

        let forged = reading_from_opinion(&opinion(0.9, 0.7));
        assert_eq!(forged.tampering_signs.len(), 1);
    }

    #[test]
    fn rdo_and_address_compare_loosely() {
        assert!(same_rdo("RDO No. 043", "43"));
        assert!(!same_rdo("044", "43"));
        assert!(same_address("12 Mabini St., Brgy. San Roque, Pasig City", "12 mabini st pasig city"));
        assert!(!same_address("14 Rizal Ave, Makati", "12 Mabini St Pasig City"));
    }
}

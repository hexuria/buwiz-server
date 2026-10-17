//! Named read models for V1. Application services map HTTP/server-fn reads
//! onto these query names. Never key a query by raw TIN.

#![allow(dead_code)]

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TaxProfileQuery {
    GetTaxProfile {
        tax_profile_id: String,
    },
    ListTaxProfilesForAccount {
        account_id: String,
    },
    GetProfileYear {
        profile_year_id: String,
    },
    ListYearForms {
        profile_year_id: String,
    },
    GetFormDraft {
        form_draft_id: String,
    },
    ListDraftsForYear {
        tax_profile_id: String,
        tax_year: i16,
    },
    ListFilings {
        tax_profile_id: String,
        tax_year: Option<i16>,
    },
    GetFilingByPeriod {
        tax_profile_id: String,
        form_code: String,
        period_key: String,
    },
}

impl TaxProfileQuery {
    pub fn name(&self) -> &'static str {
        match self {
            Self::GetTaxProfile { .. } => "GetTaxProfile",
            Self::ListTaxProfilesForAccount { .. } => "ListTaxProfilesForAccount",
            Self::GetProfileYear { .. } => "GetProfileYear",
            Self::ListYearForms { .. } => "ListYearForms",
            Self::GetFormDraft { .. } => "GetFormDraft",
            Self::ListDraftsForYear { .. } => "ListDraftsForYear",
            Self::ListFilings { .. } => "ListFilings",
            Self::GetFilingByPeriod { .. } => "GetFilingByPeriod",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v1_query_names_match_the_command_set() {
        assert_eq!(
            TaxProfileQuery::GetTaxProfile {
                tax_profile_id: "x".into()
            }
            .name(),
            "GetTaxProfile"
        );
        assert_eq!(
            TaxProfileQuery::ListTaxProfilesForAccount {
                account_id: "a".into()
            }
            .name(),
            "ListTaxProfilesForAccount"
        );
        assert_eq!(
            TaxProfileQuery::GetFilingByPeriod {
                tax_profile_id: "x".into(),
                form_code: "1701Q".into(),
                period_key: "2026-Q1".into(),
            }
            .name(),
            "GetFilingByPeriod"
        );
    }
}

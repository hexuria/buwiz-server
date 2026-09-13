//! Cloud tax-profile DTOs shared by REST, server functions, and the UI.
//!
//! `id` is the server source of truth. Clients must not treat TIN as a local
//! identity key. The API never returns raw TIN, PIN hashes, TOTP secrets,
//! IMAP passwords, or mailbox OAuth tokens.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaxProfileView {
    pub id: String,
    pub profile_id: String,
    pub owner_user_id: Option<String>,
    pub org_id: Option<String>,
    pub tin_last4: String,
    pub display_name: String,
    pub claim_status: String,
    pub full_name: String,
    pub rdo_code: String,
    pub address: String,
    pub line_of_business: String,
    pub zip_code: String,
    pub phone: String,
    pub email: String,
    pub taxpayer_type: String,
    pub tax_classification: Option<String>,
    pub is_vat_registered: bool,
    pub holder_kind: String,
    pub ownership_status: String,
    pub verification_status: String,
    pub verified_owner_user_id: Option<String>,
    #[serde(default)]
    pub account_id: Option<String>,
    #[serde(default)]
    pub branch_code: String,
    #[serde(default)]
    pub is_archived: bool,
    pub revision: u64,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaxProfileListResponse {
    pub profiles: Vec<TaxProfileView>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaxProfileCreateRequest {
    /// Attested at create time only. Never stored as a primary key.
    pub tin_root: String,
    pub branch_code: String,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default, alias = "full_name")]
    pub registered_name: String,
    pub rdo_code: String,
    #[serde(default)]
    pub line_of_business: String,
    #[serde(default, alias = "address")]
    pub registered_address: String,
    #[serde(default)]
    pub zip_code: String,
    #[serde(default)]
    pub phone: String,
    #[serde(default)]
    pub email: String,
    pub taxpayer_type: String,
    #[serde(default)]
    pub tax_classification: Option<String>,
    #[serde(default)]
    pub is_vat_registered: bool,
    #[serde(default)]
    pub business_start_date: Option<String>,
    /// When set, the profile is held by this organization (company-managed).
    #[serde(default)]
    pub organization_id: Option<String>,
    #[serde(default, alias = "org_id")]
    pub org_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaxProfilePatchRequest {
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default, alias = "full_name")]
    pub registered_name: Option<String>,
    #[serde(default)]
    pub rdo_code: Option<String>,
    #[serde(default)]
    pub line_of_business: Option<String>,
    #[serde(default, alias = "address")]
    pub registered_address: Option<String>,
    #[serde(default)]
    pub zip_code: Option<String>,
    #[serde(default)]
    pub phone: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub taxpayer_type: Option<String>,
    #[serde(default)]
    pub tax_classification: Option<String>,
    #[serde(default)]
    pub is_vat_registered: Option<bool>,
    #[serde(default)]
    pub business_start_date: Option<String>,
    /// Last `updated_at` the client observed. Required for multi-device LWW.
    #[serde(default)]
    pub updated_at: Option<String>,
    /// If present, must hash to this row's identity. Never used to rewrite TIN.
    #[serde(default)]
    pub tin_root: Option<String>,
    #[serde(default)]
    pub branch_code: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaxProfileClaimRequest {
    pub tin_root: String,
    pub branch_code: String,
    #[serde(default)]
    pub expected_revision: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaxProfileTransferRequest {
    pub id: String,
    pub organization_id: String,
    #[serde(default)]
    pub expected_revision: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MeOrg {
    pub org_id: String,
    pub name: String,
    #[serde(default)]
    pub slug: String,
    pub role: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MeResponse {
    pub user_id: String,
    pub email: Option<String>,
    pub orgs: Vec<MeOrg>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct YearFormView {
    pub form_code: String,
    pub frequency: String,
    pub active: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProfileYearView {
    pub id: String,
    pub tax_profile_id: String,
    pub tax_year: i16,
    pub registered_name: Option<String>,
    pub rdo_code: Option<String>,
    pub line_of_business: Option<String>,
    pub registered_address: Option<String>,
    pub zip_code: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub tax_classification: Option<String>,
    pub is_vat_registered: Option<bool>,
    pub forms: Vec<YearFormView>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaxProfileTinGroup {
    pub tin_last4: String,
    pub display_name: String,
    pub holder_kind: String,
    pub branches: Vec<TaxProfileView>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProfileYearListResponse {
    pub tax_profile_id: String,
    pub years: Vec<i16>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SaveProfileYearRequest {
    pub tax_year: i16,
    #[serde(default)]
    pub registered_name: Option<String>,
    #[serde(default)]
    pub rdo_code: Option<String>,
    #[serde(default)]
    pub line_of_business: Option<String>,
    #[serde(default, alias = "address")]
    pub registered_address: Option<String>,
    #[serde(default)]
    pub zip_code: Option<String>,
    #[serde(default)]
    pub phone: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub taxpayer_type: Option<String>,
    #[serde(default)]
    pub tax_classification: Option<String>,
    #[serde(default)]
    pub is_vat_registered: Option<bool>,
    #[serde(default)]
    pub business_start_date: Option<String>,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SetYearFormsRequest {
    pub tax_year: i16,
    #[serde(default)]
    pub form_codes: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CloneProfileYearRequest {
    pub from_year: i16,
    pub to_year: i16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BirFormSpec {
    pub code: &'static str,
    pub title: &'static str,
    pub frequency: &'static str,
}

/// Searchable BIR form catalog for the yearly attachment picker.
pub const BIR_FORM_CATALOG: &[BirFormSpec] = &[
    BirFormSpec { code: "0605", title: "Payment form", frequency: "on_demand" },
    BirFormSpec { code: "1905", title: "Update of exemption / registration", frequency: "on_demand" },
    BirFormSpec { code: "1600", title: "Withholding on government money payments", frequency: "monthly" },
    BirFormSpec { code: "1600PT", title: "Withholding on government money payments — PT", frequency: "monthly" },
    BirFormSpec { code: "1600VT", title: "Withholding on government money payments — VAT", frequency: "monthly" },
    BirFormSpec { code: "1600WP", title: "Withholding on government money payments — WP", frequency: "monthly" },
    BirFormSpec { code: "1601C", title: "Withholding on compensation", frequency: "monthly" },
    BirFormSpec { code: "1601E", title: "Withholding on expanded income", frequency: "monthly" },
    BirFormSpec { code: "1601F", title: "Withholding on final income", frequency: "monthly" },
    BirFormSpec { code: "1601EQ", title: "Quarterly expanded withholding", frequency: "quarterly" },
    BirFormSpec { code: "1601FQ", title: "Quarterly final withholding", frequency: "quarterly" },
    BirFormSpec { code: "1604C", title: "Annual compensation withholding", frequency: "annual" },
    BirFormSpec { code: "1604E", title: "Annual expanded withholding", frequency: "annual" },
    BirFormSpec { code: "1604F", title: "Annual final withholding", frequency: "annual" },
    BirFormSpec { code: "1701", title: "Annual income tax — individuals", frequency: "annual" },
    BirFormSpec { code: "1701A", title: "Annual income tax — individuals (optional)", frequency: "annual" },
    BirFormSpec { code: "1701Q", title: "Quarterly income tax — individuals", frequency: "quarterly" },
    BirFormSpec { code: "1702RT", title: "Annual income tax — corporations (regular)", frequency: "annual" },
    BirFormSpec { code: "1702EX", title: "Annual income tax — corporations (exempt)", frequency: "annual" },
    BirFormSpec { code: "1702MX", title: "Annual income tax — corporations (mixed)", frequency: "annual" },
    BirFormSpec { code: "1702Q", title: "Quarterly income tax — corporations", frequency: "quarterly" },
    BirFormSpec { code: "2550M", title: "Monthly VAT declaration", frequency: "monthly" },
    BirFormSpec { code: "2550Q", title: "Quarterly VAT declaration", frequency: "quarterly" },
    BirFormSpec { code: "2551Q", title: "Quarterly percentage tax", frequency: "quarterly" },
    BirFormSpec { code: "2307", title: "Certificate of creditable tax withheld", frequency: "on_demand" },
    BirFormSpec { code: "2316", title: "Certificate of compensation payment/tax withheld", frequency: "annual" },
];

pub fn bir_form_frequency(code: &str) -> &'static str {
    let needle = code.trim();
    BIR_FORM_CATALOG
        .iter()
        .find(|entry| entry.code.eq_ignore_ascii_case(needle))
        .map(|entry| entry.frequency)
        .unwrap_or("annual")
}

pub fn masked_tin_label(tin_last4: &str, branch_code: &str) -> String {
    let last4 = tin_last4
        .chars()
        .filter(|ch| ch.is_ascii_digit())
        .collect::<String>();
    let last4 = if last4.len() >= 4 {
        last4[last4.len() - 4..].to_owned()
    } else if last4.is_empty() {
        "••••".to_owned()
    } else {
        last4
    };
    let branch = if branch_code.trim().is_empty() {
        "00000"
    } else {
        branch_code.trim()
    };
    format!("•••-•••-{last4}-{branch}")
}

pub fn year_choices(existing: &[i16], current: i16) -> Vec<i16> {
    let mut years: std::collections::BTreeSet<i16> = ((current - 2)..=(current + 1)).collect();
    years.extend(existing.iter().copied());
    years.into_iter().rev().collect()
}

/// Personal accounts hold one TIN (many branches). Firm accounts hold many TINs.
pub fn group_tax_profiles_by_tin(mut profiles: Vec<TaxProfileView>) -> Vec<TaxProfileTinGroup> {
    profiles.sort_by(|left, right| {
        left.tin_last4
            .cmp(&right.tin_last4)
            .then_with(|| left.holder_kind.cmp(&right.holder_kind))
            .then_with(|| left.branch_code.cmp(&right.branch_code))
    });
    let mut groups: Vec<TaxProfileTinGroup> = Vec::new();
    for profile in profiles {
        if let Some(group) = groups.iter_mut().find(|group| {
            group.tin_last4 == profile.tin_last4 && group.holder_kind == profile.holder_kind
        }) {
            if group.display_name.trim().is_empty() {
                group.display_name = profile.display_name.clone();
            }
            group.branches.push(profile);
        } else {
            groups.push(TaxProfileTinGroup {
                tin_last4: profile.tin_last4.clone(),
                display_name: profile.display_name.clone(),
                holder_kind: profile.holder_kind.clone(),
                branches: vec![profile],
            });
        }
    }
    groups
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(last4: &str, branch: &str, name: &str, holder: &str) -> TaxProfileView {
        TaxProfileView {
            id: format!("{last4}-{branch}"),
            profile_id: format!("{last4}-{branch}"),
            owner_user_id: None,
            org_id: None,
            tin_last4: last4.to_owned(),
            display_name: name.to_owned(),
            claim_status: "owned".to_owned(),
            full_name: name.to_owned(),
            rdo_code: "039".to_owned(),
            address: String::new(),
            line_of_business: String::new(),
            zip_code: String::new(),
            phone: String::new(),
            email: String::new(),
            taxpayer_type: "individual".to_owned(),
            tax_classification: None,
            is_vat_registered: false,
            holder_kind: holder.to_owned(),
            ownership_status: "personal_exclusive".to_owned(),
            verification_status: "unverified".to_owned(),
            verified_owner_user_id: None,
            account_id: None,
            branch_code: branch.to_owned(),
            is_archived: false,
            revision: 1,
            created_at: None,
            updated_at: None,
        }
    }

    #[test]
    fn groups_branches_of_the_same_tin() {
        let groups = group_tax_profiles_by_tin(vec![
            sample("6789", "00001", "Annex", "personal"),
            sample("6789", "00000", "Head", "personal"),
            sample("1111", "00000", "Firm A", "organization"),
        ]);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].tin_last4, "1111");
        assert_eq!(groups[1].branches.len(), 2);
        assert_eq!(groups[1].branches[0].branch_code, "00000");
        assert_eq!(groups[1].branches[1].branch_code, "00001");
    }

    #[test]
    fn masked_tin_never_prints_the_root() {
        assert_eq!(masked_tin_label("6789", "00001"), "•••-•••-6789-00001");
        assert!(!masked_tin_label("6789", "00000").contains("123"));
    }

    #[test]
    fn year_choices_include_neighbors_and_history() {
        assert_eq!(year_choices(&[2022], 2026), vec![2027, 2026, 2025, 2024, 2022]);
    }

    #[test]
    fn unknown_form_defaults_to_annual() {
        assert_eq!(bir_form_frequency("2550M"), "monthly");
        assert_eq!(bir_form_frequency("9999"), "annual");
    }
}

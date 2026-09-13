//! Tax-profile application services (verified-email gate + exclusive ownership).

use crate::contracts::{
    bir_form_frequency, CloneProfileYearRequest, MeOrg, MeResponse, ProfileYearListResponse,
    ProfileYearView, SaveProfileYearRequest, SetYearFormsRequest, TaxProfileClaimRequest,
    TaxProfileCreateRequest, TaxProfileListResponse, TaxProfilePatchRequest,
    TaxProfileTransferRequest, TaxProfileView,
};
use crate::domain::{
    same_personal_tin, tin_last4, AccountHolder, CloudTaxProfileFacts, FakeOrusVerifier,
    TaxProfileCommand, TaxpayerType, TinIdentityHash, TinOwnershipVerifier,
    UnconfiguredOrusVerifier,
};
use crate::domain::tin::RegistrationKey;
use crate::error::{AuthStackError, AuthStackResult};

use super::{RequestAuth, authenticated_session_view, feature_enabled};

pub async fn current_user_me(auth: RequestAuth) -> AuthStackResult<MeResponse> {
    let session = authenticated_session_view(auth.clone()).await?;
    let user_id = session.user_id.ok_or(AuthStackError::AuthRequired)?;
    let orgs = crate::application::list_organizations(auth)
        .await?
        .organizations
        .into_iter()
        .map(|org| MeOrg {
            org_id: org.organization_id,
            name: org.name,
            slug: org.slug,
            role: org.current_user_role,
        })
        .collect();
    Ok(MeResponse {
        user_id,
        email: session.primary_email,
        orgs,
    })
}

/// ListTaxProfilesForAccount — active (non-archived) profiles for the caller.
pub async fn list_tax_profiles(
    organization_id: Option<String>,
    auth: RequestAuth,
) -> AuthStackResult<TaxProfileListResponse> {
    let session = authenticated_session_view(auth.clone()).await?;
    let user_id = session.user_id.ok_or(AuthStackError::AuthRequired)?;
    let org_id = organization_id
        .or(session.tenant_id.clone())
        .filter(|value| !value.trim().is_empty());
    if let (Some(organization_id), Some(session_id)) =
        (org_id.as_deref(), session.session_id.as_deref())
    {
        crate::auth_product::organization_for_session(session_id, organization_id).await?;
    }
    let profiles = crate::store::list_tax_profiles_for(&user_id, org_id.as_deref()).await?;
    Ok(TaxProfileListResponse { profiles })
}

/// GetTaxProfile — load by UUID, then allow the owning account or current holder/org.
pub async fn get_tax_profile(
    profile_id: String,
    auth: RequestAuth,
) -> AuthStackResult<TaxProfileView> {
    let session = authenticated_session_view(auth).await?;
    let user_id = session.user_id.ok_or(AuthStackError::AuthRequired)?;
    let loaded = crate::store::load_tax_profile_by_id(&profile_id).await?;
    if !loaded.state.exists {
        return Err(AuthStackError::not_found("tax profile not found"));
    }
    let is_account = loaded.state.account_id.as_deref() == Some(user_id.as_str());
    let is_owner = loaded.state.owner_user_id().as_deref() == Some(user_id.as_str());
    let is_personal_holder = matches!(
        &loaded.state.holder,
        Some(AccountHolder::Personal { user_id: holder }) if holder == &user_id
    );
    if !is_account && !is_owner && !is_personal_holder {
        ensure_holder_or_org(&loaded.state, &user_id, session.session_id.as_deref()).await?;
    }
    crate::store::fetch_tax_profile_view(&profile_id, &user_id).await
}

pub async fn create_tax_profile(
    request: TaxProfileCreateRequest,
    auth: RequestAuth,
) -> AuthStackResult<TaxProfileView> {
    let (user_id, session_id) = require_verified_actor(auth.clone()).await?;
    let registration = parse_registration(&request.tin_root, &request.branch_code)?;
    let pepper = crate::store::tin_identity_pepper().await?;
    let identity_hash =
        TinIdentityHash::compute(&pepper, &registration.tin_root, &registration.branch_code);
    if let Some(existing) = crate::store::load_tax_profile_by_identity_hash(&identity_hash).await?
    {
        if existing.state.exists {
            return Err(crate::store::map_tax_profile_error(
                crate::domain::TaxProfileError::AlreadyHeld {
                    holder: existing
                        .state
                        .holder
                        .clone()
                        .ok_or_else(|| AuthStackError::conflict("registration unit is already held"))?,
                },
            ));
        }
    }
    let facts = crate::store::facts_from_create(&request)?;
    let org_id = request
        .organization_id
        .as_deref()
        .or(request.org_id.as_deref());
    let holder = resolve_holder(&user_id, session_id.as_deref(), org_id).await?;
    if matches!(holder, AccountHolder::Personal { .. }) {
        ensure_personal_single_tin(&user_id, &registration).await?;
    }
    let profile_id = crate::store::new_uuid_v4()?;
    let display_name = request
        .display_name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(facts.registered_name.as_str())
        .to_owned();
    crate::store::commit_tax_profile_command(
        &profile_id,
        TaxProfileCommand::RegisterTaxProfile {
            profile_id: profile_id.clone(),
            account_id: user_id.clone(),
            holder,
            facts,
            display_name,
            tin_hash: identity_hash,
            tin_last4: tin_last4(&registration.tin_root),
            branch_code: registration.branch_code.as_str().to_owned(),
            actor_user_id: user_id.clone(),
            occurred_at: crate::store::rfc3339_now(),
        },
        0,
    )
    .await?;
    crate::store::fetch_tax_profile_view(&profile_id, &user_id).await
}

pub async fn patch_tax_profile(
    profile_id: String,
    request: TaxProfilePatchRequest,
    auth: RequestAuth,
) -> AuthStackResult<TaxProfileView> {
    let (user_id, _) = require_verified_actor(auth).await?;
    let loaded = crate::store::load_tax_profile_by_id(&profile_id).await?;
    if !loaded.state.exists {
        return Err(AuthStackError::not_found("tax profile not found"));
    }
    require_account(&loaded.state, &user_id)?;
    if let (Some(tin_root), Some(branch_code)) = (
        request.tin_root.as_deref().filter(|value| !value.trim().is_empty()),
        request
            .branch_code
            .as_deref()
            .filter(|value| !value.trim().is_empty()),
    ) {
        let registration = parse_registration(tin_root, branch_code)?;
        let pepper = crate::store::tin_identity_pepper().await?;
        let incoming =
            TinIdentityHash::compute(&pepper, &registration.tin_root, &registration.branch_code);
        if loaded.state.identity_hash.as_ref() != Some(&incoming) {
            return Err(crate::store::map_tax_profile_error(
                crate::domain::TaxProfileError::IdentityImmutable,
            ));
        }
    } else if request.tin_root.as_deref().is_some_and(|value| !value.trim().is_empty())
        || request
            .branch_code
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
    {
        return Err(AuthStackError::validation(
            "tin_root and branch_code must be attested together and cannot change identity",
        ));
    }
    let current_view = crate::store::fetch_tax_profile_view(&profile_id, &user_id).await?;
    if let Some(expected) = request.updated_at.as_deref().filter(|value| !value.is_empty()) {
        if current_view.updated_at.as_deref() != Some(expected) {
            return Err(crate::store::map_tax_profile_error(
                crate::domain::TaxProfileError::StaleWrite,
            ));
        }
    }
    let facts = merge_facts(loaded.state.facts.as_ref(), &request)?;
    crate::store::commit_tax_profile_command(
        &profile_id,
        TaxProfileCommand::UpdateTaxProfileIdentity {
            display_name: request.display_name.clone(),
            facts: Some(facts),
            expected_updated_at: None,
            actor_user_id: user_id.clone(),
            occurred_at: crate::store::rfc3339_now(),
        },
        loaded.revision,
    )
    .await?;
    crate::store::fetch_tax_profile_view(&profile_id, &user_id).await
}

pub async fn archive_tax_profile(
    profile_id: String,
    auth: RequestAuth,
) -> AuthStackResult<TaxProfileView> {
    let (user_id, _) = require_verified_actor(auth).await?;
    let loaded = crate::store::load_tax_profile_by_id(&profile_id).await?;
    if !loaded.state.exists {
        return Err(AuthStackError::not_found("tax profile not found"));
    }
    require_account(&loaded.state, &user_id)?;
    crate::store::commit_tax_profile_command(
        &profile_id,
        TaxProfileCommand::ArchiveTaxProfile {
            actor_user_id: user_id.clone(),
            occurred_at: crate::store::rfc3339_now(),
        },
        loaded.revision,
    )
    .await?;
    crate::store::fetch_tax_profile_view(&profile_id, &user_id).await
}

pub async fn restore_tax_profile(
    profile_id: String,
    auth: RequestAuth,
) -> AuthStackResult<TaxProfileView> {
    let (user_id, _) = require_verified_actor(auth).await?;
    let loaded = crate::store::load_tax_profile_by_id(&profile_id).await?;
    if !loaded.state.exists {
        return Err(AuthStackError::not_found("tax profile not found"));
    }
    require_account(&loaded.state, &user_id)?;
    crate::store::commit_tax_profile_command(
        &profile_id,
        TaxProfileCommand::RestoreTaxProfile {
            actor_user_id: user_id.clone(),
            occurred_at: crate::store::rfc3339_now(),
        },
        loaded.revision,
    )
    .await?;
    crate::store::fetch_tax_profile_view(&profile_id, &user_id).await
}

pub async fn claim_tax_profile(
    request: TaxProfileClaimRequest,
    auth: RequestAuth,
) -> AuthStackResult<TaxProfileView> {
    let (user_id, _) = require_verified_actor(auth).await?;
    let (profile_id, identity_hash, expected, proof_method) =
        attested_existing(&request, &user_id).await?;
    crate::store::commit_tax_profile_command(
        &profile_id,
        TaxProfileCommand::Claim {
            claimant: AccountHolder::Personal {
                user_id: user_id.clone(),
            },
            actor_user_id: user_id.clone(),
            identity_hash,
            proof_method,
        },
        expected,
    )
    .await?;
    crate::store::fetch_tax_profile_view(&profile_id, &user_id).await
}

pub async fn reclaim_tax_profile(
    request: TaxProfileClaimRequest,
    auth: RequestAuth,
) -> AuthStackResult<TaxProfileView> {
    let (user_id, _) = require_verified_actor(auth).await?;
    let (profile_id, identity_hash, expected, proof_method) =
        attested_existing(&request, &user_id).await?;
    crate::store::commit_tax_profile_command(
        &profile_id,
        TaxProfileCommand::Reclaim {
            actor_user_id: user_id.clone(),
            identity_hash,
            proof_method,
        },
        expected,
    )
    .await?;
    crate::store::fetch_tax_profile_view(&profile_id, &user_id).await
}

pub async fn transfer_tax_profile(
    request: TaxProfileTransferRequest,
    auth: RequestAuth,
) -> AuthStackResult<TaxProfileView> {
    let (user_id, session_id) = require_verified_actor(auth).await?;
    resolve_holder(
        &user_id,
        session_id.as_deref(),
        Some(request.organization_id.as_str()),
    )
    .await?;
    let loaded = crate::store::load_tax_profile_by_id(&request.id).await?;
    if !loaded.state.exists {
        return Err(AuthStackError::not_found("tax profile not found"));
    }
    let expected = request.expected_revision.unwrap_or(loaded.revision);
    crate::store::commit_tax_profile_command(
        &request.id,
        TaxProfileCommand::Transfer {
            new_holder: AccountHolder::Organization {
                organization_id: request.organization_id,
            },
            actor_user_id: user_id.clone(),
        },
        expected,
    )
    .await?;
    crate::store::fetch_tax_profile_view(&request.id, &user_id).await
}

pub async fn get_profile_year(
    profile_id: String,
    tax_year: i16,
    auth: RequestAuth,
) -> AuthStackResult<ProfileYearView> {
    let year = validate_tax_year(tax_year)?;
    let (_user_id, _) = require_profile_access(&profile_id, auth).await?;
    crate::store::load_or_create_profile_year(&profile_id, year).await
}

pub async fn list_profile_years(
    profile_id: String,
    auth: RequestAuth,
) -> AuthStackResult<ProfileYearListResponse> {
    let (_user_id, _) = require_profile_access(&profile_id, auth).await?;
    Ok(ProfileYearListResponse {
        tax_profile_id: profile_id.clone(),
        years: crate::store::list_profile_years(&profile_id).await?,
    })
}

pub async fn save_profile_year(
    profile_id: String,
    request: SaveProfileYearRequest,
    auth: RequestAuth,
) -> AuthStackResult<ProfileYearView> {
    let year = validate_tax_year(request.tax_year)?;
    let (user_id, session_id) = require_verified_actor(auth).await?;
    let loaded = crate::store::load_tax_profile_by_id(&profile_id).await?;
    if !loaded.state.exists {
        return Err(AuthStackError::not_found("tax profile not found"));
    }
    require_manage(&loaded.state, &user_id, session_id.as_deref()).await?;
    if let Some(expected) = request.updated_at.as_deref().filter(|value| !value.is_empty()) {
        let current_view = crate::store::fetch_tax_profile_view(&profile_id, &user_id).await?;
        if current_view.updated_at.as_deref() != Some(expected) {
            return Err(crate::store::map_tax_profile_error(
                crate::domain::TaxProfileError::StaleWrite,
            ));
        }
    }
    let year_view = crate::store::update_profile_year(
        &profile_id,
        year,
        request.registered_name.clone(),
        request.rdo_code.clone(),
        request.line_of_business.clone(),
        request.registered_address.clone(),
        request.zip_code.clone(),
        request.phone.clone(),
        request.email.clone(),
        request.tax_classification.clone(),
        request.is_vat_registered,
    )
    .await?;
    let facts = merge_facts(
        loaded.state.facts.as_ref(),
        &TaxProfilePatchRequest {
            display_name: request.display_name.clone(),
            registered_name: request.registered_name.clone(),
            rdo_code: request.rdo_code.clone(),
            line_of_business: request.line_of_business.clone(),
            registered_address: request.registered_address.clone(),
            zip_code: request.zip_code.clone(),
            phone: request.phone.clone(),
            email: request.email.clone(),
            taxpayer_type: request.taxpayer_type.clone(),
            tax_classification: request.tax_classification.clone(),
            is_vat_registered: request.is_vat_registered,
            business_start_date: request.business_start_date.clone(),
            updated_at: None,
            tin_root: None,
            branch_code: None,
        },
    )?;
    crate::store::commit_tax_profile_command(
        &profile_id,
        TaxProfileCommand::UpdateTaxProfileIdentity {
            display_name: request
                .display_name
                .or_else(|| request.registered_name.clone()),
            facts: Some(facts),
            expected_updated_at: None,
            actor_user_id: user_id,
            occurred_at: crate::store::rfc3339_now(),
        },
        loaded.revision,
    )
    .await?;
    Ok(year_view)
}

pub async fn set_year_forms(
    profile_id: String,
    request: SetYearFormsRequest,
    auth: RequestAuth,
) -> AuthStackResult<ProfileYearView> {
    let year = validate_tax_year(request.tax_year)?;
    let (user_id, session_id) = require_verified_actor(auth).await?;
    let loaded = crate::store::load_tax_profile_by_id(&profile_id).await?;
    if !loaded.state.exists {
        return Err(AuthStackError::not_found("tax profile not found"));
    }
    require_manage(&loaded.state, &user_id, session_id.as_deref()).await?;
    let mut seen = std::collections::BTreeSet::new();
    let entries = request
        .form_codes
        .into_iter()
        .filter_map(|code| {
            let code = code.trim().to_ascii_uppercase();
            if code.is_empty() || !seen.insert(code.clone()) {
                return None;
            }
            let frequency = bir_form_frequency(&code).to_owned();
            Some((code, frequency))
        })
        .collect::<Vec<_>>();
    crate::store::set_year_forms(&profile_id, year, &entries).await
}

pub async fn clone_profile_year(
    profile_id: String,
    request: CloneProfileYearRequest,
    auth: RequestAuth,
) -> AuthStackResult<ProfileYearView> {
    let from_year = validate_tax_year(request.from_year)?;
    let to_year = validate_tax_year(request.to_year)?;
    let (user_id, session_id) = require_verified_actor(auth).await?;
    let loaded = crate::store::load_tax_profile_by_id(&profile_id).await?;
    if !loaded.state.exists {
        return Err(AuthStackError::not_found("tax profile not found"));
    }
    require_manage(&loaded.state, &user_id, session_id.as_deref()).await?;
    crate::store::clone_profile_year(&profile_id, from_year, to_year).await
}

async fn require_profile_access(
    profile_id: &str,
    auth: RequestAuth,
) -> AuthStackResult<(String, Option<String>)> {
    let session = authenticated_session_view(auth).await?;
    let user_id = session.user_id.ok_or(AuthStackError::AuthRequired)?;
    let loaded = crate::store::load_tax_profile_by_id(profile_id).await?;
    if !loaded.state.exists {
        return Err(AuthStackError::not_found("tax profile not found"));
    }
    let is_account = loaded.state.account_id.as_deref() == Some(user_id.as_str());
    let is_owner = loaded.state.owner_user_id().as_deref() == Some(user_id.as_str());
    let is_personal_holder = matches!(
        &loaded.state.holder,
        Some(AccountHolder::Personal { user_id: holder }) if holder == &user_id
    );
    if !is_account && !is_owner && !is_personal_holder {
        ensure_holder_or_org(&loaded.state, &user_id, session.session_id.as_deref()).await?;
    }
    Ok((user_id, session.session_id))
}

async fn require_manage(
    state: &crate::domain::TaxProfile,
    user_id: &str,
    session_id: Option<&str>,
) -> AuthStackResult<()> {
    if state.account_id.as_deref() == Some(user_id) {
        return Ok(());
    }
    ensure_holder_or_org(state, user_id, session_id).await
}

async fn ensure_personal_single_tin(
    user_id: &str,
    incoming: &RegistrationKey,
) -> AuthStackResult<()> {
    let existing = crate::store::list_tax_profiles_for(user_id, None).await?;
    let Some(anchor) = existing
        .into_iter()
        .find(|profile| profile.holder_kind == "personal" && !profile.is_archived)
    else {
        return Ok(());
    };
    let loaded = crate::store::load_tax_profile_by_id(&anchor.profile_id).await?;
    let existing_hash = loaded.state.identity_hash.ok_or_else(|| {
        AuthStackError::store("personal tax profile is missing an identity hash")
    })?;
    let existing_branch = crate::domain::BranchCode::parse(
        loaded
            .state
            .branch_code
            .as_deref()
            .unwrap_or(crate::domain::BranchCode::HEAD_OFFICE),
    )
    .map_err(AuthStackError::validation)?;
    let pepper = crate::store::tin_identity_pepper().await?;
    if !same_personal_tin(&pepper, &existing_branch, &existing_hash, &incoming.tin_root) {
        return Err(AuthStackError::validation(
            "This personal account already holds a TIN. Add a branch under that TIN, or manage another TIN from a firm workspace.",
        ));
    }
    Ok(())
}

fn validate_tax_year(tax_year: i16) -> AuthStackResult<i16> {
    if (1990..=2100).contains(&tax_year) {
        Ok(tax_year)
    } else {
        Err(AuthStackError::validation("tax year is out of range"))
    }
}

async fn attested_existing(
    request: &TaxProfileClaimRequest,
    user_id: &str,
) -> AuthStackResult<(String, TinIdentityHash, u64, crate::domain::ProofMethod)> {
    let registration = parse_registration(&request.tin_root, &request.branch_code)?;
    let proof = verify_tin_ownership(&registration, user_id).await?;
    let pepper = crate::store::tin_identity_pepper().await?;
    let identity_hash =
        TinIdentityHash::compute(&pepper, &registration.tin_root, &registration.branch_code);
    let loaded = crate::store::load_tax_profile_by_identity_hash(&identity_hash)
        .await?
        .ok_or_else(|| AuthStackError::not_found("tax profile not found"))?;
    if !loaded.state.exists {
        return Err(AuthStackError::not_found("tax profile not found"));
    }
    let profile_id = loaded
        .state
        .profile_id
        .clone()
        .ok_or_else(|| AuthStackError::not_found("tax profile not found"))?;
    let expected = request.expected_revision.unwrap_or(loaded.revision);
    Ok((profile_id, identity_hash, expected, proof.method))
}

async fn require_verified_actor(auth: RequestAuth) -> AuthStackResult<(String, Option<String>)> {
    let session = authenticated_session_view(auth).await?;
    let user_id = session.user_id.ok_or(AuthStackError::AuthRequired)?;
    let status = crate::store::user_status(&user_id).await?;
    if status == "pending_verification" {
        return Err(AuthStackError::validation(
            "verify your email before creating or claiming tax profiles",
        ));
    }
    if status == "disabled" {
        return Err(AuthStackError::Forbidden);
    }
    Ok((user_id, session.session_id))
}

async fn resolve_holder(
    user_id: &str,
    session_id: Option<&str>,
    organization_id: Option<&str>,
) -> AuthStackResult<AccountHolder> {
    let Some(organization_id) = organization_id.map(str::trim).filter(|id| !id.is_empty()) else {
        return Ok(AccountHolder::Personal {
            user_id: user_id.to_owned(),
        });
    };
    let session_id = session_id.ok_or(AuthStackError::AuthRequired)?;
    crate::auth_product::organization_for_session(session_id, organization_id).await?;
    Ok(AccountHolder::Organization {
        organization_id: organization_id.to_owned(),
    })
}

fn require_account(
    state: &crate::domain::TaxProfile,
    user_id: &str,
) -> AuthStackResult<()> {
    if state.account_id.as_deref() == Some(user_id) {
        Ok(())
    } else {
        Err(AuthStackError::Forbidden)
    }
}

async fn ensure_holder_or_org(
    state: &crate::domain::TaxProfile,
    user_id: &str,
    session_id: Option<&str>,
) -> AuthStackResult<()> {
    match &state.holder {
        Some(AccountHolder::Personal { user_id: holder }) if holder == user_id => Ok(()),
        Some(AccountHolder::Organization { organization_id }) => {
            let session_id = session_id.ok_or(AuthStackError::AuthRequired)?;
            crate::auth_product::organization_for_session(session_id, organization_id)
                .await
                .map(|_| ())
        }
        _ => Err(AuthStackError::Forbidden),
    }
}

fn merge_facts(
    current: Option<&CloudTaxProfileFacts>,
    request: &TaxProfilePatchRequest,
) -> AuthStackResult<CloudTaxProfileFacts> {
    let current = current.ok_or_else(|| AuthStackError::not_found("tax profile not found"))?;
    let taxpayer_type = match request.taxpayer_type.as_deref() {
        Some(raw) => TaxpayerType::parse(raw).map_err(crate::store::map_tax_profile_error)?,
        None => current.taxpayer_type,
    };
    Ok(CloudTaxProfileFacts {
        registered_name: request
            .registered_name
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(current.registered_name.as_str())
            .to_owned(),
        rdo_code: request
            .rdo_code
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(current.rdo_code.as_str())
            .to_owned(),
        line_of_business: request
            .line_of_business
            .clone()
            .unwrap_or_else(|| current.line_of_business.clone()),
        registered_address: request
            .registered_address
            .clone()
            .unwrap_or_else(|| current.registered_address.clone()),
        zip_code: request
            .zip_code
            .clone()
            .unwrap_or_else(|| current.zip_code.clone()),
        phone: request
            .phone
            .clone()
            .unwrap_or_else(|| current.phone.clone()),
        email: request
            .email
            .clone()
            .unwrap_or_else(|| current.email.clone()),
        taxpayer_type,
        tax_classification: request
            .tax_classification
            .clone()
            .or_else(|| current.tax_classification.clone()),
        is_vat_registered: request
            .is_vat_registered
            .unwrap_or(current.is_vat_registered),
        eopt_tier: current.eopt_tier.clone(),
        business_start_date: match request.business_start_date.as_deref() {
            None => current.business_start_date.clone(),
            Some(value) => {
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_owned())
                }
            }
        },
        birth_date: current.birth_date.clone(),
        atc_codes: current.atc_codes.clone(),
    })
}

async fn verify_tin_ownership(
    registration: &RegistrationKey,
    user_id: &str,
) -> AuthStackResult<crate::domain::OwnershipProof> {
    if feature_enabled("ORUS_FAKE_VERIFIER", false).await {
        return FakeOrusVerifier
            .verify(registration, user_id)
            .map_err(map_orus_error);
    }
    UnconfiguredOrusVerifier
        .verify(registration, user_id)
        .map_err(map_orus_error)
}

fn parse_registration(tin_root: &str, branch_code: &str) -> AuthStackResult<RegistrationKey> {
    RegistrationKey::parse(tin_root, branch_code).map_err(AuthStackError::validation)
}

fn map_orus_error(error: crate::domain::OrusError) -> AuthStackError {
    match error {
        crate::domain::OrusError::NotConfigured => AuthStackError::configuration(error.to_string()),
        crate::domain::OrusError::Rejected { reason } => AuthStackError::validation(reason),
    }
}

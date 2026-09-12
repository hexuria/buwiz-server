//! Tax-profile application services (verified-email gate + exclusive ownership).

use crate::contracts::{
    TaxProfileClaimRequest, TaxProfileCreateRequest, TaxProfileListResponse,
    TaxProfileTransferRequest, TaxProfileView,
};
use crate::domain::{
    AccountHolder, FakeOrusVerifier, TaxProfileCommand, TinOwnershipVerifier,
    UnconfiguredOrusVerifier,
};
use crate::domain::tin::RegistrationKey;
use crate::error::{AuthStackError, AuthStackResult};

use super::{RequestAuth, authenticated_session_view, feature_enabled};

pub async fn list_tax_profiles(
    organization_id: Option<String>,
    auth: RequestAuth,
) -> AuthStackResult<TaxProfileListResponse> {
    let session = authenticated_session_view(auth.clone()).await?;
    let user_id = session
        .user_id
        .ok_or(AuthStackError::AuthRequired)?;
    let org_id = organization_id
        .or(session.tenant_id.clone())
        .filter(|value| !value.trim().is_empty());
    if let (Some(organization_id), Some(session_id)) =
        (org_id.as_deref(), session.session_id.as_deref())
    {
        crate::auth_product::organization_for_session(session_id, organization_id).await?;
    }
    let profiles =
        crate::store::list_tax_profiles_for(&user_id, org_id.as_deref()).await?;
    Ok(TaxProfileListResponse { profiles })
}

pub async fn create_tax_profile(
    request: TaxProfileCreateRequest,
    auth: RequestAuth,
) -> AuthStackResult<TaxProfileView> {
    let (user_id, session_id) = require_verified_actor(auth.clone()).await?;
    let registration = parse_registration(&request.tin_root, &request.branch_code)?;
    let facts = crate::store::facts_from_create(&request)?;
    let holder = resolve_holder(&user_id, session_id.as_deref(), request.organization_id.as_deref())
        .await?;
    let profile_id = crate::store::new_uuid_v4()?;
    let expected = request.expected_revision.unwrap_or(0);
    let (state, revision) = crate::store::commit_tax_profile_command(
        &registration,
        TaxProfileCommand::Create {
            profile_id,
            holder,
            facts,
            actor_user_id: user_id,
        },
        expected,
    )
    .await?;
    crate::store::tax_profile_view_from_state(&registration, &state, revision)
}

pub async fn claim_tax_profile(
    request: TaxProfileClaimRequest,
    auth: RequestAuth,
) -> AuthStackResult<TaxProfileView> {
    let (user_id, _) = require_verified_actor(auth).await?;
    let registration = parse_registration(&request.tin_root, &request.branch_code)?;
    let proof = verify_tin_ownership(&registration, &user_id).await?;
    let expected = match request.expected_revision {
        Some(revision) => revision,
        None => crate::store::load_tax_profile(&registration).await?.revision,
    };
    let (state, revision) = crate::store::commit_tax_profile_command(
        &registration,
        TaxProfileCommand::Claim {
            claimant: AccountHolder::Personal {
                user_id: user_id.clone(),
            },
            actor_user_id: user_id,
            proof,
        },
        expected,
    )
    .await?;
    crate::store::tax_profile_view_from_state(&registration, &state, revision)
}

pub async fn reclaim_tax_profile(
    request: TaxProfileClaimRequest,
    auth: RequestAuth,
) -> AuthStackResult<TaxProfileView> {
    let (user_id, _) = require_verified_actor(auth).await?;
    let registration = parse_registration(&request.tin_root, &request.branch_code)?;
    let proof = verify_tin_ownership(&registration, &user_id).await?;
    let expected = match request.expected_revision {
        Some(revision) => revision,
        None => crate::store::load_tax_profile(&registration).await?.revision,
    };
    let (state, revision) = crate::store::commit_tax_profile_command(
        &registration,
        TaxProfileCommand::Reclaim {
            actor_user_id: user_id,
            proof,
        },
        expected,
    )
    .await?;
    crate::store::tax_profile_view_from_state(&registration, &state, revision)
}

pub async fn transfer_tax_profile(
    request: TaxProfileTransferRequest,
    auth: RequestAuth,
) -> AuthStackResult<TaxProfileView> {
    let (user_id, session_id) = require_verified_actor(auth).await?;
    let registration = parse_registration(&request.tin_root, &request.branch_code)?;
    resolve_holder(
        &user_id,
        session_id.as_deref(),
        Some(request.organization_id.as_str()),
    )
    .await?;
    let expected = match request.expected_revision {
        Some(revision) => revision,
        None => crate::store::load_tax_profile(&registration).await?.revision,
    };
    let (state, revision) = crate::store::commit_tax_profile_command(
        &registration,
        TaxProfileCommand::Transfer {
            new_holder: AccountHolder::Organization {
                organization_id: request.organization_id,
            },
            actor_user_id: user_id,
        },
        expected,
    )
    .await?;
    crate::store::tax_profile_view_from_state(&registration, &state, revision)
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
    RegistrationKey::parse(tin_root, branch_code)
        .map_err(AuthStackError::validation)
}

fn map_orus_error(error: crate::domain::OrusError) -> AuthStackError {
    match error {
        crate::domain::OrusError::NotConfigured => AuthStackError::configuration(error.to_string()),
        crate::domain::OrusError::Rejected { reason } => AuthStackError::validation(reason),
    }
}

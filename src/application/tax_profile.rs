//! Tax-profile application services (verified-email gate + exclusive ownership).

use crate::contracts::{
    FirmPublisherAddRequest, FirmPublisherListResponse, OwnerInviteCreateRequest,
    OwnerInviteCreated, OwnerInvitePreview, OwnerInviteView,
    MeOrg, MeResponse, PassAskRequest, PassChoice, PassListResponse, PassOfferRequest,
    PassPermissionInfo, PassPresetInfo, PassUpdateRequest, TaxProfileClaimRequest,
    TaxProfileCreateRequest, TaxProfileListResponse, TaxProfilePatchRequest,
    TaxProfileView,
};
use crate::domain::{
    pass_end_from_date, tin_last4, AccountHolder, CloudTaxProfileFacts, FakeOrusVerifier,
    PassGrantee, PassPermission, PassPermissions, PassPreset, ProfileAccess, ProfileAction,
    TaxProfile, TaxProfileCommand, TaxpayerType, TinIdentityHash, TinOwnershipVerifier,
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
    if let Some(organization_id) = org_id.as_deref() {
        // Membership is checked through the session; without one, a
        // workspace view cannot be verified.
        let session_id = session.session_id.as_deref().ok_or(AuthStackError::Forbidden)?;
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
    let standing = authorize(&loaded.state, &user_id, session.session_id.as_deref(), Gate::View).await?;
    view_for(&profile_id, &user_id, standing.access).await
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
    view_after(&profile_id, &user_id, session_id.as_deref()).await
}

pub async fn patch_tax_profile(
    profile_id: String,
    request: TaxProfilePatchRequest,
    auth: RequestAuth,
) -> AuthStackResult<TaxProfileView> {
    let (user_id, session_id) = require_verified_actor(auth).await?;
    let loaded = crate::store::load_tax_profile_by_id(&profile_id).await?;
    if !loaded.state.exists {
        return Err(AuthStackError::not_found("tax profile not found"));
    }
    let standing = authorize(&loaded.state, &user_id, session_id.as_deref(), Gate::Edit).await?;
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
    if let Some(expected) = request.updated_at.as_deref().filter(|value| !value.is_empty())
        && current_view.updated_at.as_deref() != Some(expected)
    {
        return Err(crate::store::map_tax_profile_error(
            crate::domain::TaxProfileError::StaleWrite,
        ));
    }
    let facts = merge_facts(loaded.state.facts.as_ref(), &request)?;
    crate::store::commit_tax_profile_command(
        &profile_id,
        TaxProfileCommand::UpdateTaxProfileIdentity {
            display_name: request.display_name.clone(),
            facts: Some(facts),
            expected_updated_at: None,
            actor_user_id: user_id.clone(),
            actor_orgs: standing.actor_orgs,
            occurred_at: crate::store::rfc3339_now(),
        },
        loaded.revision,
    )
    .await?;
    view_after(&profile_id, &user_id, session_id.as_deref()).await
}

pub async fn archive_tax_profile(
    profile_id: String,
    auth: RequestAuth,
) -> AuthStackResult<TaxProfileView> {
    let (user_id, session_id) = require_verified_actor(auth).await?;
    let loaded = crate::store::load_tax_profile_by_id(&profile_id).await?;
    if !loaded.state.exists {
        return Err(AuthStackError::not_found("tax profile not found"));
    }
    let standing =
        authorize(&loaded.state, &user_id, session_id.as_deref(), Gate::Archive).await?;
    crate::store::commit_tax_profile_command(
        &profile_id,
        TaxProfileCommand::ArchiveTaxProfile {
            actor_user_id: user_id.clone(),
            actor_orgs: standing.actor_orgs,
            occurred_at: crate::store::rfc3339_now(),
        },
        loaded.revision,
    )
    .await?;
    view_after(&profile_id, &user_id, session_id.as_deref()).await
}

pub async fn restore_tax_profile(
    profile_id: String,
    auth: RequestAuth,
) -> AuthStackResult<TaxProfileView> {
    let (user_id, session_id) = require_verified_actor(auth).await?;
    let loaded = crate::store::load_tax_profile_by_id(&profile_id).await?;
    if !loaded.state.exists {
        return Err(AuthStackError::not_found("tax profile not found"));
    }
    let standing =
        authorize(&loaded.state, &user_id, session_id.as_deref(), Gate::Archive).await?;
    crate::store::commit_tax_profile_command(
        &profile_id,
        TaxProfileCommand::RestoreTaxProfile {
            actor_user_id: user_id.clone(),
            actor_orgs: standing.actor_orgs,
            occurred_at: crate::store::rfc3339_now(),
        },
        loaded.revision,
    )
    .await?;
    view_after(&profile_id, &user_id, session_id.as_deref()).await
}

pub async fn claim_tax_profile(
    request: TaxProfileClaimRequest,
    auth: RequestAuth,
) -> AuthStackResult<TaxProfileView> {
    let (user_id, session_id) = require_verified_actor(auth).await?;
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
    view_after(&profile_id, &user_id, session_id.as_deref()).await
}

pub async fn reclaim_tax_profile(
    request: TaxProfileClaimRequest,
    auth: RequestAuth,
) -> AuthStackResult<TaxProfileView> {
    let (user_id, session_id) = require_verified_actor(auth).await?;
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
    view_after(&profile_id, &user_id, session_id.as_deref()).await
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

// ── Access passes ───────────────────────────────────────────────────────────

/// Open passes on profiles the caller owns or held by the caller's
/// workspaces, plus the permission catalog and presets.
pub async fn list_passes(auth: RequestAuth) -> AuthStackResult<PassListResponse> {
    let session = authenticated_session_view(auth.clone()).await?;
    let user_id = session.user_id.ok_or(AuthStackError::AuthRequired)?;
    pass_list(&user_id, auth).await
}

/// The owner offers a pass to a workspace (named by its URL slug or id).
pub async fn offer_pass(
    profile_id: String,
    request: PassOfferRequest,
    auth: RequestAuth,
) -> AuthStackResult<PassListResponse> {
    let (user_id, _) = require_verified_actor(auth.clone()).await?;
    let organization_id = match (
        request
            .organization_slug
            .as_deref()
            .map(str::trim)
            .filter(|slug| !slug.is_empty()),
        request
            .organization_id
            .as_deref()
            .map(str::trim)
            .filter(|id| !id.is_empty()),
    ) {
        (Some(slug), _) => {
            crate::store::resolve_org_id_for_slug(&slug.to_ascii_lowercase()).await?
        }
        (None, Some(id)) => crate::store::active_organization_id(id)
            .await?
            .ok_or_else(|| AuthStackError::not_found("workspace not found"))?,
        (None, None) => {
            return Err(AuthStackError::validation(
                "name the workspace to give access to",
            ))
        }
    };
    let loaded = load_existing(&profile_id).await?;
    let permissions = permissions_from_choice(&request.choice, PassPermissions::empty())?;
    let now = crate::store::rfc3339_now();
    crate::store::commit_tax_profile_command(
        &profile_id,
        TaxProfileCommand::OfferPass {
            pass_id: crate::store::new_uuid_v4()?,
            grantee: PassGrantee::Organization { organization_id },
            permissions,
            expires_at: end_from_choice(&request.choice)?,
            actor_user_id: user_id.clone(),
            occurred_at: now,
        },
        loaded.revision,
    )
    .await?;
    pass_list(&user_id, auth).await
}

/// A workspace asks the owner for a pass, proving it knows the TIN.
pub async fn ask_for_pass(
    request: PassAskRequest,
    auth: RequestAuth,
) -> AuthStackResult<PassListResponse> {
    let (user_id, session_id) = require_verified_actor(auth.clone()).await?;
    let organization_id = request.organization_id.trim().to_owned();
    if !manages_passes_for(session_id.as_deref(), &organization_id).await {
        return Err(AuthStackError::Forbidden);
    }
    let registration = parse_registration(&request.tin_root, &request.branch_code)?;
    let pepper = crate::store::tin_identity_pepper().await?;
    let identity_hash =
        TinIdentityHash::compute(&pepper, &registration.tin_root, &registration.branch_code);
    let loaded = crate::store::load_tax_profile_by_identity_hash(&identity_hash)
        .await?
        .filter(|loaded| loaded.state.exists)
        .ok_or_else(|| AuthStackError::not_found("no tax profile for that TIN and branch"))?;
    refuse_legacy(&loaded)?;
    let profile_id = loaded
        .state
        .profile_id
        .clone()
        .ok_or_else(|| AuthStackError::not_found("tax profile not found"))?;
    let choice = PassChoice {
        preset: request.preset.clone(),
        permissions: request.permissions.clone(),
        allow_publish: false,
        ends_on: None,
    };
    crate::store::commit_tax_profile_command(
        &profile_id,
        TaxProfileCommand::RequestPass {
            pass_id: crate::store::new_uuid_v4()?,
            grantee: PassGrantee::Organization {
                organization_id: organization_id.clone(),
            },
            permissions: permissions_from_choice(&choice, PassPermissions::empty())?,
            identity_hash,
            actor_user_id: user_id.clone(),
            actor_managed_orgs: vec![organization_id],
            occurred_at: crate::store::rfc3339_now(),
        },
        loaded.revision,
    )
    .await?;
    pass_list(&user_id, auth).await
}

/// The workspace accepts an offered pass.
pub async fn accept_pass(
    profile_id: String,
    pass_id: String,
    auth: RequestAuth,
) -> AuthStackResult<PassListResponse> {
    let (user_id, session_id) = require_verified_actor(auth.clone()).await?;
    let loaded = load_existing(&profile_id).await?;
    let managed = managed_grantee_orgs(&loaded.state, &pass_id, session_id.as_deref()).await?;
    crate::store::commit_tax_profile_command(
        &profile_id,
        TaxProfileCommand::AcceptPass {
            pass_id,
            actor_user_id: user_id.clone(),
            actor_managed_orgs: managed,
            occurred_at: crate::store::rfc3339_now(),
        },
        loaded.revision,
    )
    .await?;
    pass_list(&user_id, auth).await
}

/// The owner approves a request. An empty choice keeps what was asked for.
pub async fn approve_pass(
    profile_id: String,
    pass_id: String,
    request: PassUpdateRequest,
    auth: RequestAuth,
) -> AuthStackResult<PassListResponse> {
    let (user_id, _) = require_verified_actor(auth.clone()).await?;
    let loaded = load_existing(&profile_id).await?;
    let current = current_pass_permissions(&loaded.state, &pass_id)?;
    crate::store::commit_tax_profile_command(
        &profile_id,
        TaxProfileCommand::ApprovePass {
            pass_id,
            permissions: Some(permissions_from_choice(&request.choice, current)?),
            expires_at: end_from_choice(&request.choice)?,
            actor_user_id: user_id.clone(),
            occurred_at: crate::store::rfc3339_now(),
        },
        loaded.revision,
    )
    .await?;
    pass_list(&user_id, auth).await
}

/// The owner changes an offered or active pass. Publishing stays on only
/// while `allow_publish` is sent as true.
pub async fn change_pass(
    profile_id: String,
    pass_id: String,
    request: PassUpdateRequest,
    auth: RequestAuth,
) -> AuthStackResult<PassListResponse> {
    let (user_id, _) = require_verified_actor(auth.clone()).await?;
    let loaded = load_existing(&profile_id).await?;
    let current = current_pass_permissions(&loaded.state, &pass_id)?;
    // A change that does not mention the end date keeps it; `""` clears it.
    let expires_at = match request.choice.ends_on.as_deref() {
        None => loaded.state.pass(&pass_id).and_then(|pass| pass.expires_at.clone()),
        Some(_) => end_from_choice(&request.choice)?,
    };
    crate::store::commit_tax_profile_command(
        &profile_id,
        TaxProfileCommand::ChangePass {
            pass_id,
            permissions: permissions_from_choice(&request.choice, current)?,
            expires_at,
            actor_user_id: user_id.clone(),
            occurred_at: crate::store::rfc3339_now(),
        },
        loaded.revision,
    )
    .await?;
    pass_list(&user_id, auth).await
}

/// Either side ends a pass: the owner revokes or declines; the workspace
/// declines an offer, withdraws its request, or gives the pass back.
pub async fn end_pass(
    profile_id: String,
    pass_id: String,
    auth: RequestAuth,
) -> AuthStackResult<PassListResponse> {
    let (user_id, session_id) = require_verified_actor(auth.clone()).await?;
    let loaded = load_existing(&profile_id).await?;
    let managed = managed_grantee_orgs(&loaded.state, &pass_id, session_id.as_deref())
        .await
        .unwrap_or_default();
    crate::store::commit_tax_profile_command(
        &profile_id,
        TaxProfileCommand::EndPass {
            pass_id,
            actor_user_id: user_id.clone(),
            actor_managed_orgs: managed,
            occurred_at: crate::store::rfc3339_now(),
        },
        loaded.revision,
    )
    .await?;
    pass_list(&user_id, auth).await
}

async fn pass_list(user_id: &str, auth: RequestAuth) -> AuthStackResult<PassListResponse> {
    let org_ids: Vec<String> = crate::application::list_organizations(auth)
        .await?
        .organizations
        .into_iter()
        .map(|org| org.organization_id)
        .collect();
    Ok(PassListResponse {
        passes: crate::store::list_passes_for(user_id, &org_ids).await?,
        invites: crate::store::list_owner_invites_for(&org_ids).await?,
        permission_catalog: PassPermission::ALL
            .into_iter()
            .map(|permission| PassPermissionInfo {
                id: permission.as_str().to_owned(),
                label: permission.label().to_owned(),
            })
            .collect(),
        presets: PassPreset::ALL
            .into_iter()
            .map(|preset| PassPresetInfo {
                id: preset.as_str().to_owned(),
                label: preset.label().to_owned(),
                permissions: preset.permissions().ids(),
                is_default: preset == PassPreset::DEFAULT,
            })
            .collect(),
    })
}

/// A profile for a pass step: it must exist and have its event history.
async fn load_existing(profile_id: &str) -> AuthStackResult<crate::store::LoadedTaxProfile> {
    let loaded = crate::store::load_tax_profile_by_id(profile_id).await?;
    if !loaded.state.exists {
        return Err(AuthStackError::not_found("tax profile not found"));
    }
    refuse_legacy(&loaded)?;
    Ok(loaded)
}

const LEGACY_PROFILE: &str =
    "this profile was stored before access passes existed and cannot be shared yet";

fn refuse_legacy(loaded: &crate::store::LoadedTaxProfile) -> AuthStackResult<()> {
    if loaded.from_projection {
        Err(AuthStackError::validation(LEGACY_PROFILE))
    } else {
        Ok(())
    }
}

/// The pass's permissions without publishing, as the base for an approve or
/// change that names no new list.
fn current_pass_permissions(state: &TaxProfile, pass_id: &str) -> AuthStackResult<PassPermissions> {
    let pass = state
        .pass(pass_id)
        .ok_or_else(|| AuthStackError::not_found("access pass not found"))?;
    Ok(PassPermissions::of(
        &pass
            .permissions
            .iter()
            .filter(|permission| *permission != PassPermission::FilingsPublish)
            .collect::<Vec<_>>(),
    ))
}

/// Permissions from a give-access choice: the explicit list, else the
/// preset, else `base` (or the default preset when `base` is empty).
fn permissions_from_choice(
    choice: &PassChoice,
    base: PassPermissions,
) -> AuthStackResult<PassPermissions> {
    let ids = match (&choice.permissions, choice.preset.as_deref().map(str::trim)) {
        (Some(list), _) => list.clone(),
        (None, Some(preset)) if !preset.is_empty() => PassPreset::parse(preset)
            .ok_or_else(|| AuthStackError::validation(format!("unknown preset `{preset}`")))?
            .permissions()
            .ids(),
        _ if !base.is_empty() => base.ids(),
        _ => PassPreset::DEFAULT.permissions().ids(),
    };
    PassPermissions::from_choice(&ids, choice.allow_publish).map_err(AuthStackError::validation)
}

fn end_from_choice(choice: &PassChoice) -> AuthStackResult<Option<String>> {
    choice
        .ends_on
        .as_deref()
        .map(str::trim)
        .filter(|date| !date.is_empty())
        .map(|date| pass_end_from_date(date).map_err(AuthStackError::validation))
        .transpose()
}

/// `[org]` when the pass is held by an organization in which the caller may
/// manage passes (`organization.update`), else empty.
async fn managed_grantee_orgs(
    state: &TaxProfile,
    pass_id: &str,
    session_id: Option<&str>,
) -> AuthStackResult<Vec<String>> {
    let pass = state
        .pass(pass_id)
        .ok_or_else(|| AuthStackError::not_found("access pass not found"))?;
    let org = pass.grantee.id().to_owned();
    Ok(if manages_passes_for(session_id, &org).await {
        vec![org]
    } else {
        Vec::new()
    })
}

async fn manages_passes_for(session_id: Option<&str>, organization_id: &str) -> bool {
    org_permissions(session_id, organization_id)
        .await
        .is_some_and(|permissions| {
            permissions
                .iter()
                .any(|p| p == crate::access::PermissionId::ORGANIZATION_UPDATE.as_str())
        })
}

// ── Owner invites ───────────────────────────────────────────────────────────

/// The firm holding a draft invites the taxpayer (by email) to own it. The
/// link carries the secret in its fragment and is returned only this once.
pub async fn invite_owner(
    profile_id: String,
    request: OwnerInviteCreateRequest,
    auth: RequestAuth,
) -> AuthStackResult<OwnerInviteCreated> {
    let (user_id, session_id) = require_verified_actor(auth).await?;
    let days = request.days.unwrap_or(crate::domain::INVITE_LIFETIME_DAYS as u32);
    if !(1..=crate::domain::INVITE_LIFETIME_DAYS as u32).contains(&days) {
        return Err(AuthStackError::validation(format!(
            "an invite works for 1 to {} days",
            crate::domain::INVITE_LIFETIME_DAYS
        )));
    }
    let loaded = load_existing(&profile_id).await?;
    let managed = match loaded.state.org_id() {
        Some(org) if manages_passes_for(session_id.as_deref(), &org).await => vec![org],
        _ => Vec::new(),
    };
    let invite_id = crate::store::new_uuid_v4()?;
    let token = crate::store::new_secret_token()?;
    let (state, _) = crate::store::commit_tax_profile_command(
        &profile_id,
        TaxProfileCommand::InviteOwner {
            invite_id: invite_id.clone(),
            email: request.email,
            token_hash: crate::store::sha256_b64url(&token),
            expires_at: crate::store::rfc3339_in_days(days),
            actor_user_id: user_id,
            actor_managed_orgs: managed,
            occurred_at: crate::store::rfc3339_now(),
        },
        loaded.revision,
    )
    .await?;
    let invite = state
        .invite(&invite_id)
        .ok_or_else(|| AuthStackError::store("invite missing after commit"))?;
    Ok(OwnerInviteCreated {
        invite: OwnerInviteView {
            invite_id: invite.invite_id.clone(),
            profile_id: profile_id.clone(),
            organization_id: invite.organization_id.clone(),
            profile_name: state.display_name.clone().unwrap_or_default(),
            tin_last4: state.tin_last4.clone().unwrap_or_default(),
            email: invite.email.clone(),
            status: invite.status.as_str().to_owned(),
            expires_at: invite.expires_at.clone(),
            created_at: invite.created_at.clone(),
        },
        link_path: format!("/invites/{invite_id}#t={token}"),
    })
}

/// The firm withdraws an open invite.
pub async fn withdraw_owner_invite(
    profile_id: String,
    invite_id: String,
    auth: RequestAuth,
) -> AuthStackResult<PassListResponse> {
    let (user_id, session_id) = require_verified_actor(auth.clone()).await?;
    let loaded = load_existing(&profile_id).await?;
    let managed = match loaded.state.invite(&invite_id) {
        Some(invite) if manages_passes_for(session_id.as_deref(), &invite.organization_id).await => {
            vec![invite.organization_id.clone()]
        }
        _ => Vec::new(),
    };
    crate::store::commit_tax_profile_command(
        &profile_id,
        TaxProfileCommand::WithdrawOwnerInvite {
            invite_id,
            actor_user_id: user_id.clone(),
            actor_managed_orgs: managed,
            occurred_at: crate::store::rfc3339_now(),
        },
        loaded.revision,
    )
    .await?;
    pass_list(&user_id, auth).await
}

/// The profile behind an invite, when `token` is its secret.
async fn invited_profile(
    invite_id: &str,
    token: &str,
) -> AuthStackResult<(crate::store::LoadedTaxProfile, crate::domain::OwnerInvite)> {
    let not_found = || AuthStackError::not_found("invite not found");
    let profile_id = crate::store::profile_id_for_invite(invite_id)
        .await?
        .ok_or_else(not_found)?;
    let loaded = load_existing(&profile_id).await?;
    let hash = crate::store::sha256_b64url(token.trim());
    let invite = loaded
        .state
        .invite(invite_id)
        .filter(|invite| !token.trim().is_empty() && invite.token_hash == hash)
        .cloned()
        .ok_or_else(not_found)?;
    Ok((loaded, invite))
}

/// What the invited taxpayer sees before accepting.
pub async fn preview_owner_invite(
    invite_id: String,
    token: String,
    auth: RequestAuth,
) -> AuthStackResult<OwnerInvitePreview> {
    let (user_id, _) = require_verified_actor(auth).await?;
    let (loaded, invite) = invited_profile(&invite_id, &token).await?;
    let now = crate::store::rfc3339_now();
    let status = if invite.status == crate::domain::InviteStatus::Open
        && !crate::domain::instant_before(&now, &invite.expires_at)
    {
        "expired".to_owned()
    } else {
        invite.status.as_str().to_owned()
    };
    let email_matches =
        crate::store::verified_email(&user_id).await?.as_deref() == Some(invite.email.as_str());
    Ok(OwnerInvitePreview {
        invite_id,
        profile_name: loaded.state.display_name.clone().unwrap_or_default(),
        tin_last4: loaded.state.tin_last4.clone().unwrap_or_default(),
        firm_name: crate::store::organization_name(&invite.organization_id)
            .await?
            .unwrap_or_default(),
        email: invite.email,
        status,
        expires_at: invite.expires_at,
        email_matches,
    })
}

/// The invited taxpayer accepts: they own the profile, and the firm keeps a
/// Full pass without publishing that they can change or end.
pub async fn accept_owner_invite(
    invite_id: String,
    token: String,
    auth: RequestAuth,
) -> AuthStackResult<TaxProfileView> {
    let (user_id, session_id) = require_verified_actor(auth).await?;
    let email = crate::store::verified_email(&user_id)
        .await?
        .ok_or_else(|| AuthStackError::validation("verify your email before accepting an invite"))?;
    let (loaded, _) = invited_profile(&invite_id, &token).await?;
    let profile_id = loaded
        .state
        .profile_id
        .clone()
        .ok_or_else(|| AuthStackError::not_found("tax profile not found"))?;
    crate::store::commit_tax_profile_command(
        &profile_id,
        TaxProfileCommand::AcceptOwnerInvite {
            invite_id,
            token_hash: crate::store::sha256_b64url(token.trim()),
            actor_user_id: user_id.clone(),
            actor_verified_email: email,
            firm_pass_id: crate::store::new_uuid_v4()?,
            occurred_at: crate::store::rfc3339_now(),
        },
        loaded.revision,
    )
    .await?;
    view_after(&profile_id, &user_id, session_id.as_deref()).await
}

// ── Firm publishers ─────────────────────────────────────────────────────────

/// The members a firm picked to publish to BIR. Any member may look.
pub async fn list_firm_publishers(
    organization_id: String,
    auth: RequestAuth,
) -> AuthStackResult<FirmPublisherListResponse> {
    let (_, session_id) = require_verified_actor(auth).await?;
    if org_permissions(session_id.as_deref(), &organization_id).await.is_none() {
        return Err(AuthStackError::Forbidden);
    }
    Ok(FirmPublisherListResponse {
        publishers: crate::store::list_firm_publishers(&organization_id).await?,
    })
}

/// Adds a member to the firm's publishers (`organization.update`, step-up).
pub async fn add_firm_publisher(
    organization_id: String,
    request: FirmPublisherAddRequest,
    auth: RequestAuth,
) -> AuthStackResult<FirmPublisherListResponse> {
    let user_id = require_publisher_manager(&organization_id, auth).await?;
    let email = crate::domain::normalize_invite_email(&request.email)
        .map_err(|_| AuthStackError::validation("enter the member's sign-in email"))?;
    if !crate::store::add_firm_publisher(&organization_id, &email, &user_id).await? {
        return Err(AuthStackError::not_found("no active member of this workspace has that email"));
    }
    Ok(FirmPublisherListResponse {
        publishers: crate::store::list_firm_publishers(&organization_id).await?,
    })
}

/// Removes a member from the firm's publishers (`organization.update`, step-up).
pub async fn remove_firm_publisher(
    organization_id: String,
    member_user_id: String,
    auth: RequestAuth,
) -> AuthStackResult<FirmPublisherListResponse> {
    require_publisher_manager(&organization_id, auth).await?;
    crate::store::remove_firm_publisher(&organization_id, member_user_id.trim()).await?;
    Ok(FirmPublisherListResponse {
        publishers: crate::store::list_firm_publishers(&organization_id).await?,
    })
}

async fn require_publisher_manager(organization_id: &str, auth: RequestAuth) -> AuthStackResult<String> {
    crate::application::validate_identifier("organization_id", organization_id)?;
    let (context, _) = crate::application::verified_context_and_permissions(auth, true).await?;
    crate::application::enforce_organization_scope(&context, organization_id).await?;
    crate::application::require_organization_permission(
        &context,
        organization_id,
        crate::access::PermissionId::ORGANIZATION_UPDATE.as_str(),
        crate::application::AssuranceRequirement::Aal2,
    )
    .await?;
    Ok(context.principal().user_id().as_str().to_owned())
}

// ── The gatekeeper ──────────────────────────────────────────────────────────

/// What a caller wants to do with a tax profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Gate {
    View,
    Edit,
    Archive,
}

/// The caller's standing on a profile and the organizations it was verified
/// against (commands carry them so the domain re-checks the same rule).
struct Standing {
    access: ProfileAccess,
    actor_orgs: Vec<String>,
}

/// The one gatekeeper for every tax-profile read and write.
///
/// The taxpayer always wins: the owner (the personal holder) may do
/// everything, always. A firm reaches someone else's profile only through a
/// working pass, which gives every member of that firm the pass's
/// permissions. A firm's own draft (a profile it created that no taxpayer
/// owns yet) gives its members the Full preset, without publishing or
/// sharing. Registering a profile grants nothing by itself.
async fn authorize(
    state: &TaxProfile,
    user_id: &str,
    session_id: Option<&str>,
    gate: Gate,
) -> AuthStackResult<Standing> {
    let standing = standing(state, user_id, session_id).await;
    let action = match gate {
        Gate::View => ProfileAction::View,
        Gate::Edit => ProfileAction::Edit,
        Gate::Archive => ProfileAction::Archive,
    };
    if !standing.access.allows(action) {
        return Err(AuthStackError::Forbidden);
    }
    Ok(standing)
}

async fn standing(state: &TaxProfile, user_id: &str, session_id: Option<&str>) -> Standing {
    let now = crate::store::rfc3339_now();
    let mut candidates: Vec<&str> = Vec::new();
    if let Some(AccountHolder::Organization { organization_id }) = &state.holder {
        candidates.push(organization_id);
    }
    for pass in state.passes.iter().filter(|pass| pass.is_usable(&now)) {
        if !candidates.contains(&pass.grantee.id()) {
            candidates.push(pass.grantee.id());
        }
    }
    let mut actor_orgs = Vec::new();
    for org in candidates {
        if org_permissions(session_id, org).await.is_some() {
            actor_orgs.push(org.to_owned());
        }
    }
    let mut access = state.access_for(user_id, &actor_orgs, &now);
    if access.has(PassPermission::FilingsPublish) && matches!(access, ProfileAccess::Pass(_)) {
        // Publishing also needs the firm's pick; if that can't be read,
        // publishing is off (fail closed).
        let publishers = crate::store::publisher_orgs(user_id, &actor_orgs)
            .await
            .unwrap_or_default();
        access = state.effective_access(user_id, &actor_orgs, &publishers, &now);
    }
    Standing { access, actor_orgs }
}

/// The caller's permissions in `organization_id`, or `None` when the caller
/// is not a member (checked through the caller's session).
async fn org_permissions(session_id: Option<&str>, organization_id: &str) -> Option<Vec<String>> {
    let summary = crate::auth_product::organization_for_session(session_id?, organization_id)
        .await
        .ok()?;
    Some(summary.permissions)
}

async fn view_for(
    profile_id: &str,
    user_id: &str,
    access: ProfileAccess,
) -> AuthStackResult<TaxProfileView> {
    let mut view = crate::store::fetch_tax_profile_view(profile_id, user_id).await?;
    crate::store::set_access(&mut view, access);
    Ok(view)
}

/// The profile as the caller sees it after a write.
async fn view_after(
    profile_id: &str,
    user_id: &str,
    session_id: Option<&str>,
) -> AuthStackResult<TaxProfileView> {
    let loaded = crate::store::load_tax_profile_by_id(profile_id).await?;
    let standing = standing(&loaded.state, user_id, session_id).await;
    view_for(profile_id, user_id, standing.access).await
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
        business_start_date: current.business_start_date.clone(),
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

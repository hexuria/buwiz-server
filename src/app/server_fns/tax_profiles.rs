#![allow(unused_imports)]

use super::common::*;
use crate::contracts::*;
use leptos::prelude::*;
use server_fn::ServerFnError;
use server_fn::codec::Json;

#[server(prefix = "/api/ui")]
pub async fn list_tax_profiles(
    organization_id: Option<String>,
) -> Result<TaxProfileListResponse, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        crate::application::list_tax_profiles(organization_id, server_fn_request_auth())
            .await
            .map_err(server_fn_error)
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = organization_id;
        unreachable!()
    }
}

#[server(prefix = "/api/ui")]
pub async fn create_tax_profile(
    tin_root: String,
    branch_code: String,
    registered_name: String,
    rdo_code: String,
    line_of_business: String,
    registered_address: String,
    zip_code: String,
    phone: String,
    email: String,
    taxpayer_type: String,
    tax_classification: Option<String>,
    is_vat_registered: bool,
    organization_id: Option<String>,
) -> Result<TaxProfileView, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        crate::application::create_tax_profile(
            TaxProfileCreateRequest {
                tin_root,
                branch_code,
                display_name: Some(registered_name.clone()),
                registered_name,
                rdo_code,
                line_of_business,
                registered_address,
                zip_code,
                phone,
                email,
                taxpayer_type,
                tax_classification,
                is_vat_registered,
                organization_id,
                org_id: None,
            },
            server_fn_request_auth(),
        )
        .await
        .map_err(server_fn_error)
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (
            tin_root,
            branch_code,
            registered_name,
            rdo_code,
            line_of_business,
            registered_address,
            zip_code,
            phone,
            email,
            taxpayer_type,
            tax_classification,
            is_vat_registered,
            organization_id,
        );
        unreachable!()
    }
}

#[server(prefix = "/api/ui")]
pub async fn patch_tax_profile(
    profile_id: String,
    display_name: Option<String>,
    registered_name: Option<String>,
    rdo_code: Option<String>,
    updated_at: Option<String>,
) -> Result<TaxProfileView, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        crate::application::patch_tax_profile(
            profile_id,
            TaxProfilePatchRequest {
                display_name,
                registered_name,
                rdo_code,
                line_of_business: None,
                registered_address: None,
                zip_code: None,
                phone: None,
                email: None,
                taxpayer_type: None,
                tax_classification: None,
                is_vat_registered: None,
                updated_at,
                tin_root: None,
                branch_code: None,
            },
            server_fn_request_auth(),
        )
        .await
        .map_err(server_fn_error)
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (profile_id, display_name, registered_name, rdo_code, updated_at);
        unreachable!()
    }
}

#[server(prefix = "/api/ui")]
pub async fn claim_tax_profile(
    tin_root: String,
    branch_code: String,
) -> Result<TaxProfileView, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        crate::application::claim_tax_profile(
            TaxProfileClaimRequest {
                tin_root,
                branch_code,
                expected_revision: None,
            },
            server_fn_request_auth(),
        )
        .await
        .map_err(server_fn_error)
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (tin_root, branch_code);
        unreachable!()
    }
}

#[server(prefix = "/api/ui")]
pub async fn reclaim_tax_profile(
    tin_root: String,
    branch_code: String,
) -> Result<TaxProfileView, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        crate::application::reclaim_tax_profile(
            TaxProfileClaimRequest {
                tin_root,
                branch_code,
                expected_revision: None,
            },
            server_fn_request_auth(),
        )
        .await
        .map_err(server_fn_error)
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (tin_root, branch_code);
        unreachable!()
    }
}

#[server(prefix = "/api/ui")]
pub async fn webmcp_enabled() -> Result<bool, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        Ok(crate::application::feature_enabled("WEBMCP_ENABLED", true).await)
    }
    #[cfg(not(feature = "ssr"))]
    unreachable!()
}

#[server(prefix = "/api/ui")]
pub async fn list_passes() -> Result<PassListResponse, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        crate::application::list_passes(server_fn_request_auth())
            .await
            .map_err(server_fn_error)
    }
    #[cfg(not(feature = "ssr"))]
    {
        unreachable!()
    }
}

/// The owner offers a pass. `permissions` excludes publishing, which only
/// `allow_publish` switches on.
#[server(prefix = "/api/ui", input = Json)]
pub async fn offer_pass(
    profile_id: String,
    organization_slug: String,
    permissions: Vec<String>,
    allow_publish: bool,
    ends_on: Option<String>,
) -> Result<PassListResponse, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        crate::application::offer_pass(
            profile_id,
            PassOfferRequest {
                organization_slug: Some(organization_slug),
                organization_id: None,
                choice: PassChoice {
                    preset: None,
                    permissions: Some(permissions),
                    allow_publish,
                    ends_on,
                },
            },
            server_fn_request_auth(),
        )
        .await
        .map_err(server_fn_error)
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (profile_id, organization_slug, permissions, allow_publish, ends_on);
        unreachable!()
    }
}

#[server(prefix = "/api/ui")]
pub async fn ask_for_pass(
    organization_id: String,
    tin_root: String,
    branch_code: String,
    preset: String,
) -> Result<PassListResponse, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        crate::application::ask_for_pass(
            PassAskRequest {
                organization_id,
                tin_root,
                branch_code,
                preset: Some(preset),
                permissions: None,
            },
            server_fn_request_auth(),
        )
        .await
        .map_err(server_fn_error)
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (organization_id, tin_root, branch_code, preset);
        unreachable!()
    }
}

/// One step on an existing pass: `accept` (workspace), `approve` (owner,
/// as asked) or `end` (either side).
#[server(prefix = "/api/ui")]
pub async fn pass_step(
    profile_id: String,
    pass_id: String,
    step: String,
) -> Result<PassListResponse, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let auth = server_fn_request_auth();
        match step.as_str() {
            "accept" => crate::application::accept_pass(profile_id, pass_id, auth).await,
            "approve" => {
                crate::application::approve_pass(
                    profile_id,
                    pass_id,
                    PassUpdateRequest::default(),
                    auth,
                )
                .await
            }
            "end" => crate::application::end_pass(profile_id, pass_id, auth).await,
            _ => Err(crate::error::AuthStackError::validation("unknown pass step")),
        }
        .map_err(server_fn_error)
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (profile_id, pass_id, step);
        unreachable!()
    }
}

#[server(prefix = "/api/ui")]
pub async fn list_agent_connections() -> Result<AgentConnectionListResponse, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        crate::application::list_agent_connections(server_fn_request_auth())
            .await
            .map_err(server_fn_error)
    }
    #[cfg(not(feature = "ssr"))]
    {
        unreachable!()
    }
}

/// Ends one of the caller's agent connections.
#[server(prefix = "/api/ui")]
pub async fn revoke_agent_connection(
    connection_id: String,
) -> Result<AgentConnectionListResponse, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        crate::application::revoke_agent_connection(connection_id, server_fn_request_auth())
            .await
            .map_err(server_fn_error)
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = connection_id;
        unreachable!()
    }
}

/// The firm holding a draft invites the taxpayer to own it. Returns the link
/// (shown once; the firm shares it itself).
#[server(prefix = "/api/ui")]
pub async fn invite_owner(
    profile_id: String,
    email: String,
    days: Option<u32>,
) -> Result<OwnerInviteCreated, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        crate::application::invite_owner(
            profile_id,
            OwnerInviteCreateRequest { email, days },
            server_fn_request_auth(),
        )
        .await
        .map_err(server_fn_error)
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (profile_id, email, days);
        unreachable!()
    }
}

#[server(prefix = "/api/ui")]
pub async fn withdraw_owner_invite(
    profile_id: String,
    invite_id: String,
) -> Result<PassListResponse, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        crate::application::withdraw_owner_invite(profile_id, invite_id, server_fn_request_auth())
            .await
            .map_err(server_fn_error)
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (profile_id, invite_id);
        unreachable!()
    }
}

#[server(prefix = "/api/ui")]
pub async fn preview_owner_invite(
    invite_id: String,
    token: String,
) -> Result<OwnerInvitePreview, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        crate::application::preview_owner_invite(invite_id, token, server_fn_request_auth())
            .await
            .map_err(server_fn_error)
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (invite_id, token);
        unreachable!()
    }
}

#[server(prefix = "/api/ui")]
pub async fn accept_owner_invite(
    invite_id: String,
    token: String,
) -> Result<TaxProfileView, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        crate::application::accept_owner_invite(invite_id, token, server_fn_request_auth())
            .await
            .map_err(server_fn_error)
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (invite_id, token);
        unreachable!()
    }
}

#[server(prefix = "/api/ui")]
pub async fn list_firm_publishers(
    organization_id: String,
) -> Result<FirmPublisherListResponse, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        crate::application::list_firm_publishers(organization_id, server_fn_request_auth())
            .await
            .map_err(server_fn_error)
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = organization_id;
        unreachable!()
    }
}

/// Adds (`remove: false`, by `email`) or removes (`remove: true`, by
/// `user_id`) a member the firm lets publish to BIR.
#[server(prefix = "/api/ui")]
pub async fn change_firm_publisher(
    organization_id: String,
    email: Option<String>,
    user_id: Option<String>,
    remove: bool,
) -> Result<FirmPublisherListResponse, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let auth = server_fn_request_auth();
        if remove {
            crate::application::remove_firm_publisher(
                organization_id,
                user_id.unwrap_or_default(),
                auth,
            )
            .await
        } else {
            crate::application::add_firm_publisher(
                organization_id,
                FirmPublisherAddRequest {
                    email: email.unwrap_or_default(),
                },
                auth,
            )
            .await
        }
        .map_err(server_fn_error)
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (organization_id, email, user_id, remove);
        unreachable!()
    }
}

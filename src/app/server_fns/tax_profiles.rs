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
pub async fn transfer_tax_profile(
    profile_id: String,
    organization_id: String,
) -> Result<TaxProfileView, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        crate::application::transfer_tax_profile(
            TaxProfileTransferRequest {
                id: profile_id,
                organization_id,
                expected_revision: None,
            },
            server_fn_request_auth(),
        )
        .await
        .map_err(server_fn_error)
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (profile_id, organization_id);
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

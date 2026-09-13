//! Postgres event log + exclusive-ownership projection for tax profiles.
//!
//! Event stream id is the profile UUID. Exclusive uniqueness is
//! `tin_identity_hash`, never raw TIN.

use serde_json::{json, Value};

use getrandom::getrandom;

use crate::domain::{
    AccountHolder, ClaimStatus, CloudTaxProfileFacts, OwnershipStatus, TaxProfile,
    TaxProfileCommand, TaxProfileError, TaxProfileEvent, TaxpayerType, TinIdentityHash,
    VerificationStatus,
};
use crate::error::{AuthStackError, AuthStackResult};

use super::{
    AtomicSqlStatement, execute_sql, execute_sql_atomic, initialize_schema_async, required_string,
    row_bool, row_i64, row_string,
};

pub(crate) struct LoadedTaxProfile {
    pub state: TaxProfile,
    pub revision: u64,
}

pub(crate) async fn tin_identity_pepper() -> AuthStackResult<[u8; 32]> {
    crate::auth_product::derived_key(crate::auth_product::TIN_IDENTITY_INFO).await
}

pub(crate) fn rfc3339_now() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    unix_to_rfc3339(now.as_secs(), now.subsec_millis())
}

fn unix_to_rfc3339(secs: u64, millis: u32) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let h = rem / 3600;
    let min = (rem % 3600) / 60;
    let s = rem % 60;
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mth = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if mth <= 2 { y + 1 } else { y };
    format!("{year:04}-{mth:02}-{d:02}T{h:02}:{min:02}:{s:02}.{millis:03}Z")
}

pub(crate) async fn load_tax_profile_by_id(profile_id: &str) -> AuthStackResult<LoadedTaxProfile> {
    initialize_schema_async().await?;
    load_stream(profile_id).await
}

pub(crate) async fn load_tax_profile_by_identity_hash(
    identity_hash: &TinIdentityHash,
) -> AuthStackResult<Option<LoadedTaxProfile>> {
    initialize_schema_async().await?;
    let rows = execute_sql(
        "SELECT profile_id::text AS profile_id FROM buwiz_server.tax_profiles \
         WHERE tin_hash = decode(?1, 'hex') \
            OR (tin_hash IS NULL AND tin_identity_hash = ?1)",
        vec![json!(identity_hash.as_str())],
    )
    .await?;
    let Some(profile_id) = rows.first().and_then(|row| row_string(row, "profile_id")) else {
        return Ok(None);
    };
    Ok(Some(load_stream(&profile_id).await?))
}

async fn load_stream(stream_id: &str) -> AuthStackResult<LoadedTaxProfile> {
    let rows = execute_sql(
        "SELECT payload FROM buwiz_server.tax_profile_events \
         WHERE stream_id = ?1 ORDER BY revision ASC",
        vec![json!(stream_id)],
    )
    .await?;
    let mut state = TaxProfile::new();
    for row in &rows {
        let payload = row
            .get("payload")
            .cloned()
            .ok_or_else(|| AuthStackError::store("tax profile event payload is missing"))?;
        let event: TaxProfileEvent = serde_json::from_value(payload)
            .map_err(|error| AuthStackError::store(format!("invalid tax profile event: {error}")))?;
        state.apply(&event);
    }
    if !state.exists {
        if let Some(projected) = load_projection_fallback(stream_id).await? {
            return Ok(projected);
        }
    }
    Ok(LoadedTaxProfile {
        revision: rows.len() as u64,
        state,
    })
}

async fn load_projection_fallback(profile_id: &str) -> AuthStackResult<Option<LoadedTaxProfile>> {
    let rows = execute_sql(
        "SELECT profile_id::text AS profile_id, \
                COALESCE(encode(tin_hash, 'hex'), tin_identity_hash) AS tin_identity_hash, \
                tin_last4, display_name, \
                registered_name, rdo_code, line_of_business, registered_address, zip_code, \
                phone, email, taxpayer_type, tax_classification, is_vat_registered, \
                holder_kind, holder_user_id::text AS holder_user_id, \
                holder_organization_id::text AS holder_organization_id, \
                ownership_status, verification_status, \
                verified_owner_user_id::text AS verified_owner_user_id, \
                account_id::text AS account_id, branch_code, \
                COALESCE(is_archived, false) AS is_archived, \
                eopt_tier, business_start_date, birth_date, atc_codes, \
                revision, updated_at::text AS updated_at \
         FROM buwiz_server.tax_profiles WHERE profile_id = ?1::text::uuid",
        vec![json!(profile_id)],
    )
    .await?;
    let Some(row) = rows.first() else {
        return Ok(None);
    };
    Ok(Some(LoadedTaxProfile {
        revision: row_i64(row, "revision").unwrap_or(1) as u64,
        state: tax_profile_from_row(row)?,
    }))
}

fn tax_profile_from_row(row: &Value) -> AuthStackResult<TaxProfile> {
    let holder_kind = required_string(row, "holder_kind")?;
    let holder = match holder_kind.as_str() {
        "personal" => AccountHolder::Personal {
            user_id: required_string(row, "holder_user_id")?,
        },
        "organization" => AccountHolder::Organization {
            organization_id: required_string(row, "holder_organization_id")?,
        },
        _ => {
            return Err(AuthStackError::store("tax profile holder_kind is invalid"));
        }
    };
    let facts = CloudTaxProfileFacts {
        registered_name: required_string(row, "registered_name")?,
        rdo_code: required_string(row, "rdo_code")?,
        line_of_business: row_string(row, "line_of_business").unwrap_or_default(),
        registered_address: row_string(row, "registered_address").unwrap_or_default(),
        zip_code: row_string(row, "zip_code").unwrap_or_default(),
        phone: row_string(row, "phone").unwrap_or_default(),
        email: row_string(row, "email").unwrap_or_default(),
        taxpayer_type: TaxpayerType::parse(&required_string(row, "taxpayer_type")?)
            .map_err(map_tax_profile_error)?,
        tax_classification: row_string(row, "tax_classification").filter(|value| !value.is_empty()),
        is_vat_registered: row_bool(row, "is_vat_registered").unwrap_or(false),
        eopt_tier: row_string(row, "eopt_tier").filter(|value| !value.is_empty()),
        business_start_date: row_string(row, "business_start_date").filter(|value| !value.is_empty()),
        birth_date: row_string(row, "birth_date").filter(|value| !value.is_empty()),
        atc_codes: row_string_list(row, "atc_codes"),
    };
    Ok(TaxProfile {
        exists: true,
        profile_id: Some(required_string(row, "profile_id")?),
        account_id: row_string(row, "account_id"),
        identity_hash: row_string(row, "tin_identity_hash")
            .map(|value| TinIdentityHash::parse(&value))
            .transpose()
            .map_err(AuthStackError::store)?,
        tin_last4: row_string(row, "tin_last4"),
        branch_code: row_string(row, "branch_code"),
        display_name: row_string(row, "display_name")
            .or_else(|| row_string(row, "registered_name")),
        holder: Some(holder),
        ownership: Some(
            OwnershipStatus::parse(&required_string(row, "ownership_status")?)
                .map_err(map_tax_profile_error)?,
        ),
        verification: VerificationStatus::parse(&required_string(row, "verification_status")?)
            .map_err(map_tax_profile_error)?,
        facts: Some(facts),
        verified_owner_user_id: row_string(row, "verified_owner_user_id"),
        is_archived: row_bool(row, "is_archived").unwrap_or(false),
        updated_at: row_string(row, "updated_at"),
    })
}

pub(crate) async fn commit_tax_profile_command(
    profile_id: &str,
    command: TaxProfileCommand,
    expected_revision: u64,
) -> AuthStackResult<(TaxProfile, u64)> {
    initialize_schema_async().await?;
    let loaded = load_stream(profile_id).await?;
    if loaded.revision != expected_revision {
        return Err(AuthStackError::conflict(
            TaxProfileError::ConcurrentModification.to_string(),
        ));
    }
    let events = loaded
        .state
        .handle(command)
        .map_err(map_tax_profile_error)?;
    if events.is_empty() {
        return Ok((loaded.state, loaded.revision));
    }

    let mut next = loaded.state;
    let mut statements = Vec::new();
    let mut revision = loaded.revision;
    for event in &events {
        revision += 1;
        next.apply(event);
        let payload = serde_json::to_value(event).map_err(|error| {
            AuthStackError::serialization(format!("tax profile event: {error}"))
        })?;
        statements.push(AtomicSqlStatement::guard(
            "INSERT INTO buwiz_server.tax_profile_events \
             (stream_id, revision, event_type, payload) \
             SELECT ?1, ?2, ?3, ?4::jsonb \
             WHERE NOT EXISTS ( \
                 SELECT 1 FROM buwiz_server.tax_profile_events \
                 WHERE stream_id = ?1 AND revision >= ?2 \
             ) \
             RETURNING stream_id",
            vec![
                json!(profile_id),
                json!(revision as i64),
                json!(event.event_type()),
                payload,
            ],
        ));
    }
    statements.push(projection_upsert_statement(&next, revision)?);
    execute_sql_atomic(statements)
        .await
        .map_err(map_concurrency_store_error)?;
    publish_tax_profile_wake(profile_id, revision).await;
    Ok((next, revision))
}

fn projection_upsert_statement(
    state: &TaxProfile,
    revision: u64,
) -> AuthStackResult<AtomicSqlStatement> {
    let profile_id = state
        .profile_id
        .clone()
        .ok_or_else(|| AuthStackError::store("projected tax profile is missing an id"))?;
    let facts = state
        .facts
        .clone()
        .ok_or_else(|| AuthStackError::store("projected tax profile is missing facts"))?;
    let holder = state
        .holder
        .clone()
        .ok_or_else(|| AuthStackError::store("projected tax profile is missing a holder"))?;
    let ownership = state
        .ownership
        .ok_or_else(|| AuthStackError::store("projected tax profile is missing ownership"))?;
    let identity_hash = state
        .identity_hash
        .as_ref()
        .ok_or_else(|| AuthStackError::store("projected tax profile is missing identity hash"))?;
    let tin_last4 = state
        .tin_last4
        .clone()
        .ok_or_else(|| AuthStackError::store("projected tax profile is missing tin_last4"))?;
    let display_name = state
        .display_name
        .clone()
        .unwrap_or_else(|| facts.registered_name.clone());
    let branch_code = state
        .branch_code
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| crate::domain::BranchCode::HEAD_OFFICE.to_owned());
    let account_id = state.account_id.clone().or_else(|| state.owner_user_id());
    let full_name = facts.registered_name.clone();
    let (holder_kind, holder_user_id, holder_organization_id) = match &holder {
        AccountHolder::Personal { user_id } => ("personal", Some(user_id.clone()), None),
        AccountHolder::Organization { organization_id } => {
            ("organization", None, Some(organization_id.clone()))
        }
    };
    let owner_user_id = state.owner_user_id();
    let org_id = state.org_id();
    let claim_status = match ownership {
        OwnershipStatus::PersonalExclusive => ClaimStatus::Owned,
        OwnershipStatus::CompanyManaged => ClaimStatus::PendingClaim,
    };
    Ok(AtomicSqlStatement::execute(
        "INSERT INTO buwiz_server.tax_profiles (\
            profile_id, stream_id, tin_root, branch_code, tin_identity_hash, tin_hash, tin_last4, \
            account_id, full_name, is_archived, eopt_tier, business_start_date, birth_date, atc_codes, \
            display_name, claim_status, org_id, owner_user_id, registered_name, rdo_code, \
            line_of_business, registered_address, zip_code, phone, email, taxpayer_type, \
            tax_classification, is_vat_registered, holder_kind, holder_user_id, \
            holder_organization_id, ownership_status, verification_status, \
            verified_owner_user_id, revision, created_at, updated_at \
         ) VALUES (\
            ?1::text::uuid, ?2, NULL, ?3, ?4, decode(?4, 'hex'), ?5, \
            ?6::text::uuid, ?7, ?8, ?9, ?10, ?11, ?12::jsonb, \
            ?13, ?14, ?15::text::uuid, ?16::text::uuid, ?17, ?18, \
            ?19, ?20, ?21, ?22, ?23, ?24, \
            ?25, ?26, ?27, ?28::text::uuid, \
            ?29::text::uuid, ?30, ?31, \
            ?32::text::uuid, ?33, \
            CURRENT_TIMESTAMP, CURRENT_TIMESTAMP \
         ) \
         ON CONFLICT (profile_id) DO UPDATE SET \
            tin_identity_hash = EXCLUDED.tin_identity_hash, \
            tin_hash = EXCLUDED.tin_hash, \
            tin_last4 = EXCLUDED.tin_last4, \
            branch_code = EXCLUDED.branch_code, \
            account_id = EXCLUDED.account_id, \
            full_name = EXCLUDED.full_name, \
            is_archived = EXCLUDED.is_archived, \
            eopt_tier = EXCLUDED.eopt_tier, \
            business_start_date = EXCLUDED.business_start_date, \
            birth_date = EXCLUDED.birth_date, \
            atc_codes = EXCLUDED.atc_codes, \
            display_name = EXCLUDED.display_name, \
            claim_status = EXCLUDED.claim_status, \
            org_id = EXCLUDED.org_id, \
            owner_user_id = EXCLUDED.owner_user_id, \
            registered_name = EXCLUDED.registered_name, \
            rdo_code = EXCLUDED.rdo_code, \
            line_of_business = EXCLUDED.line_of_business, \
            registered_address = EXCLUDED.registered_address, \
            zip_code = EXCLUDED.zip_code, \
            phone = EXCLUDED.phone, \
            email = EXCLUDED.email, \
            taxpayer_type = EXCLUDED.taxpayer_type, \
            tax_classification = EXCLUDED.tax_classification, \
            is_vat_registered = EXCLUDED.is_vat_registered, \
            holder_kind = EXCLUDED.holder_kind, \
            holder_user_id = EXCLUDED.holder_user_id, \
            holder_organization_id = EXCLUDED.holder_organization_id, \
            ownership_status = EXCLUDED.ownership_status, \
            verification_status = EXCLUDED.verification_status, \
            verified_owner_user_id = EXCLUDED.verified_owner_user_id, \
            revision = EXCLUDED.revision, \
            updated_at = CURRENT_TIMESTAMP, \
            tin_root = NULL",
        vec![
            json!(profile_id),
            json!(profile_id),
            json!(branch_code),
            json!(identity_hash.as_str()),
            json!(tin_last4),
            json!(account_id),
            json!(full_name),
            json!(state.is_archived),
            json!(facts.eopt_tier),
            json!(facts.business_start_date),
            json!(facts.birth_date),
            json!(facts.atc_codes),
            json!(display_name),
            json!(claim_status.as_str()),
            json!(org_id),
            json!(owner_user_id),
            json!(facts.registered_name),
            json!(facts.rdo_code),
            json!(facts.line_of_business),
            json!(facts.registered_address),
            json!(facts.zip_code),
            json!(facts.phone),
            json!(facts.email),
            json!(facts.taxpayer_type.as_str()),
            json!(facts.tax_classification),
            json!(facts.is_vat_registered),
            json!(holder_kind),
            json!(holder_user_id),
            json!(holder_organization_id),
            json!(ownership.as_str()),
            json!(state.verification.as_str()),
            json!(state.verified_owner_user_id),
            json!(revision as i64),
        ],
    ))
}

const TAX_PROFILE_VIEW_COLUMNS: &str = "profile_id::text AS profile_id, tin_last4, display_name, claim_status, \
                    owner_user_id::text AS owner_user_id, org_id::text AS org_id, \
                    account_id::text AS account_id, branch_code, \
                    COALESCE(is_archived, false) AS is_archived, \
                    COALESCE(full_name, registered_name) AS full_name, \
                    registered_name, rdo_code, line_of_business, registered_address, zip_code, \
                    phone, email, taxpayer_type, tax_classification, is_vat_registered, \
                    holder_kind, holder_user_id::text AS holder_user_id, \
                    holder_organization_id::text AS holder_organization_id, \
                    ownership_status, verification_status, \
                    verified_owner_user_id::text AS verified_owner_user_id, \
                    revision, created_at::text AS created_at, updated_at::text AS updated_at";

pub(crate) async fn fetch_tax_profile_view(
    profile_id: &str,
    actor_user_id: &str,
) -> AuthStackResult<crate::contracts::TaxProfileView> {
    initialize_schema_async().await?;
    let rows = execute_sql(
        &format!(
            "SELECT {TAX_PROFILE_VIEW_COLUMNS} FROM buwiz_server.tax_profiles \
             WHERE profile_id = ?1::text::uuid"
        ),
        vec![json!(profile_id)],
    )
    .await?;
    let row = rows
        .first()
        .ok_or_else(|| AuthStackError::not_found("tax profile not found"))?;
    tax_profile_view_from_row(row, actor_user_id)
}

pub(crate) async fn list_tax_profiles_for(
    user_id: &str,
    organization_id: Option<&str>,
) -> AuthStackResult<Vec<crate::contracts::TaxProfileView>> {
    initialize_schema_async().await?;
    let rows = if let Some(organization_id) = organization_id.filter(|id| !id.trim().is_empty()) {
        execute_sql(
            &format!(
                "SELECT {TAX_PROFILE_VIEW_COLUMNS} FROM buwiz_server.tax_profiles \
                 WHERE COALESCE(is_archived, false) = FALSE \
                   AND (account_id = ?1::text::uuid OR holder_user_id = ?1::text::uuid \
                        OR holder_organization_id = ?2::text::uuid OR org_id = ?2::text::uuid) \
                 ORDER BY updated_at DESC"
            ),
            vec![json!(user_id), json!(organization_id)],
        )
        .await?
    } else {
        execute_sql(
            &format!(
                "SELECT {TAX_PROFILE_VIEW_COLUMNS} FROM buwiz_server.tax_profiles \
                 WHERE COALESCE(is_archived, false) = FALSE \
                   AND (account_id = ?1::text::uuid OR holder_user_id = ?1::text::uuid OR owner_user_id = ?1::text::uuid) \
                 ORDER BY updated_at DESC"
            ),
            vec![json!(user_id)],
        )
        .await?
    };
    rows.iter()
        .map(|row| tax_profile_view_from_row(row, user_id))
        .collect()
}

pub(crate) fn tax_profile_view_from_state(
    state: &TaxProfile,
    revision: u64,
    actor_user_id: &str,
) -> AuthStackResult<crate::contracts::TaxProfileView> {
    let facts = state.facts.clone().ok_or_else(|| {
        AuthStackError::store("tax profile facts are missing after commit")
    })?;
    let holder = state.holder.clone().ok_or_else(|| {
        AuthStackError::store("tax profile holder is missing after commit")
    })?;
    let holder_kind = match holder {
        AccountHolder::Personal { .. } => "personal",
        AccountHolder::Organization { .. } => "organization",
    };
    let profile_id = state.profile_id.clone().unwrap_or_default();
    Ok(crate::contracts::TaxProfileView {
        id: profile_id.clone(),
        profile_id,
        owner_user_id: state.owner_user_id(),
        org_id: state.org_id(),
        tin_last4: state.tin_last4.clone().unwrap_or_default(),
        display_name: state
            .display_name
            .clone()
            .unwrap_or_else(|| facts.registered_name.clone()),
        claim_status: state.claim_status_for(actor_user_id).as_str().to_owned(),
        full_name: facts.registered_name.clone(),
        rdo_code: facts.rdo_code,
        address: facts.registered_address,
        line_of_business: facts.line_of_business,
        zip_code: facts.zip_code,
        phone: facts.phone,
        email: facts.email,
        taxpayer_type: facts.taxpayer_type.as_str().to_owned(),
        tax_classification: facts.tax_classification,
        is_vat_registered: facts.is_vat_registered,
        holder_kind: holder_kind.to_owned(),
        ownership_status: state
            .ownership
            .unwrap_or(OwnershipStatus::PersonalExclusive)
            .as_str()
            .to_owned(),
        verification_status: state.verification.as_str().to_owned(),
        verified_owner_user_id: state.verified_owner_user_id.clone(),
        account_id: state.account_id.clone(),
        branch_code: state
            .branch_code
            .clone()
            .unwrap_or_else(|| crate::domain::BranchCode::HEAD_OFFICE.to_owned()),
        is_archived: state.is_archived,
        revision,
        created_at: None,
        updated_at: state.updated_at.clone(),
    })
}

fn tax_profile_view_from_row(
    row: &Value,
    actor_user_id: &str,
) -> AuthStackResult<crate::contracts::TaxProfileView> {
    let profile_id = required_string(row, "profile_id")?;
    let holder_kind = required_string(row, "holder_kind")?;
    let holder_user_id = row_string(row, "holder_user_id");
    let holder_organization_id = row_string(row, "holder_organization_id");
    let ownership = OwnershipStatus::parse(&required_string(row, "ownership_status")?)
        .map_err(map_tax_profile_error)?;
    let verified_owner = row_string(row, "verified_owner_user_id");
    let claim_status = row_string(row, "claim_status").unwrap_or_else(|| {
        let holder = match holder_kind.as_str() {
            "organization" => AccountHolder::Organization {
                organization_id: holder_organization_id.clone().unwrap_or_default(),
            },
            _ => AccountHolder::Personal {
                user_id: holder_user_id.clone().unwrap_or_default(),
            },
        };
        let mut state = TaxProfile::new();
        state.exists = true;
        state.holder = Some(holder);
        state.ownership = Some(ownership);
        state.verified_owner_user_id = verified_owner.clone();
        state.claim_status_for(actor_user_id).as_str().to_owned()
    });
    let registered_name = required_string(row, "registered_name")?;
    Ok(crate::contracts::TaxProfileView {
        id: profile_id.clone(),
        profile_id,
        owner_user_id: row_string(row, "owner_user_id").or_else(|| holder_user_id.clone()),
        org_id: row_string(row, "org_id").or(holder_organization_id),
        tin_last4: row_string(row, "tin_last4").unwrap_or_default(),
        display_name: row_string(row, "display_name")
            .unwrap_or_else(|| registered_name.clone()),
        claim_status,
        full_name: registered_name,
        rdo_code: required_string(row, "rdo_code")?,
        address: row_string(row, "registered_address").unwrap_or_default(),
        line_of_business: row_string(row, "line_of_business").unwrap_or_default(),
        zip_code: row_string(row, "zip_code").unwrap_or_default(),
        phone: row_string(row, "phone").unwrap_or_default(),
        email: row_string(row, "email").unwrap_or_default(),
        taxpayer_type: required_string(row, "taxpayer_type")?,
        tax_classification: row_string(row, "tax_classification").filter(|value| !value.is_empty()),
        is_vat_registered: row_bool(row, "is_vat_registered").unwrap_or(false),
        holder_kind,
        ownership_status: ownership.as_str().to_owned(),
        verification_status: required_string(row, "verification_status")?,
        verified_owner_user_id: verified_owner,
        account_id: row_string(row, "account_id"),
        branch_code: row_string(row, "branch_code").unwrap_or_else(|| {
            crate::domain::BranchCode::HEAD_OFFICE.to_owned()
        }),
        is_archived: row_bool(row, "is_archived").unwrap_or(false),
        revision: row_i64(row, "revision").unwrap_or(1) as u64,
        created_at: row_string(row, "created_at"),
        updated_at: row_string(row, "updated_at"),
    })
}

pub(crate) async fn user_status(user_id: &str) -> AuthStackResult<String> {
    initialize_schema_async().await?;
    let rows = execute_sql(
        "SELECT status FROM auth_users WHERE user_id = ?1::text::uuid",
        vec![json!(user_id)],
    )
    .await?;
    let row = rows
        .first()
        .ok_or_else(|| AuthStackError::not_found("user not found"))?;
    required_string(row, "status")
}

pub(crate) fn facts_from_create(
    request: &crate::contracts::TaxProfileCreateRequest,
) -> AuthStackResult<CloudTaxProfileFacts> {
    Ok(CloudTaxProfileFacts {
        registered_name: request.registered_name.trim().to_owned(),
        rdo_code: request.rdo_code.trim().to_owned(),
        line_of_business: request.line_of_business.trim().to_owned(),
        registered_address: request.registered_address.trim().to_owned(),
        zip_code: request.zip_code.trim().to_owned(),
        phone: request.phone.trim().to_owned(),
        email: request.email.trim().to_owned(),
        taxpayer_type: TaxpayerType::parse(&request.taxpayer_type)
            .map_err(map_tax_profile_error)?,
        tax_classification: request
            .tax_classification
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned),
        is_vat_registered: request.is_vat_registered,
        eopt_tier: None,
        business_start_date: request
            .business_start_date
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned),
        birth_date: None,
        atc_codes: Vec::new(),
    })
}

pub(crate) fn new_uuid_v4() -> AuthStackResult<String> {
    let mut bytes = [0u8; 16];
    getrandom(&mut bytes)
        .map_err(|error| AuthStackError::store(format!("uuid entropy unavailable: {error}")))?;
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Ok(format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0],
        bytes[1],
        bytes[2],
        bytes[3],
        bytes[4],
        bytes[5],
        bytes[6],
        bytes[7],
        bytes[8],
        bytes[9],
        bytes[10],
        bytes[11],
        bytes[12],
        bytes[13],
        bytes[14],
        bytes[15]
    ))
}

fn row_string_list(row: &Value, key: &str) -> Vec<String> {
    match row.get(key) {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| item.as_str().map(ToOwned::to_owned))
            .collect(),
        Some(Value::String(raw)) => serde_json::from_str(raw).unwrap_or_default(),
        _ => Vec::new(),
    }
}

pub(crate) fn map_tax_profile_error(error: TaxProfileError) -> AuthStackError {
    match error {
        TaxProfileError::InvalidFacts { reason } => AuthStackError::validation(reason),
        TaxProfileError::AlreadyHeld { .. }
        | TaxProfileError::ConcurrentModification
        | TaxProfileError::StaleWrite
        | TaxProfileError::IdentityImmutable => AuthStackError::conflict(error.to_string()),
        TaxProfileError::NotFound => AuthStackError::not_found(error.to_string()),
        TaxProfileError::NotHolder
        | TaxProfileError::ClaimRequiresCompanyHold
        | TaxProfileError::ReclaimDenied
        | TaxProfileError::ProofMismatch => AuthStackError::Forbidden,
        TaxProfileError::AlreadyArchived | TaxProfileError::NotArchived => {
            AuthStackError::conflict(error.to_string())
        }
    }
}

fn map_concurrency_store_error(error: AuthStackError) -> AuthStackError {
    let message = error.to_string().to_ascii_lowercase();
    if message.contains("unique")
        || message.contains("duplicate")
        || message.contains("guard")
        || message.contains("minimum")
        || message.contains("23505")
    {
        AuthStackError::conflict(
            "registration unit is already held exclusively (hashed TIN identity)".to_owned(),
        )
    } else {
        error
    }
}

pub(crate) async fn publish_tax_profile_wake(stream_id: &str, revision: u64) {
    let Some(url) = crate::store::runtime_config_value("REDIS_URL")
        .await
        .filter(|value| !value.trim().is_empty())
    else {
        return;
    };
    let channel = crate::store::runtime_config_value("REDIS_CHANNEL")
        .await
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "buwiz-tax-profiles".to_owned());
    #[cfg(all(feature = "postgres", runtime_spin))]
    {
        let payload = json!({
            "kind": "tax_profile",
            "stream_id": stream_id,
            "revision": revision,
        });
        let client = ddd_cqrs_es::SpinRedisClient::new(url);
        let publisher = ddd_cqrs_es::RedisPubSubPublisher::new(client, channel);
        if let Err(error) = publisher.publish_json(&payload).await {
            tracing::warn!(
                error = %error,
                stream_id,
                "redis tax-profile wake failed; postgres remains the source of truth"
            );
        }
    }
    #[cfg(not(all(feature = "postgres", runtime_spin)))]
    {
        let _ = (url, channel, stream_id, revision);
    }
}

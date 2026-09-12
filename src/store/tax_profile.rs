//! Postgres event log + exclusive-ownership projection for tax profiles.

use serde_json::{json, Value};

use getrandom::getrandom;

use crate::domain::{
    AccountHolder, CloudTaxProfileFacts, OwnershipStatus, TaxProfile, TaxProfileCommand,
    TaxProfileError, TaxProfileEvent, TaxpayerType, VerificationStatus,
};
use crate::domain::tin::RegistrationKey;
use crate::error::{AuthStackError, AuthStackResult};

use super::{
    AtomicSqlStatement, execute_sql, execute_sql_atomic, initialize_schema_async, required_string,
    row_bool, row_i64, row_string,
};

pub(crate) struct LoadedTaxProfile {
    pub state: TaxProfile,
    pub revision: u64,
}

pub(crate) async fn load_tax_profile(
    registration: &RegistrationKey,
) -> AuthStackResult<LoadedTaxProfile> {
    initialize_schema_async().await?;
    let stream_id = registration.stream_id();
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
    Ok(LoadedTaxProfile {
        revision: rows.len() as u64,
        state,
    })
}

pub(crate) async fn commit_tax_profile_command(
    registration: &RegistrationKey,
    command: TaxProfileCommand,
    expected_revision: u64,
) -> AuthStackResult<(TaxProfile, u64)> {
    initialize_schema_async().await?;
    let loaded = load_tax_profile(registration).await?;
    if loaded.revision != expected_revision {
        return Err(AuthStackError::conflict(
            TaxProfileError::ConcurrentModification.to_string(),
        ));
    }
    let events = loaded
        .state
        .handle(registration, command)
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
                json!(registration.stream_id()),
                json!(revision as i64),
                json!(event.event_type()),
                payload,
            ],
        ));
    }
    statements.push(projection_upsert_statement(registration, &next, revision)?);
    execute_sql_atomic(statements)
        .await
        .map_err(map_concurrency_store_error)?;
    publish_tax_profile_wake(&registration.stream_id(), revision).await;
    Ok((next, revision))
}

fn projection_upsert_statement(
    registration: &RegistrationKey,
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
    let (holder_kind, holder_user_id, holder_organization_id) = match &holder {
        AccountHolder::Personal { user_id } => ("personal", Some(user_id.clone()), None),
        AccountHolder::Organization { organization_id } => (
            "organization",
            None,
            Some(organization_id.clone()),
        ),
    };
    Ok(AtomicSqlStatement::execute(
        "INSERT INTO buwiz_server.tax_profiles (\
            profile_id, stream_id, tin_root, branch_code, registered_name, rdo_code, \
            line_of_business, registered_address, zip_code, phone, email, taxpayer_type, \
            tax_classification, is_vat_registered, holder_kind, holder_user_id, \
            holder_organization_id, ownership_status, verification_status, \
            verified_owner_user_id, revision, created_at, updated_at \
         ) VALUES (\
            ?1::uuid, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, \
            ?16::uuid, ?17::uuid, ?18, ?19, ?20::uuid, ?21, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP \
         ) \
         ON CONFLICT (stream_id) DO UPDATE SET \
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
            updated_at = CURRENT_TIMESTAMP",
        vec![
            json!(profile_id),
            json!(registration.stream_id()),
            json!(registration.tin_root.as_str()),
            json!(registration.branch_code.as_str()),
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

pub(crate) async fn list_tax_profiles_for(
    user_id: &str,
    organization_id: Option<&str>,
) -> AuthStackResult<Vec<crate::contracts::TaxProfileView>> {
    initialize_schema_async().await?;
    let rows = if let Some(organization_id) = organization_id.filter(|id| !id.trim().is_empty()) {
        execute_sql(
            "SELECT profile_id::text AS profile_id, tin_root, branch_code, registered_name, \
                    rdo_code, line_of_business, registered_address, zip_code, phone, email, \
                    taxpayer_type, tax_classification, is_vat_registered, holder_kind, \
                    holder_user_id::text AS holder_user_id, \
                    holder_organization_id::text AS holder_organization_id, \
                    ownership_status, verification_status, \
                    verified_owner_user_id::text AS verified_owner_user_id, \
                    revision, created_at::text AS created_at, updated_at::text AS updated_at \
             FROM buwiz_server.tax_profiles \
             WHERE holder_user_id = ?1::uuid OR holder_organization_id = ?2::uuid \
             ORDER BY updated_at DESC",
            vec![json!(user_id), json!(organization_id)],
        )
        .await?
    } else {
        execute_sql(
            "SELECT profile_id::text AS profile_id, tin_root, branch_code, registered_name, \
                    rdo_code, line_of_business, registered_address, zip_code, phone, email, \
                    taxpayer_type, tax_classification, is_vat_registered, holder_kind, \
                    holder_user_id::text AS holder_user_id, \
                    holder_organization_id::text AS holder_organization_id, \
                    ownership_status, verification_status, \
                    verified_owner_user_id::text AS verified_owner_user_id, \
                    revision, created_at::text AS created_at, updated_at::text AS updated_at \
             FROM buwiz_server.tax_profiles \
             WHERE holder_user_id = ?1::uuid \
             ORDER BY updated_at DESC",
            vec![json!(user_id)],
        )
        .await?
    };
    rows.iter().map(tax_profile_view_from_row).collect()
}

pub(crate) fn tax_profile_view_from_state(
    registration: &RegistrationKey,
    state: &TaxProfile,
    revision: u64,
) -> AuthStackResult<crate::contracts::TaxProfileView> {
    let facts = state.facts.clone().ok_or_else(|| {
        AuthStackError::store("tax profile facts are missing after commit")
    })?;
    let holder = state.holder.clone().ok_or_else(|| {
        AuthStackError::store("tax profile holder is missing after commit")
    })?;
    let (holder_kind, holder_user_id, holder_organization_id) = match holder {
        AccountHolder::Personal { user_id } => ("personal".to_owned(), Some(user_id), None),
        AccountHolder::Organization { organization_id } => {
            ("organization".to_owned(), None, Some(organization_id))
        }
    };
    Ok(crate::contracts::TaxProfileView {
        profile_id: state.profile_id.clone().unwrap_or_default(),
        tin_root: registration.tin_root.as_str().to_owned(),
        branch_code: registration.branch_code.as_str().to_owned(),
        registered_name: facts.registered_name,
        rdo_code: facts.rdo_code,
        line_of_business: facts.line_of_business,
        registered_address: facts.registered_address,
        zip_code: facts.zip_code,
        phone: facts.phone,
        email: facts.email,
        taxpayer_type: facts.taxpayer_type.as_str().to_owned(),
        tax_classification: facts.tax_classification,
        is_vat_registered: facts.is_vat_registered,
        holder_kind,
        holder_user_id,
        holder_organization_id,
        ownership_status: state
            .ownership
            .unwrap_or(OwnershipStatus::PersonalExclusive)
            .as_str()
            .to_owned(),
        verification_status: state.verification.as_str().to_owned(),
        verified_owner_user_id: state.verified_owner_user_id.clone(),
        revision,
        created_at: None,
        updated_at: None,
    })
}

fn tax_profile_view_from_row(row: &Value) -> AuthStackResult<crate::contracts::TaxProfileView> {
    Ok(crate::contracts::TaxProfileView {
        profile_id: required_string(row, "profile_id")?,
        tin_root: required_string(row, "tin_root")?,
        branch_code: required_string(row, "branch_code")?,
        registered_name: required_string(row, "registered_name")?,
        rdo_code: required_string(row, "rdo_code")?,
        line_of_business: row_string(row, "line_of_business").unwrap_or_default(),
        registered_address: row_string(row, "registered_address").unwrap_or_default(),
        zip_code: row_string(row, "zip_code").unwrap_or_default(),
        phone: row_string(row, "phone").unwrap_or_default(),
        email: row_string(row, "email").unwrap_or_default(),
        taxpayer_type: required_string(row, "taxpayer_type")?,
        tax_classification: row_string(row, "tax_classification")
            .filter(|value| !value.is_empty()),
        is_vat_registered: row_bool(row, "is_vat_registered").unwrap_or(false),
        holder_kind: required_string(row, "holder_kind")?,
        holder_user_id: row_string(row, "holder_user_id"),
        holder_organization_id: row_string(row, "holder_organization_id"),
        ownership_status: required_string(row, "ownership_status")?,
        verification_status: required_string(row, "verification_status")?,
        verified_owner_user_id: row_string(row, "verified_owner_user_id"),
        revision: row_i64(row, "revision").unwrap_or(1) as u64,
        created_at: row_string(row, "created_at"),
        updated_at: row_string(row, "updated_at"),
    })
}

pub(crate) async fn user_status(user_id: &str) -> AuthStackResult<String> {
    initialize_schema_async().await?;
    let rows = execute_sql(
        "SELECT status FROM auth_users WHERE user_id = ?1::uuid",
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

pub(crate) fn map_tax_profile_error(error: TaxProfileError) -> AuthStackError {
    match error {
        TaxProfileError::InvalidFacts { reason } => AuthStackError::validation(reason),
        TaxProfileError::AlreadyHeld { .. } | TaxProfileError::ConcurrentModification => {
            AuthStackError::conflict(error.to_string())
        }
        TaxProfileError::NotFound => AuthStackError::not_found(error.to_string()),
        TaxProfileError::NotHolder
        | TaxProfileError::ClaimRequiresCompanyHold
        | TaxProfileError::ReclaimDenied
        | TaxProfileError::ProofMismatch => AuthStackError::Forbidden,
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
        AuthStackError::conflict(TaxProfileError::ConcurrentModification.to_string())
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
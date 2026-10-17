//! Tax years, drafts and filings (`0004_canonical_v1_read_models`,
//! `0010_record_authors`). The application checks the caller's standing and
//! the rules in `domain::records` before anything here runs.

use serde_json::{json, Value};

use crate::contracts::{
    DraftSummary, DraftView, FilingView, TaxYearOverrides, TaxYearView, YearForm,
};
use crate::domain::records::FilingStatus;
use crate::error::{AuthStackError, AuthStackResult};

use super::{
    AtomicSqlStatement, execute_sql, execute_sql_atomic, initialize_schema_async, required_string,
    is_unique_violation, row_bool, row_i64, row_string, uuid_shaped,
};

// Spin binds every JSON integer as int8 (`ddd_cqrs_es` postgres_parameters),
// so integer parameters go through `::bigint` before any narrower cast.
const TIMESTAMP: &str = "'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"'";

fn row_json(row: &Value, key: &str) -> Value {
    match row.get(key) {
        Some(Value::String(raw)) => serde_json::from_str(raw).unwrap_or(Value::Null),
        Some(value) => value.clone(),
        None => Value::Null,
    }
}

fn row_u64(row: &Value, key: &str) -> u64 {
    row_i64(row, key)
        .and_then(|value| u64::try_from(value).ok())
        .unwrap_or(0)
}

fn row_year(row: &Value) -> AuthStackResult<i32> {
    row_i64(row, "tax_year")
        .and_then(|year| i32::try_from(year).ok())
        .ok_or_else(|| AuthStackError::store("row has no tax year"))
}

// ── Tax years ───────────────────────────────────────────────────────────────

pub(crate) async fn list_tax_years(profile_id: &str) -> AuthStackResult<Vec<TaxYearView>> {
    initialize_schema_async().await?;
    let rows = execute_sql(
        &format!(
            "SELECT y.id::text AS year_id, y.tax_year::int AS tax_year, y.registered_name, \
                    y.rdo_code, y.line_of_business, y.registered_address, y.zip_code, y.phone, \
                    y.email, y.tax_classification, y.is_vat_registered, y.eopt_tier, \
                    to_char(y.updated_at AT TIME ZONE 'UTC', {TIMESTAMP}) AS updated_at, \
                    COALESCE((SELECT jsonb_agg(jsonb_build_object( \
                                'form_code', f.form_code, 'frequency', f.frequency, 'active', f.active) \
                              ORDER BY f.form_code) \
                              FROM buwiz_server.per_year_forms f WHERE f.profile_year_id = y.id), \
                             '[]'::jsonb) AS forms \
             FROM buwiz_server.profile_years y \
             WHERE y.tax_profile_id = ?1::uuid ORDER BY y.tax_year DESC"
        ),
        vec![json!(profile_id)],
    )
    .await?;
    rows.iter().map(tax_year_view).collect()
}

fn tax_year_view(row: &Value) -> AuthStackResult<TaxYearView> {
    let forms = serde_json::from_value::<Vec<YearForm>>(row_json(row, "forms")).unwrap_or_default();
    Ok(TaxYearView {
        year_id: required_string(row, "year_id")?,
        tax_year: row_year(row)?,
        overrides: TaxYearOverrides {
            registered_name: row_string(row, "registered_name"),
            rdo_code: row_string(row, "rdo_code"),
            line_of_business: row_string(row, "line_of_business"),
            registered_address: row_string(row, "registered_address"),
            zip_code: row_string(row, "zip_code"),
            phone: row_string(row, "phone"),
            email: row_string(row, "email"),
            tax_classification: row_string(row, "tax_classification"),
            is_vat_registered: row_bool(row, "is_vat_registered"),
            eopt_tier: row_string(row, "eopt_tier"),
        },
        forms,
        updated_at: row_string(row, "updated_at").unwrap_or_default(),
    })
}

/// Creates or updates one year; replaces its forms when `forms` is given.
pub(crate) async fn put_tax_year(
    new_year_id: &str,
    profile_id: &str,
    tax_year: i16,
    overrides: &TaxYearOverrides,
    forms: Option<&[YearForm]>,
) -> AuthStackResult<()> {
    initialize_schema_async().await?;
    let mut statements = vec![AtomicSqlStatement::execute(
        "INSERT INTO buwiz_server.profile_years (id, tax_profile_id, tax_year, registered_name, \
            rdo_code, line_of_business, registered_address, zip_code, phone, email, \
            tax_classification, is_vat_registered, eopt_tier, updated_at) \
         VALUES (?1::uuid, ?2::uuid, ?3::bigint::smallint, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, now()) \
         ON CONFLICT (tax_profile_id, tax_year) DO UPDATE SET \
            registered_name = EXCLUDED.registered_name, rdo_code = EXCLUDED.rdo_code, \
            line_of_business = EXCLUDED.line_of_business, \
            registered_address = EXCLUDED.registered_address, zip_code = EXCLUDED.zip_code, \
            phone = EXCLUDED.phone, email = EXCLUDED.email, \
            tax_classification = EXCLUDED.tax_classification, \
            is_vat_registered = EXCLUDED.is_vat_registered, eopt_tier = EXCLUDED.eopt_tier, \
            updated_at = now()",
        vec![
            json!(new_year_id),
            json!(profile_id),
            json!(tax_year),
            json!(overrides.registered_name),
            json!(overrides.rdo_code),
            json!(overrides.line_of_business),
            json!(overrides.registered_address),
            json!(overrides.zip_code),
            json!(overrides.phone),
            json!(overrides.email),
            json!(overrides.tax_classification),
            json!(overrides.is_vat_registered),
            json!(overrides.eopt_tier),
        ],
    )];
    if let Some(forms) = forms {
        statements.extend(replace_forms_statements(profile_id, tax_year, forms));
    }
    execute_sql_atomic(statements).await?;
    Ok(())
}

fn replace_forms_statements(profile_id: &str, tax_year: i16, forms: &[YearForm]) -> Vec<AtomicSqlStatement> {
    let mut statements = vec![AtomicSqlStatement::execute(
        "DELETE FROM buwiz_server.per_year_forms WHERE profile_year_id = \
            (SELECT id FROM buwiz_server.profile_years WHERE tax_profile_id = ?1::uuid AND tax_year = ?2::bigint::smallint)",
        vec![json!(profile_id), json!(tax_year)],
    )];
    statements.push(AtomicSqlStatement::execute(
        "INSERT INTO buwiz_server.per_year_forms (id, profile_year_id, form_code, frequency, active) \
         SELECT gen_random_uuid(), y.id, f.form_code, f.frequency, f.active \
         FROM buwiz_server.profile_years y, \
              jsonb_to_recordset(?3::jsonb) AS f(form_code text, frequency text, active boolean) \
         WHERE y.tax_profile_id = ?1::uuid AND y.tax_year = ?2::bigint::smallint",
        vec![json!(profile_id), json!(tax_year), json!(forms)],
    ));
    statements
}

/// Copies `from_year`'s forms into `to_year` (created if missing) only when
/// `to_year` has none. Returns whether forms were copied.
pub(crate) async fn clone_tax_year(
    new_year_id: &str,
    profile_id: &str,
    from_year: i16,
    to_year: i16,
) -> AuthStackResult<bool> {
    initialize_schema_async().await?;
    let rows = execute_sql_atomic(vec![
        AtomicSqlStatement::execute(
            "INSERT INTO buwiz_server.profile_years (id, tax_profile_id, tax_year, updated_at) \
             VALUES (?1::uuid, ?2::uuid, ?3::bigint::smallint, now()) \
             ON CONFLICT (tax_profile_id, tax_year) DO NOTHING",
            vec![json!(new_year_id), json!(profile_id), json!(to_year)],
        ),
        AtomicSqlStatement::query(
                "INSERT INTO buwiz_server.per_year_forms (id, profile_year_id, form_code, frequency, active) \
                 SELECT gen_random_uuid(), dest.id, f.form_code, f.frequency, f.active \
                 FROM buwiz_server.per_year_forms f \
                 JOIN buwiz_server.profile_years src ON src.id = f.profile_year_id \
                 JOIN buwiz_server.profile_years dest \
                   ON dest.tax_profile_id = src.tax_profile_id AND dest.tax_year = ?3::bigint::smallint \
                 WHERE src.tax_profile_id = ?1::uuid AND src.tax_year = ?2::bigint::smallint \
                   AND NOT EXISTS (SELECT 1 FROM buwiz_server.per_year_forms d WHERE d.profile_year_id = dest.id) \
                 RETURNING form_code",
                vec![json!(profile_id), json!(from_year), json!(to_year)],
        ),
    ])
    .await?;
    Ok(rows.last().is_some_and(|copied| !copied.is_empty()))
}

/// Whether `form_code` is an active form of that year.
pub(crate) async fn year_form_active(profile_id: &str, tax_year: i16, form_code: &str) -> AuthStackResult<bool> {
    initialize_schema_async().await?;
    let rows = execute_sql(
        "SELECT f.active FROM buwiz_server.per_year_forms f \
         JOIN buwiz_server.profile_years y ON y.id = f.profile_year_id \
         WHERE y.tax_profile_id = ?1::uuid AND y.tax_year = ?2::bigint::smallint AND f.form_code = ?3",
        vec![json!(profile_id), json!(tax_year), json!(form_code)],
    )
    .await?;
    Ok(rows.first().and_then(|row| row_bool(row, "active")).unwrap_or(false))
}

// ── Drafts ──────────────────────────────────────────────────────────────────

fn draft_columns() -> String {
    format!(
        "id::text AS draft_id, form_code, tax_year::int AS tax_year, period_key, rev, saved_once, \
         to_char(reviewed_at AT TIME ZONE 'UTC', {TIMESTAMP}) AS reviewed_at, \
         reviewed_by::text AS reviewed_by, updated_by::text AS updated_by, \
         to_char(updated_at AT TIME ZONE 'UTC', {TIMESTAMP}) AS updated_at"
    )
}

fn draft_summary(row: &Value) -> AuthStackResult<DraftSummary> {
    Ok(DraftSummary {
        draft_id: required_string(row, "draft_id")?,
        form_code: required_string(row, "form_code")?,
        tax_year: row_year(row)?,
        period_key: required_string(row, "period_key")?,
        rev: row_u64(row, "rev"),
        saved_once: row_bool(row, "saved_once").unwrap_or(false),
        reviewed_at: row_string(row, "reviewed_at"),
        reviewed_by: row_string(row, "reviewed_by"),
        updated_by: row_string(row, "updated_by"),
        updated_at: row_string(row, "updated_at").unwrap_or_default(),
    })
}

/// A profile's drafts, newest first; one year when `tax_year` is given.
pub(crate) async fn list_drafts(profile_id: &str, tax_year: Option<i16>) -> AuthStackResult<Vec<DraftSummary>> {
    initialize_schema_async().await?;
    let rows = execute_sql(
        &format!(
            "SELECT {} FROM buwiz_server.form_drafts \
             WHERE tax_profile_id = ?1::uuid AND (?2::bigint IS NULL OR tax_year = ?2::bigint) \
             ORDER BY updated_at DESC",
            draft_columns()
        ),
        vec![json!(profile_id), json!(tax_year)],
    )
    .await?;
    rows.iter().map(draft_summary).collect()
}

pub(crate) async fn get_draft(profile_id: &str, draft_id: &str) -> AuthStackResult<Option<DraftView>> {
    initialize_schema_async().await?;
    let Some(draft_id) = uuid_shaped(draft_id) else {
        return Ok(None);
    };
    let rows = execute_sql(
        &format!(
            "SELECT {}, payload FROM buwiz_server.form_drafts \
             WHERE tax_profile_id = ?1::uuid AND id::text = ?2",
            draft_columns()
        ),
        vec![json!(profile_id), json!(draft_id)],
    )
    .await?;
    let Some(row) = rows.first() else {
        return Ok(None);
    };
    Ok(Some(DraftView {
        summary: draft_summary(row)?,
        payload: row_json(row, "payload"),
    }))
}

/// The current revision of the draft for this form and period, if any.
pub(crate) async fn draft_rev(
    profile_id: &str,
    form_code: &str,
    tax_year: i16,
    period_key: &str,
) -> AuthStackResult<Option<u64>> {
    initialize_schema_async().await?;
    let rows = execute_sql(
        "SELECT rev FROM buwiz_server.form_drafts \
         WHERE tax_profile_id = ?1::uuid AND form_code = ?2 AND tax_year = ?3::bigint::smallint AND period_key = ?4",
        vec![json!(profile_id), json!(form_code), json!(tax_year), json!(period_key)],
    )
    .await?;
    Ok(rows.first().map(|row| row_u64(row, "rev")))
}

/// Writes the draft at `new_rev` only if it is still at `expected` (`None`:
/// it must not exist yet). A lost race aborts with a conflict.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn save_draft(
    new_draft_id: &str,
    profile_id: &str,
    form_code: &str,
    tax_year: i16,
    period_key: &str,
    payload: &Value,
    expected: Option<u64>,
    new_rev: u64,
    user_id: &str,
) -> AuthStackResult<()> {
    initialize_schema_async().await?;
    let statement = match expected {
        None => AtomicSqlStatement::guard(
            "INSERT INTO buwiz_server.form_drafts (id, tax_profile_id, form_code, tax_year, period_key, \
                payload, rev, saved_once, updated_by, updated_at) \
             VALUES (?1::uuid, ?2::uuid, ?3, ?4::bigint::smallint, ?5, ?6::jsonb, ?7::bigint, TRUE, ?8::uuid, now()) \
             ON CONFLICT (tax_profile_id, form_code, tax_year, period_key) DO NOTHING \
             RETURNING id::text AS draft_id",
            vec![
                json!(new_draft_id),
                json!(profile_id),
                json!(form_code),
                json!(tax_year),
                json!(period_key),
                payload.clone(),
                json!(new_rev as i64),
                json!(user_id),
            ],
        ),
        Some(current) => AtomicSqlStatement::guard(
            "UPDATE buwiz_server.form_drafts SET payload = ?5::jsonb, rev = ?6::bigint, \
                saved_once = TRUE, updated_by = ?7::uuid, updated_at = now(), \
                reviewed_at = NULL, reviewed_by = NULL \
             WHERE tax_profile_id = ?1::uuid AND form_code = ?2 AND tax_year = ?3::bigint::smallint \
               AND period_key = ?4 AND rev = ?8::bigint \
             RETURNING id::text AS draft_id",
            vec![
                json!(profile_id),
                json!(form_code),
                json!(tax_year),
                json!(period_key),
                payload.clone(),
                json!(new_rev as i64),
                json!(user_id),
                json!(current as i64),
            ],
        ),
    };
    execute_sql_atomic(vec![statement])
        .await
        .map_err(|_| AuthStackError::conflict("this draft changed on another device; load it again"))?;
    Ok(())
}

/// Marks a draft reviewed at its current revision (a later save clears it).
pub(crate) async fn mark_draft_reviewed(profile_id: &str, draft_id: &str, user_id: &str) -> AuthStackResult<bool> {
    initialize_schema_async().await?;
    let rows = execute_sql(
        "UPDATE buwiz_server.form_drafts SET reviewed_at = now(), reviewed_by = ?3::uuid \
         WHERE tax_profile_id = ?1::uuid AND id::text = ?2 RETURNING id::text AS draft_id",
        vec![json!(profile_id), json!(draft_id), json!(user_id)],
    )
    .await?;
    Ok(!rows.is_empty())
}

// ── Filings ─────────────────────────────────────────────────────────────────

fn filing_columns() -> String {
    format!(
        "id::text AS filing_id, form_draft_id::text AS draft_id, form_code, tax_year::int AS tax_year, \
         period_key, status, xml_sha256, receipt_ref, error_summary, \
         to_char(submitted_at AT TIME ZONE 'UTC', {TIMESTAMP}) AS submitted_at, \
         to_char(paid_at AT TIME ZONE 'UTC', {TIMESTAMP}) AS paid_at, \
         to_char(created_at AT TIME ZONE 'UTC', {TIMESTAMP}) AS created_at, \
         to_char(updated_at AT TIME ZONE 'UTC', {TIMESTAMP}) AS updated_at"
    )
}

fn filing_view(row: &Value) -> AuthStackResult<FilingView> {
    Ok(FilingView {
        filing_id: required_string(row, "filing_id")?,
        draft_id: row_string(row, "draft_id"),
        form_code: required_string(row, "form_code")?,
        tax_year: row_year(row)?,
        period_key: required_string(row, "period_key")?,
        status: required_string(row, "status")?,
        xml_sha256: row_string(row, "xml_sha256"),
        submitted_at: row_string(row, "submitted_at"),
        receipt_ref: row_string(row, "receipt_ref"),
        error_summary: row_string(row, "error_summary"),
        paid_at: row_string(row, "paid_at"),
        created_at: row_string(row, "created_at").unwrap_or_default(),
        updated_at: row_string(row, "updated_at").unwrap_or_default(),
    })
}

pub(crate) async fn list_filings(profile_id: &str) -> AuthStackResult<Vec<FilingView>> {
    initialize_schema_async().await?;
    let rows = execute_sql(
        &format!(
            "SELECT {} FROM buwiz_server.filings WHERE tax_profile_id = ?1::uuid ORDER BY created_at DESC",
            filing_columns()
        ),
        vec![json!(profile_id)],
    )
    .await?;
    rows.iter().map(filing_view).collect()
}

pub(crate) async fn get_filing(profile_id: &str, filing_id: &str) -> AuthStackResult<Option<FilingView>> {
    initialize_schema_async().await?;
    let Some(filing_id) = uuid_shaped(filing_id) else {
        return Ok(None);
    };
    let rows = execute_sql(
        &format!(
            "SELECT {} FROM buwiz_server.filings WHERE tax_profile_id = ?1::uuid AND id::text = ?2",
            filing_columns()
        ),
        vec![json!(profile_id), json!(filing_id)],
    )
    .await?;
    rows.first().map(filing_view).transpose()
}

/// Queues a filing of the draft. One open filing per form and period (the
/// unique index refuses a second).
pub(crate) async fn queue_filing(
    filing_id: &str,
    profile_id: &str,
    draft_id: &str,
    user_id: &str,
) -> AuthStackResult<()> {
    initialize_schema_async().await?;
    execute_sql_atomic(vec![AtomicSqlStatement::guard(
        "INSERT INTO buwiz_server.filings (id, tax_profile_id, form_draft_id, form_code, tax_year, \
            period_key, status, created_by, updated_by, created_at, updated_at) \
         SELECT ?1::uuid, d.tax_profile_id, d.id, d.form_code, d.tax_year, d.period_key, 'queued', \
                ?4::uuid, ?4::uuid, now(), now() \
         FROM buwiz_server.form_drafts d \
         WHERE d.tax_profile_id = ?2::uuid AND d.id::text = ?3 \
         RETURNING id::text AS filing_id",
        vec![json!(filing_id), json!(profile_id), json!(draft_id), json!(user_id)],
    )])
    .await
    .map_err(|error| {
        if is_unique_violation(&error) {
            AuthStackError::conflict("this form and period already has a filing on its way")
        } else {
            AuthStackError::not_found("draft not found")
        }
    })?;
    Ok(())
}

/// Moves a filing from `from` to `to`, recording what the app reported.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn advance_filing(
    profile_id: &str,
    filing_id: &str,
    from: FilingStatus,
    to: FilingStatus,
    xml_sha256: Option<&str>,
    receipt_ref: Option<&str>,
    error_summary: Option<&str>,
    user_id: &str,
) -> AuthStackResult<()> {
    initialize_schema_async().await?;
    execute_sql_atomic(vec![AtomicSqlStatement::guard(
        "UPDATE buwiz_server.filings SET status = ?4, \
            xml_sha256 = COALESCE(?5, xml_sha256), \
            submitted_at = CASE WHEN ?4 = 'submitted' THEN now() ELSE submitted_at END, \
            receipt_ref = COALESCE(?6, receipt_ref), \
            error_summary = COALESCE(?7, error_summary), \
            paid_at = CASE WHEN ?4 = 'paid' THEN now() ELSE paid_at END, \
            updated_by = ?8::uuid, updated_at = now() \
         WHERE tax_profile_id = ?1::uuid AND id::text = ?2 AND status = ?3 \
         RETURNING id::text AS filing_id",
        vec![
            json!(profile_id),
            json!(filing_id),
            json!(from.as_str()),
            json!(to.as_str()),
            json!(xml_sha256),
            json!(receipt_ref),
            json!(error_summary),
            json!(user_id),
        ],
    )])
    .await
    .map_err(|_| AuthStackError::conflict("this filing changed meanwhile; load it again"))?;
    Ok(())
}

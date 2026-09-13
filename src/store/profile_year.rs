//! Per-branch tax year facts and attached BIR forms.

use serde_json::{json, Value};

use crate::contracts::{ProfileYearView, YearFormView};
use crate::error::{AuthStackError, AuthStackResult};

use super::{
    execute_sql, initialize_schema_async, new_uuid_v4, required_string, row_bool, row_i64,
    row_string,
};

pub(crate) async fn load_or_create_profile_year(
    profile_id: &str,
    tax_year: i16,
) -> AuthStackResult<ProfileYearView> {
    initialize_schema_async().await?;
    if let Some(year) = load_profile_year(profile_id, tax_year).await? {
        return Ok(year);
    }
    let inherited = execute_sql(
        "SELECT registered_name, rdo_code, line_of_business, registered_address, zip_code, \
                phone, email, tax_classification, is_vat_registered \
         FROM buwiz_server.tax_profiles WHERE profile_id = ?1::uuid",
        vec![json!(profile_id)],
    )
    .await?;
    let row = inherited
        .first()
        .ok_or_else(|| AuthStackError::not_found("tax profile not found"))?;
    let year_id = new_uuid_v4()?;
    execute_sql(
        "INSERT INTO buwiz_server.profile_years (\
            id, tax_profile_id, tax_year, registered_name, rdo_code, line_of_business, \
            registered_address, zip_code, phone, email, tax_classification, is_vat_registered \
         ) VALUES (?1::uuid, ?2::uuid, ?3::bigint, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        vec![
            json!(year_id),
            json!(profile_id),
            json!(i64::from(tax_year)),
            json!(row_string(row, "registered_name")),
            json!(row_string(row, "rdo_code")),
            json!(row_string(row, "line_of_business")),
            json!(row_string(row, "registered_address")),
            json!(row_string(row, "zip_code")),
            json!(row_string(row, "phone")),
            json!(row_string(row, "email")),
            json!(row_string(row, "tax_classification")),
            json!(row_bool(row, "is_vat_registered").unwrap_or(false)),
        ],
    )
    .await?;
    load_profile_year(profile_id, tax_year)
        .await?
        .ok_or_else(|| AuthStackError::store("profile year insert did not return a row"))
}

pub(crate) async fn update_profile_year(
    profile_id: &str,
    tax_year: i16,
    registered_name: Option<String>,
    rdo_code: Option<String>,
    line_of_business: Option<String>,
    registered_address: Option<String>,
    zip_code: Option<String>,
    phone: Option<String>,
    email: Option<String>,
    tax_classification: Option<String>,
    is_vat_registered: Option<bool>,
) -> AuthStackResult<ProfileYearView> {
    let current = load_or_create_profile_year(profile_id, tax_year).await?;
    execute_sql(
        "UPDATE buwiz_server.profile_years SET \
            registered_name = COALESCE(?2, registered_name), \
            rdo_code = COALESCE(?3, rdo_code), \
            line_of_business = COALESCE(?4, line_of_business), \
            registered_address = COALESCE(?5, registered_address), \
            zip_code = COALESCE(?6, zip_code), \
            phone = COALESCE(?7, phone), \
            email = COALESCE(?8, email), \
            tax_classification = COALESCE(?9, tax_classification), \
            is_vat_registered = COALESCE(?10, is_vat_registered), \
            updated_at = CURRENT_TIMESTAMP \
         WHERE id = ?1::uuid",
        vec![
            json!(current.id),
            json!(registered_name),
            json!(rdo_code),
            json!(line_of_business),
            json!(registered_address),
            json!(zip_code),
            json!(phone),
            json!(email),
            json!(tax_classification),
            json!(is_vat_registered),
        ],
    )
    .await?;
    load_profile_year(profile_id, tax_year)
        .await?
        .ok_or_else(|| AuthStackError::store("profile year update did not return a row"))
}

pub(crate) async fn set_year_forms(
    profile_id: &str,
    tax_year: i16,
    entries: &[(String, String)],
) -> AuthStackResult<ProfileYearView> {
    let current = load_or_create_profile_year(profile_id, tax_year).await?;
    execute_sql(
        "DELETE FROM buwiz_server.per_year_forms WHERE profile_year_id = ?1::uuid",
        vec![json!(current.id)],
    )
    .await?;
    for (form_code, frequency) in entries {
        if form_code.trim().is_empty() {
            continue;
        }
        let form_id = new_uuid_v4()?;
        execute_sql(
            "INSERT INTO buwiz_server.per_year_forms (id, profile_year_id, form_code, frequency, active) \
             VALUES (?1::uuid, ?2::uuid, ?3, ?4, TRUE)",
            vec![
                json!(form_id),
                json!(current.id),
                json!(form_code.trim()),
                json!(frequency),
            ],
        )
        .await?;
    }
    load_profile_year(profile_id, tax_year)
        .await?
        .ok_or_else(|| AuthStackError::store("year forms did not reload"))
}

pub(crate) async fn clone_profile_year(
    profile_id: &str,
    from_year: i16,
    to_year: i16,
) -> AuthStackResult<ProfileYearView> {
    if from_year == to_year {
        return Err(AuthStackError::validation(
            "clone target year must be different from the source year",
        ));
    }
    let source = load_profile_year(profile_id, from_year)
        .await?
        .ok_or_else(|| AuthStackError::not_found("source tax year has no profile yet"))?;
    if let Some(existing) = load_profile_year(profile_id, to_year).await? {
        if !existing.forms.is_empty() {
            return Err(AuthStackError::validation(
                "destination year already has forms; clone only when the year is empty",
            ));
        }
    }
    let dest = load_or_create_profile_year(profile_id, to_year).await?;
    execute_sql(
        "UPDATE buwiz_server.profile_years SET \
            registered_name = ?2, rdo_code = ?3, line_of_business = ?4, \
            registered_address = ?5, zip_code = ?6, phone = ?7, email = ?8, \
            tax_classification = ?9, is_vat_registered = ?10, updated_at = CURRENT_TIMESTAMP \
         WHERE id = ?1::uuid",
        vec![
            json!(dest.id),
            json!(source.registered_name),
            json!(source.rdo_code),
            json!(source.line_of_business),
            json!(source.registered_address),
            json!(source.zip_code),
            json!(source.phone),
            json!(source.email),
            json!(source.tax_classification),
            json!(source.is_vat_registered),
        ],
    )
    .await?;
    let entries = source
        .forms
        .into_iter()
        .filter(|form| form.active)
        .map(|form| (form.form_code, form.frequency))
        .collect::<Vec<_>>();
    set_year_forms(profile_id, to_year, &entries).await
}

async fn load_profile_year(
    profile_id: &str,
    tax_year: i16,
) -> AuthStackResult<Option<ProfileYearView>> {
    let rows = execute_sql(
        "SELECT id::text AS id, tax_profile_id::text AS tax_profile_id, tax_year, \
                registered_name, rdo_code, line_of_business, registered_address, \
                zip_code, phone, email, tax_classification, is_vat_registered \
         FROM buwiz_server.profile_years \
         WHERE tax_profile_id = ?1::uuid AND tax_year = ?2::bigint",
        vec![json!(profile_id), json!(i64::from(tax_year))],
    )
    .await?;
    let Some(row) = rows.first() else {
        return Ok(None);
    };
    let year_id = required_string(row, "id")?;
    let form_rows = execute_sql(
        "SELECT form_code, frequency, active FROM buwiz_server.per_year_forms \
         WHERE profile_year_id = ?1::uuid ORDER BY form_code",
        vec![json!(year_id)],
    )
    .await?;
    Ok(Some(profile_year_from_row(row, form_rows)?))
}

pub(crate) async fn list_profile_years(profile_id: &str) -> AuthStackResult<Vec<i16>> {
    initialize_schema_async().await?;
    let rows = execute_sql(
        "SELECT tax_year FROM buwiz_server.profile_years \
         WHERE tax_profile_id = ?1::uuid ORDER BY tax_year DESC",
        vec![json!(profile_id)],
    )
    .await?;
    Ok(rows.iter().map(parse_tax_year).collect())
}

fn parse_tax_year(row: &Value) -> i16 {
    row_i64(row, "tax_year")
        .or_else(|| row_string(row, "tax_year").and_then(|value| value.parse().ok()))
        .unwrap_or_default() as i16
}

fn profile_year_from_row(row: &Value, form_rows: Vec<Value>) -> AuthStackResult<ProfileYearView> {
    Ok(ProfileYearView {
        id: required_string(row, "id")?,
        tax_profile_id: required_string(row, "tax_profile_id")?,
        tax_year: parse_tax_year(row),
        registered_name: row_string(row, "registered_name"),
        rdo_code: row_string(row, "rdo_code"),
        line_of_business: row_string(row, "line_of_business"),
        registered_address: row_string(row, "registered_address"),
        zip_code: row_string(row, "zip_code"),
        phone: row_string(row, "phone"),
        email: row_string(row, "email"),
        tax_classification: row_string(row, "tax_classification"),
        is_vat_registered: row_bool(row, "is_vat_registered"),
        forms: form_rows
            .iter()
            .filter_map(|form| {
                Some(YearFormView {
                    form_code: row_string(form, "form_code")?,
                    frequency: row_string(form, "frequency").unwrap_or_else(|| "annual".to_owned()),
                    active: row_bool(form, "active").unwrap_or(true),
                })
            })
            .collect(),
    })
}

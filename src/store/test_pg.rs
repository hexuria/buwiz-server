//! Test-only stand-in for Spin's Postgres host, so store and application
//! code can run against a real database in `cargo test`.
//!
//! It copies what `ddd_cqrs_es::adapters::execute_spin_pg*` and the Spin host
//! do, not what would be convenient: a JSON string binds as text, a number as
//! `int8` (or `float8`), so a query that works here binds the same way on Spin
//! (`?1::bigint`, `?1::text::uuid`). Rows come back as objects keyed by column
//! name. A column type Spin has no plain JSON form for (uuid, timestamptz,
//! arrays) comes back, as on Spin, as a string that is not the value (here
//! `<uuid column>`); that is fine for a guard that only counts rows, and a
//! test reading the value fails. Cast such columns in SQL (`::text`).
//! One deliberate difference: a query returning no rows works here, while on
//! local Spin it hangs.
//!
//! Each call opens its own connection, as the Spin host does.

use std::error::Error;

use serde_json::Value;
use tokio_postgres::types::{IsNull, ToSql, Type, to_sql_checked};
use tokio_postgres::{Client, NoTls, Row};

use crate::error::{AuthStackError, AuthStackResult};

/// One parameter, bound the way the Spin host binds a JSON value.
#[derive(Debug)]
enum Param {
    Null,
    Bool(bool),
    Int64(i64),
    Float64(f64),
    Str(String),
}

impl Param {
    fn from_json(value: Value) -> Self {
        match value {
            Value::Null => Self::Null,
            Value::Bool(value) => Self::Bool(value),
            Value::Number(number) => number.as_i64().map_or_else(
                || number.as_f64().map_or(Self::Null, Self::Float64),
                Self::Int64,
            ),
            Value::String(value) => Self::Str(value),
            other => Self::Str(other.to_string()),
        }
    }
}

impl ToSql for Param {
    fn to_sql(
        &self,
        ty: &Type,
        out: &mut bytes::BytesMut,
    ) -> Result<IsNull, Box<dyn Error + Sync + Send>> {
        match self {
            Self::Null => Ok(IsNull::Yes),
            Self::Bool(value) => value.to_sql(ty, out),
            Self::Int64(value) => value.to_sql(ty, out),
            Self::Float64(value) => value.to_sql(ty, out),
            Self::Str(value) => value.to_sql(ty, out),
        }
    }

    fn accepts(_: &Type) -> bool {
        true
    }

    // The per-value check below is what makes a text value bound to a
    // `uuid` parameter fail here as it does on Spin.
    to_sql_checked!();
}

fn check_type(param: &Param, ty: &Type) -> AuthStackResult<()> {
    let ok = match param {
        Param::Null => true,
        Param::Bool(_) => <bool as ToSql>::accepts(ty),
        Param::Int64(_) => <i64 as ToSql>::accepts(ty),
        Param::Float64(_) => <f64 as ToSql>::accepts(ty),
        Param::Str(_) => <String as ToSql>::accepts(ty),
    };
    if ok {
        Ok(())
    } else {
        Err(AuthStackError::store(format!(
            "parameter {param:?} cannot bind to {ty}; Spin would refuse it too"
        )))
    }
}

/// Postgres's own message (and code, such as `23505`), which plain
/// `Display` leaves out; the store maps unique violations by it.
fn pg_error(stage: &str, error: &tokio_postgres::Error) -> AuthStackError {
    match error.as_db_error() {
        Some(db) => AuthStackError::store(format!(
            "test pg {stage}: {} ({})",
            db.message(),
            db.code().code()
        )),
        None => AuthStackError::store(format!("test pg {stage}: {error}")),
    }
}

async fn connect(url: &str) -> AuthStackResult<Client> {
    let (client, connection) = tokio_postgres::connect(url, NoTls)
        .await
        .map_err(|error| pg_error("connect", &error))?;
    tokio::spawn(async move {
        let _ = connection.await;
    });
    Ok(client)
}

/// Same rule as `ddd_cqrs_es::adapters::spin_sql_returns_rows`.
fn returns_rows(sql: &str) -> bool {
    let upper = sql.trim_start().to_ascii_uppercase();
    if upper.starts_with("WITH ") {
        return upper.contains("SELECT") || upper.contains("RETURNING");
    }
    upper.starts_with("SELECT") || upper.contains("RETURNING")
}

async fn run(
    client: &Client,
    sql: &str,
    params: Vec<Value>,
    rows_wanted: bool,
) -> AuthStackResult<Vec<Value>> {
    let statement = client
        .prepare(sql)
        .await
        .map_err(|error| pg_error("prepare", &error))?;
    let params: Vec<Param> = params.into_iter().map(Param::from_json).collect();
    for (param, ty) in params.iter().zip(statement.params()) {
        check_type(param, ty)?;
    }
    let refs: Vec<&(dyn ToSql + Sync)> = params.iter().map(|p| p as &(dyn ToSql + Sync)).collect();
    if rows_wanted {
        let rows = client
            .query(&statement, &refs)
            .await
            .map_err(|error| pg_error("query", &error))?;
        rows.iter().map(row_json).collect()
    } else {
        client
            .execute(&statement, &refs)
            .await
            .map_err(|error| pg_error("execute", &error))?;
        Ok(Vec::new())
    }
}

fn row_json(row: &Row) -> AuthStackResult<Value> {
    let mut object = serde_json::Map::new();
    for (index, column) in row.columns().iter().enumerate() {
        object.insert(column.name().to_owned(), cell_json(row, index, column.type_())?);
    }
    Ok(Value::Object(object))
}

fn cell_json(row: &Row, index: usize, ty: &Type) -> AuthStackResult<Value> {
    fn get<'a, T: tokio_postgres::types::FromSql<'a>>(
        row: &'a Row,
        index: usize,
    ) -> AuthStackResult<Option<T>> {
        row.try_get(index)
            .map_err(|error| pg_error("decode", &error))
    }
    let value = match *ty {
        Type::BOOL => get::<bool>(row, index)?.map(Value::Bool),
        Type::INT2 => get::<i16>(row, index)?.map(|v| i64::from(v).into()),
        Type::INT4 => get::<i32>(row, index)?.map(|v| i64::from(v).into()),
        Type::INT8 => get::<i64>(row, index)?.map(Value::from),
        Type::FLOAT4 => get::<f32>(row, index)?
            .map(|v| serde_json::Number::from_f64(f64::from(v)).map_or(Value::Null, Value::Number)),
        Type::FLOAT8 => get::<f64>(row, index)?
            .map(|v| serde_json::Number::from_f64(v).map_or(Value::Null, Value::Number)),
        Type::TEXT | Type::VARCHAR | Type::BPCHAR | Type::NAME => {
            get::<String>(row, index)?.map(Value::String)
        }
        Type::JSON | Type::JSONB => get::<Value>(row, index)?,
        Type::BYTEA => get::<Vec<u8>>(row, index)?
            .map(|v| Value::String(String::from_utf8_lossy(&v).into_owned())),
        _ => Some(Value::String(format!("<{ty} column>"))),
    };
    Ok(value.unwrap_or(Value::Null))
}

pub(crate) async fn execute(url: &str, sql: &str, params: Vec<Value>) -> AuthStackResult<Vec<Value>> {
    let client = connect(url).await?;
    run(&client, sql, params, returns_rows(sql)).await
}

/// One statement of a transaction: SQL, parameters, whether it returns
/// rows, and the fewest rows it must return.
pub(crate) type TestStatement = (String, Vec<Value>, bool, usize);

pub(crate) async fn execute_atomic(
    url: &str,
    statements: Vec<TestStatement>,
) -> AuthStackResult<Vec<Vec<Value>>> {
    if statements.is_empty() {
        return Err(AuthStackError::store("transaction requires at least one statement"));
    }
    let client = connect(url).await?;
    client
        .batch_execute("BEGIN")
        .await
        .map_err(|error| pg_error("begin", &error))?;
    let mut output = Vec::with_capacity(statements.len());
    for (index, (sql, params, rows_wanted, minimum_rows)) in statements.into_iter().enumerate() {
        let result = run(&client, &sql, params, rows_wanted).await.and_then(|rows| {
            if rows.len() >= minimum_rows {
                Ok(rows)
            } else {
                Err(AuthStackError::store("transaction guard returned too few rows"))
            }
        });
        match result {
            Ok(rows) => output.push(rows),
            Err(error) => {
                let _ = client.batch_execute("ROLLBACK").await;
                return Err(AuthStackError::store(format!(
                    "Postgres transaction statement {index} failed and rolled back: {error}"
                )));
            }
        }
    }
    client
        .batch_execute("COMMIT")
        .await
        .map_err(|error| pg_error("commit", &error))?;
    Ok(output)
}

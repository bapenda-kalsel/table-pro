//! Staged-commit planning: validate a `StagedCommitPayload` against real
//! table metadata, then build placeholder SQL + ordered bind values.
//!
//! Invariants enforced here (the backend trusts no frontend claims):
//! - Primary-key identity comes from live `TableColumnMetadata`, not from
//!   `payload.primary_keys`; a mismatch means the staged view is stale.
//! - Tables without a PK can still INSERT but UPDATE/DELETE are rejected —
//!   there is no first-column pseudo-key fallback.
//! - `binds` are what actually executes (parameter binding); `preview` is
//!   the human-readable literal rendering for the review modal only.

use super::{sql_literal, Dialect, ParamStyle};
use crate::errors::{AppError, AppResult};
use crate::models::{StagedCommitPayload, StagedDiffSummary};
use serde_json::Value;

/// One validated statement: placeholder SQL + ordered binds + preview text.
#[derive(Debug)]
pub struct BoundStatement {
    pub sql: String,
    pub binds: Vec<Value>,
    pub preview: String,
}

fn err(msg: impl Into<String>) -> AppError {
    AppError::validation(msg.into())
}

fn is_scalar(v: &Value) -> bool {
    matches!(
        v,
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_)
    )
}

/// Keys of a JSON object as a sorted set for exact-equality comparison.
fn key_set(obj: &serde_json::Map<String, Value>) -> Vec<String> {
    let mut keys: Vec<String> = obj.keys().cloned().collect();
    keys.sort();
    keys.dedup();
    keys
}

fn sorted(v: &[String]) -> Vec<String> {
    let mut s = v.to_vec();
    s.sort();
    s.dedup();
    s
}

/// Validate `payload` against the real column metadata of the table and
/// build the bound execution plan. `pk_columns`/`all_columns` must come
/// from a fresh `get_table_schema` call on the same session.
pub fn build_staged_plan(
    dialect: Dialect,
    params: ParamStyle,
    payload: &StagedCommitPayload,
    pk_columns: &[String],
    all_columns: &[String],
) -> AppResult<Vec<BoundStatement>> {
    if all_columns.is_empty() {
        return Err(AppError::not_found(format!(
            "Table '{}' not found",
            payload.table_name
        )));
    }

    let all_set = sorted(all_columns);
    let real_pk = sorted(pk_columns);
    let table = dialect.qi(&payload.table_name);
    let needs_pk = !payload.updates.is_empty() || !payload.deletes.is_empty();

    if needs_pk {
        if real_pk.is_empty() {
            return Err(err(format!(
                "Table '{}' has no primary key — row updates and deletes are disabled",
                payload.table_name
            )));
        }
        let claimed = sorted(&payload.primary_keys);
        if claimed != real_pk {
            return Err(err(format!(
                "Staged view is stale: primary key of '{}' changed — refresh the table and restage changes",
                payload.table_name
            )));
        }
    }

    let mut param_idx = 0usize;
    let mut ph = move || {
        param_idx += 1;
        params.ph(param_idx)
    };

    let mut out = Vec::new();

    // INSERTS — empty-string cells are skipped so DB defaults apply
    // (pre-existing behavior).
    for (row_no, item) in payload.inserts.iter().enumerate() {
        let obj = item
            .as_object()
            .ok_or_else(|| err(format!("Insert row {} is not an object", row_no + 1)))?;
        let pairs: Vec<(&String, &Value)> = obj
            .iter()
            .filter(|(_, v)| !matches!(v, Value::String(s) if s.is_empty()))
            .collect();

        if pairs.is_empty() {
            out.push(BoundStatement {
                sql: format!("INSERT INTO {} DEFAULT VALUES", table),
                binds: vec![],
                preview: format!("INSERT INTO {} DEFAULT VALUES;", table),
            });
            continue;
        }

        for (k, v) in &pairs {
            if !all_set.contains(k) {
                return Err(err(format!(
                    "Unknown column '{}' on table '{}' — refresh and restage",
                    k, payload.table_name
                )));
            }
            if !is_scalar(v) {
                return Err(err(format!(
                    "Unsupported value for column '{}' (nested values are not allowed)",
                    k
                )));
            }
        }

        let cols: Vec<String> = pairs.iter().map(|(k, _)| dialect.qi(k)).collect();
        let placeholders: Vec<String> = pairs.iter().map(|_| ph()).collect();
        let literal_vals: Vec<String> = pairs.iter().map(|(_, v)| sql_literal(v)).collect();

        out.push(BoundStatement {
            sql: format!(
                "INSERT INTO {} ({}) VALUES ({})",
                table,
                cols.join(", "),
                placeholders.join(", ")
            ),
            binds: pairs.iter().map(|(_, v)| (*v).clone()).collect(),
            preview: format!(
                "INSERT INTO {} ({}) VALUES ({});",
                table,
                cols.join(", "),
                literal_vals.join(", ")
            ),
        });
    }

    // UPDATES — exact real-PK set required, non-empty field map.
    for (row_no, update) in payload.updates.iter().enumerate() {
        let pk_obj = update
            .primary_key_values
            .as_object()
            .ok_or_else(|| err(format!("Update {} has no primary-key identity", row_no + 1)))?;
        let fields_obj = update
            .updated_fields
            .as_object()
            .ok_or_else(|| err(format!("Update {} has no changed fields", row_no + 1)))?;

        if key_set(pk_obj) != real_pk {
            return Err(err(format!(
                "Update {} must identify the row by its primary key ({})",
                row_no + 1,
                real_pk.join(", ")
            )));
        }
        for (k, v) in pk_obj {
            if v.is_null() {
                return Err(err(format!("Primary-key column '{}' cannot be NULL", k)));
            }
            if !is_scalar(v) {
                return Err(err(format!("Unsupported primary-key value for '{}'", k)));
            }
        }
        if fields_obj.is_empty() {
            return Err(err(format!("Update {} changes no columns", row_no + 1)));
        }
        for (k, v) in fields_obj {
            if !all_set.contains(k) {
                return Err(err(format!(
                    "Unknown column '{}' on table '{}' — refresh and restage",
                    k, payload.table_name
                )));
            }
            if !is_scalar(v) {
                return Err(err(format!("Unsupported value for column '{}'", k)));
            }
        }

        // Deterministic clause order: fields sorted, PK cols in real-PK order.
        let mut field_keys: Vec<String> = fields_obj.keys().cloned().collect();
        field_keys.sort();
        let set_clauses: Vec<String> = field_keys
            .iter()
            .map(|k| format!("{} = {}", dialect.qi(k), ph()))
            .collect();
        let where_clauses: Vec<String> = real_pk
            .iter()
            .map(|k| format!("{} = {}", dialect.qi(k), ph()))
            .collect();

        let mut binds: Vec<Value> = Vec::new();
        for k in &field_keys {
            binds.push(fields_obj[k].clone());
        }
        for k in &real_pk {
            binds.push(pk_obj[k].clone());
        }

        let preview_set: Vec<String> = field_keys
            .iter()
            .map(|k| format!("{} = {}", dialect.qi(k), sql_literal(&fields_obj[k])))
            .collect();
        let preview_where: Vec<String> = real_pk
            .iter()
            .map(|k| format!("{} = {}", dialect.qi(k), sql_literal(&pk_obj[k])))
            .collect();

        out.push(BoundStatement {
            sql: format!(
                "UPDATE {} SET {} WHERE {}",
                table,
                set_clauses.join(", "),
                where_clauses.join(" AND ")
            ),
            binds,
            preview: format!(
                "UPDATE {} SET {} WHERE {};",
                table,
                preview_set.join(", "),
                preview_where.join(" AND ")
            ),
        });
    }

    // DELETES — exact real-PK set required.
    for (row_no, del) in payload.deletes.iter().enumerate() {
        let pk_obj = del
            .as_object()
            .ok_or_else(|| err(format!("Delete {} has no primary-key identity", row_no + 1)))?;

        if key_set(pk_obj) != real_pk {
            return Err(err(format!(
                "Delete {} must identify the row by its primary key ({})",
                row_no + 1,
                real_pk.join(", ")
            )));
        }
        for (k, v) in pk_obj {
            if v.is_null() {
                return Err(err(format!("Primary-key column '{}' cannot be NULL", k)));
            }
            if !is_scalar(v) {
                return Err(err(format!("Unsupported primary-key value for '{}'", k)));
            }
        }

        let where_clauses: Vec<String> = real_pk
            .iter()
            .map(|k| format!("{} = {}", dialect.qi(k), ph()))
            .collect();
        let preview_where: Vec<String> = real_pk
            .iter()
            .map(|k| format!("{} = {}", dialect.qi(k), sql_literal(&pk_obj[k])))
            .collect();

        out.push(BoundStatement {
            sql: format!(
                "DELETE FROM {} WHERE {}",
                table,
                where_clauses.join(" AND ")
            ),
            binds: real_pk.iter().map(|k| pk_obj[k].clone()).collect(),
            preview: format!(
                "DELETE FROM {} WHERE {};",
                table,
                preview_where.join(" AND ")
            ),
        });
    }

    Ok(out)
}

/// Build the review diff (literal-rendered SQL) from a validated plan's
/// previews. Counts come from the payload itself.
pub fn diff_from_plan(payload: &StagedCommitPayload, plan: &[BoundStatement]) -> StagedDiffSummary {
    StagedDiffSummary {
        insert_count: payload.inserts.len(),
        update_count: payload.updates.len(),
        delete_count: payload.deletes.len(),
        generated_sql: plan.iter().map(|s| s.preview.clone()).collect(),
    }
}

//! Shared, dialect-aware SQL helpers used by every driver implementation.
//! Everything here is pure (no I/O, no pool) so it can be unit-tested
//! without a live database.

use crate::models::{StagedCommitPayload, StagedDiffSummary};

/// Identifier quoting dialect: `"x"` for ANSI/SQLite/Postgres, `` `x` `` for MySQL.
#[derive(Clone, Copy)]
pub enum Dialect {
    Ansi,
    Backtick,
}

impl Dialect {
    pub fn qi(&self, ident: &str) -> String {
        match self {
            Dialect::Ansi => format!("\"{}\"", ident.replace('"', "\"\"")),
            Dialect::Backtick => format!("`{}`", ident.replace('`', "``")),
        }
    }

    /// Quote a possibly schema-qualified name (`schema`.`table`).
    pub fn qi_path(&self, name: &str) -> String {
        name.split('.')
            .map(|p| self.qi(p))
            .collect::<Vec<_>>()
            .join(".")
    }
}

/// A dialect with $N placeholders (PostgreSQL).
#[derive(Clone, Copy)]
pub enum ParamStyle {
    Question, // ? for SQLite/MySQL
    DollarN,  // $1..$n for PostgreSQL
}

impl ParamStyle {
    pub fn ph(&self, idx: usize) -> String {
        match self {
            ParamStyle::Question => "?".to_string(),
            ParamStyle::DollarN => format!("${}", idx),
        }
    }
}

/// Render a JSON value as an escaped SQL literal.
pub fn sql_literal(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Null => "NULL".to_string(),
        serde_json::Value::String(s) => format!("'{}'", s.replace('\'', "''")),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => if *b { "1" } else { "0" }.to_string(),
        _ => format!("'{}'", v.to_string().replace('\'', "''")),
    }
}

/// Build the SQL diff for a staged commit payload (inserts/updates/deletes).
/// Empty-string cell values are skipped so DB defaults / AUTO_INCREMENT apply.
pub fn preview_staged_sql(dialect: Dialect, payload: &StagedCommitPayload) -> StagedDiffSummary {
    let mut generated_sql = Vec::new();
    let table = dialect.qi(&payload.table_name);

    // 1. INSERTS
    for item in &payload.inserts {
        if let Some(obj) = item.as_object() {
            let pairs: Vec<(&String, &serde_json::Value)> = obj
                .iter()
                .filter(|(_, v)| !matches!(v, serde_json::Value::String(s) if s.is_empty()))
                .collect();

            if pairs.is_empty() {
                generated_sql.push(format!("INSERT INTO {} DEFAULT VALUES;", table));
                continue;
            }

            let cols: Vec<String> = pairs.iter().map(|(k, _)| dialect.qi(k)).collect();
            let vals: Vec<String> = pairs.iter().map(|(_, v)| sql_literal(v)).collect();
            generated_sql.push(format!(
                "INSERT INTO {} ({}) VALUES ({});",
                table,
                cols.join(", "),
                vals.join(", ")
            ));
        }
    }

    // 2. UPDATES
    for update in &payload.updates {
        if let (Some(pk_obj), Some(fields_obj)) = (
            update.primary_key_values.as_object(),
            update.updated_fields.as_object(),
        ) {
            let set_clauses: Vec<String> = fields_obj
                .iter()
                .map(|(k, v)| format!("{} = {}", dialect.qi(k), sql_literal(v)))
                .collect();
            let where_clauses: Vec<String> = pk_obj
                .iter()
                .map(|(k, v)| format!("{} = {}", dialect.qi(k), sql_literal(v)))
                .collect();
            generated_sql.push(format!(
                "UPDATE {} SET {} WHERE {};",
                table,
                set_clauses.join(", "),
                where_clauses.join(" AND ")
            ));
        }
    }

    // 3. DELETES
    for del in &payload.deletes {
        if let Some(pk_obj) = del.as_object() {
            let where_clauses: Vec<String> = pk_obj
                .iter()
                .map(|(k, v)| format!("{} = {}", dialect.qi(k), sql_literal(v)))
                .collect();
            generated_sql.push(format!(
                "DELETE FROM {} WHERE {};",
                table,
                where_clauses.join(" AND ")
            ));
        }
    }

    StagedDiffSummary {
        insert_count: payload.inserts.len(),
        update_count: payload.updates.len(),
        delete_count: payload.deletes.len(),
        generated_sql,
    }
}

/// Serialize rows (as JSON maps in consistent column order) to an export format.
/// `ddl` is emitted at the top of SQL dumps.
/// `headers` must match the order of each row's values (insertion order of the maps).
pub fn export_rows_bytes(
    format: &str,
    table: &str,
    ddl: Option<&str>,
    headers: &[String],
    rows: &[serde_json::Map<String, serde_json::Value>],
    dialect: Dialect,
) -> Result<Vec<u8>, String> {
    match format {
        "csv" => {
            let mut wtr = csv::Writer::from_writer(Vec::new());
            wtr.write_record(headers).map_err(|e| e.to_string())?;
            for r in rows {
                let vals: Vec<String> = headers
                    .iter()
                    .map(|h| match r.get(h) {
                        Some(serde_json::Value::Null) | None => String::new(),
                        Some(serde_json::Value::String(s)) => s.clone(),
                        Some(other) => other.to_string(),
                    })
                    .collect();
                wtr.write_record(&vals).map_err(|e| e.to_string())?;
            }
            wtr.into_inner().map_err(|e| e.to_string())
        }
        "json" => {
            let arr: Vec<serde_json::Value> = rows
                .iter()
                .map(|r| serde_json::Value::Object(r.clone()))
                .collect();
            Ok(serde_json::to_string_pretty(&arr)
                .map_err(|e| e.to_string())?
                .into_bytes())
        }
        "sql" => {
            let mut out = String::new();
            if let Some(d) = ddl {
                if !d.trim().is_empty() {
                    out.push_str(&format!("{};\n\n", d.trim_end_matches(';')));
                }
            }
            for r in rows {
                let cols: Vec<String> = headers.iter().map(|h| dialect.qi(h)).collect();
                let vals: Vec<String> = headers
                    .iter()
                    .map(|h| sql_literal(r.get(h).unwrap_or(&serde_json::Value::Null)))
                    .collect();
                out.push_str(&format!(
                    "INSERT INTO {} ({}) VALUES ({});\n",
                    dialect.qi(table),
                    cols.join(", "),
                    vals.join(", ")
                ));
            }
            Ok(out.into_bytes())
        }
        "xlsx" => {
            let mut wb = rust_xlsxwriter::Workbook::new();
            let safe: String = table
                .chars()
                .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
                .take(31)
                .collect();
            let ws = wb
                .add_worksheet()
                .set_name(if safe.is_empty() { "export" } else { &safe })
                .map_err(|e| e.to_string())?;

            let bold = rust_xlsxwriter::Format::new().set_bold();
            for (c, h) in headers.iter().enumerate() {
                ws.write_string_with_format(0, c as u16, h, &bold)
                    .map_err(|e| e.to_string())?;
            }
            for (ri, r) in rows.iter().enumerate() {
                for (ci, h) in headers.iter().enumerate() {
                    let (row32, col16) = ((ri + 1) as u32, ci as u16);
                    match r.get(h) {
                        None | Some(serde_json::Value::Null) => {}
                        Some(serde_json::Value::Bool(b)) => {
                            ws.write_boolean(row32, col16, *b).map_err(|e| e.to_string())?;
                        }
                        Some(serde_json::Value::Number(n)) => {
                            ws.write_number(row32, col16, n.as_f64().unwrap_or(0.0))
                                .map_err(|e| e.to_string())?;
                        }
                        Some(other) => {
                            let s = match other {
                                serde_json::Value::String(s) => s.clone(),
                                o => o.to_string(),
                            };
                            ws.write_string(row32, col16, &s).map_err(|e| e.to_string())?;
                        }
                    }
                }
            }
            ws.autofit();
            wb.save_to_buffer().map_err(|e| e.to_string())
        }
        other => Err(format!("Unsupported export format: {}", other)),
    }
}

/// Build a logical SQL dump (DDL + INSERTs + extra objects) — used for backups
/// on engines without a file-level backup primitive.
pub fn dump_content(
    dialect: Dialect,
    table_dumps: &[(String /*ddl*/, String /*table*/, Vec<serde_json::Map<String, serde_json::Value>>)],
    extra_ddls: &[String],
) -> String {
    let mut out = String::from("-- TablePro backup\nBEGIN TRANSACTION;\n\n");
    for (ddl, table, rows) in table_dumps {
        if !ddl.trim().is_empty() {
            out.push_str(&format!("{};\n", ddl.trim_end_matches(';')));
        }
        for r in rows {
            let cols: Vec<String> = r.keys().map(|k| dialect.qi(k)).collect();
            let vals: Vec<String> = r.values().map(sql_literal).collect();
            out.push_str(&format!(
                "INSERT INTO {} ({}) VALUES ({});\n",
                dialect.qi(table),
                cols.join(", "),
                vals.join(", ")
            ));
        }
        out.push('\n');
    }
    for s in extra_ddls {
        if !s.trim().is_empty() {
            out.push_str(&format!("{};\n", s.trim_end_matches(';')));
        }
    }
    out.push_str("\nCOMMIT;\n");
    out
}

/// Validate CSV header names so generated DDL stays safe, and produce the
/// CREATE TABLE IF NOT EXISTS statement used by CSV import.
pub fn import_table_ddl(
    dialect: Dialect,
    table: &str,
    headers: &[String],
) -> Result<String, String> {
    if headers.is_empty() {
        return Err("CSV file has no header row".to_string());
    }
    for h in headers {
        if h.is_empty() || !h.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return Err(format!(
                "Invalid CSV header '{}'. Headers must be alphanumeric/underscore.",
                h
            ));
        }
    }
    let col_defs: Vec<String> = headers
        .iter()
        .map(|h| format!("{} TEXT", dialect.qi(h)))
        .collect();
    Ok(format!(
        "CREATE TABLE IF NOT EXISTS {} ({});",
        dialect.qi(table),
        col_defs.join(", ")
    ))
}

/// Parameterized INSERT for CSV import rows.
pub fn import_insert_sql(
    dialect: Dialect,
    params: ParamStyle,
    table: &str,
    headers: &[String],
) -> String {
    let placeholders: Vec<String> = (1..=headers.len()).map(|i| params.ph(i)).collect();
    format!(
        "INSERT INTO {} ({}) VALUES ({})",
        dialect.qi(table),
        headers.iter().map(|h| dialect.qi(h)).collect::<Vec<_>>().join(", "),
        placeholders.join(", ")
    )
}

/// Allowed referential actions for FOREIGN KEY clauses (whitelist).
pub const FK_ACTIONS: [&str; 5] = ["NO ACTION", "RESTRICT", "CASCADE", "SET NULL", "SET DEFAULT"];

/// Whitelist a referential action; anything else collapses to NO ACTION.
pub fn sanitize_fk_action(action: &str) -> &'static str {
    let up = action.trim().to_uppercase();
    FK_ACTIONS
        .iter()
        .copied()
        .find(|a| *a == up)
        .unwrap_or("NO ACTION")
}

/// Build an `ALTER TABLE ... ADD CONSTRAINT ... FOREIGN KEY` statement
/// (valid ANSI + MySQL; PostgreSQL requires the constraint name too).
pub fn add_fk_sql(
    dialect: Dialect,
    constraint_name: &str,
    table: &str,
    column: &str,
    ref_table: &str,
    ref_column: &str,
    on_delete: &str,
    on_update: &str,
) -> String {
    format!(
        "ALTER TABLE {} ADD CONSTRAINT {} FOREIGN KEY ({}) REFERENCES {}({}) ON DELETE {} ON UPDATE {};",
        dialect.qi(table),
        dialect.qi(constraint_name),
        dialect.qi(column),
        dialect.qi(ref_table),
        dialect.qi(ref_column),
        sanitize_fk_action(on_delete),
        sanitize_fk_action(on_update),
    )
}

/// Sanitize a user-supplied sort direction (defense in depth: whitelist only).
pub fn sanitize_sort_dir(dir: Option<&str>) -> &'static str {
    match dir.map(|d| d.trim().to_uppercase()).as_deref() {
        Some("DESC") => "DESC",
        _ => "ASC",
    }
}

/// Build a SELECT ... LIMIT/OFFSET query honoring filter/sort inputs.
pub fn table_data_query(
    dialect: Dialect,
    table: &str,
    limit: u32,
    offset: u32,
    filter: Option<&str>,
    sort_by: Option<&str>,
    sort_dir: Option<&str>,
) -> String {
    let mut q = format!("SELECT * FROM {}", dialect.qi(table));
    if let Some(f) = filter {
        if !f.trim().is_empty() {
            q.push_str(&format!(" WHERE {}", f));
        }
    }
    if let Some(s) = sort_by {
        if !s.trim().is_empty() {
            q.push_str(&format!(" ORDER BY {} {}", dialect.qi(s), sanitize_sort_dir(sort_dir)));
        }
    }
    q.push_str(&format!(" LIMIT {} OFFSET {}", limit, offset));
    q
}

pub fn count_query(dialect: Dialect, table: &str, filter: Option<&str>) -> String {
    let mut q = format!("SELECT COUNT(*) AS n FROM {}", dialect.qi(table));
    if let Some(f) = filter {
        if !f.trim().is_empty() {
            q.push_str(&format!(" WHERE {}", f));
        }
    }
    q
}

#[cfg(test)]
mod tests;

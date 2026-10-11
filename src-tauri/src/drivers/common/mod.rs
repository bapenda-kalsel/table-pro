//! Shared, dialect-aware SQL helpers used by every driver implementation.
//! Everything here is pure (no I/O, no pool) so it can be unit-tested
//! without a live database.

pub mod staged;

use crate::errors::{AppError, AppResult};
use crate::models::ForeignKeySpec;

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
) -> AppResult<Vec<u8>> {
    match format {
        "csv" => {
            let mut wtr = csv::Writer::from_writer(Vec::new());
            wtr.write_record(headers).map_err(AppError::from)?;
            for r in rows {
                let vals: Vec<String> = headers
                    .iter()
                    .map(|h| match r.get(h) {
                        Some(serde_json::Value::Null) | None => String::new(),
                        Some(serde_json::Value::String(s)) => s.clone(),
                        Some(other) => other.to_string(),
                    })
                    .collect();
                wtr.write_record(&vals).map_err(AppError::from)?;
            }
            wtr.into_inner().map_err(AppError::from)
        }
        "json" => {
            let arr: Vec<serde_json::Value> = rows
                .iter()
                .map(|r| serde_json::Value::Object(r.clone()))
                .collect();
            Ok(serde_json::to_string_pretty(&arr)
                .map_err(AppError::from)?
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
                .map(|c| {
                    if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                        c
                    } else {
                        '_'
                    }
                })
                .take(31)
                .collect();
            let ws = wb
                .add_worksheet()
                .set_name(if safe.is_empty() { "export" } else { &safe })
                .map_err(AppError::from)?;

            let bold = rust_xlsxwriter::Format::new().set_bold();
            for (c, h) in headers.iter().enumerate() {
                ws.write_string_with_format(0, c as u16, h, &bold)
                    .map_err(AppError::from)?;
            }
            for (ri, r) in rows.iter().enumerate() {
                for (ci, h) in headers.iter().enumerate() {
                    let (row32, col16) = ((ri + 1) as u32, ci as u16);
                    match r.get(h) {
                        None | Some(serde_json::Value::Null) => {}
                        Some(serde_json::Value::Bool(b)) => {
                            ws.write_boolean(row32, col16, *b).map_err(AppError::from)?;
                        }
                        Some(serde_json::Value::Number(n)) => {
                            ws.write_number(row32, col16, n.as_f64().unwrap_or(0.0))
                                .map_err(AppError::from)?;
                        }
                        Some(other) => {
                            let s = match other {
                                serde_json::Value::String(s) => s.clone(),
                                o => o.to_string(),
                            };
                            ws.write_string(row32, col16, &s).map_err(AppError::from)?;
                        }
                    }
                }
            }
            ws.autofit();
            wb.save_to_buffer().map_err(AppError::from)
        }
        other => Err(AppError::unsupported(format!(
            "Unsupported export format: {}",
            other
        ))),
    }
}

/// One table's contribution to a logical dump: DDL, name, and row maps.
pub type TableDump = (
    String,
    String,
    Vec<serde_json::Map<String, serde_json::Value>>,
);

/// Build a logical SQL dump (DDL + INSERTs + extra objects) — used for backups
/// on engines without a file-level backup primitive.
pub fn dump_content(dialect: Dialect, table_dumps: &[TableDump], extra_ddls: &[String]) -> String {
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
pub fn import_table_ddl(dialect: Dialect, table: &str, headers: &[String]) -> AppResult<String> {
    if headers.is_empty() {
        return Err(AppError::validation("CSV file has no header row"));
    }
    for h in headers {
        if h.is_empty() || !h.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return Err(AppError::validation(format!(
                "Invalid CSV header '{}'. Headers must be alphanumeric/underscore.",
                h
            )));
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
        headers
            .iter()
            .map(|h| dialect.qi(h))
            .collect::<Vec<_>>()
            .join(", "),
        placeholders.join(", ")
    )
}

/// Allowed referential actions for FOREIGN KEY clauses (whitelist).
pub const FK_ACTIONS: [&str; 5] = [
    "NO ACTION",
    "RESTRICT",
    "CASCADE",
    "SET NULL",
    "SET DEFAULT",
];

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
pub fn add_fk_sql(dialect: Dialect, spec: &ForeignKeySpec) -> String {
    format!(
        "ALTER TABLE {} ADD CONSTRAINT {} FOREIGN KEY ({}) REFERENCES {}({}) ON DELETE {} ON UPDATE {};",
        dialect.qi(&spec.table),
        dialect.qi(&spec.constraint_name),
        dialect.qi(&spec.column),
        dialect.qi(&spec.ref_table),
        dialect.qi(&spec.ref_column),
        sanitize_fk_action(&spec.on_delete),
        sanitize_fk_action(&spec.on_update),
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
            q.push_str(&format!(
                " ORDER BY {} {}",
                dialect.qi(s),
                sanitize_sort_dir(sort_dir)
            ));
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

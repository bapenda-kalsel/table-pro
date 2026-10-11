//! Write-risk analysis and production approval.
//!
//! Policy:
//! - Every mutating entry point calls `ApprovalStore::require` — the guard
//!   is backend-enforced, not a UI hint.
//! - Approvals are one-shot tokens bound to (connection id, session
//!   generation, exact request). Reconnecting or changing the request
//!   invalidates them; a used token cannot replay.
//! - `analyze_sql` parses the WHOLE batch with a dialect parser. Anything
//!   unparseable or unclassified is `Unknown`, and `Unknown` requires
//!   confirmation exactly like `Write` — fail safe, never fail open.
//! - This is accidental-write protection for a trusted desktop UI, not an
//!   authorization boundary: database roles remain authoritative.

use crate::errors::{AppError, AppResult, ConfirmationChallenge};
use crate::models::DatabaseDriverType;
use crate::services::connections::LiveConnection;
use sqlparser::ast::Statement;
use sqlparser::dialect::{Dialect, MySqlDialect, PostgreSqlDialect, SQLiteDialect};
use sqlparser::parser::Parser;
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SqlRisk {
    /// Every statement in the batch is provably read-only.
    Read,
    /// At least one statement is a known mutating operation.
    Write,
    /// Parse failed or a statement type we do not classify — treat as write
    /// for policy purposes.
    Unknown,
}

fn dialect_for(engine: &DatabaseDriverType) -> Box<dyn Dialect> {
    match engine {
        DatabaseDriverType::SQLite => Box::new(SQLiteDialect {}),
        DatabaseDriverType::MySQL => Box::new(MySqlDialect {}),
        DatabaseDriverType::PostgreSQL => Box::new(PostgreSqlDialect {}),
    }
}

/// Parse `sql` in the engine's dialect and classify the entire batch.
/// Returns the worst risk across all statements.
pub fn analyze_sql(sql: &str, engine: &DatabaseDriverType) -> SqlRisk {
    if sql.trim().is_empty() {
        return SqlRisk::Read;
    }
    let dialect = dialect_for(engine);
    let statements = match Parser::parse_sql(dialect.as_ref(), sql) {
        Ok(s) => s,
        // Unparseable batch: could contain anything (dialect-specific DML,
        // multi-statement tricks). Fail closed.
        Err(_) => return SqlRisk::Unknown,
    };
    statements
        .iter()
        .map(stmt_risk)
        .max_by_key(|r| match r {
            SqlRisk::Read => 0,
            SqlRisk::Write => 1,
            SqlRisk::Unknown => 2,
        })
        .unwrap_or(SqlRisk::Read)
}

/// Classify one parsed statement. Anything not on the read whitelist or
/// the explicit write list falls back to `Unknown` — new statement kinds
/// added by sqlparser upgrades stay conservative automatically.
fn stmt_risk(stmt: &Statement) -> SqlRisk {
    use sqlparser::ast::Statement as S;
    match stmt {
        S::Query(_) => SqlRisk::Read,

        // EXPLAIN without ANALYZE only plans; with ANALYZE it executes.
        S::Explain { analyze, .. } => {
            if *analyze {
                SqlRisk::Write
            } else {
                SqlRisk::Read
            }
        }

        // Metadata reads.
        S::ShowVariable { .. }
        | S::ShowVariables { .. }
        | S::ShowTables { .. }
        | S::ShowColumns { .. }
        | S::ShowCreate { .. }
        | S::ShowDatabases { .. } => SqlRisk::Read,

        // Explicit mutations and side-effecting commands.
        S::Insert(_)
        | S::Update { .. }
        | S::Delete(_)
        | S::Merge { .. }
        | S::Copy { .. }
        | S::CopyIntoSnowflake { .. }
        | S::CreateTable(_)
        | S::CreateView { .. }
        | S::CreateIndex(_)
        | S::AlterTable { .. }
        | S::Drop { .. }
        | S::Truncate(_)
        | S::Grant { .. }
        | S::Revoke { .. }
        | S::Comment { .. }
        | S::Call(_)
        | S::Execute { .. }
        | S::Prepare { .. }
        | S::Deallocate { .. }
        | S::Pragma { .. }
        | S::Analyze { .. }
        | S::Vacuum { .. }
        | S::Kill { .. }
        | S::Use { .. } => SqlRisk::Write,

        _ => SqlRisk::Unknown,
    }
}

struct Approval {
    connection_id: String,
    generation: u64,
    request_sig: String,
    used: bool,
}

/// One-shot production approvals, bound to connection+session+request.
pub struct ApprovalStore {
    inner: Mutex<HashMap<String, Approval>>,
}

/// Cap before a stale-token flood forces a clear — fail closed either way.
const MAX_PENDING: usize = 512;

impl ApprovalStore {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
        }
    }

    /// Gate a mutating operation on a possibly-production session.
    ///
    /// `request_sig` must uniquely identify the exact request (e.g. the SQL
    /// text or the canonical staged payload). Returns `ConfirmationRequired`
    /// carrying a one-shot token when approval is needed but absent, and
    /// `ConfirmationInvalid` when a supplied token doesn't match.
    /// On non-production sessions this is a no-op.
    pub fn require(
        &self,
        conn: &LiveConnection,
        request_sig: &str,
        token: Option<&str>,
        summary: String,
    ) -> AppResult<()> {
        if !conn.environment.requires_write_confirmation() {
            return Ok(());
        }
        let mut map = self.inner.lock().unwrap();
        match token {
            Some(t) => match map.get_mut(t) {
                Some(ap)
                    if !ap.used
                        && ap.connection_id == conn.id
                        && ap.generation == conn.generation
                        && ap.request_sig == request_sig =>
                {
                    ap.used = true;
                    Ok(())
                }
                _ => Err(AppError::confirmation_invalid(
                    "Approval expired or does not match this request — review and approve again.",
                )),
            },
            None => {
                if map.len() >= MAX_PENDING {
                    map.retain(|_, a| !a.used);
                    if map.len() >= MAX_PENDING {
                        map.clear();
                    }
                }
                let token = uuid::Uuid::new_v4().to_string();
                map.insert(
                    token.clone(),
                    Approval {
                        connection_id: conn.id.clone(),
                        generation: conn.generation,
                        request_sig: request_sig.to_string(),
                        used: false,
                    },
                );
                Err(AppError::ConfirmationRequired {
                    challenge: Box::new(ConfirmationChallenge {
                        token,
                        summary,
                        connection_name: conn.connection_name.clone(),
                    }),
                })
            }
        }
    }
}

impl Default for ApprovalStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;

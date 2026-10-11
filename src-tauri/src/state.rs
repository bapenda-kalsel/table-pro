use crate::drivers::DatabaseDriver;
use crate::models::{detect_statement_type, SqlLogEntry};
use crate::vault;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub type DriverMap = Arc<RwLock<HashMap<String, Box<dyn DatabaseDriver>>>>;
pub type LogList = Arc<RwLock<Vec<SqlLogEntry>>>;
/// connection_id -> (display name, environment tag) for enriching log/audit entries.
pub type ConnMetaMap = Arc<RwLock<HashMap<String, (String, String)>>>;

#[derive(Clone)]
pub struct AppState {
    pub drivers: DriverMap,
    pub logs: LogList,
    pub conn_meta: ConnMetaMap,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            drivers: Arc::new(RwLock::new(HashMap::new())),
            logs: Arc::new(RwLock::new(Vec::new())),
            conn_meta: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Cache connection display metadata (called on connect) for audit enrichment.
    pub async fn register_conn_meta(&self, id: &str, name: String, env: String) {
        let mut meta = self.conn_meta.write().await;
        meta.insert(id.to_string(), (name, env));
    }

    pub async fn log_sql(
        &self,
        connection_id: Option<String>,
        query: String,
        duration_ms: f64,
        is_error: bool,
        error_message: Option<String>,
        affected_rows: Option<u64>,
    ) {
        let (conn_name, env) = match &connection_id {
            Some(cid) => self
                .conn_meta
                .read()
                .await
                .get(cid)
                .cloned()
                .map(|(n, e)| (Some(n), Some(e)))
                .unwrap_or((None, None)),
            None => (None, None),
        };

        let entry = SqlLogEntry {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: chrono::Local::now()
                .format("%Y-%m-%d %H:%M:%S%.3f")
                .to_string(),
            statement_type: Some(detect_statement_type(&query)),
            connection_id,
            connection_name: conn_name,
            environment: env,
            query,
            duration_ms,
            is_error,
            error_message,
            affected_rows,
        };

        // Persist to the durable audit trail (best-effort, never blocks the caller).
        vault::append_audit(&entry);

        let mut logs = self.logs.write().await;
        // Keep last 500 entries
        if logs.len() > 500 {
            logs.remove(0);
        }
        logs.push(entry);
    }
}

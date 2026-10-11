//! AI SQL assistant — multi-provider (OpenAI-compatible, Anthropic,
//! Gemini). Users bring their own API key; keys live in the OS keychain
//! via `CredentialStore` under `ai:<provider>`, while non-secret config
//! (provider, model, base_url) is a JSON file in the vault dir.
//!
//! The service only *suggests* SQL — it never executes it. Generated SQL
//! goes through the same editor/preview/production-guard path as
//! hand-written SQL, so the AI cannot bypass safety checks.

use crate::errors::{AppError, AppResult};
use crate::models::{DatabaseDriverType, TableColumnMetadata};
use crate::vault;
use crate::vault::credentials::{CredentialStore, KeychainStore};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AiProvider {
    Openai,
    Anthropic,
    Gemini,
}

impl AiProvider {
    pub fn key_account(&self) -> String {
        format!("ai:{}", self.as_str())
    }
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Openai => "openai",
            Self::Anthropic => "anthropic",
            Self::Gemini => "gemini",
        }
    }
    pub fn default_model(&self) -> &'static str {
        match self {
            Self::Openai => "gpt-4o-mini",
            Self::Anthropic => "claude-haiku-4-5",
            Self::Gemini => "gemini-2.0-flash",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "openai" => Some(Self::Openai),
            "anthropic" => Some(Self::Anthropic),
            "gemini" => Some(Self::Gemini),
            _ => None,
        }
    }
}

/// Non-secret config persisted to `~/.tablepro/ai.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct AiFileConfig {
    provider: String,
    model: String,
    base_url: Option<String>,
}

impl Default for AiFileConfig {
    fn default() -> Self {
        Self {
            provider: "openai".into(),
            model: AiProvider::Openai.default_model().into(),
            base_url: None,
        }
    }
}

/// Config surface returned to the frontend — never includes the key.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiConfigView {
    pub provider: String,
    pub model: String,
    pub base_url: Option<String>,
    pub has_key: bool,
    pub configured: bool,
}

pub struct AiService {
    http: reqwest::Client,
    credentials: Arc<dyn CredentialStore>,
}

impl AiService {
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(45))
                .build()
                .expect("reqwest client"),
            credentials: Arc::new(KeychainStore::new()),
        }
    }

    #[cfg(test)]
    pub fn for_test(credentials: Arc<dyn CredentialStore>) -> Self {
        Self {
            http: reqwest::Client::new(),
            credentials,
        }
    }

    fn read_config(&self) -> AppResult<AiFileConfig> {
        let path = vault::file_in_config_dir("ai.json");
        Ok(vault::json_file::load_doc(&path)?.unwrap_or_default())
    }

    fn write_config(&self, cfg: &AiFileConfig) -> AppResult<()> {
        let path = vault::file_in_config_dir("ai.json");
        vault::json_file::write_doc_atomic(&path, cfg)
    }

    pub async fn get_config(&self) -> AppResult<AiConfigView> {
        let cfg = self.read_config()?;
        let provider = AiProvider::parse(&cfg.provider).ok_or_else(|| {
            AppError::validation(format!("Unknown AI provider '{}'", cfg.provider))
        })?;
        let has_key = self
            .credentials
            .get(&provider.key_account())?
            .map(|k| !k.is_empty())
            .unwrap_or(false);
        Ok(AiConfigView {
            provider: cfg.provider,
            model: cfg.model,
            base_url: cfg.base_url,
            has_key,
            configured: has_key,
        })
    }

    /// Save provider/model/base_url; `api_key` of `Some("")` clears the
    /// stored key, `Some(key)` replaces it, `None` leaves it alone.
    pub async fn save_config(
        &self,
        provider: String,
        model: String,
        base_url: Option<String>,
        api_key: Option<String>,
    ) -> AppResult<AiConfigView> {
        let parsed = AiProvider::parse(&provider)
            .ok_or_else(|| AppError::validation(format!("Unknown AI provider '{}'", provider)))?;
        if model.trim().is_empty() {
            return Err(AppError::validation("Model name is required"));
        }
        self.write_config(&AiFileConfig {
            provider: provider.clone(),
            model: model.trim().into(),
            base_url: base_url.filter(|u| !u.trim().is_empty()),
        })?;
        if let Some(key) = api_key {
            if key.is_empty() {
                self.credentials.delete(&parsed.key_account())?;
            } else {
                self.credentials.set(&parsed.key_account(), key.trim())?;
            }
        }
        self.get_config().await
    }

    /// Ask the configured provider for SQL given the schema context and a
    /// natural-language request. The prompt never contains credentials.
    pub async fn generate_sql(
        &self,
        engine: &DatabaseDriverType,
        schema_text: &str,
        prompt: &str,
        current_sql: Option<&str>,
    ) -> AppResult<String> {
        let cfg = self.read_config()?;
        let provider = AiProvider::parse(&cfg.provider).ok_or_else(|| {
            AppError::validation(format!("Unknown AI provider '{}'", cfg.provider))
        })?;
        let key = self
            .credentials
            .get(&provider.key_account())?
            .filter(|k| !k.is_empty())
            .ok_or_else(|| {
                AppError::validation(format!(
                    "No API key stored for {} — open AI settings and add your key.",
                    provider.as_str()
                ))
            })?;

        let dialect = match engine {
            DatabaseDriverType::SQLite => "SQLite",
            DatabaseDriverType::MySQL => "MySQL",
            DatabaseDriverType::PostgreSQL => "PostgreSQL",
        };
        let system = format!(
            "You are a senior {} expert embedded in a database client. \
             Output ONLY the raw SQL statement(s) answering the request — \
             no explanation, no markdown fences, no comments. \
             Use identifiers exactly as shown in the schema.",
            dialect
        );
        let mut user = format!("SCHEMA:\n{}\n\nREQUEST: {}", schema_text, prompt);
        if let Some(cur) = current_sql.filter(|s| !s.trim().is_empty()) {
            user.push_str(&format!(
                "\n\nCURRENT SQL (fix or improve it if the request refers to it):\n{}",
                cur
            ));
        }

        let raw = self
            .call_provider(&provider, &cfg, &key, &system, &user)
            .await?;
        Ok(extract_sql(&raw))
    }

    async fn call_provider(
        &self,
        provider: &AiProvider,
        cfg: &AiFileConfig,
        key: &str,
        system: &str,
        user: &str,
    ) -> AppResult<String> {
        match provider {
            AiProvider::Openai => {
                let base = cfg
                    .base_url
                    .clone()
                    .unwrap_or_else(|| "https://api.openai.com/v1".into());
                let body = serde_json::json!({
                    "model": cfg.model,
                    "messages": [
                        { "role": "system", "content": system },
                        { "role": "user", "content": user }
                    ],
                    "temperature": 0.1
                });
                let resp = self
                    .http
                    .post(format!("{}/chat/completions", base.trim_end_matches('/')))
                    .bearer_auth(key)
                    .json(&body)
                    .send()
                    .await
                    .map_err(|e| AppError::connection(format!("AI request failed: {}", e)))?;
                let resp = provider_error_if_any(resp, "openai")?;
                let resp: serde_json::Value = resp
                    .json()
                    .await
                    .map_err(|e| AppError::connection(format!("Invalid AI response: {}", e)))?;
                resp["choices"][0]["message"]["content"]
                    .as_str()
                    .map(|s| s.to_string())
                    .ok_or_else(|| AppError::internal("Empty response from AI provider"))
            }
            AiProvider::Anthropic => {
                let base = cfg
                    .base_url
                    .clone()
                    .unwrap_or_else(|| "https://api.anthropic.com/v1".into());
                let body = serde_json::json!({
                    "model": cfg.model,
                    "max_tokens": 1024,
                    "system": system,
                    "messages": [{ "role": "user", "content": user }]
                });
                let resp = self
                    .http
                    .post(format!("{}/messages", base.trim_end_matches('/')))
                    .header("x-api-key", key)
                    .header("anthropic-version", "2023-06-01")
                    .json(&body)
                    .send()
                    .await
                    .map_err(|e| AppError::connection(format!("AI request failed: {}", e)))?;
                let resp = provider_error_if_any(resp, "anthropic")?;
                let resp: serde_json::Value = resp
                    .json()
                    .await
                    .map_err(|e| AppError::connection(format!("Invalid AI response: {}", e)))?;
                resp["content"][0]["text"]
                    .as_str()
                    .map(|s| s.to_string())
                    .ok_or_else(|| AppError::internal("Empty response from AI provider"))
            }
            AiProvider::Gemini => {
                let base = cfg
                    .base_url
                    .clone()
                    .unwrap_or_else(|| "https://generativelanguage.googleapis.com/v1beta".into());
                let body = serde_json::json!({
                    "system_instruction": { "parts": [{ "text": system }] },
                    "contents": [{ "role": "user", "parts": [{ "text": user }] }],
                    "generationConfig": { "temperature": 0.1 }
                });
                let resp = self
                    .http
                    .post(format!(
                        "{}/models/{}:generateContent",
                        base.trim_end_matches('/'),
                        cfg.model
                    ))
                    .header("x-goog-api-key", key)
                    .json(&body)
                    .send()
                    .await
                    .map_err(|e| AppError::connection(format!("AI request failed: {}", e)))?;
                let resp = provider_error_if_any(resp, "gemini")?;
                let resp: serde_json::Value = resp
                    .json()
                    .await
                    .map_err(|e| AppError::connection(format!("Invalid AI response: {}", e)))?;
                resp["candidates"][0]["content"]["parts"][0]["text"]
                    .as_str()
                    .map(|s| s.to_string())
                    .ok_or_else(|| AppError::internal("Empty response from AI provider"))
            }
        }
    }
}

impl Default for AiService {
    fn default() -> Self {
        Self::new()
    }
}

/// Surface a provider's error body (status text, not the key).
fn provider_error_if_any(resp: reqwest::Response, provider: &str) -> AppResult<reqwest::Response> {
    if resp.status().is_success() {
        return Ok(resp);
    }
    let status = resp.status().as_u16();
    Err(match status {
        401 | 403 => AppError::credential_unavailable(format!(
            "{} rejected the API key ({}). Check the key in AI settings.",
            provider, status
        )),
        429 => AppError::validation(format!(
            "{} rate limit reached — wait a moment and try again.",
            provider
        )),
        _ => AppError::connection(format!("{} API error: HTTP {}", provider, status)),
    })
}

/// Strip markdown fences / prose wrappers from model output.
pub fn extract_sql(raw: &str) -> String {
    let text = raw.trim();
    if let Some(start) = text.find("```") {
        let after = &text[start + 3..];
        let after = after
            .strip_prefix("sql")
            .unwrap_or(after)
            .trim_start_matches('\n');
        if let Some(end) = after.find("```") {
            return after[..end].trim().to_string();
        }
    }
    text.to_string()
}

/// Render a compact schema summary for the prompt (bounded so context
/// stays small — cap tables and columns, never include row data).
pub fn schema_context(tables: &[(String, Vec<TableColumnMetadata>)], max_tables: usize) -> String {
    let mut out = String::new();
    for (name, cols) in tables.iter().take(max_tables) {
        let col_list: Vec<String> = cols
            .iter()
            .take(20)
            .map(|c| {
                let pk = if c.is_primary_key { " PK" } else { "" };
                format!("{} {}{}", c.name, c.data_type, pk)
            })
            .collect();
        out.push_str(&format!("{}({})\n", name, col_list.join(", ")));
    }
    if tables.len() > max_tables {
        out.push_str(&format!(
            "-- and {} more tables\n",
            tables.len() - max_tables
        ));
    }
    out
}

#[cfg(test)]
mod tests;

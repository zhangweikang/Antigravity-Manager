//! Perplexity Web Login Authentication Module
//!
//! This module handles Perplexity authentication via web login,
//! capturing session cookies for API access without requiring an API key.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use tokio::sync::mpsc;

// Perplexity login URL
const PERPLEXITY_LOGIN_URL: &str = "https://www.perplexity.ai/";
const PERPLEXITY_API_BASE: &str = "https://www.perplexity.ai";
const PERPLEXITY_ACCOUNTS_FILE: &str = "perplexity_accounts.json";

/// Perplexity account authenticated via web login
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerplexityWebAccount {
    pub id: String,
    pub email: Option<String>,
    pub name: String,
    pub cookies: String,               // Serialized cookie string
    pub session_token: Option<String>, // Extracted session token
    pub created_at: i64,               // Unix timestamp
    pub expires_at: Option<i64>,       // Unix timestamp
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub proxy_enabled: bool,
    pub last_used: Option<i64>, // Unix timestamp
}

fn default_true() -> bool {
    true
}

impl PerplexityWebAccount {
    pub fn new(name: String, cookies: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            email: None,
            name,
            cookies,
            session_token: None,
            created_at: Utc::now().timestamp(),
            expires_at: None,
            enabled: true,
            proxy_enabled: true,
            last_used: None,
        }
    }
}

/// Login flow state
struct PerplexityLoginState {
    #[allow(dead_code)]
    state: String,
    result_tx: mpsc::Sender<Result<PerplexityWebAccount, String>>,
    result_rx: Option<mpsc::Receiver<Result<PerplexityWebAccount, String>>>,
}

static LOGIN_STATE: OnceLock<Mutex<Option<PerplexityLoginState>>> = OnceLock::new();

fn get_login_state() -> &'static Mutex<Option<PerplexityLoginState>> {
    LOGIN_STATE.get_or_init(|| Mutex::new(None))
}

/// Get the path to the perplexity accounts file
fn get_accounts_path() -> Result<PathBuf, String> {
    let data_dir = crate::modules::account::get_data_dir()?;
    Ok(data_dir.join(PERPLEXITY_ACCOUNTS_FILE))
}

/// Load all Perplexity accounts from disk
pub fn list_accounts() -> Result<Vec<PerplexityWebAccount>, String> {
    let path = get_accounts_path()?;
    if !path.exists() {
        return Ok(Vec::new());
    }

    let content =
        fs::read_to_string(&path).map_err(|e| format!("Failed to read accounts file: {}", e))?;

    if content.trim().is_empty() {
        return Ok(Vec::new());
    }

    let accounts: Vec<PerplexityWebAccount> = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse accounts file: {}", e))?;

    Ok(accounts)
}

/// Save all Perplexity accounts to disk
fn save_accounts(accounts: &[PerplexityWebAccount]) -> Result<(), String> {
    let path = get_accounts_path()?;
    let content = serde_json::to_string_pretty(accounts)
        .map_err(|e| format!("Failed to serialize accounts: {}", e))?;

    fs::write(&path, content).map_err(|e| format!("Failed to write accounts file: {}", e))?;

    Ok(())
}

/// Add a new Perplexity account
pub fn add_account(account: PerplexityWebAccount) -> Result<PerplexityWebAccount, String> {
    let mut accounts = list_accounts()?;

    // Check if account with same name already exists
    if accounts.iter().any(|a| a.name == account.name) {
        return Err(format!(
            "Account with name '{}' already exists",
            account.name
        ));
    }

    accounts.push(account.clone());
    save_accounts(&accounts)?;

    Ok(account)
}

/// Delete a Perplexity account by ID
pub fn delete_account(id: String) -> Result<(), String> {
    let mut accounts = list_accounts()?;
    let initial_len = accounts.len();

    accounts.retain(|a| a.id != id);

    if accounts.len() == initial_len {
        return Err(format!("Account with ID '{}' not found", id));
    }

    save_accounts(&accounts)?;
    Ok(())
}

/// Get the next available account for rotation (Simple Round Robin)
pub fn get_next_account() -> Option<PerplexityWebAccount> {
    if let Ok(accounts) = list_accounts() {
        // Filter enabled accounts
        let enabled: Vec<_> = accounts
            .into_iter()
            .filter(|a| a.enabled && a.proxy_enabled)
            .collect();
        if enabled.is_empty() {
            return None;
        }

        // Detailed selection logic can be added here (e.g., check last_used)
        // For now, just pick the first one or random
        use rand::seq::SliceRandom;
        let mut rng = rand::thread_rng();
        enabled.choose(&mut rng).cloned()
    } else {
        None
    }
}

/// Prepare login flow - creates channels and returns login URL
pub fn prepare_login() -> Result<String, String> {
    // Clean up any existing state
    if let Ok(mut state) = get_login_state().lock() {
        *state = None;
    }

    let state_str = uuid::Uuid::new_v4().to_string();
    let (tx, rx) = mpsc::channel(1);

    if let Ok(mut state) = get_login_state().lock() {
        *state = Some(PerplexityLoginState {
            state: state_str,
            result_tx: tx,
            result_rx: Some(rx),
        });
    }

    Ok(PERPLEXITY_LOGIN_URL.to_string())
}

/// Submit captured cookies from the webview
pub async fn submit_cookies(name: String, cookies: String) -> Result<(), String> {
    // Validate cookies by making a test request
    let is_valid = validate_cookies(&cookies).await?;
    if !is_valid {
        return Err("Invalid or expired cookies".to_string());
    }

    // Create the account
    let account = PerplexityWebAccount::new(name, cookies);

    // Save the account to disk
    add_account(account.clone())?;

    // If there is an active login flow (e.g. from complete_login), notify it
    // But we don't error if there isn't one, because the user might just be using the submit command directly
    let tx_option = {
        if let Ok(lock) = get_login_state().lock() {
            lock.as_ref().map(|state| state.result_tx.clone())
        } else {
            None
        }
    };

    if let Some(tx) = tx_option {
        let _ = tx.send(Ok(account)).await;
    }

    Ok(())
}

/// Wait for login to complete
pub async fn wait_for_login() -> Result<PerplexityWebAccount, String> {
    let mut rx = {
        let mut lock = get_login_state().lock().map_err(|e| e.to_string())?;
        let state = lock.as_mut().ok_or("No active login flow")?;
        state.result_rx.take().ok_or("Login already in progress")?
    };

    match rx.recv().await {
        Some(Ok(account)) => {
            // Clean up state
            if let Ok(mut lock) = get_login_state().lock() {
                *lock = None;
            }
            Ok(account)
        }
        Some(Err(e)) => {
            if let Ok(mut lock) = get_login_state().lock() {
                *lock = None;
            }
            Err(e)
        }
        None => {
            if let Ok(mut lock) = get_login_state().lock() {
                *lock = None;
            }
            Err("Login flow cancelled".to_string())
        }
    }
}

/// Cancel the current login flow
pub fn cancel_login() {
    if let Ok(mut lock) = get_login_state().lock() {
        *lock = None;
    }
}

/// Validate cookies by making a test request to Perplexity
pub async fn validate_cookies(cookies: &str) -> Result<bool, String> {
    let client = reqwest::Client::builder()
        .build()
        .map_err(|e| format!("Failed to build client: {}", e))?;

    // Try to access a protected endpoint to validate the session
    let response = client
        .get(format!("{}/api/auth/session", PERPLEXITY_API_BASE))
        .header("Cookie", cookies)
        .header(
            "User-Agent",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36",
        )
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    // If we get a 200 and valid JSON, the session is valid
    if response.status().is_success() {
        if let Ok(text) = response.text().await {
            // Check if the response indicates an authenticated session
            if text.contains("user") || text.contains("email") {
                return Ok(true);
            }
        }
    }

    Ok(false)
}

/// Extract useful fields from cookies string
pub fn extract_session_info(cookies: &str) -> Option<String> {
    // Look for common session cookie names
    for part in cookies.split(';') {
        let part = part.trim();
        if part.starts_with("__Secure-next-auth.session-token")
            || part.starts_with("next-auth.session-token")
        {
            if let Some((_, value)) = part.split_once('=') {
                return Some(value.to_string());
            }
        }
    }
    None
}

/// Send a chat request to Perplexity implementation via Web Interface
/// returns reqwest::Response which can be streamed or parsed as JSON
pub async fn send_chat_request(
    account: &PerplexityWebAccount,
    openai_body: &serde_json::Value,
) -> Result<reqwest::Response, String> {
    let client = reqwest::Client::builder()
        .cookie_store(true)
        .build()
        .map_err(|e| format!("Failed to build client: {}", e))?;

    // Map OpenAI model to Perplexity internal model
    let model = openai_body
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("sonar-pro");

    // Extract messages and get the last user message for `query_str`
    let messages = openai_body
        .get("messages")
        .and_then(|v| v.as_array())
        .ok_or("Missing messages")?;

    let last_message = messages.last().ok_or("Empty messages")?;
    let query_str = last_message
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    // Determine mode and model_preference
    // Mapping logic based on perplexity-ai/perplexity/config.py
    let (mode, model_pref) = match model {
        "sonar" => (
            "concise",
            serde_json::json!({ "auto": { "null": "turbo" } }),
        ), // auto -> turbo
        "sonar-pro" => (
            "copilot",
            serde_json::json!({ "pro": { "null": "pplx_pro" } }),
        ), // pro -> pplx_pro
        "sonar-reasoning" => (
            "copilot",
            serde_json::json!({ "reasoning": { "null": "pplx_reasoning" } }),
        ), // reasoning -> pplx_reasoning
        "sonar-reasoning-pro" => (
            "copilot",
            serde_json::json!({ "reasoning": { "null": "pplx_reasoning" } }),
        ), // reasoning -> pplx_reasoning (fallback)
        _ => (
            "copilot",
            serde_json::json!({ "pro": { "null": "pplx_pro" } }),
        ), // Default
    };

    // Construct internal API body
    // Based on perplexity-ai logic
    let internal_body = serde_json::json!({
        "query_str": query_str,
        "params": {
            "attachments": [],
            "frontend_context_uuid": uuid::Uuid::new_v4().to_string(),
            "frontend_uuid": uuid::Uuid::new_v4().to_string(),
            "is_incognito": false,
            "language": "en-US",
            "last_backend_uuid": serde_json::Value::Null,
            "mode": mode,
            "model_preference": model_pref,
            "source": "default",
            "sources": ["web"],
            "version": "2.18",
            // "timestamp": Utc::now().timestamp_millis(), // Optional, sometimes seen
        }
    });

    let url = "https://www.perplexity.ai/rest/sse/perplexity_ask";

    // Use the same client builder
    let response = client
        .post(url)
        .header("Cookie", &account.cookies)
        .header("Referer", "https://www.perplexity.ai/")
        .header("Content-Type", "application/json")
        // Headers from perplexity-ai config.py
        .header("accept", "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.7")
        .header("accept-language", "en-US,en;q=0.9")
        .header("cache-control", "max-age=0")
        .header("dnt", "1")
        .header("priority", "u=0, i")
        .header("sec-ch-ua", r#""Not;A=Brand";v="24", "Chromium";v="128""#)
        .header("sec-ch-ua-arch", r#""x86""#)
        .header("sec-ch-ua-bitness", r#""64""#)
        .header("sec-ch-ua-full-version", r#""128.0.6613.120""#)
        .header("sec-ch-ua-mobile", "?0")
        .header("sec-ch-ua-platform", r#""Windows""#)
        .header("sec-ch-ua-platform-version", r#""19.0.0""#)
        .header("sec-fetch-dest", "document")
        .header("sec-fetch-mode", "navigate")
        .header("sec-fetch-site", "same-origin")
        .header("sec-fetch-user", "?1")
        .header("upgrade-insecure-requests", "1")
        .header("user-agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36")
        .json(&internal_body)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        // Log the full error for debugging
        tracing::error!(
            "[PerplexityWeb] Request failed. Status: {}, Body: {}",
            status,
            text
        );

        return Err(format!(
            "Web API Error {}: {}. Cloudflare/Auth likely blocked usage.",
            status, text
        ));
    }

    Ok(response)
}

/// Make an authenticated request to Perplexity internal API
pub async fn make_authenticated_request(
    cookies: &str,
    endpoint: &str,
    body: Option<serde_json::Value>,
) -> Result<reqwest::Response, String> {
    let client = reqwest::Client::builder()
        .build()
        .map_err(|e| format!("Failed to build client: {}", e))?;
    let url = format!("{}{}", PERPLEXITY_API_BASE, endpoint);

    let mut request = if let Some(json_body) = body {
        client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&json_body)
    } else {
        client.get(&url)
    };

    request = request
        .header("Cookie", cookies)
        .header(
            "User-Agent",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
        )
        .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.7")
        .header("Accept-Language", "en-US,en;q=0.9")
        .header("Cache-Control", "max-age=0")
        .header("Origin", PERPLEXITY_API_BASE)
        .header("Referer", format!("{}/", PERPLEXITY_API_BASE))
        .header("Sec-Fetch-Dest", "document")
        .header("Sec-Fetch-Mode", "navigate")
        .header("Sec-Fetch-Site", "same-origin")
        .header("Sec-Fetch-User", "?1")
        .header("Upgrade-Insecure-Requests", "1");

    request
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_session_info() {
        let cookies = "__Secure-next-auth.session-token=abc123; other=value";
        assert_eq!(extract_session_info(cookies), Some("abc123".to_string()));
    }

    #[test]
    fn test_prepare_login() {
        let url = prepare_login();
        assert!(url.is_ok());
        assert_eq!(url.unwrap(), PERPLEXITY_LOGIN_URL);
    }
}

//! Perplexity account management commands

use crate::modules::perplexity_auth;
use tauri::AppHandle;

/// Start Perplexity web login flow
#[tauri::command]
pub async fn perplexity_start_login(app_handle: AppHandle) -> Result<String, String> {
    let login_url = perplexity_auth::prepare_login()?;

    // Open the login URL in the default browser
    use tauri_plugin_opener::OpenerExt;
    app_handle
        .opener()
        .open_url(&login_url, None::<String>)
        .map_err(|e| format!("Failed to open browser: {}", e))?;

    Ok(login_url)
}

/// Submit cookies captured from the webview
#[tauri::command]
pub async fn perplexity_submit_cookies(name: String, cookies: String) -> Result<(), String> {
    perplexity_auth::submit_cookies(name, cookies).await
}

/// Wait for login completion
#[tauri::command]
pub async fn perplexity_complete_login() -> Result<perplexity_auth::PerplexityWebAccount, String> {
    perplexity_auth::wait_for_login().await
}

/// Cancel the login flow
#[tauri::command]
pub fn perplexity_cancel_login() {
    perplexity_auth::cancel_login();
}

/// Validate existing cookies
#[tauri::command]
pub async fn perplexity_validate_cookies(cookies: String) -> Result<bool, String> {
    perplexity_auth::validate_cookies(&cookies).await
}

/// Get Perplexity login URL for manual login
#[tauri::command]
pub fn perplexity_get_login_url() -> Result<String, String> {
    Ok("https://www.perplexity.ai/".to_string())
}

#[tauri::command]
pub async fn perplexity_list_accounts() -> Result<Vec<perplexity_auth::PerplexityWebAccount>, String>
{
    perplexity_auth::list_accounts()
}

#[tauri::command]
pub async fn perplexity_delete_account(id: String) -> Result<(), String> {
    perplexity_auth::delete_account(id)
}

#[tauri::command]
pub async fn perplexity_get_config() -> Result<crate::models::config::PerplexityConfig, String> {
    let config = crate::modules::config::load_app_config()?;
    Ok(config.perplexity)
}

#[tauri::command]
pub async fn perplexity_save_config(
    config: crate::models::config::PerplexityConfig,
) -> Result<(), String> {
    let mut app_config = crate::modules::config::load_app_config()?;
    app_config.perplexity = config;
    crate::modules::config::save_app_config(&app_config)
}

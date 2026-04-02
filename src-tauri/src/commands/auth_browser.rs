//! 内置浏览器认证命令
//!
//! 提供前端调用的 Tauri 命令，用于管理内置浏览器认证。

use crate::auth_browser::{AuthBrowserConfig, AuthBrowserManager};
use crate::credentials::{CredentialInfo, CredentialsManager, DecryptedCredential};
use tauri::State;

/// 全局浏览器管理器状态
pub struct AuthBrowserState {
    pub manager: AuthBrowserManager,
}

impl Default for AuthBrowserState {
    fn default() -> Self {
        Self {
            manager: AuthBrowserManager::new(),
        }
    }
}

/// 全局凭据管理器状态
pub struct CredentialsState {
    pub manager: Option<CredentialsManager>,
}

impl CredentialsState {
    pub fn new(app_data_dir: std::path::PathBuf) -> Self {
        match CredentialsManager::new(app_data_dir) {
            Ok(manager) => Self {
                manager: Some(manager),
            },
            Err(e) => {
                log::error!("Failed to initialize CredentialsManager: {}", e);
                Self { manager: None }
            }
        }
    }

    fn get_manager(&self) -> Result<&CredentialsManager, String> {
        self.manager
            .as_ref()
            .ok_or_else(|| "Credentials manager not initialized".to_string())
    }
}

// ============== 浏览器管理命令 ==============

/// 打开内置认证浏览器
#[tauri::command]
pub async fn open_auth_browser(
    app: tauri::AppHandle,
    config: AuthBrowserConfig,
    state: State<'_, AuthBrowserState>,
) -> Result<String, String> {
    state.manager.open_browser(&app, config).await
}

/// 打开 GitHub Copilot 认证浏览器
#[tauri::command]
pub async fn open_copilot_auth_browser(
    app: tauri::AppHandle,
    verification_url: String,
    prefill_username: Option<String>,
    prefill_password: Option<String>,
    state: State<'_, AuthBrowserState>,
) -> Result<String, String> {
    let config = AuthBrowserConfig::github_copilot(&verification_url)
        .with_credentials(prefill_username, prefill_password);

    state.manager.open_browser(&app, config).await
}

/// 关闭认证浏览器
#[tauri::command]
pub async fn close_auth_browser(
    app: tauri::AppHandle,
    label: String,
    state: State<'_, AuthBrowserState>,
) -> Result<(), String> {
    state.manager.close_browser(&app, &label).await
}

/// 关闭指定 provider 的所有浏览器
#[tauri::command]
pub async fn close_auth_browser_for_provider(
    app: tauri::AppHandle,
    provider: String,
    state: State<'_, AuthBrowserState>,
) -> Result<(), String> {
    state.manager.close_browser_for_provider(&app, &provider).await
}

/// 获取活动的浏览器窗口列表
#[tauri::command]
pub async fn get_active_auth_browsers(
    state: State<'_, AuthBrowserState>,
) -> Result<Vec<String>, String> {
    Ok(state.manager.get_active_windows().await)
}

/// 注入脚本到浏览器
#[tauri::command]
pub async fn inject_auth_browser_script(
    app: tauri::AppHandle,
    label: String,
    script: String,
    state: State<'_, AuthBrowserState>,
) -> Result<(), String> {
    state.manager.inject_script(&app, &label, &script).await
}

// ============== 凭据管理命令 ==============

/// 保存凭据
#[tauri::command]
pub async fn save_auth_credential(
    provider: String,
    username: String,
    password: String,
    display_name: Option<String>,
    state: State<'_, CredentialsState>,
) -> Result<(), String> {
    let manager = state.get_manager()?;
    manager.save_credential(&provider, &username, &password, display_name)
}

/// 获取凭据列表（不包含密码）
#[tauri::command]
pub async fn get_auth_credentials(
    provider: String,
    state: State<'_, CredentialsState>,
) -> Result<Vec<CredentialInfo>, String> {
    let manager = state.get_manager()?;
    manager.get_credentials(&provider)
}

/// 获取单个凭据（包含密码，用于自动填充）
#[tauri::command]
pub async fn get_auth_credential(
    provider: String,
    username: String,
    state: State<'_, CredentialsState>,
) -> Result<Option<DecryptedCredential>, String> {
    let manager = state.get_manager()?;
    manager.get_credential(&provider, &username)
}

/// 更新凭据最后使用时间
#[tauri::command]
pub async fn update_credential_last_used(
    provider: String,
    username: String,
    state: State<'_, CredentialsState>,
) -> Result<(), String> {
    let manager = state.get_manager()?;
    manager.update_last_used(&provider, &username)
}

/// 删除凭据
#[tauri::command]
pub async fn delete_auth_credential(
    provider: String,
    username: String,
    state: State<'_, CredentialsState>,
) -> Result<(), String> {
    let manager = state.get_manager()?;
    manager.delete_credential(&provider, &username)
}

/// 删除 provider 的所有凭据
#[tauri::command]
pub async fn delete_all_auth_credentials(
    provider: String,
    state: State<'_, CredentialsState>,
) -> Result<(), String> {
    let manager = state.get_manager()?;
    manager.delete_all_credentials(&provider)
}

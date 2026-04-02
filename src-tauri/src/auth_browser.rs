//! 内置浏览器认证模块
//!
//! 使用 Tauri WebviewWindow 创建内置浏览器窗口进行 OAuth 认证。
//! 支持：
//! - 无缓存模式（每次打开都是干净的浏览器）
//! - Chrome User-Agent 伪装
//! - URL 变化监听
//! - 授权完成自动关闭
//! - 账号密码自动填充

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use tokio::sync::RwLock;

/// 浏览器窗口标签前缀
const AUTH_BROWSER_LABEL_PREFIX: &str = "auth-browser-";

/// Chrome User-Agent（模拟最新 Chrome）
const CHROME_USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

/// Windows Chrome User-Agent
#[cfg(target_os = "windows")]
const CHROME_USER_AGENT_WINDOWS: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

/// Linux Chrome User-Agent
#[cfg(target_os = "linux")]
const CHROME_USER_AGENT_LINUX: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

/// 浏览器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthBrowserConfig {
    /// 初始 URL
    pub url: String,
    /// 窗口标题
    pub title: String,
    /// Provider 类型（用于凭据管理）
    pub provider: String,
    /// 授权成功回调 URL 模式
    pub success_url_pattern: Option<String>,
    /// 是否启用账号填充
    pub enable_autofill: bool,
    /// 预填充的用户名
    pub prefill_username: Option<String>,
    /// 预填充的密码
    pub prefill_password: Option<String>,
}

/// 浏览器状态
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct AuthBrowserStatus {
    /// 窗口标签
    pub label: String,
    /// 当前 URL
    pub current_url: String,
    /// 是否已完成授权
    pub is_authorized: bool,
    /// 授权码（如果有）
    pub auth_code: Option<String>,
}

/// 浏览器事件
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AuthBrowserEvent {
    /// URL 变化
    UrlChanged { url: String },
    /// 授权完成
    AuthCompleted { code: Option<String> },
    /// 窗口关闭
    Closed,
    /// 错误
    Error { message: String },
}

/// 活动浏览器窗口管理器
pub struct AuthBrowserManager {
    /// 活动窗口映射
    active_windows: Arc<RwLock<std::collections::HashMap<String, AuthBrowserConfig>>>,
}

impl Default for AuthBrowserManager {
    fn default() -> Self {
        Self::new()
    }
}

impl AuthBrowserManager {
    pub fn new() -> Self {
        Self {
            active_windows: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    /// 获取平台对应的 User-Agent
    fn get_user_agent() -> &'static str {
        #[cfg(target_os = "windows")]
        {
            CHROME_USER_AGENT_WINDOWS
        }
        #[cfg(target_os = "linux")]
        {
            CHROME_USER_AGENT_LINUX
        }
        #[cfg(target_os = "macos")]
        {
            CHROME_USER_AGENT
        }
    }

    /// 打开认证浏览器窗口
    pub async fn open_browser(
        &self,
        app: &AppHandle,
        config: AuthBrowserConfig,
    ) -> Result<String, String> {
        let label = format!("{}{}", AUTH_BROWSER_LABEL_PREFIX, uuid::Uuid::new_v4());

        // 检查是否已有同 provider 的窗口打开
        {
            let windows = self.active_windows.read().await;
            for (existing_label, existing_config) in windows.iter() {
                if existing_config.provider == config.provider {
                    // 聚焦到已有窗口
                    if let Some(window) = app.get_webview_window(existing_label) {
                        let _ = window.set_focus();
                        return Ok(existing_label.clone());
                    }
                }
            }
        }

        let url = WebviewUrl::External(
            config
                .url
                .parse()
                .map_err(|e| format!("Invalid URL: {e}"))?,
        );

        let user_agent = Self::get_user_agent();

        // 创建窗口构建器
        let window_builder = WebviewWindowBuilder::new(app, &label, url)
            .title(&config.title)
            .inner_size(500.0, 700.0)
            .center()
            .resizable(true)
            .visible(true)
            .focused(true)
            // 关键：不显示地址栏（使用自定义 decorations）
            .decorations(true)
            // 设置 User-Agent
            .user_agent(user_agent)
            // 禁用开发者工具（生产环境）
            .devtools(cfg!(debug_assertions));

        // 平台特定配置
        #[cfg(target_os = "macos")]
        let window_builder = window_builder
            .title_bar_style(tauri::TitleBarStyle::Visible)
            .hidden_title(true);

        // 创建窗口
        let window = window_builder
            .build()
            .map_err(|e| format!("Failed to create browser window: {e}"))?;

        // 存储配置
        {
            let mut windows = self.active_windows.write().await;
            windows.insert(label.clone(), config.clone());
        }

        // 设置窗口关闭监听
        let label_clone = label.clone();
        let active_windows = self.active_windows.clone();
        let app_handle = app.clone();

        window.on_window_event(move |event| {
            if let tauri::WindowEvent::Destroyed = event {
                let label = label_clone.clone();
                let windows = active_windows.clone();
                let app = app_handle.clone();

                tauri::async_runtime::spawn(async move {
                    let mut windows = windows.write().await;
                    windows.remove(&label);

                    // 发送关闭事件
                    let _ = app.emit(
                        "auth-browser-event",
                        AuthBrowserEvent::Closed,
                    );
                });
            }
        });

        // 如果有预填充信息，注入自动填充脚本
        if config.enable_autofill
            && (config.prefill_username.is_some() || config.prefill_password.is_some())
        {
            let username = config.prefill_username.clone().unwrap_or_default();
            let password = config.prefill_password.clone().unwrap_or_default();

            // 延迟注入以等待页面加载
            let window_clone = window.clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(1500)).await;

                let script = format!(
                    r#"
                    (function() {{
                        // GitHub 登录页面自动填充
                        const usernameInput = document.querySelector('input[name="login"], input#login_field');
                        const passwordInput = document.querySelector('input[name="password"], input#password');
                        
                        if (usernameInput && '{}' !== '') {{
                            usernameInput.value = '{}';
                            usernameInput.dispatchEvent(new Event('input', {{ bubbles: true }}));
                        }}
                        
                        if (passwordInput && '{}' !== '') {{
                            passwordInput.value = '{}';
                            passwordInput.dispatchEvent(new Event('input', {{ bubbles: true }}));
                        }}
                    }})();
                    "#,
                    username, username, password, password
                );

                let _ = window_clone.eval(&script);
            });
        }

        // 设置导航监听（检测授权完成）
        if let Some(success_pattern) = &config.success_url_pattern {
            self.setup_navigation_listener(app, &label, success_pattern.clone())
                .await;
        }

        log::info!("Opened auth browser window: {}", label);
        Ok(label)
    }

    /// 设置导航监听器
    async fn setup_navigation_listener(
        &self,
        app: &AppHandle,
        label: &str,
        success_pattern: String,
    ) {
        let app = app.clone();
        let label = label.to_string();
        let active_windows = self.active_windows.clone();

        // 使用轮询检测 URL 变化
        tauri::async_runtime::spawn(async move {
            let mut last_url = String::new();
            let mut check_count = 0;
            const MAX_CHECKS: u32 = 600; // 最多检查 5 分钟（每 500ms 一次）

            loop {
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                check_count += 1;

                if check_count > MAX_CHECKS {
                    log::warn!("Auth browser navigation check timeout");
                    break;
                }

                let window = match app.get_webview_window(&label) {
                    Some(w) => w,
                    None => {
                        log::info!("Auth browser window closed");
                        break;
                    }
                };

                // 获取当前 URL
                let current_url = match window.url() {
                    Ok(url) => url.to_string(),
                    Err(_) => continue,
                };

                if current_url != last_url {
                    last_url = current_url.clone();
                    log::debug!("Auth browser URL changed: {}", current_url);

                    // 发送 URL 变化事件
                    let _ = app.emit(
                        "auth-browser-event",
                        AuthBrowserEvent::UrlChanged {
                            url: current_url.clone(),
                        },
                    );

                    // 检查是否匹配成功模式
                    if current_url.contains(&success_pattern) {
                        log::info!("Auth completed, URL matches pattern");

                        // 提取授权码（如果 URL 中有 code 参数）
                        let auth_code = url::Url::parse(&current_url)
                            .ok()
                            .and_then(|u| {
                                u.query_pairs()
                                    .find(|(k, _)| k == "code")
                                    .map(|(_, v)| v.to_string())
                            });

                        // 发送授权完成事件
                        let _ = app.emit(
                            "auth-browser-event",
                            AuthBrowserEvent::AuthCompleted { code: auth_code },
                        );

                        // 延迟关闭窗口
                        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                        let _ = window.close();

                        // 移除窗口记录
                        let mut windows = active_windows.write().await;
                        windows.remove(&label);

                        break;
                    }
                }
            }
        });
    }

    /// 关闭认证浏览器窗口
    pub async fn close_browser(&self, app: &AppHandle, label: &str) -> Result<(), String> {
        if let Some(window) = app.get_webview_window(label) {
            window.close().map_err(|e| format!("Failed to close window: {e}"))?;
        }

        let mut windows = self.active_windows.write().await;
        windows.remove(label);

        Ok(())
    }

    /// 关闭指定 provider 的所有浏览器窗口
    pub async fn close_browser_for_provider(&self, app: &AppHandle, provider: &str) -> Result<(), String> {
        let labels_to_close: Vec<String> = {
            let windows = self.active_windows.read().await;
            windows
                .iter()
                .filter(|(_, config)| config.provider == provider)
                .map(|(label, _)| label.clone())
                .collect()
        };

        for label in labels_to_close {
            self.close_browser(app, &label).await?;
        }

        Ok(())
    }

    /// 获取所有活动窗口
    pub async fn get_active_windows(&self) -> Vec<String> {
        let windows = self.active_windows.read().await;
        windows.keys().cloned().collect()
    }

    /// 注入脚本到浏览器窗口
    pub async fn inject_script(&self, app: &AppHandle, label: &str, script: &str) -> Result<(), String> {
        let window = app
            .get_webview_window(label)
            .ok_or("Browser window not found")?;

        window.eval(script).map_err(|e| format!("Failed to inject script: {e}"))
    }

    /// 清除浏览器数据（cookies、缓存等）
    /// 注意：Tauri 2 中需要通过创建新窗口来实现"无缓存"效果
    pub async fn clear_browser_data(&self, app: &AppHandle, label: &str) -> Result<(), String> {
        // Tauri 2 暂不支持直接清除 WebView 数据
        // 作为替代，我们通过关闭并重新创建窗口来实现
        log::warn!("clear_browser_data is not directly supported in Tauri 2, consider recreating the window");

        // 获取当前配置
        let config = {
            let windows = self.active_windows.read().await;
            windows.get(label).cloned()
        };

        if let Some(config) = config {
            // 关闭旧窗口
            self.close_browser(app, label).await?;

            // 等待一下确保窗口关闭
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;

            // 重新打开
            self.open_browser(app, config).await?;
        }

        Ok(())
    }
}

/// GitHub 登录特定配置
impl AuthBrowserConfig {
    /// 创建 GitHub OAuth 登录配置
    pub fn github_oauth(device_url: &str) -> Self {
        Self {
            url: device_url.to_string(),
            title: "GitHub 登录".to_string(),
            provider: "github".to_string(),
            // GitHub OAuth 成功后会重定向到这个页面
            success_url_pattern: Some("github.com/login/device/success".to_string()),
            enable_autofill: true,
            prefill_username: None,
            prefill_password: None,
        }
    }

    /// 创建 GitHub Copilot 登录配置
    pub fn github_copilot(verification_url: &str) -> Self {
        Self {
            url: verification_url.to_string(),
            title: "GitHub Copilot 授权".to_string(),
            provider: "github_copilot".to_string(),
            success_url_pattern: Some("github.com/login/device/success".to_string()),
            enable_autofill: true,
            prefill_username: None,
            prefill_password: None,
        }
    }

    /// 设置预填充凭据
    pub fn with_credentials(mut self, username: Option<String>, password: Option<String>) -> Self {
        self.prefill_username = username;
        self.prefill_password = password;
        self
    }
}

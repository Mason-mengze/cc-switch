//! 账号密码凭据管理模块
//!
//! 使用 AES-256-GCM 加密存储用户的账号密码，支持多账号管理。
//! 数据存储在应用数据目录的 credentials.json 文件中。

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// 凭据存储版本
const CREDENTIALS_VERSION: u32 = 1;

/// 加密盐（用于派生密钥）
const ENCRYPTION_SALT: &[u8] = b"cc-switch-credentials-v1";

/// 单个凭据条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialEntry {
    /// 用户名
    pub username: String,
    /// 加密后的密码（base64 编码）
    pub encrypted_password: String,
    /// 加密随机数（base64 编码）
    pub nonce: String,
    /// 显示名称（可选，用于 UI 展示）
    pub display_name: Option<String>,
    /// 创建时间
    pub created_at: String,
    /// 最后使用时间
    pub last_used_at: Option<String>,
}

/// Provider 的凭据列表
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProviderCredentials {
    /// 凭据列表
    pub entries: Vec<CredentialEntry>,
}

/// 凭据存储文件结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialsStore {
    /// 版本号
    pub version: u32,
    /// 按 provider 分组的凭据
    pub providers: HashMap<String, ProviderCredentials>,
}

impl Default for CredentialsStore {
    fn default() -> Self {
        Self {
            version: CREDENTIALS_VERSION,
            providers: HashMap::new(),
        }
    }
}

/// 凭据管理器
pub struct CredentialsManager {
    /// 存储文件路径
    store_path: PathBuf,
    /// 加密密钥（派生自机器标识）
    encryption_key: [u8; 32],
}

impl CredentialsManager {
    /// 创建新的凭据管理器
    pub fn new(app_data_dir: PathBuf) -> Result<Self, String> {
        let store_path = app_data_dir.join("credentials.json");
        let encryption_key = Self::derive_encryption_key()?;

        Ok(Self {
            store_path,
            encryption_key,
        })
    }

    /// 派生加密密钥
    /// 使用机器标识 + 盐生成唯一密钥
    fn derive_encryption_key() -> Result<[u8; 32], String> {
        let machine_id = Self::get_machine_id()?;
        let mut hasher = Sha256::new();
        hasher.update(machine_id.as_bytes());
        hasher.update(ENCRYPTION_SALT);
        let result = hasher.finalize();
        let mut key = [0u8; 32];
        key.copy_from_slice(&result);
        Ok(key)
    }

    /// 获取机器标识
    #[cfg(target_os = "macos")]
    fn get_machine_id() -> Result<String, String> {
        use std::process::Command;
        let output = Command::new("ioreg")
            .args(["-rd1", "-c", "IOPlatformExpertDevice"])
            .output()
            .map_err(|e| format!("Failed to get machine ID: {e}"))?;

        let output_str = String::from_utf8_lossy(&output.stdout);
        for line in output_str.lines() {
            if line.contains("IOPlatformUUID") {
                if let Some(uuid) = line.split('"').nth(3) {
                    return Ok(uuid.to_string());
                }
            }
        }
        // Fallback: use username + hostname
        Ok(format!(
            "{}-{}",
            whoami::username(),
            whoami::fallible::hostname().unwrap_or_else(|_| "unknown".to_string())
        ))
    }

    #[cfg(target_os = "windows")]
    fn get_machine_id() -> Result<String, String> {
        use winreg::enums::*;
        use winreg::RegKey;

        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        let key = hklm
            .open_subkey("SOFTWARE\\Microsoft\\Cryptography")
            .map_err(|e| format!("Failed to open registry: {e}"))?;

        let machine_guid: String = key
            .get_value("MachineGuid")
            .map_err(|e| format!("Failed to get MachineGuid: {e}"))?;

        Ok(machine_guid)
    }

    #[cfg(target_os = "linux")]
    fn get_machine_id() -> Result<String, String> {
        // Try /etc/machine-id first
        if let Ok(id) = fs::read_to_string("/etc/machine-id") {
            return Ok(id.trim().to_string());
        }
        // Fallback to /var/lib/dbus/machine-id
        if let Ok(id) = fs::read_to_string("/var/lib/dbus/machine-id") {
            return Ok(id.trim().to_string());
        }
        // Final fallback: use username + hostname
        Ok(format!(
            "{}-{}",
            whoami::username(),
            whoami::fallible::hostname().unwrap_or_else(|_| "unknown".to_string())
        ))
    }

    /// 加密密码
    fn encrypt_password(&self, password: &str) -> Result<(String, String), String> {
        let cipher =
            Aes256Gcm::new_from_slice(&self.encryption_key).map_err(|e| format!("Cipher error: {e}"))?;

        // 生成随机 nonce
        let mut nonce_bytes = [0u8; 12];
        rand::rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        // 加密
        let ciphertext = cipher
            .encrypt(nonce, password.as_bytes())
            .map_err(|e| format!("Encryption error: {e}"))?;

        Ok((BASE64.encode(ciphertext), BASE64.encode(nonce_bytes)))
    }

    /// 解密密码
    fn decrypt_password(&self, encrypted: &str, nonce_str: &str) -> Result<String, String> {
        let cipher =
            Aes256Gcm::new_from_slice(&self.encryption_key).map_err(|e| format!("Cipher error: {e}"))?;

        let ciphertext = BASE64
            .decode(encrypted)
            .map_err(|e| format!("Base64 decode error: {e}"))?;
        let nonce_bytes = BASE64
            .decode(nonce_str)
            .map_err(|e| format!("Nonce decode error: {e}"))?;

        let nonce = Nonce::from_slice(&nonce_bytes);

        let plaintext = cipher
            .decrypt(nonce, ciphertext.as_ref())
            .map_err(|e| format!("Decryption error: {e}"))?;

        String::from_utf8(plaintext).map_err(|e| format!("UTF-8 decode error: {e}"))
    }

    /// 加载凭据存储
    fn load_store(&self) -> Result<CredentialsStore, String> {
        if !self.store_path.exists() {
            return Ok(CredentialsStore::default());
        }

        let content =
            fs::read_to_string(&self.store_path).map_err(|e| format!("Failed to read credentials: {e}"))?;

        serde_json::from_str(&content).map_err(|e| format!("Failed to parse credentials: {e}"))
    }

    /// 保存凭据存储
    fn save_store(&self, store: &CredentialsStore) -> Result<(), String> {
        let content =
            serde_json::to_string_pretty(store).map_err(|e| format!("Failed to serialize credentials: {e}"))?;

        // 确保父目录存在
        if let Some(parent) = self.store_path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("Failed to create directory: {e}"))?;
        }

        fs::write(&self.store_path, content).map_err(|e| format!("Failed to write credentials: {e}"))
    }

    /// 保存凭据
    pub fn save_credential(
        &self,
        provider: &str,
        username: &str,
        password: &str,
        display_name: Option<String>,
    ) -> Result<(), String> {
        let mut store = self.load_store()?;

        let (encrypted_password, nonce) = self.encrypt_password(password)?;

        let entry = CredentialEntry {
            username: username.to_string(),
            encrypted_password,
            nonce,
            display_name,
            created_at: chrono::Utc::now().to_rfc3339(),
            last_used_at: None,
        };

        let provider_creds = store.providers.entry(provider.to_string()).or_default();

        // 检查是否已存在相同用户名的凭据，如果存在则更新
        if let Some(existing) = provider_creds.entries.iter_mut().find(|e| e.username == username) {
            existing.encrypted_password = entry.encrypted_password;
            existing.nonce = entry.nonce;
            existing.display_name = entry.display_name;
        } else {
            provider_creds.entries.push(entry);
        }

        self.save_store(&store)
    }

    /// 获取凭据列表（不包含密码）
    pub fn get_credentials(&self, provider: &str) -> Result<Vec<CredentialInfo>, String> {
        let store = self.load_store()?;

        let empty_vec = Vec::new();
        let entries = store
            .providers
            .get(provider)
            .map(|p| &p.entries)
            .unwrap_or(&empty_vec);

        Ok(entries
            .iter()
            .map(|e| CredentialInfo {
                username: e.username.clone(),
                display_name: e.display_name.clone(),
                created_at: e.created_at.clone(),
                last_used_at: e.last_used_at.clone(),
            })
            .collect())
    }

    /// 获取凭据（包含解密后的密码）
    pub fn get_credential(&self, provider: &str, username: &str) -> Result<Option<DecryptedCredential>, String> {
        let store = self.load_store()?;

        let entry = store
            .providers
            .get(provider)
            .and_then(|p| p.entries.iter().find(|e| e.username == username));

        match entry {
            Some(e) => {
                let password = self.decrypt_password(&e.encrypted_password, &e.nonce)?;
                Ok(Some(DecryptedCredential {
                    username: e.username.clone(),
                    password,
                    display_name: e.display_name.clone(),
                }))
            }
            None => Ok(None),
        }
    }

    /// 更新最后使用时间
    pub fn update_last_used(&self, provider: &str, username: &str) -> Result<(), String> {
        let mut store = self.load_store()?;

        if let Some(provider_creds) = store.providers.get_mut(provider) {
            if let Some(entry) = provider_creds.entries.iter_mut().find(|e| e.username == username) {
                entry.last_used_at = Some(chrono::Utc::now().to_rfc3339());
                return self.save_store(&store);
            }
        }

        Ok(())
    }

    /// 删除凭据
    pub fn delete_credential(&self, provider: &str, username: &str) -> Result<(), String> {
        let mut store = self.load_store()?;

        if let Some(provider_creds) = store.providers.get_mut(provider) {
            provider_creds.entries.retain(|e| e.username != username);
        }

        self.save_store(&store)
    }

    /// 删除 provider 的所有凭据
    pub fn delete_all_credentials(&self, provider: &str) -> Result<(), String> {
        let mut store = self.load_store()?;
        store.providers.remove(provider);
        self.save_store(&store)
    }
}

/// 凭据信息（不包含密码，用于列表展示）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialInfo {
    pub username: String,
    pub display_name: Option<String>,
    pub created_at: String,
    pub last_used_at: Option<String>,
}

/// 解密后的凭据（包含密码，用于自动填充）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecryptedCredential {
    pub username: String,
    pub password: String,
    pub display_name: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_save_and_get_credential() {
        let dir = tempdir().unwrap();
        let manager = CredentialsManager::new(dir.path().to_path_buf()).unwrap();

        manager
            .save_credential("github", "testuser", "testpassword", Some("Test User".to_string()))
            .unwrap();

        let creds = manager.get_credentials("github").unwrap();
        assert_eq!(creds.len(), 1);
        assert_eq!(creds[0].username, "testuser");

        let decrypted = manager.get_credential("github", "testuser").unwrap().unwrap();
        assert_eq!(decrypted.password, "testpassword");
    }

    #[test]
    fn test_delete_credential() {
        let dir = tempdir().unwrap();
        let manager = CredentialsManager::new(dir.path().to_path_buf()).unwrap();

        manager
            .save_credential("github", "user1", "pass1", None)
            .unwrap();
        manager
            .save_credential("github", "user2", "pass2", None)
            .unwrap();

        manager.delete_credential("github", "user1").unwrap();

        let creds = manager.get_credentials("github").unwrap();
        assert_eq!(creds.len(), 1);
        assert_eq!(creds[0].username, "user2");
    }
}

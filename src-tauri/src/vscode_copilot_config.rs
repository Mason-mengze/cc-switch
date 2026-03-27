use crate::error::AppError;
use crate::settings::get_vscode_copilot_override_dir;
use serde_json::json;
use std::path::PathBuf;

const ENABLED_PROVIDERS_FILE: &str = "enabled-providers.json";

pub fn get_vscode_copilot_dir() -> PathBuf {
    if let Some(override_dir) = get_vscode_copilot_override_dir() {
        return override_dir;
    }

    crate::config::get_home_dir().join(".cc-switch").join("vscode-copilot")
}

pub fn get_enabled_providers_path() -> PathBuf {
    get_vscode_copilot_dir().join(ENABLED_PROVIDERS_FILE)
}

pub fn read_enabled_provider_ids() -> Result<Option<Vec<String>>, AppError> {
    let path = get_enabled_providers_path();
    if !path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&path).map_err(|e| AppError::io(&path, e))?;
    let parsed = serde_json::from_str::<Vec<String>>(&content)
        .or_else(|_| serde_json::from_str::<serde_json::Value>(&content).map(parse_enabled_ids))
        .map_err(|e| AppError::json(&path, e))?;

    Ok(Some(parsed))
}

fn parse_enabled_ids(value: serde_json::Value) -> Vec<String> {
    value
        .get("enabledProviderIds")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|item| item.as_str().map(|s| s.trim().to_string()))
        .filter(|s| !s.is_empty())
        .collect()
}

pub fn write_enabled_provider_ids(ids: &[String]) -> Result<(), AppError> {
    let path = get_enabled_providers_path();
    crate::config::write_json_file(&path, &json!(ids))
}

pub fn add_enabled_provider_id(id: &str) -> Result<(), AppError> {
    let mut ids = read_enabled_provider_ids()?.unwrap_or_default();
    if !ids.iter().any(|existing| existing == id) {
        ids.push(id.to_string());
    }
    write_enabled_provider_ids(&ids)
}

pub fn remove_enabled_provider_id(id: &str) -> Result<(), AppError> {
    let Some(existing) = read_enabled_provider_ids()? else {
        return Ok(());
    };

    let filtered: Vec<String> = existing.into_iter().filter(|item| item != id).collect();
    write_enabled_provider_ids(&filtered)
}

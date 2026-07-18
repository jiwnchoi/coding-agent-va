use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::shared::logger::{LogLevel, Logger};

const CONFIG_DIRECTORY_NAME: &str = "coding-agent-va";
const CONFIG_FILE_NAME: &str = "config.toml";
static CONFIG_FILE_LOCK: Mutex<()> = Mutex::new(());

#[derive(Clone, Copy, Default, Deserialize, Serialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(rename_all = "kebab-case")]
pub enum AppTheme {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Clone, Copy, Default, Deserialize, Serialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(rename_all = "kebab-case")]
pub enum AppFont {
    #[default]
    Geist,
    SystemSans,
    SystemSerif,
}

#[derive(Clone, Copy, Default, Deserialize, Serialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(rename_all = "kebab-case")]
pub enum MonacoTheme {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Clone, Default, Deserialize, Serialize, TS)]
#[serde(default, rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub struct RuntimeHomes {
    pub claude: String,
    pub codex: String,
    pub pi: String,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(rename_all = "lowercase")]
pub enum DescriptionReasoning {
    #[default]
    None,
    Minimal,
    Low,
    Medium,
    High,
    Xhigh,
    Max,
}

#[derive(Clone, Deserialize, Serialize, TS)]
#[serde(default, rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub struct DescriptionProviderSettings {
    pub model: String,
    pub reasoning: DescriptionReasoning,
}

impl Default for DescriptionProviderSettings {
    fn default() -> Self {
        Self {
            model: String::new(),
            reasoning: DescriptionReasoning::None,
        }
    }
}

#[derive(Clone, Deserialize, Serialize, TS)]
#[serde(default, rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub struct DescriptionSettings {
    pub codex: DescriptionProviderSettings,
    pub claude: DescriptionProviderSettings,
    pub pi: DescriptionProviderSettings,
}

impl Default for DescriptionSettings {
    fn default() -> Self {
        Self {
            codex: DescriptionProviderSettings {
                model: "gpt-5.6-luna".to_string(),
                reasoning: DescriptionReasoning::None,
            },
            claude: DescriptionProviderSettings {
                model: "claude-haiku-4-5".to_string(),
                reasoning: DescriptionReasoning::None,
            },
            pi: DescriptionProviderSettings::default(),
        }
    }
}

#[derive(Clone, Default, Deserialize, Serialize, TS)]
#[serde(default, rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub struct AppSettings {
    pub theme: AppTheme,
    pub font: AppFont,
    pub monaco_theme: MonacoTheme,
    pub show_read_files: bool,
    pub keyboard_shortcuts: BTreeMap<String, String>,
    pub runtime_homes: RuntimeHomes,
    pub descriptions: DescriptionSettings,
}

#[derive(Deserialize, Serialize)]
#[serde(default)]
struct StoredAppSettings {
    theme: AppTheme,
    font: AppFont,
    monaco_theme: MonacoTheme,
    show_read_files: bool,
    keyboard_shortcuts: BTreeMap<String, String>,
    runtime_homes: RuntimeHomes,
    descriptions: DescriptionSettings,
}

impl Default for StoredAppSettings {
    fn default() -> Self {
        AppSettings::default().into()
    }
}

impl From<StoredAppSettings> for AppSettings {
    fn from(settings: StoredAppSettings) -> Self {
        Self {
            theme: settings.theme,
            font: settings.font,
            monaco_theme: settings.monaco_theme,
            show_read_files: settings.show_read_files,
            keyboard_shortcuts: settings.keyboard_shortcuts,
            runtime_homes: settings.runtime_homes,
            descriptions: settings.descriptions,
        }
    }
}

impl From<AppSettings> for StoredAppSettings {
    fn from(settings: AppSettings) -> Self {
        Self {
            theme: settings.theme,
            font: settings.font,
            monaco_theme: settings.monaco_theme,
            show_read_files: settings.show_read_files,
            keyboard_shortcuts: settings.keyboard_shortcuts,
            runtime_homes: settings.runtime_homes,
            descriptions: settings.descriptions,
        }
    }
}

fn config_path() -> Result<PathBuf, String> {
    let home = std::env::var_os("HOME").ok_or_else(|| "HOME is not configured".to_string())?;
    Ok(PathBuf::from(home)
        .join(".config")
        .join(CONFIG_DIRECTORY_NAME)
        .join(CONFIG_FILE_NAME))
}

#[tauri::command]
pub async fn load_app_settings() -> Result<AppSettings, String> {
    tauri::async_runtime::spawn_blocking(load_app_settings_from_disk)
        .await
        .map_err(|error| format!("settings load task failed: {error}"))?
}

fn load_app_settings_from_disk() -> Result<AppSettings, String> {
    let _guard = CONFIG_FILE_LOCK
        .lock()
        .map_err(|_| "failed to lock application settings".to_string())?;
    let path = config_path()?;
    if !path.exists() {
        let _ = Logger::log(LogLevel::Info, "Using default application settings", None);
        return Ok(AppSettings::default());
    }

    let contents = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let settings = toml::from_str::<StoredAppSettings>(&contents)
        .map(AppSettings::from)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))?;
    let _ = Logger::log(LogLevel::Info, "Loaded application settings", None);
    Ok(settings)
}

#[tauri::command]
pub async fn save_app_settings(settings: AppSettings) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || save_app_settings_to_disk(settings))
        .await
        .map_err(|error| format!("settings save task failed: {error}"))?
}

fn save_app_settings_to_disk(settings: AppSettings) -> Result<(), String> {
    let _guard = CONFIG_FILE_LOCK
        .lock()
        .map_err(|_| "failed to lock application settings".to_string())?;
    let path = config_path()?;
    let directory = path
        .parent()
        .ok_or_else(|| "configuration path has no parent directory".to_string())?;
    fs::create_dir_all(directory)
        .map_err(|error| format!("failed to create {}: {error}", directory.display()))?;

    let contents = toml::to_string_pretty(&StoredAppSettings::from(settings))
        .map_err(|error| format!("failed to serialize settings: {error}"))?;
    let temporary_path = path.with_extension("toml.tmp");
    fs::write(&temporary_path, contents)
        .map_err(|error| format!("failed to write {}: {error}", temporary_path.display()))?;
    fs::rename(&temporary_path, &path)
        .map_err(|error| format!("failed to replace {}: {error}", path.display()))?;
    let _ = Logger::log(LogLevel::Debug, "Saved application settings", None);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{AppSettings, StoredAppSettings};

    #[test]
    fn config_round_trip_preserves_defaults() {
        let serialized = toml::to_string_pretty(&StoredAppSettings::default())
            .expect("serialize default settings");
        let parsed =
            toml::from_str::<StoredAppSettings>(&serialized).expect("parse serialized settings");
        let settings = AppSettings::from(parsed);

        assert!(!settings.show_read_files);
        assert!(serialized.contains("monaco_theme = \"system\""));
        assert!(serialized.contains("[runtime_homes]"));
        assert!(serialized.contains("model = \"gpt-5.6-luna\""));
    }

    #[test]
    fn partial_config_uses_defaults() {
        let parsed = toml::from_str::<StoredAppSettings>("theme = \"dark\"")
            .expect("parse partial settings");
        let settings = AppSettings::from(parsed);

        assert!(!settings.show_read_files);
        assert!(settings.runtime_homes.codex.is_empty());
        assert_eq!(settings.descriptions.claude.model, "claude-haiku-4-5");
    }
}

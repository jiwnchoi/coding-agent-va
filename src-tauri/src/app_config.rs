use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

const CONFIG_DIRECTORY_NAME: &str = "coding-agent-va";
const CONFIG_FILE_NAME: &str = "config.toml";

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

#[derive(Clone, Deserialize, Serialize, TS)]
#[serde(default, rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub struct AppSettings {
    pub theme: AppTheme,
    pub font: AppFont,
    pub monaco_theme: MonacoTheme,
    pub hide_committed_files: bool,
    pub keyboard_shortcuts: BTreeMap<String, String>,
    pub runtime_homes: RuntimeHomes,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: AppTheme::default(),
            font: AppFont::default(),
            monaco_theme: MonacoTheme::default(),
            hide_committed_files: true,
            keyboard_shortcuts: BTreeMap::new(),
            runtime_homes: RuntimeHomes::default(),
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(default)]
struct StoredAppSettings {
    theme: AppTheme,
    font: AppFont,
    monaco_theme: MonacoTheme,
    hide_committed_files: bool,
    keyboard_shortcuts: BTreeMap<String, String>,
    runtime_homes: RuntimeHomes,
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
            hide_committed_files: settings.hide_committed_files,
            keyboard_shortcuts: settings.keyboard_shortcuts,
            runtime_homes: settings.runtime_homes,
        }
    }
}

impl From<AppSettings> for StoredAppSettings {
    fn from(settings: AppSettings) -> Self {
        Self {
            theme: settings.theme,
            font: settings.font,
            monaco_theme: settings.monaco_theme,
            hide_committed_files: settings.hide_committed_files,
            keyboard_shortcuts: settings.keyboard_shortcuts,
            runtime_homes: settings.runtime_homes,
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
pub fn load_app_settings() -> Result<AppSettings, String> {
    let path = config_path()?;
    if !path.exists() {
        return Ok(AppSettings::default());
    }

    let contents = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    toml::from_str::<StoredAppSettings>(&contents)
        .map(AppSettings::from)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))
}

#[tauri::command]
pub fn save_app_settings(settings: AppSettings) -> Result<(), String> {
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
        .map_err(|error| format!("failed to replace {}: {error}", path.display()))
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

        assert!(settings.hide_committed_files);
        assert!(serialized.contains("monaco_theme = \"system\""));
        assert!(serialized.contains("[runtime_homes]"));
    }

    #[test]
    fn partial_config_uses_defaults() {
        let parsed = toml::from_str::<StoredAppSettings>("theme = \"dark\"")
            .expect("parse partial settings");
        let settings = AppSettings::from(parsed);

        assert!(settings.hide_committed_files);
        assert!(settings.runtime_homes.codex.is_empty());
    }
}

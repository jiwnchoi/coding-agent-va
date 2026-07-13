use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use ts_rs::TS;

const LOG_DIRECTORY_NAME: &str = "coding-agent-va";
const LOG_FILE_NAME: &str = "app.log";
static LOG_FILE_LOCK: Mutex<()> = Mutex::new(());

#[derive(Clone, Copy, Debug, Deserialize, Serialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(rename = "LogLevel")]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub struct LogEntry {
    pub timestamp: String,
    pub level: LogLevel,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<BTreeMap<String, String>>,
}

pub struct Logger;

impl Logger {
    pub fn log(
        level: LogLevel,
        message: impl Into<String>,
        context: Option<BTreeMap<String, String>>,
    ) -> Result<(), String> {
        let _guard = LOG_FILE_LOCK
            .lock()
            .map_err(|_| "failed to lock application log".to_string())?;
        let entry = LogEntry {
            timestamp: Utc::now().to_rfc3339(),
            level,
            message: message.into(),
            context,
        };
        let path = log_path()?;
        fs::create_dir_all(path.parent().expect("log path has a parent"))
            .map_err(|error| format!("failed to create log directory: {error}"))?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|error| format!("failed to open {}: {error}", path.display()))?;
        serde_json::to_writer(&mut file, &entry)
            .map_err(|error| format!("failed to serialize log entry: {error}"))?;
        file.write_all(b"\n")
            .map_err(|error| format!("failed to write log entry: {error}"))
    }

    pub fn entries() -> Result<Vec<LogEntry>, String> {
        let _guard = LOG_FILE_LOCK
            .lock()
            .map_err(|_| "failed to lock application log".to_string())?;
        let path = log_path()?;
        if !path.exists() {
            return Ok(Vec::new());
        }
        let contents = fs::read_to_string(&path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        Ok(contents
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect())
    }

    pub fn clear() -> Result<(), String> {
        let _guard = LOG_FILE_LOCK
            .lock()
            .map_err(|_| "failed to lock application log".to_string())?;
        let path = log_path()?;
        if path.exists() {
            fs::write(&path, "")
                .map_err(|error| format!("failed to clear {}: {error}", path.display()))?;
        }
        Ok(())
    }
}

fn log_path() -> Result<PathBuf, String> {
    let home = std::env::var_os("HOME").ok_or_else(|| "HOME is not configured".to_string())?;
    Ok(PathBuf::from(home)
        .join(".config")
        .join(LOG_DIRECTORY_NAME)
        .join(LOG_FILE_NAME))
}

#[tauri::command]
pub async fn write_log(
    level: LogLevel,
    message: String,
    context: Option<BTreeMap<String, String>>,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || Logger::log(level, message, context))
        .await
        .map_err(|error| format!("log write task failed: {error}"))?
}

#[tauri::command]
pub async fn get_logs() -> Result<Vec<LogEntry>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let mut entries = Logger::entries()?;
        entries.reverse();
        entries.truncate(500);
        Ok(entries)
    })
    .await
    .map_err(|error| format!("log read task failed: {error}"))?
}

#[tauri::command]
pub async fn clear_logs() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(Logger::clear)
        .await
        .map_err(|error| format!("log clear task failed: {error}"))?
}

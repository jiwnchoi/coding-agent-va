use std::time::{SystemTime, UNIX_EPOCH};

use super::json::json_str;

pub(crate) fn payload_timestamp_ms(payload: &serde_json::Value) -> u64 {
    json_str(payload, &["timestamp"])
        .and_then(timestamp_string_to_ms)
        .unwrap_or_default()
}

pub(crate) fn entry_timestamp_ms(entry: &serde_json::Value) -> u64 {
    json_str(entry, &["timestamp"])
        .and_then(timestamp_string_to_ms)
        .unwrap_or_default()
}

pub(crate) fn now_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

pub(crate) fn system_time_to_ms(system_time: SystemTime) -> Option<u64> {
    system_time
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_millis() as u64)
}

fn timestamp_string_to_ms(timestamp: &str) -> Option<u64> {
    chrono::DateTime::parse_from_rfc3339(timestamp)
        .ok()
        .map(|value| value.timestamp_millis() as u64)
}

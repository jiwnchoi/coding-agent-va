use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::Engine;
use reqwest::blocking::{Client, Response};
use serde_json::{json, Map, Value};

const RESPONSES_URL: &str = "https://chatgpt.com/backend-api/codex/responses";
const TOKEN_URL: &str = "https://auth.openai.com/oauth/token";
const CODEX_CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
const CODEX_ORIGINATOR: &str = "codex_cli_rs";
const REFRESH_MARGIN: Duration = Duration::from_secs(5 * 60);
const REFRESH_INTERVAL: Duration = Duration::from_secs(55 * 60);

static AUTH_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

struct Auth {
    access_token: String,
    account_id: String,
}

pub(super) fn run(
    runtime_home: &Path,
    transcript_path: &Path,
    model: &str,
    reasoning_effort: &str,
    instructions: &str,
    prompt: &str,
    on_chunk: impl Fn(&str) -> Result<(), String>,
) -> Result<String, String> {
    if model.is_empty() {
        return Err("Codex Responses API requires a model".to_string());
    }

    let client = Client::builder()
        .build()
        .map_err(|error| format!("failed to create Codex HTTP client: {error}"))?;
    let auth = load_auth(&client, &runtime_home.join("auth.json"))?;
    let client_version = load_client_version(runtime_home)?;
    let input = load_session_input(transcript_path, prompt)?;
    let body = json!({
        "model": model,
        "instructions": instructions,
        "input": input,
        "tools": [],
        "tool_choice": "none",
        "parallel_tool_calls": false,
        "reasoning": {
            "effort": reasoning_effort,
            "summary": "auto"
        },
        "include": ["reasoning.encrypted_content"],
        "store": false,
        "stream": true
    });

    let response = client
        .post(RESPONSES_URL)
        .bearer_auth(&auth.access_token)
        .header("chatgpt-account-id", &auth.account_id)
        .header("originator", CODEX_ORIGINATOR)
        .header(
            reqwest::header::USER_AGENT,
            format!("{CODEX_ORIGINATOR}/{client_version}"),
        )
        .header("OpenAI-Beta", "responses=experimental")
        .json(&body)
        .send()
        .map_err(|error| format!("Codex Responses API request failed: {error}"))?;
    stream_response(response, on_chunk)
}

fn load_client_version(runtime_home: &Path) -> Result<String, String> {
    let candidates = [
        ("models_cache.json", "client_version"),
        ("version.json", "latest_version"),
    ];
    for (file_name, field) in candidates {
        let path = runtime_home.join(file_name);
        let Ok(contents) = fs::read_to_string(&path) else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<Value>(&contents) else {
            continue;
        };
        if let Some(version) = value
            .get(field)
            .and_then(Value::as_str)
            .filter(|version| !version.is_empty())
        {
            return Ok(version.to_string());
        }
    }
    Err(format!(
        "Codex client version is unavailable in {}; run Codex once to refresh its model cache",
        runtime_home.display()
    ))
}

fn load_session_input(transcript_path: &Path, prompt: &str) -> Result<Vec<Value>, String> {
    let file = fs::File::open(transcript_path).map_err(|error| {
        format!(
            "failed to open Codex session log {}: {error}",
            transcript_path.display()
        )
    })?;
    let mut history = Vec::new();

    for line in BufReader::new(file).lines() {
        let line = line.map_err(|error| format!("failed to read Codex session log: {error}"))?;
        let Ok(entry) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        match entry.get("type").and_then(Value::as_str) {
            Some("response_item") => {
                if let Some(mut item) = entry.get("payload").cloned() {
                    strip_item_ids(&mut item);
                    history.push(item);
                }
            }
            Some("compacted") => {
                if let Some(replacement) = entry
                    .pointer("/payload/replacement_history")
                    .and_then(Value::as_array)
                {
                    history = replacement.clone();
                    for item in &mut history {
                        strip_item_ids(item);
                    }
                }
            }
            Some("event_msg")
                if entry.pointer("/payload/type").and_then(Value::as_str)
                    == Some("thread_rolled_back") =>
            {
                let turns = entry
                    .pointer("/payload/num_turns")
                    .and_then(Value::as_u64)
                    .unwrap_or(0) as usize;
                drop_last_user_turns(&mut history, turns);
            }
            _ => {}
        }
    }

    history.push(json!({
        "type": "message",
        "role": "user",
        "content": [{ "type": "input_text", "text": prompt }]
    }));
    Ok(history)
}

fn strip_item_ids(item: &mut Value) {
    let Some(object) = item.as_object_mut() else {
        return;
    };
    object.remove("id");
    object.remove("turn_id");
}

fn drop_last_user_turns(history: &mut Vec<Value>, count: usize) {
    for _ in 0..count {
        let Some(index) = history.iter().rposition(is_user_message) else {
            history.clear();
            return;
        };
        history.truncate(index);
    }
}

fn is_user_message(item: &Value) -> bool {
    item.get("type").and_then(Value::as_str) == Some("message")
        && item.get("role").and_then(Value::as_str) == Some("user")
}

fn load_auth(client: &Client, auth_path: &Path) -> Result<Auth, String> {
    let lock = AUTH_LOCK.get_or_init(|| Mutex::new(()));
    let _guard = lock
        .lock()
        .map_err(|_| "failed to lock Codex authentication".to_string())?;
    let text = fs::read_to_string(auth_path).map_err(|error| {
        format!(
            "failed to read Codex auth {}; run `codex login`: {error}",
            auth_path.display()
        )
    })?;
    let mut auth_file: Value = serde_json::from_str(&text)
        .map_err(|error| format!("Codex auth file is invalid JSON: {error}"))?;

    if should_refresh(&auth_file) {
        refresh_auth(client, auth_path, &mut auth_file)?;
    }

    let access_token = token_string(&auth_file, "access_token")
        .ok_or_else(|| "Codex access token is unavailable; run `codex login`".to_string())?;
    let account_id = token_string(&auth_file, "account_id")
        .or_else(|| token_string(&auth_file, "id_token").and_then(|token| account_id(&token)))
        .ok_or_else(|| "Codex account id is unavailable; run `codex login`".to_string())?;
    Ok(Auth {
        access_token,
        account_id,
    })
}

fn should_refresh(auth_file: &Value) -> bool {
    let Some(access_token) = token_string(auth_file, "access_token") else {
        return true;
    };
    if jwt_claims(&access_token)
        .and_then(|claims| claims.get("exp").and_then(Value::as_u64))
        .is_some_and(|expiry| {
            SystemTime::now() + REFRESH_MARGIN >= UNIX_EPOCH + Duration::from_secs(expiry)
        })
    {
        return true;
    }

    auth_file
        .get("last_refresh")
        .and_then(Value::as_str)
        .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
        .map(SystemTime::from)
        .is_some_and(|refreshed| {
            SystemTime::now()
                .duration_since(refreshed)
                .is_ok_and(|elapsed| elapsed >= REFRESH_INTERVAL)
        })
}

fn refresh_auth(client: &Client, auth_path: &Path, auth_file: &mut Value) -> Result<(), String> {
    let refresh_token = token_string(auth_file, "refresh_token")
        .ok_or_else(|| "Codex refresh token is unavailable; run `codex login`".to_string())?;
    let response = client
        .post(TOKEN_URL)
        .json(&json!({
            "grant_type": "refresh_token",
            "refresh_token": refresh_token,
            "client_id": CODEX_CLIENT_ID,
            "scope": "openid profile email offline_access"
        }))
        .send()
        .map_err(|error| format!("failed to refresh Codex authentication: {error}"))?;
    let status = response.status();
    let payload: Value = response
        .json()
        .map_err(|error| format!("Codex token refresh returned invalid JSON: {error}"))?;
    if !status.is_success() {
        return Err(format!(
            "Codex token refresh failed ({status}): {}",
            api_error_message(&payload)
        ));
    }

    let refreshed_access = payload
        .get("access_token")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "Codex token refresh returned no access token".to_string())?;
    let tokens = auth_file
        .get_mut("tokens")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| "Codex auth file has no tokens object".to_string())?;
    tokens.insert(
        "access_token".to_string(),
        Value::String(refreshed_access.to_string()),
    );
    for key in ["id_token", "refresh_token"] {
        if let Some(value) = payload.get(key).and_then(Value::as_str) {
            tokens.insert(key.to_string(), Value::String(value.to_string()));
        }
    }
    let refreshed_account_id = tokens
        .get("id_token")
        .and_then(Value::as_str)
        .and_then(account_id);
    if let Some(account_id) = refreshed_account_id {
        tokens.insert("account_id".to_string(), Value::String(account_id));
    }
    auth_file
        .as_object_mut()
        .ok_or_else(|| "Codex auth file must contain a JSON object".to_string())?
        .insert(
            "last_refresh".to_string(),
            Value::String(chrono::Utc::now().to_rfc3339()),
        );
    fs::write(
        auth_path,
        serde_json::to_string_pretty(auth_file)
            .map_err(|error| format!("failed to serialize refreshed Codex auth: {error}"))?,
    )
    .map_err(|error| format!("failed to save refreshed Codex auth: {error}"))
}

fn token_string(auth_file: &Value, key: &str) -> Option<String> {
    auth_file
        .get("tokens")?
        .get(key)?
        .as_str()
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn account_id(id_token: &str) -> Option<String> {
    let claims = jwt_claims(id_token)?;
    claims
        .get("https://api.openai.com/auth")
        .and_then(|auth| auth.get("chatgpt_account_id"))
        .or_else(|| claims.get("chatgpt_account_id"))
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn jwt_claims(token: &str) -> Option<Map<String, Value>> {
    let payload = token.split('.').nth(1)?;
    let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .ok()?;
    serde_json::from_slice::<Value>(&decoded)
        .ok()?
        .as_object()
        .cloned()
}

fn stream_response(
    response: Response,
    on_chunk: impl Fn(&str) -> Result<(), String>,
) -> Result<String, String> {
    let status = response.status();
    if !status.is_success() {
        let body = response
            .text()
            .unwrap_or_else(|error| format!("failed to read error response: {error}"));
        return Err(format!(
            "Codex Responses API failed ({status}): {}",
            body.chars().take(2_000).collect::<String>()
        ));
    }

    let mut description = String::new();
    let mut fallback = String::new();
    let mut data = String::new();
    for line in BufReader::new(response).lines() {
        let line =
            line.map_err(|error| format!("failed to read Codex response stream: {error}"))?;
        if line.is_empty() {
            handle_sse_data(&data, &mut description, &mut fallback, &on_chunk)?;
            data.clear();
        } else if let Some(value) = line.strip_prefix("data:") {
            if !data.is_empty() {
                data.push('\n');
            }
            data.push_str(value.trim_start());
        }
    }
    handle_sse_data(&data, &mut description, &mut fallback, &on_chunk)?;
    if description.is_empty() && !fallback.is_empty() {
        on_chunk(&fallback)?;
        return Ok(fallback);
    }
    Ok(description)
}

fn handle_sse_data(
    data: &str,
    description: &mut String,
    fallback: &mut String,
    on_chunk: &impl Fn(&str) -> Result<(), String>,
) -> Result<(), String> {
    if data.is_empty() || data == "[DONE]" {
        return Ok(());
    }
    let event: Value = serde_json::from_str(data)
        .map_err(|error| format!("Codex returned invalid stream JSON: {error}"))?;
    match event.get("type").and_then(Value::as_str) {
        Some("response.output_text.delta") => {
            if let Some(delta) = event.get("delta").and_then(Value::as_str) {
                description.push_str(delta);
                on_chunk(delta)?;
            }
        }
        Some("response.completed") => {
            *fallback = completed_output_text(&event).unwrap_or_default();
        }
        Some("error" | "response.failed" | "response.incomplete") => {
            return Err(format!(
                "Codex response failed: {}",
                api_error_message(&event)
            ));
        }
        _ => {}
    }
    Ok(())
}

fn completed_output_text(event: &Value) -> Option<String> {
    let output = event.pointer("/response/output")?.as_array()?;
    Some(
        output
            .iter()
            .filter_map(|item| item.get("content").and_then(Value::as_array))
            .flatten()
            .filter(|item| item.get("type").and_then(Value::as_str) == Some("output_text"))
            .filter_map(|item| item.get("text").and_then(Value::as_str))
            .collect(),
    )
}

fn api_error_message(value: &Value) -> String {
    value
        .pointer("/error/message")
        .or_else(|| value.get("error_description"))
        .or_else(|| value.get("message"))
        .and_then(Value::as_str)
        .unwrap_or("unknown error")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn session_input_replays_compacted_history_and_newer_items() {
        let path = std::env::temp_dir().join(format!(
            "coding-agent-va-codex-history-{}-{}.jsonl",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut file = fs::File::create(&path).unwrap();
        writeln!(file, r#"{{"type":"response_item","payload":{{"type":"message","role":"user","content":[]}}}}"#).unwrap();
        writeln!(file, r#"{{"type":"compacted","payload":{{"replacement_history":[{{"type":"message","role":"user","content":[{{"type":"input_text","text":"summary"}}]}}]}}}}"#).unwrap();
        writeln!(file, r#"{{"type":"response_item","payload":{{"type":"message","role":"assistant","content":[{{"type":"output_text","text":"answer"}}]}}}}"#).unwrap();

        let input = load_session_input(&path, "describe").unwrap();
        fs::remove_file(path).unwrap();
        assert_eq!(input.len(), 3);
        assert_eq!(
            input[0].pointer("/content/0/text").and_then(Value::as_str),
            Some("summary")
        );
        assert_eq!(
            input[2].pointer("/content/0/text").and_then(Value::as_str),
            Some("describe")
        );
    }

    #[test]
    fn rollback_removes_the_last_user_turn_and_its_outputs() {
        let mut history = vec![
            json!({"type":"message","role":"user"}),
            json!({"type":"message","role":"assistant"}),
            json!({"type":"message","role":"user"}),
            json!({"type":"function_call"}),
        ];
        drop_last_user_turns(&mut history, 1);
        assert_eq!(history.len(), 2);
    }

    #[test]
    fn output_text_delta_streams_description() {
        let mut description = String::new();
        let mut fallback = String::new();
        let streamed = std::cell::RefCell::new(String::new());
        handle_sse_data(
            r#"{"type":"response.output_text.delta","delta":"hello"}"#,
            &mut description,
            &mut fallback,
            &|chunk| {
                streamed.borrow_mut().push_str(chunk);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(description, "hello");
        assert_eq!(*streamed.borrow(), "hello");
    }

    #[test]
    fn client_version_prefers_the_model_cache() {
        let home = std::env::temp_dir().join(format!(
            "coding-agent-va-codex-version-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&home).unwrap();
        fs::write(
            home.join("models_cache.json"),
            r#"{"client_version":"0.144.3"}"#,
        )
        .unwrap();
        fs::write(home.join("version.json"), r#"{"latest_version":"0.145.0"}"#).unwrap();

        assert_eq!(load_client_version(&home).unwrap(), "0.144.3");
        fs::remove_dir_all(home).unwrap();
    }
}

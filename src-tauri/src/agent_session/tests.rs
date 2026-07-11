use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use watchexec::WatchedPath;

use super::paths::normalize_absolute_activity_path;
use super::protocols::{AgentSessionProtocol, ClaudeSessionProtocol, PiSessionProtocol};
use super::types::{AgentSessionProvider, SessionWatchTarget};
use super::watch::{push_watch_target, watched_paths_from_targets};

#[test]
fn pi_protocol_lists_sessions_and_extracts_file_activity() {
    let temp_dir = create_temp_dir("pi-protocol");
    let workspace = temp_dir.join("workspace");
    fs::create_dir_all(&workspace).expect("create workspace");
    let read_path = workspace.join("README.md");
    let edited_path = workspace.join("src.ts");
    fs::write(&read_path, "hello").expect("write read file");
    fs::write(&edited_path, "old").expect("write edited file");
    init_git_repo(&workspace);
    fs::write(&edited_path, "new").expect("modify edited file");

    let sessions_dir = temp_dir.join("sessions").join("--tmp-workspace--");
    fs::create_dir_all(&sessions_dir).expect("create sessions dir");
    let transcript_path = sessions_dir.join("2026-07-05T00-00-00Z_session.jsonl");
    let mut transcript = File::create(&transcript_path).expect("create transcript");
    writeln!(
        transcript,
        r#"{{"type":"session","version":3,"id":"pi-session","timestamp":"2026-07-05T00:00:00Z","cwd":"{}"}}"#,
        workspace.display()
    )
    .expect("write header");
    writeln!(
        transcript,
        r#"{{"type":"message","id":"1","parentId":null,"timestamp":"2026-07-05T00:00:01Z","message":{{"role":"user","content":[{{"type":"text","text":"Implement the thing"}}]}}}}"#
    )
    .expect("write user");
    writeln!(
        transcript,
        r#"{{"type":"message","id":"2","parentId":"1","timestamp":"2026-07-05T00:00:02Z","message":{{"role":"assistant","content":[{{"type":"toolCall","name":"read","arguments":{{"path":"README.md"}}}},{{"type":"toolCall","name":"write","arguments":{{"path":"src.ts"}}}}]}}}}"#
    )
    .expect("write assistant");

    let protocol = PiSessionProtocol;
    let sessions = protocol.list_sessions(&temp_dir);
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].provider, AgentSessionProvider::Pi);
    assert_eq!(sessions[0].title, "Implement the thing");

    let activity = protocol
        .read_file_activity(
            &transcript_path,
            Some(workspace.to_str().expect("utf8 workspace")),
            true,
        )
        .expect("read activity");
    assert_eq!(
        activity.read_files,
        vec![normalize_absolute_activity_path(&read_path)]
    );
    assert_eq!(
        activity.edited_files,
        vec![normalize_absolute_activity_path(&edited_path)]
    );
}

#[test]
fn claude_protocol_lists_sessions_and_extracts_file_activity() {
    let temp_dir = create_temp_dir("claude-protocol");
    let workspace = temp_dir.join("workspace");
    fs::create_dir_all(&workspace).expect("create workspace");
    let read_path = workspace.join("README.md");
    let edited_path = workspace.join("src.ts");
    fs::write(&read_path, "hello").expect("write read file");
    fs::write(&edited_path, "old").expect("write edited file");
    init_git_repo(&workspace);
    fs::write(&edited_path, "new").expect("modify edited file");

    let projects_dir = temp_dir.join("projects").join("-tmp-workspace");
    fs::create_dir_all(&projects_dir).expect("create projects dir");
    let transcript_path = projects_dir.join("claude-session.jsonl");
    let mut transcript = File::create(&transcript_path).expect("create transcript");
    writeln!(
        transcript,
        r#"{{"parentUuid":null,"type":"user","message":{{"role":"user","content":"Fix the bug"}},"uuid":"1","timestamp":"2026-07-05T00:00:01Z","cwd":"{}","sessionId":"claude-session"}}"#,
        workspace.display()
    )
    .expect("write user");
    writeln!(
        transcript,
        r#"{{"type":"ai-title","aiTitle":"Fix bug title","sessionId":"claude-session"}}"#
    )
    .expect("write title");
    let assistant_entry = serde_json::json!({
        "type": "assistant",
        "message": {
            "role": "assistant",
            "content": [
                {
                    "type": "tool_use",
                    "name": "Read",
                    "input": { "file_path": read_path.display().to_string() }
                },
                {
                    "type": "tool_use",
                    "name": "Edit",
                    "input": { "file_path": edited_path.display().to_string() }
                }
            ]
        },
        "uuid": "2",
        "timestamp": "2026-07-05T00:00:02Z",
        "cwd": workspace.display().to_string(),
        "sessionId": "claude-session"
    });
    writeln!(transcript, "{assistant_entry}").expect("write assistant");

    let protocol = ClaudeSessionProtocol;
    let sessions = protocol.list_sessions(&temp_dir);
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].provider, AgentSessionProvider::Claude);
    assert_eq!(sessions[0].title, "Fix bug title");

    let activity = protocol
        .read_file_activity(
            &transcript_path,
            Some(workspace.to_str().expect("utf8 workspace")),
            true,
        )
        .expect("read activity");
    assert_eq!(
        activity.read_files,
        vec![normalize_absolute_activity_path(&read_path)]
    );
    assert_eq!(
        activity.edited_files,
        vec![normalize_absolute_activity_path(&edited_path)]
    );
}

#[test]
fn provider_watch_filters_match_expected_paths() {
    let codex_home = Path::new("/tmp/codex-home");
    let claude_home = Path::new("/tmp/claude-home");
    let pi_home = Path::new("/tmp/pi-home");

    assert!(AgentSessionProvider::Codex
        .protocol()
        .is_relevant_session_path(
            &codex_home.join("sessions/a/rollout-2026-test.jsonl"),
            codex_home
        ));
    assert!(AgentSessionProvider::Claude
        .protocol()
        .is_relevant_session_path(
            &claude_home.join("projects/-tmp/session.jsonl"),
            claude_home
        ));
    assert!(AgentSessionProvider::Pi
        .protocol()
        .is_relevant_session_path(&pi_home.join("sessions/--tmp--/session.jsonl"), pi_home));
    assert!(!AgentSessionProvider::Pi
        .protocol()
        .is_relevant_session_path(&pi_home.join("auth.json"), pi_home));
}

#[test]
fn watch_targets_preserve_recursive_mode() {
    let path = PathBuf::from("/tmp/coding-agent-va-watch-target");
    let targets = vec![
        SessionWatchTarget {
            path: path.display().to_string(),
            recursive: false,
            exists: true,
            reason: "watch exact file".to_string(),
        },
        SessionWatchTarget {
            path: path.join("sessions").display().to_string(),
            recursive: true,
            exists: true,
            reason: "watch session tree".to_string(),
        },
    ];

    assert_eq!(
        watched_paths_from_targets(&targets),
        vec![
            WatchedPath::non_recursive(&path),
            WatchedPath::recursive(path.join("sessions")),
        ]
    );
}

#[test]
fn duplicate_watch_targets_upgrade_to_recursive() {
    let mut targets = Vec::new();
    let path = PathBuf::from("/tmp/coding-agent-va-watch-target");

    push_watch_target(
        &mut targets,
        path.clone(),
        false,
        true,
        "watch exact file".to_string(),
    );
    push_watch_target(
        &mut targets,
        path,
        true,
        false,
        "watch session tree".to_string(),
    );

    assert_eq!(targets.len(), 1);
    assert!(targets[0].recursive);
    assert!(targets[0].exists);
    assert!(targets[0].reason.contains("watch exact file"));
    assert!(targets[0].reason.contains("watch session tree"));
}

fn create_temp_dir(label: &str) -> PathBuf {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!("coding-agent-va-{label}-{unique}"));
    fs::create_dir_all(&temp_dir).expect("create temp dir");
    temp_dir
}

fn init_git_repo(repo_dir: &Path) {
    run_git(repo_dir, &["init"]);
    run_git(repo_dir, &["config", "user.email", "agent@example.com"]);
    run_git(repo_dir, &["config", "user.name", "Agent"]);
    run_git(repo_dir, &["add", "."]);
    run_git(repo_dir, &["commit", "-m", "initial"]);
}

fn run_git(repo_dir: &Path, args: &[&str]) {
    let output = Command::new("git")
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .arg("-C")
        .arg(repo_dir)
        .args(args)
        .output()
        .expect("run git");
    assert!(
        output.status.success(),
        "git command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

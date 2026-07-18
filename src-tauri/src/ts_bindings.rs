use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use ts_rs::{Config, TS};

use crate::agent_session::{
    AgentRuntimeSource, AgentSessionDetails, AgentSessionFileActivity, AgentSessionFileDiff,
    AgentSessionImpactedFileRelation, AgentSessionList, AgentSessionNodeDescriptionRequest,
    AgentSessionNodeDescriptionResponse, AgentSessionNodeDescriptionStreamEvent,
    AgentSessionPromptTurn, AgentSessionProvider, AgentSessionSummary, AgentSessionTask,
    AgentSessionTaskStatus, DescriptionGraphNode, DescriptionGraphRelation,
    SessionWatchEventPayload, SessionWatchPlan, SessionWatchRegistration, SessionWatchTarget,
};
use crate::app_config::{
    AppFont, AppSettings, AppTheme, DescriptionProviderSettings, DescriptionReasoning,
    DescriptionSettings, MonacoTheme, RuntimeHomes,
};
use crate::indexer::graph::{
    ArchitectureEdge, ArchitectureGraph, ArchitectureNode, EdgeKind, NodeKind,
};
use crate::shared::logger::{LogEntry, LogLevel};

const GENERATED_BINDINGS_HEADER: &str =
    "// This file is generated from Rust types. Do not edit by hand.\n\n";

fn bindings_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("src")
        .join("shared")
        .join("lib")
        .join("generated")
        .join("bindings.ts")
}

fn push_binding<T: TS>(bindings: &mut String, config: &Config) {
    let declaration = T::decl(config);
    if declaration.starts_with("type ") {
        bindings.push_str("export ");
    }
    bindings.push_str(&declaration);
    bindings.push_str("\n\n");
}

fn generated_typescript_bindings() -> String {
    let config = Config::default().with_large_int("number");
    let mut bindings = String::from(GENERATED_BINDINGS_HEADER);

    push_binding::<AppTheme>(&mut bindings, &config);
    push_binding::<AppFont>(&mut bindings, &config);
    push_binding::<MonacoTheme>(&mut bindings, &config);
    push_binding::<RuntimeHomes>(&mut bindings, &config);
    push_binding::<DescriptionReasoning>(&mut bindings, &config);
    push_binding::<DescriptionProviderSettings>(&mut bindings, &config);
    push_binding::<DescriptionSettings>(&mut bindings, &config);
    push_binding::<AppSettings>(&mut bindings, &config);
    push_binding::<LogLevel>(&mut bindings, &config);
    push_binding::<LogEntry>(&mut bindings, &config);
    push_binding::<AgentSessionProvider>(&mut bindings, &config);
    push_binding::<AgentRuntimeSource>(&mut bindings, &config);
    push_binding::<AgentSessionSummary>(&mut bindings, &config);
    push_binding::<AgentSessionList>(&mut bindings, &config);
    push_binding::<AgentSessionTaskStatus>(&mut bindings, &config);
    push_binding::<AgentSessionTask>(&mut bindings, &config);
    push_binding::<AgentSessionDetails>(&mut bindings, &config);
    push_binding::<AgentSessionPromptTurn>(&mut bindings, &config);
    push_binding::<SessionWatchTarget>(&mut bindings, &config);
    push_binding::<SessionWatchPlan>(&mut bindings, &config);
    push_binding::<SessionWatchRegistration>(&mut bindings, &config);
    push_binding::<SessionWatchEventPayload>(&mut bindings, &config);
    push_binding::<AgentSessionFileActivity>(&mut bindings, &config);
    push_binding::<AgentSessionImpactedFileRelation>(&mut bindings, &config);
    push_binding::<AgentSessionFileDiff>(&mut bindings, &config);
    push_binding::<DescriptionGraphNode>(&mut bindings, &config);
    push_binding::<DescriptionGraphRelation>(&mut bindings, &config);
    push_binding::<AgentSessionNodeDescriptionRequest>(&mut bindings, &config);
    push_binding::<AgentSessionNodeDescriptionResponse>(&mut bindings, &config);
    push_binding::<AgentSessionNodeDescriptionStreamEvent>(&mut bindings, &config);
    push_binding::<NodeKind>(&mut bindings, &config);
    push_binding::<EdgeKind>(&mut bindings, &config);
    push_binding::<ArchitectureNode>(&mut bindings, &config);
    push_binding::<ArchitectureEdge>(&mut bindings, &config);
    push_binding::<ArchitectureGraph>(&mut bindings, &config);

    format_typescript_bindings(&bindings)
}

fn format_typescript_bindings(bindings: &str) -> String {
    let formatter_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("node_modules")
        .join(".bin")
        .join("oxfmt");
    let Ok(mut child) = Command::new(formatter_path)
        .arg("--stdin-filepath=bindings.ts")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
    else {
        return bindings.to_string();
    };

    if let Some(mut stdin) = child.stdin.take() {
        if stdin.write_all(bindings.as_bytes()).is_err() {
            return bindings.to_string();
        }
    }

    let Ok(output) = child.wait_with_output() else {
        return bindings.to_string();
    };

    if !output.status.success() {
        return bindings.to_string();
    }

    String::from_utf8(output.stdout).unwrap_or_else(|_| bindings.to_string())
}

#[test]
fn typescript_bindings_are_current() {
    let path = bindings_path();
    let generated = generated_typescript_bindings();

    if std::env::var_os("UPDATE_TS_BINDINGS").is_some() {
        fs::create_dir_all(path.parent().expect("bindings path has a parent"))
            .expect("create generated bindings directory");
        fs::write(path, generated).expect("write generated TypeScript bindings");
        return;
    }

    let existing = fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "read generated TypeScript bindings at {}: {error}. Run `just generate-ts-bindings`.",
            path.display()
        )
    });

    assert_eq!(
        existing, generated,
        "generated TypeScript bindings are out of date. Run `just generate-ts-bindings`."
    );
}

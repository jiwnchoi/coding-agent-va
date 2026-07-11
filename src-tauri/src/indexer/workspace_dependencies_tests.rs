use super::workspace_dependencies::{find_impacted_file_relations, find_impacted_files};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn finds_relative_importers_of_changed_file() {
    let workspace_root = create_temp_workspace("changed");
    let src_dir = workspace_root.join("src");
    fs::create_dir_all(&src_dir).expect("create src dir");
    fs::write(
        src_dir.join("app.ts"),
        "import { value } from './dep';\nconsole.log(value);\n",
    )
    .expect("write app");
    fs::write(src_dir.join("dep.ts"), "export const value = 1;\n").expect("write dep");

    let impacted_files =
        find_impacted_files(&workspace_root, &[src_dir.join("dep.ts")]).expect("index deps");

    assert_eq!(impacted_files, vec!["src/app.ts"]);
    fs::remove_dir_all(workspace_root).expect("cleanup workspace");
}

#[test]
fn finds_importers_of_deleted_file_when_path_is_missing() {
    let workspace_root = create_temp_workspace("deleted");
    let src_dir = workspace_root.join("src");
    fs::create_dir_all(&src_dir).expect("create src dir");
    fs::write(
        src_dir.join("app.ts"),
        "import { missingValue } from './deleted';\nconsole.log(missingValue);\n",
    )
    .expect("write app");

    let impacted_files =
        find_impacted_files(&workspace_root, &[src_dir.join("deleted.ts")]).expect("index deps");

    assert_eq!(impacted_files, vec!["src/app.ts"]);
    fs::remove_dir_all(workspace_root).expect("cleanup workspace");
}

#[test]
fn explains_why_files_are_impacted() {
    let workspace_root = create_temp_workspace("relations");
    let src_dir = workspace_root.join("src");
    fs::create_dir_all(&src_dir).expect("create src dir");
    fs::write(
        src_dir.join("app.ts"),
        "import { value } from './dep';\nconsole.log(value);\n",
    )
    .expect("write app");
    fs::write(src_dir.join("dep.ts"), "export const value = 1;\n").expect("write dep");

    let impacted_relations =
        find_impacted_file_relations(&workspace_root, &[src_dir.join("dep.ts")])
            .expect("index dependency relations");

    assert_eq!(impacted_relations.len(), 1);
    assert_eq!(impacted_relations[0].changed_file, "src/dep.ts");
    assert_eq!(impacted_relations[0].impacted_file, "src/app.ts");
    assert_eq!(impacted_relations[0].import_specifier, "./dep");
    fs::remove_dir_all(workspace_root).expect("cleanup workspace");
}

#[test]
fn finds_rust_importers_for_crate_and_super_paths() {
    let workspace_root = create_temp_workspace("rust-imports");
    let crate_root = workspace_root.join("backend");
    let activity_dir = crate_root.join("src/agent_session/activity");
    fs::create_dir_all(&activity_dir).expect("create Rust source dirs");
    fs::write(
        crate_root.join("Cargo.toml"),
        "[package]\nname='test'\nversion='0.1.0'\n",
    )
    .expect("write Cargo manifest");
    fs::write(
        crate_root.join("src/agent_session/json.rs"),
        "pub fn json_str() {}\n",
    )
    .expect("write json module");
    fs::write(activity_dir.join("shell.rs"), "pub fn collect() {}\n").expect("write shell module");
    fs::write(
        activity_dir.join("codex.rs"),
        "use crate::agent_session::json::json_str;\nuse super::shell::collect;\n",
    )
    .expect("write importer");

    let changed_files = [
        crate_root.join("src/agent_session/json.rs"),
        activity_dir.join("shell.rs"),
    ];
    let impacted_files =
        find_impacted_files(&workspace_root, &changed_files).expect("index Rust dependencies");
    let impacted_relations = find_impacted_file_relations(&workspace_root, &changed_files)
        .expect("index Rust dependency relations");

    assert_eq!(
        impacted_files,
        vec!["backend/src/agent_session/activity/codex.rs"]
    );
    assert_eq!(impacted_relations.len(), 2);
    fs::remove_dir_all(workspace_root).expect("cleanup workspace");
}

fn create_temp_workspace(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("current time")
        .as_nanos();
    let workspace_root = std::env::temp_dir().join(format!("coding-agent-va-{label}-{unique}"));
    fs::create_dir_all(&workspace_root).expect("create temp workspace");
    workspace_root
}

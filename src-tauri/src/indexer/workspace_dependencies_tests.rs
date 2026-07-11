use super::workspace_dependencies::{
    find_impacted_file_relations, find_impacted_files, find_session_impacted_file_relations,
    SessionFileEdit,
};
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
fn limits_impact_to_files_using_the_changed_declaration() {
    let workspace_root = create_temp_workspace("changed-symbol");
    let src_dir = workspace_root.join("src");
    fs::create_dir_all(&src_dir).expect("create src dir");
    fs::write(
        src_dir.join("dep.ts"),
        "export function changed() { return 2; }\nexport function untouched() { return 1; }\n",
    )
    .expect("write dependency");
    fs::write(
        src_dir.join("changed-user.ts"),
        "import { changed } from './dep';\nchanged();\n",
    )
    .expect("write changed symbol user");
    fs::write(
        src_dir.join("untouched-user.ts"),
        "import { untouched } from './dep';\nuntouched();\n",
    )
    .expect("write untouched symbol user");

    let relations = find_session_impacted_file_relations(
        &workspace_root,
        &[SessionFileEdit {
            path: src_dir.join("dep.ts"),
            fragments: vec!["return 2".to_string()],
        }],
    )
    .expect("index changed declaration");

    assert_eq!(relations.len(), 1);
    assert_eq!(relations[0].impacted_file, "src/changed-user.ts");
    fs::remove_dir_all(workspace_root).expect("cleanup workspace");
}

#[test]
fn recognizes_value_access_and_inheritance_as_actual_uses() {
    let workspace_root = create_temp_workspace("usage-kinds");
    let src_dir = workspace_root.join("src");
    fs::create_dir_all(&src_dir).expect("create src dir");
    fs::write(
        src_dir.join("dep.ts"),
        "export const VALUE = 2;\nexport class Base {}\n",
    )
    .expect("write dependency");
    fs::write(
        src_dir.join("value-user.ts"),
        "import { VALUE } from './dep';\nconsole.log(VALUE);\n",
    )
    .expect("write value user");
    fs::write(
        src_dir.join("child.ts"),
        "import { Base } from './dep';\nclass Child extends Base {}\n",
    )
    .expect("write subclass");

    let relations = find_session_impacted_file_relations(
        &workspace_root,
        &[
            SessionFileEdit {
                path: src_dir.join("dep.ts"),
                fragments: vec!["VALUE = 2".to_string()],
            },
            SessionFileEdit {
                path: src_dir.join("dep.ts"),
                fragments: vec!["class Base".to_string()],
            },
        ],
    )
    .expect("index value and inheritance uses");

    let impacted = relations
        .into_iter()
        .map(|relation| relation.impacted_file)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        impacted,
        std::collections::BTreeSet::from([
            "src/child.ts".to_string(),
            "src/value-user.ts".to_string()
        ])
    );
    fs::remove_dir_all(workspace_root).expect("cleanup workspace");
}

#[test]
fn limits_method_changes_to_files_calling_that_method() {
    let workspace_root = create_temp_workspace("changed-method");
    let src_dir = workspace_root.join("src");
    fs::create_dir_all(&src_dir).expect("create src dir");
    fs::write(
        src_dir.join("service.ts"),
        "export class Service {\n  changed() { return 2; }\n  untouched() { return 1; }\n}\n",
    )
    .expect("write service");
    fs::write(
        src_dir.join("changed-user.ts"),
        "import { Service } from './service';\nexport function run(service: Service) { service.changed(); }\n",
    )
    .expect("write changed method user");
    fs::write(
        src_dir.join("untouched-user.ts"),
        "import { Service } from './service';\nexport function run(service: Service) { service.untouched(); }\n",
    )
    .expect("write untouched method user");

    let relations = find_session_impacted_file_relations(
        &workspace_root,
        &[SessionFileEdit {
            path: src_dir.join("service.ts"),
            fragments: vec!["changed() { return 2; }".to_string()],
        }],
    )
    .expect("index changed method");

    assert_eq!(relations.len(), 1);
    assert_eq!(relations[0].impacted_file, "src/changed-user.ts");
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
fn propagates_impact_through_reexport_only_modules() {
    let workspace_root = create_temp_workspace("reexports");
    let src_dir = workspace_root.join("src");
    let feature_dir = src_dir.join("feature");
    fs::create_dir_all(&feature_dir).expect("create feature dir");
    fs::write(
        src_dir.join("app.ts"),
        "import { value } from './feature';\nexport const result = value;\n",
    )
    .expect("write app");
    fs::write(
        src_dir.join("consumer.ts"),
        "import { result } from './app';\nconsole.log(result);\n",
    )
    .expect("write transitive consumer");
    fs::write(
        feature_dir.join("index.ts"),
        "// Public API\nexport { value } from './public';\n",
    )
    .expect("write outer re-export");
    fs::write(
        feature_dir.join("public.ts"),
        "export { value } from './value';\n",
    )
    .expect("write inner re-export");
    fs::write(feature_dir.join("value.ts"), "export const value = 1;\n")
        .expect("write implementation");

    let impacted_relations = find_session_impacted_file_relations(
        &workspace_root,
        &[SessionFileEdit {
            path: feature_dir.join("value.ts"),
            fragments: vec!["value = 1".to_string()],
        }],
    )
    .expect("index dependencies through re-exports");

    assert_eq!(impacted_relations.len(), 1);
    assert_eq!(impacted_relations[0].changed_file, "src/feature/value.ts");
    assert_eq!(impacted_relations[0].impacted_file, "src/app.ts");
    assert_eq!(impacted_relations[0].import_specifier, "./feature");
    fs::remove_dir_all(workspace_root).expect("cleanup workspace");
}

#[test]
fn forwards_reexport_edges_from_mixed_modules() {
    let workspace_root = create_temp_workspace("runtime-export");
    let src_dir = workspace_root.join("src");
    fs::create_dir_all(&src_dir).expect("create src dir");
    fs::write(
        src_dir.join("index.ts"),
        "export { value } from './value';\nexport const version = 1;\n",
    )
    .expect("write mixed module");
    fs::write(src_dir.join("value.ts"), "export const value = 1;\n").expect("write implementation");
    fs::write(
        src_dir.join("app.ts"),
        "import { value } from './index';\nconsole.log(value);\n",
    )
    .expect("write consumer");

    let impacted_files = find_session_impacted_file_relations(
        &workspace_root,
        &[SessionFileEdit {
            path: src_dir.join("value.ts"),
            fragments: vec!["value = 1".to_string()],
        }],
    )
    .expect("index mixed module dependency")
    .into_iter()
    .map(|relation| relation.impacted_file)
    .collect::<Vec<_>>();

    assert_eq!(impacted_files, vec!["src/app.ts"]);
    fs::remove_dir_all(workspace_root).expect("cleanup workspace");
}

#[test]
fn propagates_through_separate_import_and_export_statements() {
    let workspace_root = create_temp_workspace("separate-reexport");
    let src_dir = workspace_root.join("src");
    fs::create_dir_all(&src_dir).expect("create src dir");
    fs::write(src_dir.join("value.ts"), "export const value = 1;\n").expect("write implementation");
    fs::write(
        src_dir.join("index.ts"),
        "import { value } from './value';\nexport { value };\n",
    )
    .expect("write separate re-export");
    fs::write(
        src_dir.join("app.ts"),
        "import { value } from './index';\nconsole.log(value);\n",
    )
    .expect("write consumer");

    let relations = find_session_impacted_file_relations(
        &workspace_root,
        &[SessionFileEdit {
            path: src_dir.join("value.ts"),
            fragments: vec!["value = 1".to_string()],
        }],
    )
    .expect("index separate re-export");

    assert_eq!(relations.len(), 1);
    assert_eq!(relations[0].impacted_file, "src/app.ts");
    fs::remove_dir_all(workspace_root).expect("cleanup workspace");
}

#[test]
fn propagates_through_rust_module_forwarders() {
    let workspace_root = create_temp_workspace("rust-forwarders");
    let src_dir = workspace_root.join("src");
    let api_dir = src_dir.join("api");
    fs::create_dir_all(&api_dir).expect("create Rust module dirs");
    fs::write(
        workspace_root.join("Cargo.toml"),
        "[package]\nname='test'\nversion='0.1.0'\n",
    )
    .expect("write Cargo manifest");
    fs::write(
        api_dir.join("mod.rs"),
        "pub mod value;\npub use value::value as public_value;\n",
    )
    .expect("write module forwarder");
    fs::write(api_dir.join("value.rs"), "pub fn value() {}\n").expect("write implementation");
    fs::write(
        src_dir.join("consumer.rs"),
        "use crate::api::public_value;\nfn consume() { public_value(); }\n",
    )
    .expect("write consumer");

    let impacted_files = find_session_impacted_file_relations(
        &workspace_root,
        &[SessionFileEdit {
            path: api_dir.join("value.rs"),
            fragments: vec!["fn value".to_string()],
        }],
    )
    .expect("index Rust dependencies through module forwarder")
    .into_iter()
    .map(|relation| relation.impacted_file)
    .collect::<Vec<_>>();

    assert_eq!(impacted_files, vec!["src/consumer.rs"]);
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

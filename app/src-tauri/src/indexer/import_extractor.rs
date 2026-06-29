use super::language::SupportedLanguage;
use tree_sitter::Node;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedImport {
    pub specifier: String,
}

pub fn extract_imports(
    language: SupportedLanguage,
    source: &str,
    root: Node<'_>,
) -> Vec<ExtractedImport> {
    let mut imports = Vec::new();
    collect_imports(language, source, root, &mut imports);

    imports
}

fn extract_import_specifier(source: &str, node: Node<'_>) -> Option<String> {
    let named_fields = [
        "source",
        "path",
        "module_name",
        "value",
        "argument",
        "library",
    ];
    for field_name in named_fields {
        if let Some(field_node) = node.child_by_field_name(field_name) {
            if let Some(text) = normalize_text(source, field_node) {
                return Some(text);
            }
        }
    }

    find_import_text(source, node)
}

fn normalize_text(source: &str, node: Node<'_>) -> Option<String> {
    let raw = node.utf8_text(source.as_bytes()).ok()?.trim();
    let value = raw
        .trim_matches('"')
        .trim_matches('\'')
        .trim_matches('<')
        .trim_matches('>');
    if value.is_empty() {
        return None;
    }

    Some(value.to_string())
}

fn import_node_kinds(language: SupportedLanguage) -> &'static [&'static str] {
    match language {
        SupportedLanguage::JavaScript
        | SupportedLanguage::Jsx
        | SupportedLanguage::TypeScript
        | SupportedLanguage::Tsx => &["import_statement", "export_statement", "call_expression"],
        SupportedLanguage::Rust => &["use_declaration", "extern_crate_declaration", "mod_item"],
        SupportedLanguage::Python => &["import_statement", "import_from_statement"],
        SupportedLanguage::Go => &["import_declaration", "import_spec"],
        SupportedLanguage::Java => &["import_declaration"],
        SupportedLanguage::C | SupportedLanguage::Cpp => &["preproc_include"],
        SupportedLanguage::CSharp => &["using_directive"],
        SupportedLanguage::Php => &[
            "namespace_use_declaration",
            "require_expression",
            "include_expression",
        ],
        SupportedLanguage::Ruby => &["call"],
        SupportedLanguage::Swift => &["import_declaration"],
        SupportedLanguage::Kotlin => &["import_header"],
        SupportedLanguage::Bash => &["command"],
        SupportedLanguage::Json
        | SupportedLanguage::Yaml
        | SupportedLanguage::Toml
        | SupportedLanguage::Html
        | SupportedLanguage::Css => &[],
    }
}

fn collect_imports(
    language: SupportedLanguage,
    source: &str,
    node: Node<'_>,
    imports: &mut Vec<ExtractedImport>,
) {
    if import_node_kinds(language).contains(&node.kind()) {
        if let Some(specifier) = extract_import_specifier(source, node) {
            imports.push(ExtractedImport { specifier });
        }
    }

    for index in 0..node.named_child_count() {
        let Some(child) = node.named_child(index as u32) else {
            continue;
        };
        collect_imports(language, source, child, imports);
    }
}

fn find_import_text(source: &str, node: Node<'_>) -> Option<String> {
    for index in 0..node.named_child_count() {
        let Some(child) = node.named_child(index as u32) else {
            continue;
        };
        if matches!(
            child.kind(),
            "string"
                | "string_literal"
                | "interpreted_string_literal"
                | "raw_string_literal"
                | "system_lib_string"
                | "path"
                | "identifier"
                | "scoped_identifier"
                | "namespace_identifier"
        ) {
            if let Some(text) = normalize_text(source, child) {
                return Some(text);
            }
        }

        if let Some(text) = find_import_text(source, child) {
            return Some(text);
        }
    }

    None
}

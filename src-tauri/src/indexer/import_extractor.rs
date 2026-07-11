use super::language::SupportedLanguage;
use tree_sitter::Node;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedImport {
    pub specifier: String,
    pub statement: String,
    pub forwards: bool,
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
            imports.push(ExtractedImport {
                specifier,
                statement: node
                    .utf8_text(source.as_bytes())
                    .unwrap_or_default()
                    .to_string(),
                forwards: is_forwarding_node(language, node),
            });
        }
    }

    for index in 0..node.named_child_count() {
        let Some(child) = node.named_child(index as u32) else {
            continue;
        };
        collect_imports(language, source, child, imports);
    }
}

pub fn extract_code_identifiers(
    language: SupportedLanguage,
    source: &str,
    root: Node<'_>,
) -> std::collections::HashSet<String> {
    let mut identifiers = std::collections::HashSet::new();
    collect_code_identifiers(language, source, root, &mut identifiers);
    identifiers
}

fn collect_code_identifiers(
    language: SupportedLanguage,
    source: &str,
    node: Node<'_>,
    identifiers: &mut std::collections::HashSet<String>,
) {
    if is_dependency_only_node(language, node) {
        return;
    }
    if is_identifier_kind(node.kind()) {
        if let Ok(identifier) = node.utf8_text(source.as_bytes()) {
            identifiers.insert(identifier.to_string());
        }
    }
    for index in 0..node.named_child_count() {
        if let Some(child) = node.named_child(index as u32) {
            collect_code_identifiers(language, source, child, identifiers);
        }
    }
}

fn is_dependency_only_node(language: SupportedLanguage, node: Node<'_>) -> bool {
    match language {
        SupportedLanguage::JavaScript
        | SupportedLanguage::Jsx
        | SupportedLanguage::TypeScript
        | SupportedLanguage::Tsx => {
            node.kind() == "import_statement"
                || (node.kind() == "export_statement"
                    && (node.child_by_field_name("source").is_some()
                        || has_child_kind(node, "export_clause")))
        }
        SupportedLanguage::Rust => matches!(node.kind(), "use_declaration" | "mod_item"),
        _ => import_node_kinds(language).contains(&node.kind()),
    }
}

fn is_identifier_kind(kind: &str) -> bool {
    matches!(
        kind,
        "identifier"
            | "type_identifier"
            | "property_identifier"
            | "field_identifier"
            | "shorthand_property_identifier"
            | "namespace_identifier"
            | "constant"
            | "name"
    )
}

fn has_child_kind(node: Node<'_>, kind: &str) -> bool {
    (0..node.named_child_count()).any(|index| {
        node.named_child(index as u32)
            .is_some_and(|child| child.kind() == kind)
    })
}

fn is_forwarding_node(language: SupportedLanguage, node: Node<'_>) -> bool {
    match language {
        SupportedLanguage::JavaScript
        | SupportedLanguage::Jsx
        | SupportedLanguage::TypeScript
        | SupportedLanguage::Tsx => {
            node.kind() == "export_statement" && node.child_by_field_name("source").is_some()
        }
        SupportedLanguage::Rust => {
            (node.kind() == "mod_item" && !has_child_kind(node, "declaration_list"))
                || (node.kind() == "use_declaration" && has_child_kind(node, "visibility_modifier"))
        }
        _ => false,
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
                | "string_fragment"
                | "interpreted_string_literal"
                | "raw_string_literal"
                | "system_lib_string"
        ) {
            if let Some(text) = normalize_text(source, child) {
                return Some(text);
            }
        }

        if let Some(text) = find_string_like_text(source, child) {
            return Some(text);
        }
    }

    for index in 0..node.named_child_count() {
        let Some(child) = node.named_child(index as u32) else {
            continue;
        };
        if matches!(
            child.kind(),
            "path" | "identifier" | "scoped_identifier" | "namespace_identifier"
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

fn find_string_like_text(source: &str, node: Node<'_>) -> Option<String> {
    for index in 0..node.named_child_count() {
        let Some(child) = node.named_child(index as u32) else {
            continue;
        };

        if matches!(
            child.kind(),
            "string"
                | "string_literal"
                | "string_fragment"
                | "interpreted_string_literal"
                | "raw_string_literal"
                | "system_lib_string"
        ) {
            if let Some(text) = normalize_text(source, child) {
                return Some(text);
            }
        }

        if let Some(text) = find_string_like_text(source, child) {
            return Some(text);
        }
    }

    None
}

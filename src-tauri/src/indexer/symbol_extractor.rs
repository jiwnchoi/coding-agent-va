use super::language::SupportedLanguage;
use tree_sitter::Node;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedSymbol {
    pub kind: String,
    pub name: String,
    pub start_byte: usize,
    pub end_byte: usize,
}

pub fn extract_symbols(
    language: SupportedLanguage,
    source: &str,
    root: Node<'_>,
) -> Vec<ExtractedSymbol> {
    let mut cursor = root.walk();
    root.named_children(&mut cursor)
        .flat_map(|child| symbols_from_top_level_node(language, source, child))
        .collect()
}

pub fn extract_declarations(
    language: SupportedLanguage,
    source: &str,
    root: Node<'_>,
) -> Vec<ExtractedSymbol> {
    let mut symbols = Vec::new();
    collect_symbols(language, source, root, &mut symbols);
    symbols
}

fn symbols_from_top_level_node(
    language: SupportedLanguage,
    source: &str,
    node: Node<'_>,
) -> Vec<ExtractedSymbol> {
    if matches!(node.kind(), "lexical_declaration" | "variable_declaration") {
        let mut cursor = node.walk();
        let variables = node
            .named_children(&mut cursor)
            .filter(|child| child.kind() == "variable_declarator")
            .filter_map(|child| symbol_from_named_node(source, child))
            .collect::<Vec<_>>();
        if !variables.is_empty() {
            return variables;
        }
    }
    if let Some(symbol) = symbol_from_node(language, source, node) {
        return vec![symbol];
    }
    if !matches!(node.kind(), "export_statement" | "decorated_definition") {
        return Vec::new();
    }
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .flat_map(|child| symbols_from_top_level_node(language, source, child))
        .collect()
}

fn collect_symbols(
    language: SupportedLanguage,
    source: &str,
    node: Node<'_>,
    symbols: &mut Vec<ExtractedSymbol>,
) {
    if matches!(node.kind(), "lexical_declaration" | "variable_declaration") {
        let mut cursor = node.walk();
        for variable in node
            .named_children(&mut cursor)
            .filter(|child| child.kind() == "variable_declarator")
            .filter_map(|child| symbol_from_named_node(source, child))
        {
            symbols.push(variable);
        }
    } else if let Some(symbol) = symbol_from_node(language, source, node) {
        symbols.push(symbol);
    }
    for index in 0..node.named_child_count() {
        if let Some(child) = node.named_child(index as u32) {
            collect_symbols(language, source, child, symbols);
        }
    }
}

fn symbol_from_named_node(source: &str, node: Node<'_>) -> Option<ExtractedSymbol> {
    let name = node
        .child_by_field_name("name")?
        .utf8_text(source.as_bytes())
        .ok()?
        .to_string();
    Some(ExtractedSymbol {
        kind: node.kind().to_string(),
        name,
        start_byte: node.start_byte(),
        end_byte: node.end_byte(),
    })
}

fn symbol_from_node(
    language: SupportedLanguage,
    source: &str,
    node: Node<'_>,
) -> Option<ExtractedSymbol> {
    let kind = node.kind();
    if !symbol_node_kinds(language).contains(&kind) {
        return None;
    }

    let name = node
        .child_by_field_name("name")
        .and_then(|name_node| name_node.utf8_text(source.as_bytes()).ok())
        .map(ToOwned::to_owned)
        .or_else(|| fallback_symbol_name(source, node))?;

    Some(ExtractedSymbol {
        kind: kind.to_string(),
        name,
        start_byte: node.start_byte(),
        end_byte: node.end_byte(),
    })
}

fn fallback_symbol_name(source: &str, node: Node<'_>) -> Option<String> {
    find_named_descendant_text(source, node)
}

fn symbol_node_kinds(language: SupportedLanguage) -> &'static [&'static str] {
    match language {
        SupportedLanguage::JavaScript
        | SupportedLanguage::Jsx
        | SupportedLanguage::TypeScript
        | SupportedLanguage::Tsx => &[
            "function_declaration",
            "method_definition",
            "class_declaration",
            "interface_declaration",
            "type_alias_declaration",
            "enum_declaration",
            "lexical_declaration",
            "variable_declaration",
        ],
        SupportedLanguage::Rust => &[
            "function_item",
            "struct_item",
            "enum_item",
            "trait_item",
            "impl_item",
            "mod_item",
            "type_item",
            "const_item",
        ],
        SupportedLanguage::Python => &["function_definition", "class_definition"],
        SupportedLanguage::Go => &[
            "function_declaration",
            "method_declaration",
            "type_declaration",
            "const_declaration",
            "var_declaration",
        ],
        SupportedLanguage::Java => &[
            "class_declaration",
            "interface_declaration",
            "enum_declaration",
            "annotation_type_declaration",
            "method_declaration",
            "record_declaration",
        ],
        SupportedLanguage::C => &[
            "function_definition",
            "struct_specifier",
            "enum_specifier",
            "type_definition",
        ],
        SupportedLanguage::Cpp => &[
            "function_definition",
            "class_specifier",
            "struct_specifier",
            "enum_specifier",
            "namespace_definition",
        ],
        SupportedLanguage::CSharp => &[
            "class_declaration",
            "interface_declaration",
            "enum_declaration",
            "struct_declaration",
            "method_declaration",
            "record_declaration",
        ],
        SupportedLanguage::Php => &[
            "class_declaration",
            "interface_declaration",
            "trait_declaration",
            "function_definition",
            "enum_declaration",
        ],
        SupportedLanguage::Ruby => &["method", "class", "module"],
        SupportedLanguage::Swift => &[
            "class_declaration",
            "struct_declaration",
            "enum_declaration",
            "protocol_declaration",
            "function_declaration",
            "typealias_declaration",
        ],
        SupportedLanguage::Kotlin => &[
            "class_declaration",
            "object_declaration",
            "function_declaration",
            "type_alias",
            "property_declaration",
        ],
        SupportedLanguage::Bash => &["function_definition"],
        SupportedLanguage::Json
        | SupportedLanguage::Yaml
        | SupportedLanguage::Toml
        | SupportedLanguage::Html
        | SupportedLanguage::Css => &[],
    }
}

fn find_named_descendant_text(source: &str, node: Node<'_>) -> Option<String> {
    let named_child_count = node.named_child_count();
    for index in 0..named_child_count {
        let Some(child) = node.named_child(index as u32) else {
            continue;
        };
        let kind = child.kind();

        if matches!(
            kind,
            "identifier" | "type_identifier" | "property_identifier" | "name" | "constant"
        ) {
            if let Ok(text) = child.utf8_text(source.as_bytes()) {
                return Some(text.to_string());
            }
        }

        if let Some(text) = find_named_descendant_text(source, child) {
            return Some(text);
        }
    }

    None
}

use super::language::SupportedLanguage;
use tree_sitter::{InputEdit, Language, Parser, Point, Tree};

pub fn parse_source(language: SupportedLanguage, source: &str) -> Result<Tree, String> {
    parse_source_with_old_tree(language, source, None)
}

pub fn parse_source_incrementally(
    language: SupportedLanguage,
    old_source: &str,
    new_source: &str,
    old_tree: &Tree,
) -> Result<Tree, String> {
    let mut edited_tree = old_tree.clone();
    edited_tree.edit(&input_edit(old_source, new_source));
    parse_source_with_old_tree(language, new_source, Some(&edited_tree))
}

fn parse_source_with_old_tree(
    language: SupportedLanguage,
    source: &str,
    old_tree: Option<&Tree>,
) -> Result<Tree, String> {
    let mut parser = Parser::new();
    parser
        .set_language(&language_grammar(language))
        .map_err(|error| format!("failed to load {} parser: {error}", language.as_str()))?;

    parser
        .parse(source, old_tree)
        .ok_or_else(|| format!("failed to parse {} source", language.as_str()))
}

fn input_edit(old_source: &str, new_source: &str) -> InputEdit {
    let old_bytes = old_source.as_bytes();
    let new_bytes = new_source.as_bytes();
    let mut start_byte = old_bytes
        .iter()
        .zip(new_bytes)
        .position(|(old, new)| old != new)
        .unwrap_or_else(|| old_bytes.len().min(new_bytes.len()));
    while !old_source.is_char_boundary(start_byte) || !new_source.is_char_boundary(start_byte) {
        start_byte -= 1;
    }
    let mut unchanged_suffix = old_bytes[start_byte..]
        .iter()
        .rev()
        .zip(new_bytes[start_byte..].iter().rev())
        .take_while(|(old, new)| old == new)
        .count();
    while !old_source.is_char_boundary(old_bytes.len() - unchanged_suffix)
        || !new_source.is_char_boundary(new_bytes.len() - unchanged_suffix)
    {
        unchanged_suffix -= 1;
    }
    let old_end_byte = old_bytes.len() - unchanged_suffix;
    let new_end_byte = new_bytes.len() - unchanged_suffix;

    InputEdit {
        start_byte,
        old_end_byte,
        new_end_byte,
        start_position: point_at(old_bytes, start_byte),
        old_end_position: point_at(old_bytes, old_end_byte),
        new_end_position: point_at(new_bytes, new_end_byte),
    }
}

fn point_at(source: &[u8], byte_offset: usize) -> Point {
    let prefix = &source[..byte_offset];
    let row = prefix.iter().filter(|byte| **byte == b'\n').count();
    let column = prefix
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or(prefix.len(), |newline| prefix.len() - newline - 1);
    Point::new(row, column)
}

pub fn language_grammar(language: SupportedLanguage) -> Language {
    match language {
        SupportedLanguage::JavaScript | SupportedLanguage::Jsx => {
            tree_sitter_javascript::LANGUAGE.into()
        }
        SupportedLanguage::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        SupportedLanguage::Tsx => tree_sitter_typescript::LANGUAGE_TSX.into(),
        SupportedLanguage::Rust => tree_sitter_rust::LANGUAGE.into(),
        SupportedLanguage::Python => tree_sitter_python::LANGUAGE.into(),
        SupportedLanguage::Go => tree_sitter_go::LANGUAGE.into(),
        SupportedLanguage::Java => tree_sitter_java::LANGUAGE.into(),
        SupportedLanguage::C => tree_sitter_c::LANGUAGE.into(),
        SupportedLanguage::Cpp => tree_sitter_cpp::LANGUAGE.into(),
        SupportedLanguage::CSharp => tree_sitter_c_sharp::LANGUAGE.into(),
        SupportedLanguage::Php => tree_sitter_php::LANGUAGE_PHP.into(),
        SupportedLanguage::Ruby => tree_sitter_ruby::LANGUAGE.into(),
        SupportedLanguage::Swift => tree_sitter_swift::LANGUAGE.into(),
        SupportedLanguage::Kotlin => tree_sitter_kotlin_ng::LANGUAGE.into(),
        SupportedLanguage::Bash => tree_sitter_bash::LANGUAGE.into(),
        SupportedLanguage::Json => tree_sitter_json::LANGUAGE.into(),
        SupportedLanguage::Yaml => tree_sitter_yaml::LANGUAGE.into(),
        SupportedLanguage::Toml => tree_sitter_toml_ng::LANGUAGE.into(),
        SupportedLanguage::Html => tree_sitter_html::LANGUAGE.into(),
        SupportedLanguage::Css => tree_sitter_css::LANGUAGE.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_source, parse_source_incrementally};
    use crate::indexer::language::SupportedLanguage;

    #[test]
    fn incremental_parse_matches_a_fresh_parse() {
        let old_source =
            "import { value } from './value';\nexport const label = '한글';\nexport const answer = value;\n";
        let new_source =
            "import { value } from './value';\nexport const label = '한국어';\nexport const answer = value + 1;\n";
        let old_tree = parse_source(SupportedLanguage::TypeScript, old_source).expect("old tree");
        let incremental = parse_source_incrementally(
            SupportedLanguage::TypeScript,
            old_source,
            new_source,
            &old_tree,
        )
        .expect("incremental tree");
        let fresh = parse_source(SupportedLanguage::TypeScript, new_source).expect("fresh tree");

        assert_eq!(
            incremental.root_node().to_sexp(),
            fresh.root_node().to_sexp()
        );
    }
}

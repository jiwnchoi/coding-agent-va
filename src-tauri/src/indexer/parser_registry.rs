use super::language::SupportedLanguage;
use tree_sitter::{Language, Parser, Tree};

pub fn parse_source(language: SupportedLanguage, source: &str) -> Result<Tree, String> {
    let mut parser = Parser::new();
    parser
        .set_language(&language_grammar(language))
        .map_err(|error| format!("failed to load {} parser: {error}", language.as_str()))?;

    parser
        .parse(source, None)
        .ok_or_else(|| format!("failed to parse {} source", language.as_str()))
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

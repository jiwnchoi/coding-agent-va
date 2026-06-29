use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum SupportedLanguage {
    JavaScript,
    Jsx,
    TypeScript,
    Tsx,
    Rust,
    Python,
    Go,
    Java,
    C,
    Cpp,
    CSharp,
    Php,
    Ruby,
    Swift,
    Kotlin,
    Bash,
    Json,
    Yaml,
    Toml,
    Html,
    Css,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct LanguageSupport {
    pub id: SupportedLanguage,
    pub display_name: &'static str,
    pub extensions: &'static [&'static str],
    pub file_names: &'static [&'static str],
}

const SUPPORTED_LANGUAGES: [LanguageSupport; 21] = [
    LanguageSupport {
        id: SupportedLanguage::JavaScript,
        display_name: "JavaScript",
        extensions: &["js", "mjs", "cjs"],
        file_names: &[],
    },
    LanguageSupport {
        id: SupportedLanguage::Jsx,
        display_name: "JSX",
        extensions: &["jsx"],
        file_names: &[],
    },
    LanguageSupport {
        id: SupportedLanguage::TypeScript,
        display_name: "TypeScript",
        extensions: &["ts", "mts", "cts"],
        file_names: &[],
    },
    LanguageSupport {
        id: SupportedLanguage::Tsx,
        display_name: "TSX",
        extensions: &["tsx"],
        file_names: &[],
    },
    LanguageSupport {
        id: SupportedLanguage::Rust,
        display_name: "Rust",
        extensions: &["rs"],
        file_names: &[],
    },
    LanguageSupport {
        id: SupportedLanguage::Python,
        display_name: "Python",
        extensions: &["py"],
        file_names: &[],
    },
    LanguageSupport {
        id: SupportedLanguage::Go,
        display_name: "Go",
        extensions: &["go"],
        file_names: &[],
    },
    LanguageSupport {
        id: SupportedLanguage::Java,
        display_name: "Java",
        extensions: &["java"],
        file_names: &[],
    },
    LanguageSupport {
        id: SupportedLanguage::C,
        display_name: "C",
        extensions: &["c", "h"],
        file_names: &[],
    },
    LanguageSupport {
        id: SupportedLanguage::Cpp,
        display_name: "C++",
        extensions: &["cc", "cpp", "cxx", "hh", "hpp", "hxx"],
        file_names: &[],
    },
    LanguageSupport {
        id: SupportedLanguage::CSharp,
        display_name: "C#",
        extensions: &["cs"],
        file_names: &[],
    },
    LanguageSupport {
        id: SupportedLanguage::Php,
        display_name: "PHP",
        extensions: &["php", "phtml"],
        file_names: &[],
    },
    LanguageSupport {
        id: SupportedLanguage::Ruby,
        display_name: "Ruby",
        extensions: &["rb"],
        file_names: &["Gemfile", "Rakefile"],
    },
    LanguageSupport {
        id: SupportedLanguage::Swift,
        display_name: "Swift",
        extensions: &["swift"],
        file_names: &[],
    },
    LanguageSupport {
        id: SupportedLanguage::Kotlin,
        display_name: "Kotlin",
        extensions: &["kt", "kts"],
        file_names: &[],
    },
    LanguageSupport {
        id: SupportedLanguage::Bash,
        display_name: "Bash",
        extensions: &["sh", "bash"],
        file_names: &[".bashrc", ".bash_profile", ".zshrc"],
    },
    LanguageSupport {
        id: SupportedLanguage::Json,
        display_name: "JSON",
        extensions: &["json"],
        file_names: &[],
    },
    LanguageSupport {
        id: SupportedLanguage::Yaml,
        display_name: "YAML",
        extensions: &["yaml", "yml"],
        file_names: &[],
    },
    LanguageSupport {
        id: SupportedLanguage::Toml,
        display_name: "TOML",
        extensions: &["toml"],
        file_names: &[],
    },
    LanguageSupport {
        id: SupportedLanguage::Html,
        display_name: "HTML",
        extensions: &["html", "htm"],
        file_names: &[],
    },
    LanguageSupport {
        id: SupportedLanguage::Css,
        display_name: "CSS",
        extensions: &["css"],
        file_names: &[],
    },
];

pub fn supported_language_snapshots() -> Vec<LanguageSupport> {
    SUPPORTED_LANGUAGES.to_vec()
}

pub fn detect_language(path: &Path) -> Option<SupportedLanguage> {
    let file_name = path.file_name()?.to_str()?;
    let extension = path.extension().and_then(|value| value.to_str());

    SUPPORTED_LANGUAGES.iter().find_map(|language| {
        if language.file_names.contains(&file_name) {
            return Some(language.id);
        }

        extension
            .filter(|value| language.extensions.contains(value))
            .map(|_| language.id)
    })
}

impl SupportedLanguage {
    pub fn as_str(self) -> &'static str {
        match self {
            SupportedLanguage::JavaScript => "javascript",
            SupportedLanguage::Jsx => "jsx",
            SupportedLanguage::TypeScript => "typescript",
            SupportedLanguage::Tsx => "tsx",
            SupportedLanguage::Rust => "rust",
            SupportedLanguage::Python => "python",
            SupportedLanguage::Go => "go",
            SupportedLanguage::Java => "java",
            SupportedLanguage::C => "c",
            SupportedLanguage::Cpp => "cpp",
            SupportedLanguage::CSharp => "csharp",
            SupportedLanguage::Php => "php",
            SupportedLanguage::Ruby => "ruby",
            SupportedLanguage::Swift => "swift",
            SupportedLanguage::Kotlin => "kotlin",
            SupportedLanguage::Bash => "bash",
            SupportedLanguage::Json => "json",
            SupportedLanguage::Yaml => "yaml",
            SupportedLanguage::Toml => "toml",
            SupportedLanguage::Html => "html",
            SupportedLanguage::Css => "css",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{detect_language, SupportedLanguage};
    use std::path::Path;

    #[test]
    fn detects_extensions_and_special_names() {
        assert_eq!(
            detect_language(Path::new("/tmp/example.tsx")),
            Some(SupportedLanguage::Tsx)
        );
        assert_eq!(
            detect_language(Path::new("/tmp/Gemfile")),
            Some(SupportedLanguage::Ruby)
        );
        assert_eq!(
            detect_language(Path::new("/tmp/.zshrc")),
            Some(SupportedLanguage::Bash)
        );
    }
}

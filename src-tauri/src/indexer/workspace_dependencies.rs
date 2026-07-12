use super::import_extractor::{extract_code_identifiers, extract_imports};
use super::language::{detect_language, SupportedLanguage};
use super::parser_registry::parse_source;
use super::symbol_extractor::{extract_declarations, extract_symbols};
use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
use std::fs;
use std::path::{Component, Path, PathBuf};
use walkdir::WalkDir;

#[cfg(test)]
pub fn find_impacted_files(
    workspace_root: &Path,
    changed_files: &[PathBuf],
) -> Result<Vec<String>, String> {
    let impacted_relations = find_impacted_file_relations(workspace_root, changed_files)?;

    Ok(impacted_relations
        .into_iter()
        .map(|relation| relation.impacted_file)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect())
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ImpactedFileRelation {
    pub changed_file: String,
    pub impacted_file: String,
    pub import_specifier: String,
}

#[derive(Debug)]
struct ImporterRelation {
    importer: PathBuf,
    import_specifier: String,
    statement: String,
    code_identifiers: HashSet<String>,
    forwards: bool,
}

#[derive(Clone, Debug)]
pub struct SessionFileEdit {
    pub path: PathBuf,
    pub fragments: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ChangedSymbols {
    exported: BTreeSet<String>,
    usages: BTreeSet<String>,
}

#[derive(Debug)]
struct ImportAlias {
    pattern_prefix: String,
    pattern_suffix: String,
    target_prefix: PathBuf,
    target_suffix: String,
}

#[cfg(test)]
pub fn find_impacted_file_relations(
    workspace_root: &Path,
    changed_files: &[PathBuf],
) -> Result<Vec<ImpactedFileRelation>, String> {
    let edits = changed_files
        .iter()
        .cloned()
        .map(|path| SessionFileEdit {
            path,
            fragments: Vec::new(),
        })
        .collect::<Vec<_>>();
    find_session_impacted_file_relations(workspace_root, &edits)
}

pub fn find_session_impacted_file_relations(
    workspace_root: &Path,
    edits: &[SessionFileEdit],
) -> Result<Vec<ImpactedFileRelation>, String> {
    if !workspace_root.exists() {
        return Ok(Vec::new());
    }

    let workspace_root = workspace_root
        .canonicalize()
        .map_err(|error| format!("failed to canonicalize workspace path: {error}"))?;
    let changed_files = edits
        .iter()
        .map(|edit| normalize_path(&edit.path, &workspace_root))
        .collect::<HashSet<_>>();
    let mut known_files = collect_known_files(&workspace_root)?;
    known_files.extend(changed_files.iter().cloned());
    let import_aliases = load_import_aliases(&workspace_root);

    let mut importers_by_dependency = HashMap::<PathBuf, Vec<ImporterRelation>>::new();

    for importer in &known_files {
        if !importer.is_file() {
            continue;
        }

        let Some(language) = detect_language(importer) else {
            continue;
        };
        let Ok(source) = fs::read_to_string(importer) else {
            continue;
        };
        let Ok(tree) = parse_source(language, &source) else {
            continue;
        };
        let code_identifiers = extract_code_identifiers(language, &source, tree.root_node());
        for import in extract_imports(language, &source, tree.root_node()) {
            for resolved in resolve_internal_import(
                importer,
                &import.specifier,
                language,
                &workspace_root,
                &known_files,
                &import_aliases,
            ) {
                importers_by_dependency
                    .entry(resolved)
                    .or_default()
                    .push(ImporterRelation {
                        importer: importer.clone(),
                        import_specifier: import.specifier.clone(),
                        statement: import.statement.clone(),
                        code_identifiers: code_identifiers.clone(),
                        forwards: import.forwards,
                    });
            }
        }
    }

    let mut impacted_relations = BTreeSet::<ImpactedFileRelation>::new();
    for edit in edits {
        let changed_file_path = normalize_path(&edit.path, &workspace_root);
        let Some(changed_file) = relative_workspace_path(&changed_file_path, &workspace_root)
        else {
            continue;
        };
        let changed_symbols = changed_symbols(&changed_file_path, &edit.fragments);
        let mut pending = VecDeque::from([(changed_file_path.clone(), changed_symbols)]);
        let mut visited_reexports = HashSet::new();

        while let Some((dependency, symbols)) = pending.pop_front() {
            let Some(importers) = importers_by_dependency.get(&dependency) else {
                continue;
            };
            for relation in importers {
                if changed_files.contains(&relation.importer) {
                    continue;
                }
                let referenced_symbols = symbols.as_ref().map(|symbols| {
                    let exported = referenced_names(&relation.statement, &symbols.exported);
                    let usages = if symbols.usages == symbols.exported {
                        exported.clone()
                    } else {
                        symbols.usages.clone()
                    };
                    ChangedSymbols { exported, usages }
                });
                if referenced_symbols
                    .as_ref()
                    .is_some_and(|symbols| symbols.exported.is_empty())
                {
                    continue;
                }
                let is_actual_use =
                    referenced_symbols
                        .as_ref()
                        .map_or(!relation.forwards, |symbols| {
                            symbols
                                .usages
                                .iter()
                                .any(|symbol| relation.code_identifiers.contains(symbol))
                        });
                if !is_actual_use {
                    let visit_key =
                        format!("{}:{:?}", relation.importer.display(), referenced_symbols);
                    if visited_reexports.insert(visit_key) {
                        pending.push_back((relation.importer.clone(), referenced_symbols));
                    }
                    continue;
                }
                let Some(impacted_file) =
                    relative_workspace_path(&relation.importer, &workspace_root)
                else {
                    continue;
                };
                impacted_relations.insert(ImpactedFileRelation {
                    changed_file: changed_file.clone(),
                    impacted_file,
                    import_specifier: relation.import_specifier.clone(),
                });
            }
        }
    }

    Ok(impacted_relations.into_iter().collect())
}

fn changed_symbols(path: &Path, fragments: &[String]) -> Option<ChangedSymbols> {
    if fragments.is_empty() || fragments.iter().any(String::is_empty) || !path.is_file() {
        return None;
    }
    let language = detect_language(path)?;
    let source = fs::read_to_string(path).ok()?;
    let tree = parse_source(language, &source).ok()?;
    let symbols = extract_declarations(language, &source, tree.root_node());
    let mut changed_ranges = Vec::new();
    for fragment in fragments {
        append_matching_ranges(&source, fragment, &mut changed_ranges);
        for line in fragment.lines().filter(|line| !line.trim().is_empty()) {
            append_matching_ranges(&source, line, &mut changed_ranges);
        }
    }
    let mut exported = BTreeSet::new();
    let mut usages = BTreeSet::new();
    for (start, end) in changed_ranges {
        let mut overlapping = symbols
            .iter()
            .filter(|symbol| start < symbol.end_byte && symbol.start_byte < end)
            .collect::<Vec<_>>();
        overlapping.sort_by_key(|symbol| symbol.end_byte - symbol.start_byte);
        let Some(first) = overlapping.first() else {
            continue;
        };
        let usage = overlapping
            .iter()
            .find(|symbol| is_callable_declaration(&symbol.kind))
            .copied()
            .unwrap_or(first);
        let exported_symbol = overlapping.last().unwrap_or(first);
        usages.insert(usage.name.clone());
        exported.insert(exported_symbol.name.clone());
    }
    if let Some(boundaries) = promote_private_top_level_changes(
        language,
        &source,
        &symbols,
        &extract_symbols(language, &source, tree.root_node()),
        &usages,
    ) {
        exported = boundaries.clone();
        usages = boundaries;
    }
    (!exported.is_empty()).then_some(ChangedSymbols { exported, usages })
}

fn promote_private_top_level_changes(
    language: SupportedLanguage,
    source: &str,
    declarations: &[super::symbol_extractor::ExtractedSymbol],
    top_level: &[super::symbol_extractor::ExtractedSymbol],
    changed_usages: &BTreeSet<String>,
) -> Option<BTreeSet<String>> {
    let changed_top_level = top_level
        .iter()
        .filter(|symbol| changed_usages.contains(&symbol.name))
        .collect::<Vec<_>>();
    if changed_top_level.is_empty()
        || changed_top_level
            .iter()
            .any(|symbol| is_public_declaration(language, source, symbol))
    {
        return None;
    }

    let mut pending = changed_top_level
        .into_iter()
        .map(|symbol| symbol.name.clone())
        .collect::<VecDeque<_>>();
    let mut visited = HashSet::new();
    let mut boundaries = BTreeSet::new();
    while let Some(changed_name) = pending.pop_front() {
        if !visited.insert(changed_name.clone()) {
            continue;
        }
        for caller in top_level {
            if caller.name == changed_name || !declaration_references(source, caller, &changed_name)
            {
                continue;
            }
            if is_public_declaration(language, source, caller) {
                boundaries.insert(caller.name.clone());
            } else {
                pending.push_back(caller.name.clone());
            }
        }
    }

    let declaration_names = declarations
        .iter()
        .map(|declaration| declaration.name.as_str())
        .collect::<HashSet<_>>();
    boundaries.retain(|boundary| declaration_names.contains(boundary.as_str()));
    Some(boundaries)
}

fn declaration_references(
    source: &str,
    declaration: &super::symbol_extractor::ExtractedSymbol,
    name: &str,
) -> bool {
    source
        .get(declaration.start_byte..declaration.end_byte)
        .is_some_and(|text| identifier_words(text).any(|word| word == name))
}

fn identifier_words(source: &str) -> impl Iterator<Item = &str> {
    source
        .split(|character: char| !(character.is_alphanumeric() || character == '_'))
        .filter(|word| !word.is_empty())
}

fn is_public_declaration(
    language: SupportedLanguage,
    source: &str,
    declaration: &super::symbol_extractor::ExtractedSymbol,
) -> bool {
    let line_start = source[..declaration.start_byte]
        .rfind('\n')
        .map_or(0, |index| index + 1);
    let prefix = &source[line_start..declaration.start_byte];
    match language {
        SupportedLanguage::JavaScript
        | SupportedLanguage::Jsx
        | SupportedLanguage::TypeScript
        | SupportedLanguage::Tsx => {
            identifier_words(prefix).any(|word| word == "export")
                || source.lines().any(|line| {
                    line.trim_start().starts_with("export {")
                        && identifier_words(line).any(|word| word == declaration.name)
                })
        }
        SupportedLanguage::Rust => identifier_words(prefix).any(|word| word == "pub"),
        _ => true,
    }
}

fn is_callable_declaration(kind: &str) -> bool {
    matches!(
        kind,
        "function_declaration"
            | "method_definition"
            | "function_item"
            | "function_definition"
            | "method_declaration"
            | "method"
    )
}

fn append_matching_ranges(source: &str, fragment: &str, ranges: &mut Vec<(usize, usize)>) {
    if fragment.is_empty() {
        return;
    }
    ranges.extend(
        source
            .match_indices(fragment)
            .map(|(start, value)| (start, start + value.len())),
    );
}

fn referenced_names(statement: &str, symbols: &BTreeSet<String>) -> BTreeSet<String> {
    let words = statement
        .split(|character: char| !(character.is_alphanumeric() || character == '_'))
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>();
    if let Some(alias_index) = words.iter().position(|word| *word == "as") {
        if words.first() == Some(&"import") && words.get(alias_index.wrapping_sub(1)) == Some(&"*")
        {
            return words
                .get(alias_index + 1)
                .map(|alias| BTreeSet::from([(*alias).to_string()]))
                .unwrap_or_default();
        }
    }
    let mut referenced = BTreeSet::new();
    for symbol in symbols {
        for (index, word) in words.iter().enumerate() {
            if *word != symbol {
                continue;
            }
            let name = if words.get(index + 1) == Some(&"as") {
                words.get(index + 2).copied().unwrap_or(word)
            } else {
                word
            };
            referenced.insert(name.to_string());
        }
    }
    if !referenced.is_empty() {
        for symbol in symbols {
            if !words.iter().any(|word| *word == symbol) {
                referenced.insert(symbol.clone());
            }
        }
    }
    if referenced.is_empty() && (statement.contains('*') || statement.trim_start().contains("mod "))
    {
        return symbols.clone();
    }
    referenced
}

fn collect_known_files(workspace_root: &Path) -> Result<HashSet<PathBuf>, String> {
    let mut known_files = HashSet::new();

    for entry in WalkDir::new(workspace_root)
        .into_iter()
        .filter_entry(|entry| !is_ignored(entry.path()))
    {
        let entry = entry.map_err(|error| format!("failed to walk workspace: {error}"))?;
        if entry.file_type().is_file() {
            known_files.insert(normalize_path(entry.path(), workspace_root));
        }
    }

    Ok(known_files)
}

fn resolve_internal_import(
    importer: &Path,
    specifier: &str,
    language: SupportedLanguage,
    workspace_root: &Path,
    known_files: &HashSet<PathBuf>,
    import_aliases: &[ImportAlias],
) -> Vec<PathBuf> {
    if specifier.is_empty() {
        return Vec::new();
    }

    if language == SupportedLanguage::Rust {
        return resolve_rust_import(importer, specifier, workspace_root, known_files);
    }

    let base_directory = importer.parent().unwrap_or(workspace_root);
    let base_path = if specifier.starts_with('/') {
        workspace_root.join(specifier.trim_start_matches('/'))
    } else if specifier.starts_with('.') {
        base_directory.join(specifier)
    } else if let Some(path) = resolve_import_alias(specifier, import_aliases) {
        path
    } else {
        return Vec::new();
    };

    candidate_paths(&base_path, language)
        .into_iter()
        .map(|candidate| normalize_path(&candidate, workspace_root))
        .filter(|candidate| known_files.contains(candidate))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn load_import_aliases(workspace_root: &Path) -> Vec<ImportAlias> {
    ["tsconfig.json", "jsconfig.json"]
        .into_iter()
        .filter_map(|name| fs::read_to_string(workspace_root.join(name)).ok())
        .filter_map(|source| serde_json::from_str::<serde_json::Value>(&source).ok())
        .flat_map(|config| {
            let compiler_options = config.get("compilerOptions").cloned().unwrap_or_default();
            let base_url = compiler_options
                .get("baseUrl")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(".");
            let base_path = workspace_root.join(base_url);
            compiler_options
                .get("paths")
                .and_then(serde_json::Value::as_object)
                .into_iter()
                .flat_map(move |paths| {
                    let base_path = base_path.clone();
                    paths.iter().flat_map(move |(pattern, targets)| {
                        let base_path = base_path.clone();
                        targets
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter_map(serde_json::Value::as_str)
                            .map(move |target| import_alias(pattern, target, &base_path))
                    })
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

fn import_alias(pattern: &str, target: &str, base_path: &Path) -> ImportAlias {
    let (pattern_prefix, pattern_suffix) = split_alias_pattern(pattern);
    let (target_prefix, target_suffix) = split_alias_pattern(target);
    ImportAlias {
        pattern_prefix: pattern_prefix.to_string(),
        pattern_suffix: pattern_suffix.to_string(),
        target_prefix: base_path.join(target_prefix),
        target_suffix: target_suffix.to_string(),
    }
}

fn split_alias_pattern(pattern: &str) -> (&str, &str) {
    pattern.split_once('*').unwrap_or((pattern, ""))
}

fn resolve_import_alias(specifier: &str, aliases: &[ImportAlias]) -> Option<PathBuf> {
    aliases.iter().find_map(|alias| {
        let remainder = specifier.strip_prefix(&alias.pattern_prefix)?;
        let wildcard = remainder.strip_suffix(&alias.pattern_suffix)?;
        Some(
            alias
                .target_prefix
                .join(format!("{wildcard}{}", alias.target_suffix)),
        )
    })
}

fn resolve_rust_import(
    importer: &Path,
    specifier: &str,
    workspace_root: &Path,
    known_files: &HashSet<PathBuf>,
) -> Vec<PathBuf> {
    let importer_directory = importer.parent().unwrap_or(workspace_root);
    let (base_directory, module_path) = if let Some(path) = specifier.strip_prefix("crate::") {
        (rust_source_root(importer, workspace_root), path)
    } else if let Some(path) = specifier.strip_prefix("self::") {
        (importer_directory.to_path_buf(), path)
    } else if specifier.starts_with("super::") {
        let mut base = importer_directory.to_path_buf();
        let mut path = specifier;
        let mut is_first_parent = true;
        while let Some(remainder) = path.strip_prefix("super::") {
            if is_first_parent {
                is_first_parent = false;
            } else {
                base.pop();
            }
            path = remainder;
        }
        (base, path)
    } else {
        (importer_directory.to_path_buf(), specifier)
    };

    let components = module_path.split("::").collect::<Vec<_>>();
    let mut resolved = BTreeSet::new();
    for component_count in (1..=components.len()).rev() {
        let base_path = components[..component_count]
            .iter()
            .fold(base_directory.clone(), |path, component| {
                path.join(component)
            });
        for candidate in [base_path.with_extension("rs"), base_path.join("mod.rs")] {
            let candidate = normalize_path(&candidate, workspace_root);
            if known_files.contains(&candidate) {
                resolved.insert(candidate);
            }
        }
        if !resolved.is_empty() {
            break;
        }
    }

    resolved.into_iter().collect()
}

fn rust_source_root(importer: &Path, workspace_root: &Path) -> PathBuf {
    importer
        .ancestors()
        .find(|ancestor| ancestor.join("Cargo.toml").is_file())
        .map(|crate_root| crate_root.join("src"))
        .unwrap_or_else(|| workspace_root.to_path_buf())
}

fn candidate_paths(base_path: &Path, language: SupportedLanguage) -> Vec<PathBuf> {
    let mut candidates = vec![base_path.to_path_buf()];

    if base_path.extension().is_none() {
        for extension in language_extensions(language) {
            candidates.push(base_path.with_extension(extension));
        }

        for index_name in index_file_names(language) {
            candidates.push(base_path.join(index_name));
        }
    }

    candidates
}

fn language_extensions(language: SupportedLanguage) -> &'static [&'static str] {
    match language {
        SupportedLanguage::JavaScript => &["js", "mjs", "cjs", "jsx", "ts", "tsx"],
        SupportedLanguage::Jsx => &["jsx", "js", "tsx", "ts"],
        SupportedLanguage::TypeScript => &["ts", "tsx", "js", "jsx", "mts", "cts"],
        SupportedLanguage::Tsx => &["tsx", "ts", "jsx", "js"],
        SupportedLanguage::Json => &["json"],
        SupportedLanguage::Css => &["css"],
        SupportedLanguage::Html => &["html", "htm"],
        SupportedLanguage::Python => &["py"],
        SupportedLanguage::Ruby => &["rb"],
        SupportedLanguage::Php => &["php", "phtml"],
        SupportedLanguage::Swift => &["swift"],
        SupportedLanguage::Kotlin => &["kt", "kts"],
        SupportedLanguage::Java => &["java"],
        SupportedLanguage::Go => &["go"],
        SupportedLanguage::Rust => &["rs"],
        SupportedLanguage::C => &["c", "h"],
        SupportedLanguage::Cpp => &["cc", "cpp", "cxx", "hh", "hpp", "hxx"],
        SupportedLanguage::CSharp => &["cs"],
        SupportedLanguage::Bash => &["sh", "bash"],
        SupportedLanguage::Yaml => &["yaml", "yml"],
        SupportedLanguage::Toml => &["toml"],
    }
}

fn index_file_names(language: SupportedLanguage) -> Vec<String> {
    language_extensions(language)
        .iter()
        .map(|extension| format!("index.{extension}"))
        .collect()
}

fn normalize_path(path: &Path, workspace_root: &Path) -> PathBuf {
    let absolute_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        workspace_root.join(path)
    };

    absolute_path
        .canonicalize()
        .unwrap_or_else(|_| normalize_missing_path(&absolute_path))
}

fn normalize_missing_path(path: &Path) -> PathBuf {
    let mut suffix = Vec::new();
    let mut existing_ancestor = Some(path);

    while let Some(ancestor) = existing_ancestor {
        if ancestor.exists() {
            let mut normalized = ancestor
                .canonicalize()
                .unwrap_or_else(|_| ancestor.to_path_buf());
            for component in suffix.iter().rev() {
                normalized.push(component);
            }
            return normalized;
        }

        if let Some(file_name) = ancestor.file_name() {
            suffix.push(file_name.to_os_string());
        }
        existing_ancestor = ancestor.parent();
    }

    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::RootDir | Component::Prefix(_) | Component::Normal(_) => {
                normalized.push(component.as_os_str());
            }
        }
    }
    normalized
}

fn relative_workspace_path(path: &Path, workspace_root: &Path) -> Option<String> {
    path.strip_prefix(workspace_root)
        .ok()
        .map(|path| path.display().to_string())
}

fn is_ignored(path: &Path) -> bool {
    path.components().any(|component| {
        matches!(
            component.as_os_str().to_str(),
            Some(".git" | "node_modules" | "target" | "dist" | "build" | ".next" | ".turbo")
        )
    })
}

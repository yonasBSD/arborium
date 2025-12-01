//! Configuration types and parsing for grammars

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use pulldown_cmark::{html, Parser};

/// Grammar configuration from GRAMMARS.toml
#[derive(Debug)]
#[allow(dead_code)]
pub struct GrammarConfig {
    pub name: String,
    pub repo: String,
    pub commit: String,
    pub license: String,
}

/// Configuration for generating a grammar crate
///
/// This can be auto-detected from the filesystem, or loaded from a
/// `grammar-crate-config.toml` file in the grammar directory for special cases.
#[derive(Debug)]
pub struct GrammarCrateConfig {
    /// Grammar name (e.g., "rust", "python")
    pub name: String,
    /// C function name suffix (e.g., "rust" for tree_sitter_rust)
    pub c_symbol: String,
    /// Source files to compile (relative to src/)
    pub source_files: Vec<String>,
    /// Whether highlights.scm exists
    pub has_highlights: bool,
    /// Whether injections.scm exists
    pub has_injections: bool,
    /// Whether locals.scm exists
    pub has_locals: bool,
    /// Query path prefix (for grammars with nested query directories)
    pub query_path: String,
    /// Additional languages exported by this grammar (e.g., "tsx" for typescript)
    pub extra_languages: Vec<(String, String)>, // (c_symbol, export_name)
    /// Sample files for testing (paths relative to crate root)
    #[allow(dead_code)]
    pub samples: Vec<String>,
    /// For sub-grammars: the parent repo name (e.g., "typescript" for tsx in tree-sitter-typescript)
    /// Used to find queries in grammars/tree-sitter-{parent_repo}/queries/ instead of tree-sitter-{name}
    pub parent_repo: Option<String>,
    /// Base languages whose queries should be included before this grammar's queries
    /// e.g., ["javascript"] for TypeScript means JavaScript queries are prepended
    #[allow(dead_code)]
    pub inherits_queries_from: Vec<String>,
}

/// Sample metadata parsed from info.toml
pub struct SampleInfo {
    pub path: Option<String>,
    pub description: Option<String>,
    pub link: Option<String>,
    pub license: Option<String>,
}

/// Language metadata parsed from info.toml for the demo
#[derive(Debug, Default)]
pub struct LanguageInfo {
    pub id: String,        // crate identifier, e.g., "cpp"
    pub name: String,      // pretty display name, e.g., "C++"
    pub tag: String,
    pub icon: Option<String>,
    pub aliases: Vec<String>,
    pub inventor: Option<String>,
    pub year: Option<u32>,
    pub description: Option<String>,
    pub link: Option<String>,
    pub trivia: Option<String>,
}

/// Parse GRAMMARS.toml
pub fn parse_grammars_toml(repo_root: &Path) -> Result<BTreeMap<String, GrammarConfig>, Box<dyn std::error::Error>> {
    let path = repo_root.join("GRAMMARS.toml");
    let contents = fs::read_to_string(&path)?;

    let mut grammars = BTreeMap::new();
    let mut current_name: Option<String> = None;
    let mut current_repo: Option<String> = None;
    let mut current_commit: Option<String> = None;
    let mut current_license: Option<String> = None;

    for line in contents.lines() {
        let line = line.trim();

        // Skip comments and empty lines
        if line.starts_with('#') || line.is_empty() {
            continue;
        }

        // Section header [name]
        if line.starts_with('[') && line.ends_with(']') {
            // Save previous grammar if any
            if let (Some(name), Some(repo), Some(commit), Some(license)) =
                (current_name.take(), current_repo.take(), current_commit.take(), current_license.take())
            {
                grammars.insert(name.clone(), GrammarConfig { name, repo, commit, license });
            }
            current_name = Some(line[1..line.len()-1].to_string());
            continue;
        }

        // Key = value
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            let value = value.trim().trim_matches('"');

            match key {
                "repo" => current_repo = Some(value.to_string()),
                "commit" => current_commit = Some(value.to_string()),
                "license" => current_license = Some(value.to_string()),
                _ => {}
            }
        }
    }

    // Save last grammar
    if let (Some(name), Some(repo), Some(commit), Some(license)) =
        (current_name, current_repo, current_commit, current_license)
    {
        grammars.insert(name.clone(), GrammarConfig { name, repo, commit, license });
    }

    Ok(grammars)
}

/// Parse grammar-crate-config.toml from a grammar directory
pub fn parse_grammar_crate_config(config_path: &Path) -> Option<toml::Value> {
    if config_path.exists() {
        eprintln!("  [DEBUG] Found config: {}", config_path.display());
    }
    let content = fs::read_to_string(config_path).ok()?;
    let parsed = content.parse::<toml::Value>().ok();
    if parsed.is_some() {
        eprintln!("  [DEBUG] Parsed config with parent_repo: {:?}",
            parsed.as_ref().and_then(|v| v.get("parent_repo")));
    }
    parsed
}

/// Detect grammar configuration from filesystem and optional config file
///
/// Looks for grammar-crate-config.toml in the grammar directory for overrides.
/// Falls back to auto-detection for any unspecified values.
pub fn detect_grammar_config(repo_root: &Path, grammar_dir: &Path, name: &str) -> GrammarCrateConfig {
    let src_dir = grammar_dir.join("src");

    // For sub-grammars, check parent directory for queries
    let queries_dir = if grammar_dir.join("queries").exists() {
        grammar_dir.join("queries")
    } else if let Some(parent) = grammar_dir.parent() {
        if parent.join("queries").exists() {
            parent.join("queries")
        } else {
            grammar_dir.join("queries") // will be detected as not existing
        }
    } else {
        grammar_dir.join("queries")
    };

    // Load config file if it exists
    let config_file = grammar_dir.join("grammar-crate-config.toml");
    eprintln!("  [DEBUG] Looking for config at: {} (exists: {})", config_file.display(), config_file.exists());
    let config = parse_grammar_crate_config(&config_file);

    // Extract values from config or use defaults
    let c_symbol = config
        .as_ref()
        .and_then(|c| c.get("c_symbol"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| name.replace('-', "_"));

    let query_path = config
        .as_ref()
        .and_then(|c| c.get("query_path"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_default();

    let parent_repo = config
        .as_ref()
        .and_then(|c| c.get("parent_repo"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let inherits_queries_from = config
        .as_ref()
        .and_then(|c| c.get("inherits_queries_from"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    let extra_languages = config
        .as_ref()
        .and_then(|c| c.get("extra_languages"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| {
                    let c_sym = v.get("c_symbol")?.as_str()?;
                    let export = v.get("export_name")?.as_str()?;
                    Some((c_sym.to_string(), export.to_string()))
                })
                .collect()
        })
        .unwrap_or_default();

    // Detect source files
    let mut source_files = vec!["parser.c".to_string()];
    if src_dir.join("scanner.c").exists() {
        source_files.push("scanner.c".to_string());
    }
    if src_dir.join("scanner.cc").exists() {
        source_files.push("scanner.cc".to_string());
    }

    // Detect queries - apply query_path prefix
    let query_base = if query_path.is_empty() {
        queries_dir.clone()
    } else {
        queries_dir.join(&query_path)
    };
    let has_highlights = query_base.join("highlights.scm").exists();
    let has_injections = query_base.join("injections.scm").exists();
    let has_locals = query_base.join("locals.scm").exists();

    // Read samples from info.toml if it exists
    let crate_dir = repo_root.join("crates").join(format!("arborium-{}", name));
    let info_toml = crate_dir.join("info.toml");
    let samples = if info_toml.exists() {
        parse_samples_from_info_toml(&info_toml)
    } else {
        vec![]
    };

    GrammarCrateConfig {
        name: name.to_string(),
        c_symbol,
        source_files,
        has_highlights,
        has_injections,
        has_locals,
        query_path,
        extra_languages,
        samples,
        parent_repo,
        inherits_queries_from,
    }
}

/// Parse [[samples]] entries from info.toml
pub fn parse_samples_from_info_toml(path: &Path) -> Vec<String> {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return vec![],
    };

    let mut samples = Vec::new();
    let mut in_samples_block = false;

    for line in content.lines() {
        let line = line.trim();

        if line == "[[samples]]" {
            in_samples_block = true;
            continue;
        }

        if in_samples_block && line.starts_with("path") {
            if let Some(value) = line.split('=').nth(1) {
                let value = value.split('#').next().unwrap_or(value);
                let value = value.trim().trim_matches('"').trim_matches('\'');
                if !value.is_empty() {
                    samples.push(value.to_string());
                }
            }
            in_samples_block = false;
        }
    }

    samples
}

/// Parse the first [[samples]] entry from info.toml
pub fn parse_sample_info(content: &str) -> Option<SampleInfo> {
    let mut in_samples_block = false;
    let mut path = None;
    let mut description = None;
    let mut link = None;
    let mut license = None;

    for line in content.lines() {
        let line = line.trim();

        if line == "[[samples]]" {
            if in_samples_block {
                // Already found first sample, stop
                break;
            }
            in_samples_block = true;
            continue;
        }

        if in_samples_block {
            if let Some(value) = extract_toml_string(line, "path") {
                path = Some(value);
            } else if let Some(value) = extract_toml_string(line, "description") {
                description = Some(value);
            } else if let Some(value) = extract_toml_string(line, "link") {
                link = Some(value);
            } else if let Some(value) = extract_toml_string(line, "license") {
                license = Some(value);
            }
        }
    }

    // Only return if we found a path
    if path.is_some() {
        Some(SampleInfo { path, description, link, license })
    } else {
        None
    }
}

/// Convert SampleInfo to JSON for the demo (without path)
pub fn sample_info_to_json(info: &SampleInfo) -> Option<serde_json::Value> {
    if info.description.is_none() && info.link.is_none() {
        return None;
    }
    let mut obj = serde_json::Map::new();
    if let Some(d) = &info.description {
        obj.insert("description".to_string(), serde_json::Value::String(d.clone()));
    }
    if let Some(l) = &info.link {
        obj.insert("link".to_string(), serde_json::Value::String(l.clone()));
    }
    if let Some(lic) = &info.license {
        obj.insert("license".to_string(), serde_json::Value::String(lic.clone()));
    }
    Some(serde_json::Value::Object(obj))
}

/// Render inline Markdown to HTML, stripping the outer <p> tags
pub fn render_markdown_inline(markdown: &str) -> String {
    let parser = Parser::new(markdown);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    // Strip outer <p> tags for inline use
    let trimmed = html_output.trim();
    if trimmed.starts_with("<p>") && trimmed.ends_with("</p>") {
        trimmed[3..trimmed.len() - 4].to_string()
    } else {
        trimmed.to_string()
    }
}

/// Helper to extract a string value from a TOML line like `key = "value"`
pub fn extract_toml_string(line: &str, key: &str) -> Option<String> {
    if line.starts_with(key) {
        if let Some(value) = line.split('=').nth(1) {
            let value = value.split('#').next().unwrap_or(value); // strip comments
            let value = value.trim().trim_matches('"').trim_matches('\'');
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

/// Parse LanguageInfo from info.toml content
pub fn parse_language_info(content: &str) -> Option<LanguageInfo> {
    let mut info = LanguageInfo::default();

    for line in content.lines() {
        let line = line.trim();

        // Stop when we hit samples section
        if line.starts_with("[[") {
            break;
        }

        if let Some(value) = extract_toml_string(line, "id") {
            info.id = value;
        } else if let Some(value) = extract_toml_string(line, "name") {
            info.name = value;
        } else if let Some(value) = extract_toml_string(line, "tag") {
            info.tag = value;
        } else if let Some(value) = extract_toml_string(line, "icon") {
            info.icon = Some(value);
        } else if let Some(value) = extract_toml_string(line, "inventor") {
            info.inventor = Some(value);
        } else if let Some(value) = extract_toml_string(line, "description") {
            info.description = Some(value);
        } else if let Some(value) = extract_toml_string(line, "link") {
            info.link = Some(value);
        } else if let Some(value) = extract_toml_string(line, "trivia") {
            info.trivia = Some(value);
        } else if line.starts_with("year") {
            if let Some(value) = line.split('=').nth(1) {
                let value = value.split('#').next().unwrap_or(value).trim();
                if let Ok(year) = value.parse::<u32>() {
                    info.year = Some(year);
                }
            }
        } else if line.starts_with("aliases") {
            // Parse aliases = ["a", "b", "c"]
            if let Some(value) = line.split('=').nth(1) {
                let value = value.trim();
                if value.starts_with('[') && value.ends_with(']') {
                    let inner = &value[1..value.len()-1];
                    info.aliases = inner
                        .split(',')
                        .map(|s| s.trim().trim_matches('"').trim_matches('\'').to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                }
            }
        }
    }

    if info.name.is_empty() {
        None
    } else {
        Some(info)
    }
}

/// Convert LanguageInfo to JSON for the demo
pub fn language_info_to_json(info: &LanguageInfo, sample: Option<&SampleInfo>) -> serde_json::Value {
    let mut obj = serde_json::Map::new();

    obj.insert("id".to_string(), serde_json::Value::String(info.id.clone()));
    obj.insert("name".to_string(), serde_json::Value::String(info.name.clone()));
    obj.insert("tag".to_string(), serde_json::Value::String(info.tag.clone()));

    if let Some(icon) = &info.icon {
        obj.insert("icon".to_string(), serde_json::Value::String(icon.clone()));
    }

    if !info.aliases.is_empty() {
        let aliases: Vec<serde_json::Value> = info.aliases.iter()
            .map(|a| serde_json::Value::String(a.clone()))
            .collect();
        obj.insert("aliases".to_string(), serde_json::Value::Array(aliases));
    }

    if let Some(inventor) = &info.inventor {
        obj.insert("inventor".to_string(), serde_json::Value::String(inventor.clone()));
    }

    if let Some(year) = info.year {
        obj.insert("year".to_string(), serde_json::Value::Number(year.into()));
    }

    if let Some(description) = &info.description {
        let html = render_markdown_inline(description);
        obj.insert("description".to_string(), serde_json::Value::String(html));
    }

    if let Some(link) = &info.link {
        obj.insert("url".to_string(), serde_json::Value::String(link.clone()));
    }

    if let Some(trivia) = &info.trivia {
        let html = render_markdown_inline(trivia);
        obj.insert("trivia".to_string(), serde_json::Value::String(html));
    }

    // Add sample metadata if available
    if let Some(sample) = sample {
        if let Some(json) = sample_info_to_json(sample) {
            obj.insert("sample".to_string(), json);
        }
    }

    serde_json::Value::Object(obj)
}

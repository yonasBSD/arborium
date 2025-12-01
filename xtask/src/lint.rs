//! Linting for info.toml files

use std::collections::HashSet;
use std::fs;
use std::path::Path;

use owo_colors::OwoColorize;

use crate::util::find_repo_root;

/// Required fields for info.toml
const REQUIRED_FIELDS: &[&str] = &[
    "name",
    "repo",
    "commit",
    "license",
    "description",
];

/// Recommended fields for info.toml
const RECOMMENDED_FIELDS: &[&str] = &[
    "inventor",
    "year",
    "link",
    "trivia",
    "handpicked",
];

/// Sample required fields
const SAMPLE_REQUIRED_FIELDS: &[&str] = &[
    "path",
    "description",
    "link",
    "license",
];

/// Crates that don't need info.toml (internal/utility crates, sub-grammars)
const SKIP_CRATES: &[&str] = &[
    "sysroot",
    "test-harness",
    "yuri",
    // Sub-grammars that are part of a parent grammar
    "asciidoc_inline",
    "markdown-inline",
];

/// Result of linting a single info.toml file
#[derive(Default)]
struct LintResult {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl LintResult {
    fn has_issues(&self) -> bool {
        !self.errors.is_empty() || !self.warnings.is_empty()
    }
}

/// Lint all info.toml files in the crates directory
pub fn lint_info_toml() {
    let repo_root = find_repo_root().expect("Could not find repo root");
    let crates_dir = repo_root.join("crates");

    println!("{}", "Linting info.toml files...".cyan().bold());
    println!();

    let mut total_errors = 0;
    let mut total_warnings = 0;
    let mut crates_checked = 0;
    let mut crates_missing_info = Vec::new();

    // Find all arborium-* crates
    let mut entries: Vec<_> = fs::read_dir(&crates_dir)
        .expect("Could not read crates directory")
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            name.starts_with("arborium-") && e.path().is_dir()
        })
        .collect();

    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let crate_name = entry.file_name().to_string_lossy().to_string();
        let lang_name = crate_name.strip_prefix("arborium-").unwrap_or(&crate_name);

        // Skip internal/utility crates
        if SKIP_CRATES.contains(&lang_name) {
            continue;
        }

        let info_path = entry.path().join("info.toml");

        if !info_path.exists() {
            crates_missing_info.push(lang_name.to_string());
            continue;
        }

        crates_checked += 1;
        let result = lint_single_info_toml(&info_path, lang_name);

        if result.has_issues() {
            println!("{} {}", "●".yellow(), lang_name.bold());

            for error in &result.errors {
                println!("  {} {}", "error:".red().bold(), error);
                total_errors += 1;
            }

            for warning in &result.warnings {
                println!("  {} {}", "warning:".yellow(), warning);
                total_warnings += 1;
            }

            println!();
        }
    }

    // Summary
    println!("{}", "─".repeat(60));
    println!();

    if !crates_missing_info.is_empty() {
        println!("{} {} crate(s) missing info.toml:",
            "✗".red(),
            crates_missing_info.len());
        for name in &crates_missing_info {
            println!("  {} {}", "error:".red().bold(), format!("crates/arborium-{}/info.toml not found", name));
        }
        println!();
        total_errors += crates_missing_info.len();
    }

    println!("Checked {} crate(s)", crates_checked + crates_missing_info.len());

    if total_errors > 0 {
        println!("{} {} error(s)", "✗".red(), total_errors);
    }
    if total_warnings > 0 {
        println!("{} {} warning(s)", "⚠".yellow(), total_warnings);
    }
    if total_errors == 0 && total_warnings == 0 {
        println!("{} All info.toml files are valid!", "✓".green());
    }

    if total_errors > 0 {
        std::process::exit(1);
    }
}

/// Lint a single info.toml file
fn lint_single_info_toml(path: &Path, lang_name: &str) -> LintResult {
    let mut result = LintResult::default();

    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            result.errors.push(format!("Could not read file: {}", e));
            return result;
        }
    };

    // Parse the TOML content
    let parsed: Result<toml::Value, _> = content.parse();
    let table = match parsed {
        Ok(toml::Value::Table(t)) => t,
        Ok(_) => {
            result.errors.push("info.toml root must be a table".to_string());
            return result;
        }
        Err(e) => {
            result.errors.push(format!("Invalid TOML: {}", e));
            return result;
        }
    };

    // Collect all keys for checking unknown fields
    let known_fields: HashSet<&str> = [
        "name", "repo", "commit", "license", "description",
        "inventor", "year", "link", "trivia", "handpicked",
        "tag", "icon", "tier", "aliases", "samples", "subdir",
        "wikipedia", // deprecated but still recognized
    ].into_iter().collect();

    // Check for unknown top-level fields
    for key in table.keys() {
        if !known_fields.contains(key.as_str()) {
            result.warnings.push(format!("Unknown field: {}", key));
        }
    }

    // Check required fields
    for field in REQUIRED_FIELDS {
        if !table.contains_key(*field) {
            result.errors.push(format!("Missing required field: {}", field));
        } else if let Some(val) = table.get(*field) {
            if val.as_str().is_some_and(|s| s.is_empty()) {
                result.errors.push(format!("Required field '{}' is empty", field));
            }
        }
    }

    // Check recommended fields
    for field in RECOMMENDED_FIELDS {
        if !table.contains_key(*field) {
            result.warnings.push(format!("Missing recommended field: {}", field));
        } else if let Some(val) = table.get(*field) {
            if val.as_str().is_some_and(|s| s.is_empty()) {
                result.warnings.push(format!("Recommended field '{}' is empty", field));
            }
        }
    }

    // Validate field types
    if let Some(name) = table.get("name") {
        if !name.is_str() {
            result.errors.push("'name' must be a string".to_string());
        }
    }

    if let Some(year) = table.get("year") {
        match year {
            toml::Value::Integer(y) => {
                if *y < 1940 || *y > 2030 {
                    result.warnings.push(format!("'year' looks suspicious: {}", y));
                }
            }
            _ => result.errors.push("'year' must be an integer".to_string()),
        }
    }

    if let Some(repo) = table.get("repo") {
        if let Some(s) = repo.as_str() {
            if s != "local" && !s.starts_with("https://") && !s.starts_with("http://") {
                result.warnings.push(format!("'repo' should be a URL or 'local': {}", s));
            }
        } else {
            result.errors.push("'repo' must be a string".to_string());
        }
    }

    if let Some(url) = table.get("url") {
        if let Some(s) = url.as_str() {
            if !s.starts_with("https://") && !s.starts_with("http://") {
                result.errors.push(format!("'url' must be a valid URL: {}", s));
            }
        } else {
            result.errors.push("'url' must be a string".to_string());
        }
    }

    // wikipedia is deprecated - use link instead
    if table.contains_key("wikipedia") {
        result.warnings.push("'wikipedia' is deprecated, use 'link' instead".to_string());
    }

    // Validate link field
    if let Some(link) = table.get("link") {
        if let Some(s) = link.as_str() {
            if !s.starts_with("https://") && !s.starts_with("http://") {
                result.errors.push(format!("'link' must be a valid URL: {}", s));
            }
        } else {
            result.errors.push("'link' must be a string".to_string());
        }
    }

    // Validate handpicked field
    if let Some(handpicked) = table.get("handpicked") {
        if !handpicked.is_bool() {
            result.errors.push("'handpicked' must be a boolean".to_string());
        }
    }

    if let Some(aliases) = table.get("aliases") {
        if !aliases.is_array() {
            result.errors.push("'aliases' must be an array".to_string());
        } else if let Some(arr) = aliases.as_array() {
            for (i, alias) in arr.iter().enumerate() {
                if !alias.is_str() {
                    result.errors.push(format!("'aliases[{}]' must be a string", i));
                }
            }
        }
    }

    if let Some(tag) = table.get("tag") {
        if let Some(s) = tag.as_str() {
            let valid_tags = ["code", "markup", "config", "data", "shell", "query", "build"];
            if !valid_tags.contains(&s) {
                result.warnings.push(format!("'tag' should be one of: {:?}", valid_tags));
            }
        } else {
            result.errors.push("'tag' must be a string".to_string());
        }
    }

    // Check samples array
    if let Some(samples) = table.get("samples") {
        if let Some(arr) = samples.as_array() {
            for (i, sample) in arr.iter().enumerate() {
                if let Some(sample_table) = sample.as_table() {
                    // Check sample required fields
                    for field in SAMPLE_REQUIRED_FIELDS {
                        if !sample_table.contains_key(*field) {
                            result.warnings.push(format!("samples[{}] missing field: {}", i, field));
                        }
                    }

                    // Check sample path exists
                    if let Some(path_val) = sample_table.get("path") {
                        if let Some(sample_path) = path_val.as_str() {
                            let full_path = path.parent().unwrap().join(sample_path);
                            if !full_path.exists() {
                                result.errors.push(format!("samples[{}].path does not exist: {}", i, sample_path));
                            }
                        }
                    }
                } else {
                    result.errors.push(format!("samples[{}] must be a table", i));
                }
            }
        } else {
            result.errors.push("'samples' must be an array of tables (use [[samples]])".to_string());
        }
    }

    // Check that name matches directory
    if let Some(name) = table.get("name").and_then(|v| v.as_str()) {
        let expected_name = lang_name.replace('-', "_");
        let actual_name = name.to_lowercase().replace('-', "_").replace(' ', "_");
        // Allow some flexibility - just warn if very different
        if !actual_name.contains(&expected_name) && !expected_name.contains(&actual_name) {
            result.warnings.push(format!(
                "'name' ({}) doesn't match crate name (arborium-{})",
                name, lang_name
            ));
        }
    }

    result
}

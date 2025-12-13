//! Test harness for arborium grammar crates.
//!
//! This crate provides utilities for testing tree-sitter grammars and their queries.
//!
//! # Usage
//!
//! In your grammar crate's lib.rs tests:
//!
//! ```ignore
//! #[cfg(test)]
//! mod tests {
//!     use super::*;
//!
//!     #[test]
//!     fn test_grammar() {
//!         arborium_test_harness::test_grammar(
//!             language(),
//!             "rust",
//!             HIGHLIGHTS_QUERY,
//!             INJECTIONS_QUERY,
//!             LOCALS_QUERY,
//!             env!("CARGO_MANIFEST_DIR"),
//!         );
//!     }
//! }
//! ```

pub use arborium_highlight;
pub use arborium_tree_sitter as tree_sitter;

use std::fs;
use std::path::Path;

use arborium_highlight::{CompiledGrammar, GrammarConfig, ParseContext};
use arborium_tree_sitter::Language;

// Re-export CAPTURE_NAMES from arborium-theme as HIGHLIGHT_NAMES for convenience
pub use arborium_theme::CAPTURE_NAMES as HIGHLIGHT_NAMES_FULL;

/// Tests a grammar by validating its queries and highlighting all samples.
///
/// This function:
/// 1. Validates that the queries compile correctly
/// 2. Finds sample files in the samples/ directory
/// 3. Highlights each sample file and verifies we get highlights
///
/// # Arguments
///
/// * `language` - The tree-sitter Language
/// * `name` - The grammar name (e.g., "rust")
/// * `highlights_query` - The highlights.scm content
/// * `injections_query` - The injections.scm content
/// * `locals_query` - The locals.scm content (currently unused by arborium-highlight)
/// * `crate_dir` - Path to the crate directory (use `env!("CARGO_MANIFEST_DIR")`)
///
/// # Panics
///
/// Panics if query validation fails, highlighting produces errors, or no highlights are found.
pub fn test_grammar(
    language: Language,
    name: &str,
    highlights_query: &str,
    injections_query: &str,
    _locals_query: &str,
    crate_dir: &str,
) {
    // Create grammar config
    let config = GrammarConfig {
        language,
        highlights_query,
        injections_query,
        locals_query: "", // Not used by arborium-highlight yet
    };

    // Validate queries compile by creating the grammar
    let grammar = CompiledGrammar::new(config).unwrap_or_else(|e| {
        panic!(
            "Query validation failed for {}: {:?}\n\
             This usually means highlights.scm references a node type that doesn't exist in the grammar.\n\
             Check the grammar's node-types.json to see valid node types.",
            name, e
        );
    });

    // Create a parse context for this grammar
    let mut ctx = ParseContext::for_grammar(&grammar).unwrap_or_else(|e| {
        panic!("Failed to create parse context for {}: {:?}", name, e);
    });

    // Find samples from arborium.kdl
    let crate_path = Path::new(crate_dir);
    let kdl_path = crate_path.join("arborium.kdl");
    let samples: Vec<_> = if kdl_path.exists() {
        parse_samples_from_kdl(&kdl_path)
            .into_iter()
            .map(|p| crate_path.join(p))
            .collect()
    } else {
        vec![]
    };

    if samples.is_empty() {
        // No samples - just verify query compiles (already done above)
        return;
    }

    // Test each sample - must produce at least one highlight
    for sample_path in &samples {
        let sample_code = fs::read_to_string(sample_path).unwrap_or_else(|e| {
            panic!(
                "Failed to read sample file {} for {}: {}",
                sample_path.display(),
                name,
                e
            );
        });

        // Parse with the grammar
        let result = grammar.parse(&mut ctx, &sample_code);

        // Count highlight spans
        let highlight_count = result.spans.len();

        // Verify we got highlights
        if highlight_count == 0 {
            panic!(
                "No highlights produced for {} in {}.\n\
                 Sample has {} bytes.\n\
                 This likely means the highlights.scm query doesn't match anything in the sample.",
                sample_path.display(),
                name,
                sample_code.len()
            );
        }
    }
}

/// Parse sample paths from arborium.kdl
///
/// Looks for `sample { path "..." }` blocks and extracts the path values.
fn parse_samples_from_kdl(path: &Path) -> Vec<String> {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return vec![],
    };

    let mut samples = Vec::new();
    let mut in_sample_block = false;
    let mut brace_depth = 0;

    for line in content.lines() {
        let trimmed = line.trim();

        // Track sample blocks
        if trimmed.starts_with("sample") && trimmed.contains('{') {
            in_sample_block = true;
            brace_depth = 1;
            continue;
        }

        if in_sample_block {
            // Track brace depth
            brace_depth += trimmed.matches('{').count();
            brace_depth = brace_depth.saturating_sub(trimmed.matches('}').count());

            if brace_depth == 0 {
                in_sample_block = false;
                continue;
            }

            // Look for path "..."
            if trimmed.starts_with("path")
                && let Some(start) = trimmed.find('"')
                && let Some(end) = trimmed[start + 1..].find('"')
            {
                let path_value = &trimmed[start + 1..start + 1 + end];
                if !path_value.is_empty() {
                    samples.push(path_value.to_string());
                }
            }
        }
    }

    samples
}

/// Standard highlight names used by arborium.
///
/// **Deprecated**: Use [`arborium_theme::CAPTURE_NAMES`] instead, which is the
/// canonical source of truth for all capture names.
///
/// This constant is kept for backwards compatibility.
pub const HIGHLIGHT_NAMES: &[&str] = arborium_theme::CAPTURE_NAMES;

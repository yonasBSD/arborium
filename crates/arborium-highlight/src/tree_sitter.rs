//! Tree-sitter based Grammar implementation.
//!
//! This module provides `TreeSitterGrammar` which implements the `Grammar` trait
//! using tree-sitter for parsing and highlighting queries.
//!
//! # Example
//!
//! ```rust,ignore
//! use arborium_highlight::tree_sitter::{TreeSitterGrammar, TreeSitterGrammarConfig};
//!
//! let config = TreeSitterGrammarConfig {
//!     language: tree_sitter_rust::LANGUAGE.into(),
//!     highlights_query: tree_sitter_rust::HIGHLIGHTS_QUERY,
//!     injections_query: tree_sitter_rust::INJECTIONS_QUERY,
//!     locals_query: tree_sitter_rust::LOCALS_QUERY,
//! };
//!
//! let mut grammar = TreeSitterGrammar::new(config).unwrap();
//! let result = grammar.parse("fn main() {}");
//! ```

use crate::types::{Injection, ParseResult, Span};
use crate::Grammar;
use streaming_iterator::StreamingIterator;
use tree_sitter_patched_arborium::{Language, Parser, Query, QueryCursor, Tree};

/// Configuration for creating a TreeSitterGrammar.
pub struct TreeSitterGrammarConfig {
    /// The tree-sitter Language
    pub language: Language,
    /// The highlights query (required for syntax highlighting)
    pub highlights_query: &'static str,
    /// The injections query (for embedded languages)
    pub injections_query: &'static str,
    /// The locals query (for local variable tracking)
    pub locals_query: &'static str,
}

/// Error when creating a TreeSitterGrammar
#[derive(Debug)]
pub enum TreeSitterGrammarError {
    /// Failed to set the parser language
    LanguageError,
    /// Failed to compile a query
    QueryError(String),
}

impl std::fmt::Display for TreeSitterGrammarError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TreeSitterGrammarError::LanguageError => write!(f, "Failed to set parser language"),
            TreeSitterGrammarError::QueryError(e) => write!(f, "Query compilation error: {}", e),
        }
    }
}

impl std::error::Error for TreeSitterGrammarError {}

/// A tree-sitter based grammar that implements the Grammar trait.
///
/// This parses text using tree-sitter and runs highlight/injection queries
/// to produce spans and injection points.
pub struct TreeSitterGrammar {
    parser: Parser,
    highlights_query: Query,
    injections_query: Option<Query>,
    query_cursor: QueryCursor,
    /// Cached tree from last parse (for incremental parsing in the future)
    last_tree: Option<Tree>,
}

impl TreeSitterGrammar {
    /// Create a new TreeSitterGrammar from configuration.
    pub fn new(config: TreeSitterGrammarConfig) -> Result<Self, TreeSitterGrammarError> {
        let mut parser = Parser::new();
        parser
            .set_language(&config.language)
            .map_err(|_| TreeSitterGrammarError::LanguageError)?;

        let highlights_query = Query::new(&config.language, config.highlights_query)
            .map_err(|e| TreeSitterGrammarError::QueryError(e.to_string()))?;

        let injections_query = if config.injections_query.is_empty() {
            None
        } else {
            Some(
                Query::new(&config.language, config.injections_query)
                    .map_err(|e| TreeSitterGrammarError::QueryError(e.to_string()))?,
            )
        };

        Ok(Self {
            parser,
            highlights_query,
            injections_query,
            query_cursor: QueryCursor::new(),
            last_tree: None,
        })
    }

    /// Get the injection content and language capture indices for the injection query.
    fn get_injection_capture_indices(&self) -> Option<(Option<u32>, Option<u32>)> {
        let query = self.injections_query.as_ref()?;
        let mut content_idx = None;
        let mut language_idx = None;

        for (i, name) in query.capture_names().iter().enumerate() {
            match *name {
                "injection.content" => content_idx = Some(i as u32),
                "injection.language" => language_idx = Some(i as u32),
                _ => {}
            }
        }

        Some((content_idx, language_idx))
    }
}

impl Grammar for TreeSitterGrammar {
    fn parse(&mut self, text: &str) -> ParseResult {
        // Parse the text
        let tree = match self.parser.parse(text, self.last_tree.as_ref()) {
            Some(tree) => tree,
            None => return ParseResult::default(),
        };

        let root_node = tree.root_node();
        let source = text.as_bytes();

        // Collect highlight spans
        let mut spans = Vec::new();

        let mut matches = self
            .query_cursor
            .matches(&self.highlights_query, root_node, source);

        while let Some(m) = matches.next() {
            for capture in m.captures {
                let capture_name = self.highlights_query.capture_names()[capture.index as usize];

                // Skip internal captures (start with _)
                if capture_name.starts_with('_') {
                    continue;
                }

                // Skip injection-related captures
                if capture_name.starts_with("injection.") {
                    continue;
                }

                let node = capture.node;
                spans.push(Span {
                    start: node.start_byte() as u32,
                    end: node.end_byte() as u32,
                    capture: capture_name.to_string(),
                });
            }
        }

        // Collect injections
        let mut injections = Vec::new();

        if let Some(ref injections_query) = self.injections_query {
            if let Some((content_idx, language_idx)) = self.get_injection_capture_indices() {
                let mut matches = self.query_cursor.matches(injections_query, root_node, source);

                while let Some(m) = matches.next() {
                    let mut content_node = None;
                    let mut language_name = None;
                    let mut include_children = false;

                    // Check for #set! injection.language property
                    for prop in injections_query.property_settings(m.pattern_index) {
                        match prop.key.as_ref() {
                            "injection.language" => {
                                if let Some(ref value) = prop.value {
                                    language_name = Some(value.to_string());
                                }
                            }
                            "injection.include-children" => {
                                include_children = true;
                            }
                            _ => {}
                        }
                    }

                    // Get captures
                    for capture in m.captures {
                        if Some(capture.index) == content_idx {
                            content_node = Some(capture.node);
                        } else if Some(capture.index) == language_idx {
                            // Language can come from captured text
                            if language_name.is_none() {
                                if let Ok(lang) = capture.node.utf8_text(source) {
                                    language_name = Some(lang.to_string());
                                }
                            }
                        }
                    }

                    if let (Some(node), Some(lang)) = (content_node, language_name) {
                        injections.push(Injection {
                            start: node.start_byte() as u32,
                            end: node.end_byte() as u32,
                            language: lang,
                            include_children,
                        });
                    }
                }
            }
        }

        // Cache the tree for potential future incremental parsing
        self.last_tree = Some(tree);

        ParseResult { spans, injections }
    }
}

#[cfg(test)]
mod tests {
    // Tests would go here but require actual tree-sitter grammars
}

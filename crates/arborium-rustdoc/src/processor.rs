//! Main processor that transforms rustdoc output directories.

use crate::css::generate_rustdoc_theme_css;
use crate::html::{TransformError, TransformResult, transform_html};
use arborium::Highlighter;
use fs_extra::dir::CopyOptions;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Options for the processor.
#[derive(Debug, Clone)]
pub struct ProcessOptions {
    /// Input directory containing rustdoc output.
    pub input_dir: PathBuf,
    /// Output directory (if None, modifies in place).
    pub output_dir: Option<PathBuf>,
    /// Whether to show verbose output.
    pub verbose: bool,
}

/// Statistics from processing.
#[derive(Debug, Default)]
pub struct ProcessorStats {
    /// Number of HTML files processed.
    pub files_processed: usize,
    /// Number of code blocks highlighted.
    pub blocks_highlighted: usize,
    /// Number of code blocks skipped.
    pub blocks_skipped: usize,
    /// CSS file that was modified.
    pub css_file_modified: Option<PathBuf>,
    /// Languages that were not supported.
    pub unsupported_languages: Vec<String>,
}

/// Processor for rustdoc output.
pub struct Processor {
    options: ProcessOptions,
}

impl Processor {
    /// Create a new processor with the given options.
    pub fn new(options: ProcessOptions) -> Self {
        Self { options }
    }

    /// Process the rustdoc output directory.
    pub fn process(&mut self) -> Result<ProcessorStats, ProcessError> {
        let mut stats = ProcessorStats::default();

        // Determine the actual output directory
        let output_dir = self
            .options
            .output_dir
            .as_ref()
            .unwrap_or(&self.options.input_dir);

        // If output_dir is different from input_dir, copy everything first
        if let Some(ref out) = self.options.output_dir
            && out != &self.options.input_dir
        {
            // Remove output directory if it exists (clean slate)
            if out.exists() {
                fs::remove_dir_all(out)?;
            }
            fs::create_dir_all(out)?;

            // List all items in the input directory
            let items: Vec<PathBuf> = fs::read_dir(&self.options.input_dir)?
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .collect();

            // Copy items directly into output (not nested)
            let options = CopyOptions::new().overwrite(true);
            fs_extra::copy_items(&items, out, &options)
                .map_err(|e| ProcessError::Io(std::io::Error::other(e.to_string())))?;
        }

        // Step 1: Find and patch the rustdoc CSS file
        if let Some(css_path) = self.find_and_patch_css(output_dir)? {
            stats.css_file_modified = Some(css_path);
        }

        // Step 2: Process all HTML files
        for entry in WalkDir::new(output_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "html"))
        {
            let path = entry.path();

            if self.options.verbose {
                eprintln!("Processing: {}", path.display());
            }

            match self.process_html_file(path) {
                Ok(result) => {
                    stats.files_processed += 1;
                    stats.blocks_highlighted += result.blocks_highlighted;
                    stats.blocks_skipped += result.blocks_skipped;

                    for lang in result.unsupported_languages {
                        if !stats.unsupported_languages.contains(&lang) {
                            stats.unsupported_languages.push(lang);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Warning: Failed to process {}: {}", path.display(), e);
                }
            }
        }

        Ok(stats)
    }

    /// Find the rustdoc CSS file and append arborium theme CSS.
    fn find_and_patch_css(&self, output_dir: &Path) -> Result<Option<PathBuf>, ProcessError> {
        let static_files = output_dir.join("static.files");

        if !static_files.exists() {
            return Err(ProcessError::CssPatch(format!(
                "static.files directory not found at {}. Is this a rustdoc output directory?",
                static_files.display()
            )));
        }

        // Find rustdoc-*.css file
        let css_file = fs::read_dir(&static_files)?
            .filter_map(|e| e.ok())
            .find(|e| {
                e.file_name()
                    .to_str()
                    .is_some_and(|n| n.starts_with("rustdoc-") && n.ends_with(".css"))
            })
            .map(|e| e.path());

        let Some(css_path) = css_file else {
            return Err(ProcessError::CssPatch(format!(
                "No rustdoc-*.css file found in {}",
                static_files.display()
            )));
        };

        // Read existing CSS
        let mut css_content = fs::read_to_string(&css_path)?;

        // Check if we've already patched it
        if css_content.contains("/* arborium syntax highlighting") {
            return Ok(Some(css_path));
        }

        // Generate and append arborium theme CSS
        let arborium_css = generate_rustdoc_theme_css();
        css_content.push_str(&arborium_css);

        // Write back
        fs::write(&css_path, css_content)?;

        Ok(Some(css_path))
    }

    /// Process a single HTML file.
    fn process_html_file(&self, path: &Path) -> Result<TransformResult, ProcessError> {
        let html = fs::read_to_string(path)?;

        // Create a fresh highlighter for each file (consumed by transform_html)
        let highlighter = Highlighter::new();
        let (transformed, result) = transform_html(&html, highlighter)?;

        // Only write if we actually changed something
        if result.blocks_highlighted > 0 {
            fs::write(path, transformed)?;
        }

        Ok(result)
    }
}

/// Errors that can occur during processing.
#[derive(Debug)]
pub enum ProcessError {
    /// IO error.
    Io(std::io::Error),
    /// HTML transformation error.
    Transform(TransformError),
    /// CSS patching error.
    CssPatch(String),
}

impl From<std::io::Error> for ProcessError {
    fn from(e: std::io::Error) -> Self {
        ProcessError::Io(e)
    }
}

impl From<TransformError> for ProcessError {
    fn from(e: TransformError) -> Self {
        ProcessError::Transform(e)
    }
}

impl std::fmt::Display for ProcessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProcessError::Io(e) => write!(f, "IO error: {}", e),
            ProcessError::Transform(e) => write!(f, "Transform error: {}", e),
            ProcessError::CssPatch(msg) => write!(f, "CSS patch error: {}", msg),
        }
    }
}

impl std::error::Error for ProcessError {}

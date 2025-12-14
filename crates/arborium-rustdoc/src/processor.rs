//! Main processor that transforms rustdoc output directories.

use crate::css::generate_rustdoc_theme_css;
use crate::html::{TransformError, TransformResult, transform_html};
use arborium::{GrammarStore, Highlighter};
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
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
    /// Total bytes read from input HTML files.
    pub bytes_input: u64,
    /// Total bytes written to output HTML files.
    pub bytes_output: u64,
    /// Time spent processing HTML files (excludes clone time).
    pub process_duration: Duration,
}

impl ProcessorStats {
    /// Calculate HTML inflation ratio (output / input).
    pub fn html_inflation_ratio(&self) -> f64 {
        if self.bytes_input == 0 {
            1.0
        } else {
            self.bytes_output as f64 / self.bytes_input as f64
        }
    }

    /// Calculate HTML inflation percentage ((output - input) / input * 100).
    pub fn html_inflation_percent(&self) -> f64 {
        if self.bytes_input == 0 {
            0.0
        } else {
            (self.bytes_output as f64 - self.bytes_input as f64) / self.bytes_input as f64 * 100.0
        }
    }

    /// Calculate processing throughput in MB/s (excludes clone time).
    pub fn throughput_mb_s(&self) -> f64 {
        let secs = self.process_duration.as_secs_f64();
        if secs == 0.0 {
            0.0
        } else {
            (self.bytes_input as f64 / (1024.0 * 1024.0)) / secs
        }
    }
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
        use std::time::Instant;

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

            // Show spinner while cloning
            let spinner = ProgressBar::new_spinner();
            spinner.set_style(
                ProgressStyle::default_spinner()
                    .template("{spinner:.green} {msg}")
                    .unwrap(),
            );
            spinner.set_message("Cloning directory tree...");
            spinner.enable_steady_tick(Duration::from_millis(80));

            // Use clonetree for fast copy-on-write cloning (instant on APFS)
            clonetree::clone_tree(&self.options.input_dir, out, &clonetree::Options::new())
                .map_err(|e| ProcessError::Io(std::io::Error::other(e.to_string())))?;

            spinner.finish_with_message("Clone complete");
        }

        // Step 1: Find and patch the rustdoc CSS file
        let css_file_modified = self.find_and_patch_css(output_dir)?;

        // Step 2: Collect all HTML files to process
        let html_files: Vec<PathBuf> = WalkDir::new(output_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "html"))
            .map(|e| e.path().to_path_buf())
            .collect();

        // Create a shared grammar store for all highlighters
        let store = Arc::new(GrammarStore::new());

        // Create progress bar for file processing
        let progress = ProgressBar::new(html_files.len() as u64);
        progress.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({per_sec})")
                .unwrap()
                .progress_chars("█▉▊▋▌▍▎▏  ")
        );

        let process_start = Instant::now();

        // Atomic counters for parallel aggregation
        let files_processed = AtomicUsize::new(0);
        let blocks_highlighted = AtomicUsize::new(0);
        let blocks_skipped = AtomicUsize::new(0);
        let bytes_input = AtomicUsize::new(0);
        let bytes_output = AtomicUsize::new(0);
        let unsupported_languages = Mutex::new(Vec::<String>::new());

        let verbose = self.options.verbose;

        // Process files in parallel using rayon
        // for_each_init creates one Highlighter per thread (not per file!)
        html_files.par_iter().for_each_init(
            || Highlighter::with_store(store.clone()),
            |highlighter, path| {
                if verbose {
                    eprintln!("Processing: {}", path.display());
                }

                match Self::process_html_file_with_highlighter(path, highlighter) {
                    Ok((result, input_size, output_size)) => {
                        files_processed.fetch_add(1, Ordering::Relaxed);
                        blocks_highlighted.fetch_add(result.blocks_highlighted, Ordering::Relaxed);
                        blocks_skipped.fetch_add(result.blocks_skipped, Ordering::Relaxed);
                        bytes_input.fetch_add(input_size, Ordering::Relaxed);
                        bytes_output.fetch_add(output_size, Ordering::Relaxed);

                        if !result.unsupported_languages.is_empty() {
                            let mut langs = unsupported_languages.lock().unwrap();
                            for lang in result.unsupported_languages {
                                if !langs.contains(&lang) {
                                    langs.push(lang);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        progress.println(format!(
                            "Warning: Failed to process {}: {}",
                            path.display(),
                            e
                        ));
                    }
                }
                progress.inc(1);
            },
        );

        let process_duration = process_start.elapsed();
        progress.finish_and_clear();

        Ok(ProcessorStats {
            files_processed: files_processed.load(Ordering::Relaxed),
            blocks_highlighted: blocks_highlighted.load(Ordering::Relaxed),
            blocks_skipped: blocks_skipped.load(Ordering::Relaxed),
            css_file_modified,
            unsupported_languages: unsupported_languages.into_inner().unwrap(),
            bytes_input: bytes_input.load(Ordering::Relaxed) as u64,
            bytes_output: bytes_output.load(Ordering::Relaxed) as u64,
            process_duration,
        })
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

    /// Process a single HTML file, returning (result, input_bytes, output_bytes).
    fn process_html_file_with_highlighter(
        path: &Path,
        highlighter: &mut Highlighter,
    ) -> Result<(TransformResult, usize, usize), ProcessError> {
        let html = fs::read_to_string(path)?;
        let input_size = html.len();

        // Quick check: skip lol_html parsing if there's no language- class at all
        // This is a fast substring check that avoids expensive HTML parsing for most files
        if !html.contains("language-") {
            return Ok((TransformResult::default(), input_size, input_size));
        }

        let (transformed, result) = transform_html(&html, highlighter)?;
        let output_size = transformed.len();

        // Only write if we actually changed something
        if result.blocks_highlighted > 0 {
            fs::write(path, &transformed)?;
        }

        Ok((result, input_size, output_size))
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

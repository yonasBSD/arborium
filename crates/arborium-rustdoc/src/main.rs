//! arborium-rustdoc CLI - Post-process rustdoc output with syntax highlighting.

use anyhow::{Result, bail};
use arborium_rustdoc::{ProcessOptions, Processor};
use facet::Facet;
use facet_args as args;
use owo_colors::OwoColorize;
use std::path::PathBuf;
use std::time::Instant;

/// Post-process rustdoc HTML output to add syntax highlighting for non-Rust code blocks.
///
/// This tool transforms rustdoc-generated documentation by adding tree-sitter based
/// syntax highlighting for code blocks in languages other than Rust.
#[derive(Debug, Facet)]
struct Args {
    /// Input directory containing rustdoc output (e.g., target/doc)
    #[facet(args::positional)]
    input: PathBuf,

    /// Output directory (defaults to modifying input in place)
    #[facet(args::positional, default)]
    output: Option<PathBuf>,

    /// Show verbose output
    #[facet(args::named, args::short = 'v', default)]
    verbose: bool,
}

/// Format a size difference as a human-readable string with appropriate unit.
fn format_size_diff(bytes: i64) -> String {
    let sign = if bytes >= 0 { "+" } else { "" };
    let abs_bytes = bytes.unsigned_abs();

    if abs_bytes >= 1024 * 1024 * 1024 {
        format!(
            "{}{:.2} GB",
            sign,
            bytes as f64 / (1024.0 * 1024.0 * 1024.0)
        )
    } else if abs_bytes >= 1024 * 1024 {
        format!("{}{:.2} MB", sign, bytes as f64 / (1024.0 * 1024.0))
    } else if abs_bytes >= 1024 {
        format!("{}{:.2} KB", sign, bytes as f64 / 1024.0)
    } else {
        format!("{}{} B", sign, bytes)
    }
}

fn main() -> Result<()> {
    let args: Args = facet_args::from_std_args()?;

    // Validate input directory
    if !args.input.exists() {
        bail!("Input directory does not exist: {}", args.input.display());
    }

    if !args.input.is_dir() {
        bail!("Input path is not a directory: {}", args.input.display());
    }

    // Create processor
    let options = ProcessOptions {
        input_dir: args.input.clone(),
        output_dir: args.output.clone(),
        verbose: args.verbose,
    };

    let mut processor = Processor::new(options);

    // Print header
    eprintln!(
        "{} Processing rustdoc output: {}",
        "arborium-rustdoc".green().bold(),
        args.input.display()
    );

    if let Some(out) = &args.output {
        eprintln!("  Output: {}", out.display());
    } else {
        eprintln!("  {} Modifying in place", "Note:".yellow());
    }

    eprintln!();

    // Process
    let start = Instant::now();
    let stats = processor.process()?;
    let elapsed = start.elapsed();

    // Print results
    eprintln!("{}", "Results:".bold());
    eprintln!(
        "  {} HTML files processed",
        stats.files_processed.to_string().cyan()
    );
    eprintln!(
        "  {} code blocks highlighted",
        stats.blocks_highlighted.to_string().green()
    );
    eprintln!(
        "  {} code blocks skipped (Rust or unsupported)",
        stats.blocks_skipped.to_string().yellow()
    );

    if let Some(ref css_path) = stats.css_file_modified {
        eprintln!("  {} CSS patched: {}", "✓".green(), css_path.display());
    }

    // Size statistics
    let input_mb = stats.bytes_input as f64 / (1024.0 * 1024.0);
    let output_mb = stats.bytes_output as f64 / (1024.0 * 1024.0);
    let inflation_pct = stats.html_inflation_percent();

    eprintln!();
    eprintln!("{}", "Size statistics:".bold());
    eprintln!("  Input:  {:.2} MB", input_mb);
    eprintln!("  Output: {:.2} MB", output_mb);
    let size_diff = stats.bytes_output as i64 - stats.bytes_input as i64;
    let size_diff_str = format_size_diff(size_diff);

    if inflation_pct >= 0.0 {
        eprintln!(
            "  HTML inflation: {} ({})",
            format!("+{:.2}%", inflation_pct).yellow(),
            size_diff_str.yellow()
        );
    } else {
        eprintln!(
            "  HTML deflation: {} ({})",
            format!("{:.2}%", inflation_pct).green(),
            size_diff_str.green()
        );
    }

    if !stats.unsupported_languages.is_empty() {
        eprintln!(
            "\n  {} Unsupported languages: {}",
            "Note:".yellow(),
            stats.unsupported_languages.join(", ")
        );
    }

    eprintln!(
        "\n  Completed in {:.2}s (processing: {:.2}s @ {:.1} MB/s)",
        elapsed.as_secs_f64(),
        stats.process_duration.as_secs_f64(),
        stats.throughput_mb_s()
    );

    Ok(())
}

//! Shared utilities for xtask commands

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use owo_colors::OwoColorize;

/// Find the repository root by looking for Cargo.toml with [workspace] and GRAMMARS.toml
pub fn find_repo_root() -> Option<PathBuf> {
    let cwd = env::current_dir().ok()?;
    let mut current = cwd.clone();

    loop {
        // Look for Cargo.toml with [workspace] and GRAMMARS.toml
        let cargo_toml = current.join("Cargo.toml");
        let grammars_toml = current.join("GRAMMARS.toml");
        if cargo_toml.exists() && grammars_toml.exists() {
            if let Ok(contents) = fs::read_to_string(&cargo_toml) {
                if contents.contains("[workspace]") {
                    return Some(current);
                }
            }
        }
        if !current.pop() {
            return None;
        }
    }
}

/// Execute a named step, exiting on error
pub fn step<F>(name: &str, f: F)
where
    F: FnOnce() -> Result<(), Box<dyn std::error::Error>>,
{
    println!("\n{} {}", "==>".cyan().bold(), name.bold());
    if let Err(e) = f() {
        eprintln!("{} {}", "Error:".red().bold(), e);
        std::process::exit(1);
    }
}

/// Check if a command exists in PATH
pub fn command_exists(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Format a byte count as a human-readable string
pub fn format_size(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

/// Escape a string for use in a JavaScript string literal
pub fn escape_for_js(s: &str) -> String {
    let mut result = String::with_capacity(s.len() + 32);
    for c in s.chars() {
        match c {
            '\\' => result.push_str("\\\\"),
            '"' => result.push_str("\\\""),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            // Handle special HTML sequences that can cause issues
            '<' => result.push_str("\\x3c"),
            '>' => result.push_str("\\x3e"),
            '&' => result.push_str("\\x26"),
            _ => result.push(c),
        }
    }
    result
}

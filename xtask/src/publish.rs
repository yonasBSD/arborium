//! Publishing to crates.io and npm.
//!
//! This module handles publishing arborium packages to both registries,
//! with proper handling for already-published versions.

use camino::{Utf8Path, Utf8PathBuf};
use miette::{Context, IntoDiagnostic, Result};
use owo_colors::OwoColorize;
use std::process::{Command, Stdio};

/// Publish all crates to crates.io.
///
/// Uses `cargo publish --workspace` which gracefully skips already-published versions.
pub fn publish_crates(repo_root: &Utf8Path, dry_run: bool) -> Result<()> {
    println!("{}", "Publishing to crates.io...".cyan().bold());

    let mut cmd = Command::new("cargo");
    cmd.arg("publish").arg("--workspace");

    if dry_run {
        cmd.arg("--dry-run");
        cmd.arg("--allow-dirty"); // Allow dirty for dry-run testing
        println!("{}", "  (dry run, --allow-dirty)".yellow());
    }

    cmd.current_dir(repo_root);
    cmd.stdout(Stdio::inherit());
    cmd.stderr(Stdio::inherit());

    let status = cmd
        .status()
        .into_diagnostic()
        .wrap_err("Failed to run cargo publish")?;

    if !status.success() {
        return Err(miette::miette!(
            "cargo publish failed with exit code {:?}",
            status.code()
        ));
    }

    println!("{} crates.io publish complete", "✓".green());
    Ok(())
}

/// Publish all npm packages.
///
/// Handles EPUBLISHCONFLICT gracefully by checking if versions exist first.
pub fn publish_npm(repo_root: &Utf8Path, plugins_dir: &Utf8Path, dry_run: bool) -> Result<()> {
    println!("{}", "Publishing to npm...".cyan().bold());

    if dry_run {
        println!("{}", "  (dry run)".yellow());
    }

    // Find all package directories (each should have a package.json)
    let packages = find_npm_packages(plugins_dir)?;

    if packages.is_empty() {
        println!("{} No npm packages found in {}", "!".yellow(), plugins_dir);
        return Ok(());
    }

    println!("  Found {} packages to publish", packages.len());

    let mut published = 0;
    let mut skipped = 0;
    let mut failed = 0;

    for package_dir in &packages {
        match publish_single_npm_package(package_dir, dry_run)? {
            NpmPublishResult::Published => published += 1,
            NpmPublishResult::AlreadyExists => skipped += 1,
            NpmPublishResult::Failed => failed += 1,
        }
    }

    // Also publish the main @arborium/arborium package if it exists
    let main_package = repo_root.join("packages/arborium");
    if main_package.exists() && main_package.join("package.json").exists() {
        println!("  Publishing main package @arborium/arborium...");
        match publish_single_npm_package(&main_package, dry_run)? {
            NpmPublishResult::Published => published += 1,
            NpmPublishResult::AlreadyExists => skipped += 1,
            NpmPublishResult::Failed => failed += 1,
        }
    }

    println!();
    if failed == 0 {
        println!(
            "{} npm publish complete: {} published, {} skipped (already exist), {} failed",
            "✓".green(),
            published,
            skipped,
            failed
        );
    } else {
        println!(
            "{} npm publish complete: {} published, {} skipped (already exist), {} failed",
            "!".yellow(),
            published,
            skipped,
            failed
        );
    }

    if failed > 0 {
        return Err(miette::miette!("{} packages failed to publish", failed));
    }

    Ok(())
}

/// Publish everything (crates.io + npm).
pub fn publish_all(repo_root: &Utf8Path, plugins_dir: &Utf8Path, dry_run: bool) -> Result<()> {
    // Publish to crates.io first
    publish_crates(repo_root, dry_run)?;

    println!();

    // Then publish to npm
    publish_npm(repo_root, plugins_dir, dry_run)?;

    println!();
    println!("{} All publishing complete!", "✓".green().bold());
    Ok(())
}

/// Result of attempting to publish a single npm package.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NpmPublishResult {
    Published,
    AlreadyExists,
    Failed,
}

/// Find all npm package directories in the plugins dir.
fn find_npm_packages(plugins_dir: &Utf8Path) -> Result<Vec<Utf8PathBuf>> {
    let mut packages = Vec::new();

    if !plugins_dir.exists() {
        return Ok(packages);
    }

    for entry in plugins_dir
        .read_dir_utf8()
        .into_diagnostic()
        .wrap_err_with(|| format!("Failed to read directory: {}", plugins_dir))?
    {
        let entry = entry.into_diagnostic()?;
        let path = entry.path();

        if path.is_dir() && path.join("package.json").exists() {
            packages.push(path.to_path_buf());
        }
    }

    packages.sort();
    Ok(packages)
}

/// Read package name and version from package.json.
fn read_package_info(package_dir: &Utf8Path) -> Result<(String, String)> {
    let package_json_path = package_dir.join("package.json");
    let content = fs_err::read_to_string(&package_json_path)
        .into_diagnostic()
        .wrap_err_with(|| format!("Failed to read {}", package_json_path))?;

    // Simple JSON parsing - extract name and version
    let name = extract_json_string(&content, "name")
        .ok_or_else(|| miette::miette!("No 'name' field in {}", package_json_path))?;
    let version = extract_json_string(&content, "version")
        .ok_or_else(|| miette::miette!("No 'version' field in {}", package_json_path))?;

    Ok((name, version))
}

/// Extract a string value from JSON (simple regex-based extraction).
fn extract_json_string(json: &str, key: &str) -> Option<String> {
    let pattern = format!(r#""{}"\s*:\s*"([^"]*)""#, regex::escape(key));
    let re = regex::Regex::new(&pattern).ok()?;
    re.captures(json)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_string())
}

/// Check if a package version already exists on npm.
fn npm_version_exists(package_name: &str, version: &str) -> Result<bool> {
    let output = Command::new("npm")
        .args(["view", &format!("{}@{}", package_name, version), "version"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .into_diagnostic()
        .wrap_err("Failed to run npm view")?;

    // If the command succeeds and outputs the version, it exists
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.trim() == version)
    } else {
        // Command failed - version doesn't exist (or other error, but we'll try to publish)
        Ok(false)
    }
}

/// Publish a single npm package.
fn publish_single_npm_package(package_dir: &Utf8Path, dry_run: bool) -> Result<NpmPublishResult> {
    let (name, version) = read_package_info(package_dir)?;

    print!("  {} {}@{}...", "→".blue(), name, version);

    // Check if version already exists
    if !dry_run {
        match npm_version_exists(&name, &version) {
            Ok(true) => {
                println!(" {}", "already exists, skipping".yellow());
                return Ok(NpmPublishResult::AlreadyExists);
            }
            Ok(false) => {
                // Continue to publish
            }
            Err(e) => {
                // Couldn't check, try to publish anyway
                eprintln!(" {} checking version: {}", "warning".yellow(), e);
            }
        }
    }

    if dry_run {
        println!(" {}", "would publish (dry run)".cyan());
        return Ok(NpmPublishResult::Published);
    }

    // Actually publish
    let output = Command::new("npm")
        .args(["publish", "--access", "public"])
        .current_dir(package_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .into_diagnostic()
        .wrap_err("Failed to run npm publish")?;

    if output.status.success() {
        println!(" {}", "published".green());
        return Ok(NpmPublishResult::Published);
    }

    // Check if it's EPUBLISHCONFLICT
    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.contains("EPUBLISHCONFLICT") || stderr.contains("cannot publish over existing") {
        println!(" {}", "already exists, skipping".yellow());
        return Ok(NpmPublishResult::AlreadyExists);
    }

    // Real error
    println!(" {}", "FAILED".red());
    eprintln!("    stderr: {}", stderr);
    Ok(NpmPublishResult::Failed)
}

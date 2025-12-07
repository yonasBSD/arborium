//! Publishing to crates.io and npm.
//!
//! This module handles publishing arborium packages to both registries,
//! with proper handling for already-published versions and dependency ordering.
//!
//! ## Crate Groups
//!
//! Crates are organized into groups for publishing:
//! - `pre`: Shared crates that grammars depend on (tree-sitter, sysroot, test-harness)
//! - Language groups (cedar, fern, birch, etc.): Grammar crates in `langs/group-*/`
//! - `post`: Umbrella crates that depend on grammars (arborium)

use camino::{Utf8Path, Utf8PathBuf};
use miette::{Context, IntoDiagnostic, Result};
use owo_colors::OwoColorize;
use std::process::{Command, Stdio};

/// Crates in the "pre" group - must be published before grammar crates.
/// These are shared dependencies that grammar crates rely on.
const PRE_CRATES: &[&str] = &[
    "tree-sitter",           // tree-sitter-patched-arborium
    "tree-sitter-highlight", // tree-sitter-highlight-patched-arborium
    "crates/arborium-sysroot",
    "crates/arborium-test-harness",
    "crates/arborium-theme", // Theme and highlight definitions
];

/// Crates in the "post" group - must be published after grammar crates.
/// These are umbrella crates that optionally depend on grammar crates.
const POST_CRATES: &[&str] = &["crates/arborium"];

/// Publish crates to crates.io.
///
/// If `group` is None, publishes everything in order: pre, then all groups, then post.
/// If `group` is Some("pre"), publishes only the pre crates.
/// If `group` is Some("post"), publishes only the post crates.
/// If `group` is Some(name), publishes only crates from that language group.
pub fn publish_crates(repo_root: &Utf8Path, group: Option<&str>, dry_run: bool) -> Result<()> {
    println!("{}", "Publishing to crates.io...".cyan().bold());

    if dry_run {
        println!("{}", "  (dry run)".yellow());
    }

    let langs_dir = repo_root.join("langs");

    match group {
        Some("pre") => {
            println!("  Publishing {} group", "pre".cyan());
            publish_crate_list(repo_root, PRE_CRATES, dry_run)?;
            print_crates_next_steps(&langs_dir, Some("pre"))?;
            Ok(())
        }
        Some("post") => {
            println!("  Publishing {} group", "post".cyan());
            publish_crate_list(repo_root, POST_CRATES, dry_run)?;
            print_crates_next_steps(&langs_dir, Some("post"))?;
            Ok(())
        }
        Some(group_name) => {
            println!("  Publishing {} group", group_name.cyan());
            let crates = find_group_crates(&langs_dir, group_name)?;
            if crates.is_empty() {
                println!(
                    "{} No crates found for group '{}' in {}",
                    "!".yellow(),
                    group_name,
                    langs_dir
                );
                return Ok(());
            }
            publish_crate_paths(&crates, dry_run)?;
            print_crates_next_steps(&langs_dir, Some(group_name))?;
            Ok(())
        }
        None => {
            // Publish everything in order
            println!("  Publishing all groups in order: pre → groups → post");
            println!();

            // 1. Pre crates
            println!("  {} Publishing {} group...", "●".cyan(), "pre".bold());
            publish_crate_list(repo_root, PRE_CRATES, dry_run)?;
            println!();

            // 2. All language groups
            let groups = find_all_groups(&langs_dir)?;
            for group_name in &groups {
                println!("  {} Publishing {} group...", "●".cyan(), group_name.bold());
                let crates = find_group_crates(&langs_dir, group_name)?;
                publish_crate_paths(&crates, dry_run)?;
                println!();
            }

            // 3. Post crates
            println!("  {} Publishing {} group...", "●".cyan(), "post".bold());
            publish_crate_list(repo_root, POST_CRATES, dry_run)?;

            println!();
            println!("{} All crates published!", "✓".green().bold());
            Ok(())
        }
    }
}

/// Print next steps hint for crates publishing.
fn print_crates_next_steps(langs_dir: &Utf8Path, current_group: Option<&str>) -> Result<()> {
    let all_groups = find_all_groups(langs_dir)?;

    println!();
    println!("{}", "Next steps:".bold());

    match current_group {
        Some("pre") => {
            // After pre, suggest language groups
            println!("  {} Continue with language groups:", "→".blue());
            for group in &all_groups {
                println!(
                    "      {} --group {}",
                    "cargo xtask publish crates".cyan(),
                    group
                );
            }
            println!(
                "  {} Then finish with: {} --group post",
                "→".blue(),
                "cargo xtask publish crates".cyan()
            );
        }
        Some("post") => {
            // After post, suggest npm
            println!(
                "  {} {} to publish npm packages",
                "→".blue(),
                "cargo xtask publish npm".cyan()
            );
        }
        Some(group_name) => {
            // After a language group, suggest next group or post
            let current_idx = all_groups.iter().position(|g| g == group_name);
            if let Some(idx) = current_idx {
                if idx + 1 < all_groups.len() {
                    println!(
                        "  {} {} --group {} (next language group)",
                        "→".blue(),
                        "cargo xtask publish crates".cyan(),
                        all_groups[idx + 1]
                    );
                } else {
                    println!(
                        "  {} {} --group post (all language groups done)",
                        "→".blue(),
                        "cargo xtask publish crates".cyan()
                    );
                }
            }
        }
        None => {
            // Published everything
            println!(
                "  {} {} to publish npm packages",
                "→".blue(),
                "cargo xtask publish npm".cyan()
            );
        }
    }

    Ok(())
}

/// Publish npm packages.
///
/// If `group` is None, publishes all npm packages.
/// If `group` is Some(name), publishes only packages from that language group.
pub fn publish_npm(
    repo_root: &Utf8Path,
    langs_dir: &Utf8Path,
    group: Option<&str>,
    dry_run: bool,
) -> Result<()> {
    println!("{}", "Publishing to npm...".cyan().bold());

    if dry_run {
        println!("{}", "  (dry run)".yellow());
    }

    let packages = match group {
        Some(group_name) => {
            println!("  Publishing {} group", group_name.cyan());
            find_group_npm_packages(langs_dir, group_name)?
        }
        None => find_npm_packages(langs_dir)?,
    };

    if packages.is_empty() {
        let location = group
            .map(|g| format!("group '{}'", g))
            .unwrap_or_else(|| langs_dir.to_string());
        println!("{} No npm packages found in {}", "!".yellow(), location);
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

    // Also publish the main @arborium/arborium package if it exists and no group filter
    if group.is_none() {
        let main_package = repo_root.join("packages/arborium");
        if main_package.exists() && main_package.join("package.json").exists() {
            println!("  Publishing main package @arborium/arborium...");
            match publish_single_npm_package(&main_package, dry_run)? {
                NpmPublishResult::Published => published += 1,
                NpmPublishResult::AlreadyExists => skipped += 1,
                NpmPublishResult::Failed => failed += 1,
            }
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
pub fn publish_all(repo_root: &Utf8Path, langs_dir: &Utf8Path, dry_run: bool) -> Result<()> {
    // Publish to crates.io first (all groups in order)
    publish_crates(repo_root, None, dry_run)?;

    println!();

    // Then publish to npm (all packages)
    publish_npm(repo_root, langs_dir, None, dry_run)?;

    println!();
    println!("{} All publishing complete!", "✓".green().bold());
    Ok(())
}

// =============================================================================
// Crate publishing helpers
// =============================================================================

/// Result of attempting to publish a single crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CratePublishResult {
    Published,
    AlreadyExists,
    Failed,
}

/// Publish a list of crates by their relative paths from repo root.
fn publish_crate_list(repo_root: &Utf8Path, crates: &[&str], dry_run: bool) -> Result<()> {
    let paths: Vec<Utf8PathBuf> = crates.iter().map(|c| repo_root.join(c)).collect();
    publish_crate_paths(&paths, dry_run)
}

/// Publish crates from a list of paths.
fn publish_crate_paths(crates: &[Utf8PathBuf], dry_run: bool) -> Result<()> {
    let mut published = 0;
    let mut skipped = 0;
    let mut failed = 0;

    for crate_dir in crates {
        match publish_single_crate(crate_dir, dry_run)? {
            CratePublishResult::Published => published += 1,
            CratePublishResult::AlreadyExists => skipped += 1,
            CratePublishResult::Failed => failed += 1,
        }
    }

    if failed > 0 {
        return Err(miette::miette!("{} crates failed to publish", failed));
    }

    println!(
        "    {} published, {} skipped, {} failed",
        published.to_string().green(),
        skipped.to_string().yellow(),
        failed.to_string().red()
    );

    Ok(())
}

/// Read crate name and version from Cargo.toml.
fn read_crate_info(crate_dir: &Utf8Path) -> Result<(String, String)> {
    let cargo_toml_path = crate_dir.join("Cargo.toml");
    let content = fs_err::read_to_string(&cargo_toml_path)
        .into_diagnostic()
        .wrap_err_with(|| format!("Failed to read {}", cargo_toml_path))?;

    // Simple TOML parsing - extract name and version
    let name = extract_toml_string(&content, "name")
        .ok_or_else(|| miette::miette!("No 'name' field in {}", cargo_toml_path))?;

    // Version might be "X.Y.Z" or { workspace = true }
    let version = if content.contains("version.workspace = true")
        || content.contains("version = { workspace = true }")
    {
        // Read from workspace - for now just use a placeholder that we'll check against crates.io
        "workspace".to_string()
    } else {
        extract_toml_string(&content, "version")
            .ok_or_else(|| miette::miette!("No 'version' field in {}", cargo_toml_path))?
    };

    Ok((name, version))
}

/// Extract a string value from TOML (simple regex-based extraction).
fn extract_toml_string(toml: &str, key: &str) -> Option<String> {
    let pattern = format!(r#"(?m)^{}\s*=\s*"([^"]*)""#, regex::escape(key));
    let re = regex::Regex::new(&pattern).ok()?;
    re.captures(toml)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_string())
}

/// Check if a crate version already exists on crates.io.
fn crate_version_exists(crate_name: &str, version: &str) -> Result<bool> {
    // For workspace versions, we can't easily check without parsing the root Cargo.toml
    if version == "workspace" {
        return Ok(false);
    }

    let output = Command::new("cargo")
        .args(["search", crate_name, "--limit", "1"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .into_diagnostic()
        .wrap_err("Failed to run cargo search")?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        // Output looks like: crate_name = "version" # description
        if stdout.contains(&format!("\"{}\"", version)) {
            return Ok(true);
        }
    }

    Ok(false)
}

/// Publish a single crate.
fn publish_single_crate(crate_dir: &Utf8Path, dry_run: bool) -> Result<CratePublishResult> {
    // Check if Cargo.toml exists
    if !crate_dir.join("Cargo.toml").exists() {
        println!(
            "  {} {} - {}",
            "→".blue(),
            crate_dir,
            "no Cargo.toml, skipping".yellow()
        );
        return Ok(CratePublishResult::AlreadyExists);
    }

    // Check if publish = false
    let cargo_toml = fs_err::read_to_string(crate_dir.join("Cargo.toml"))
        .into_diagnostic()
        .wrap_err_with(|| format!("Failed to read Cargo.toml in {}", crate_dir))?;
    if cargo_toml.contains("publish = false") {
        println!(
            "  {} {} - {}",
            "→".blue(),
            crate_dir.file_name().unwrap_or("?"),
            "publish = false, skipping".dimmed()
        );
        return Ok(CratePublishResult::AlreadyExists);
    }

    let (name, version) = read_crate_info(crate_dir)?;
    let display_version = if version == "workspace" {
        "workspace".dimmed().to_string()
    } else {
        version.clone()
    };

    print!("  {} {}@{}...", "→".blue(), name, display_version);

    // Check if version already exists (skip for workspace versions - cargo publish will handle it)
    if !dry_run && version != "workspace" {
        match crate_version_exists(&name, &version) {
            Ok(true) => {
                println!(" {}", "already exists, skipping".yellow());
                return Ok(CratePublishResult::AlreadyExists);
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
        return Ok(CratePublishResult::Published);
    }

    // Actually publish
    let output = Command::new("cargo")
        .args(["publish", "--allow-dirty"])
        .current_dir(crate_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .into_diagnostic()
        .wrap_err("Failed to run cargo publish")?;

    if output.status.success() {
        println!(" {}", "published".green());
        return Ok(CratePublishResult::Published);
    }

    // Check if it's an "already published" error
    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.contains("already uploaded")
        || stderr.contains("already exists")
        || stderr.contains("is already published")
    {
        println!(" {}", "already exists, skipping".yellow());
        return Ok(CratePublishResult::AlreadyExists);
    }

    // Real error
    println!(" {}", "FAILED".red());
    eprintln!("    stderr: {}", stderr);
    Ok(CratePublishResult::Failed)
}

/// Find all language group names in langs/.
fn find_all_groups(langs_dir: &Utf8Path) -> Result<Vec<String>> {
    let mut groups = Vec::new();

    if !langs_dir.exists() {
        return Ok(groups);
    }

    for entry in langs_dir
        .read_dir_utf8()
        .into_diagnostic()
        .wrap_err_with(|| format!("Failed to read directory: {}", langs_dir))?
    {
        let entry = entry.into_diagnostic()?;
        let path = entry.path();

        if !path.is_dir() {
            continue;
        }

        let name = path.file_name().unwrap_or("");
        if let Some(group_name) = name.strip_prefix("group-") {
            groups.push(group_name.to_string());
        }
    }

    groups.sort();
    Ok(groups)
}

/// Find all crates in a specific language group.
fn find_group_crates(langs_dir: &Utf8Path, group_name: &str) -> Result<Vec<Utf8PathBuf>> {
    let mut crates = Vec::new();

    let group_dir = langs_dir.join(format!("group-{}", group_name));
    if !group_dir.exists() {
        return Ok(crates);
    }

    for lang_entry in group_dir
        .read_dir_utf8()
        .into_diagnostic()
        .wrap_err_with(|| format!("Failed to read directory: {}", group_dir))?
    {
        let lang_entry = lang_entry.into_diagnostic()?;
        let lang_path = lang_entry.path();

        if !lang_path.is_dir() {
            continue;
        }

        // Check for crate/Cargo.toml
        let crate_dir = lang_path.join("crate");
        if crate_dir.is_dir() && crate_dir.join("Cargo.toml").exists() {
            crates.push(crate_dir);
        }
    }

    crates.sort();
    Ok(crates)
}

// =============================================================================
// NPM publishing helpers
// =============================================================================

/// Result of attempting to publish a single npm package.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NpmPublishResult {
    Published,
    AlreadyExists,
    Failed,
}

/// Find all npm package directories in the langs dir.
///
/// The layout is: langs/group-*/lang/npm/package.json
fn find_npm_packages(langs_dir: &Utf8Path) -> Result<Vec<Utf8PathBuf>> {
    let mut packages = Vec::new();

    if !langs_dir.exists() {
        return Ok(packages);
    }

    // Iterate over group directories (group-*)
    for group_entry in langs_dir
        .read_dir_utf8()
        .into_diagnostic()
        .wrap_err_with(|| format!("Failed to read directory: {}", langs_dir))?
    {
        let group_entry = group_entry.into_diagnostic()?;
        let group_path = group_entry.path();

        if !group_path.is_dir() {
            continue;
        }
        let group_name = group_path.file_name().unwrap_or("");
        if !group_name.starts_with("group-") {
            continue;
        }

        // Iterate over language directories within the group
        for lang_entry in group_path
            .read_dir_utf8()
            .into_diagnostic()
            .wrap_err_with(|| format!("Failed to read directory: {}", group_path))?
        {
            let lang_entry = lang_entry.into_diagnostic()?;
            let lang_path = lang_entry.path();

            if !lang_path.is_dir() {
                continue;
            }

            // Check for npm/package.json
            let npm_dir = lang_path.join("npm");
            if npm_dir.is_dir() && npm_dir.join("package.json").exists() {
                packages.push(npm_dir);
            }
        }
    }

    packages.sort();
    Ok(packages)
}

/// Find npm packages in a specific language group.
fn find_group_npm_packages(langs_dir: &Utf8Path, group_name: &str) -> Result<Vec<Utf8PathBuf>> {
    let mut packages = Vec::new();

    let group_dir = langs_dir.join(format!("group-{}", group_name));
    if !group_dir.exists() {
        return Ok(packages);
    }

    for lang_entry in group_dir
        .read_dir_utf8()
        .into_diagnostic()
        .wrap_err_with(|| format!("Failed to read directory: {}", group_dir))?
    {
        let lang_entry = lang_entry.into_diagnostic()?;
        let lang_path = lang_entry.path();

        if !lang_path.is_dir() {
            continue;
        }

        // Check for npm/package.json
        let npm_dir = lang_path.join("npm");
        if npm_dir.is_dir() && npm_dir.join("package.json").exists() {
            packages.push(npm_dir);
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

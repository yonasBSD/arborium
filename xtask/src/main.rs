//! xtask for arborium - development tasks
//!
//! Usage: cargo xtask <command>
//!
//! Commands:
//!   regenerate       Regenerate tree-sitter grammars and clean up vendored directories
//!   check-updates    Check if any vendored grammars have newer versions available
//!   vendor <name>    Update a specific grammar to the latest version
//!   generate-crates  Generate arborium-* crates for all vendored grammars
//!   generate-demo    Generate demo/index.html from template and example files
//!   generate-readme  Generate README.md from GRAMMARS.toml
//!   serve-demo       Build and serve the WASM demo locally

use std::collections::{BTreeMap, HashMap, HashSet};
use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use indicatif::{ProgressBar, ProgressStyle};
use owo_colors::OwoColorize;
use rayon::prelude::*;

/// Files to keep in each grammar directory (note: all .js and .mjs files are also kept)
const KEEP_FILES: &[&str] = &["grammar.js", "package.json", "LICENSE", "LICENSE.md", "COPYING.txt", "README.md"];

/// Directories to keep
const KEEP_DIRS: &[&str] = &["src", "queries", "grammar", "common", "rules", "lib"];

/// File extensions to keep in src/ (all .c, .h, .cc files needed for compilation)
const KEEP_SRC_EXTENSIONS: &[&str] = &[".c", ".cc", ".h", ".json"];

/// Directories to keep in src/
const KEEP_SRC_DIRS: &[&str] = &["tree_sitter"];

/// Grammar dependencies: (grammar_name, &[dependencies])
/// These are grammars that require other grammars to be generated first
const GRAMMAR_DEPS: &[(&str, &[&str])] = &[
    ("tree-sitter-cpp", &["tree-sitter-c"]),
    ("tree-sitter-scss", &["tree-sitter-css"]),
    ("tree-sitter-typescript", &["tree-sitter-javascript"]),
    ("tree-sitter-starlark", &["tree-sitter-python"]),
];

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_usage();
        std::process::exit(1);
    }

    match args[1].as_str() {
        "regenerate" => {
            let filter = args.get(2).map(|s| s.as_str());
            regenerate(filter);
        }
        "check-updates" => check_updates(),
        "vendor" => {
            if args.len() < 3 {
                eprintln!("Usage: cargo xtask vendor <grammar-name>");
                eprintln!("Example: cargo xtask vendor rust");
                std::process::exit(1);
            }
            vendor_grammar(&args[2]);
        }
        "generate-crates" => generate_grammar_crates(),
        "generate-demo" => generate_demo(),
        "generate-readme" => generate_readme(),
        "serve-demo" => {
            let mut addr = "127.0.0.1".to_string();
            let mut port: Option<u16> = None;
            let mut dev_mode = false;
            let mut i = 2;
            while i < args.len() {
                match args[i].as_str() {
                    "-a" | "--address" => {
                        if i + 1 < args.len() {
                            addr = args[i + 1].clone();
                            i += 2;
                        } else {
                            eprintln!("Error: -a requires an address argument");
                            std::process::exit(1);
                        }
                    }
                    "-p" | "--port" => {
                        if i + 1 < args.len() {
                            port = Some(args[i + 1].parse().unwrap_or_else(|_| {
                                eprintln!("Error: invalid port number");
                                std::process::exit(1);
                            }));
                            i += 2;
                        } else {
                            eprintln!("Error: -p requires a port argument");
                            std::process::exit(1);
                        }
                    }
                    "--dev" => {
                        dev_mode = true;
                        i += 1;
                    }
                    _ => {
                        eprintln!("Unknown option: {}", args[i]);
                        eprintln!("Usage: cargo xtask serve-demo [-a <address>] [-p <port>] [--dev]");
                        std::process::exit(1);
                    }
                }
            }
            serve_demo(&addr, port, dev_mode);
        }
        "help" | "--help" | "-h" => print_usage(),
        cmd => {
            eprintln!("Unknown command: {}", cmd);
            print_usage();
            std::process::exit(1);
        }
    }
}

fn print_usage() {
    eprintln!("Usage: cargo xtask <command>");
    eprintln!();
    eprintln!("Commands:");
    eprintln!("  regenerate [name] Regenerate tree-sitter grammars (optionally filter by name)");
    eprintln!("  check-updates    Check if any vendored grammars have newer versions available");
    eprintln!("  vendor <name>    Update a specific grammar to the latest version");
    eprintln!("  generate-crates  Generate arborium-* crates for all vendored grammars");
    eprintln!("  generate-demo    Generate demo/index.html from template and example files");
    eprintln!("  generate-readme  Generate README.md from GRAMMARS.toml");
    eprintln!("  serve-demo       Build and serve the WASM demo locally");
    eprintln!("                   Options: -a <address> (default: 127.0.0.1)");
    eprintln!("                            -p <port> (default: auto-select 8000-8010)");
    eprintln!("                            --dev (fast dev build: -O1, no wasm-opt, fast compression)");
    eprintln!("  help             Show this help message");
}

/// Result of processing a single grammar
struct GrammarResult {
    name: String,
    duration: Duration,
    success: bool,
    error: Option<String>,
}

fn regenerate(filter: Option<&str>) {
    // Configure rayon to use 12 threads
    rayon::ThreadPoolBuilder::new()
        .num_threads(12)
        .build_global()
        .ok();

    let repo_root = find_repo_root().expect("Could not find repo root");
    println!("{} {}", "Repo root:".cyan(), repo_root.display());

    // Step 1: Ensure tree-sitter CLI is installed
    println!("\n{}", "==> Checking tree-sitter CLI".cyan().bold());
    if let Err(e) = ensure_tree_sitter_cli() {
        eprintln!("{} {}", "Error:".red().bold(), e);
        std::process::exit(1);
    }

    // Step 2: Find all vendored grammars
    println!("\n{}", "==> Finding vendored grammars".cyan().bold());
    let mut grammars = match find_grammars(&repo_root) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("{} {}", "Error:".red().bold(), e);
            std::process::exit(1);
        }
    };

    // Filter grammars if a filter was provided
    if let Some(filter) = filter {
        let filter_with_prefix = if filter.starts_with("tree-sitter-") {
            filter.to_string()
        } else {
            format!("tree-sitter-{}", filter)
        };
        grammars.retain(|p| {
            let name = p.file_name().unwrap().to_string_lossy();
            name == filter_with_prefix || name.contains(filter)
        });
        if grammars.is_empty() {
            eprintln!("{} No grammars matching '{}'", "Error:".red().bold(), filter);
            std::process::exit(1);
        }
        println!("  Filtered to {} grammar(s) matching '{}'", grammars.len(), filter);
    }

    // Step 3: Group grammars by dependency level for parallel processing
    println!("\n{}", "==> Grouping grammars by dependency level".cyan().bold());
    let levels = match group_by_dependency_level(&grammars) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("{} {}", "Error:".red().bold(), e);
            std::process::exit(1);
        }
    };

    for (i, level) in levels.iter().enumerate() {
        let names: Vec<_> = level.iter().map(|p| p.file_name().unwrap().to_string_lossy()).collect();
        println!("  Level {}: {} grammars ({})", i, level.len(), names.join(", ").dimmed());
    }

    // Step 4: Set up node_modules symlinks for grammar dependencies
    println!("\n{}", "==> Setting up node_modules for dependencies".cyan().bold());
    if let Err(e) = setup_node_modules(&repo_root) {
        eprintln!("{} {}", "Error:".red().bold(), e);
        std::process::exit(1);
    }

    // Step 5: Process grammars level by level in parallel
    let total_grammars: usize = levels.iter().map(|l| l.len()).sum();
    let results: Arc<Mutex<Vec<GrammarResult>>> = Arc::new(Mutex::new(Vec::new()));
    let in_progress: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let completed: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));

    // Single progress bar for all grammars
    let pb = ProgressBar::new(total_grammars as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] {msg}")
            .unwrap(),
    );
    pb.enable_steady_tick(Duration::from_millis(100));

    let start_time = Instant::now();

    // Helper to update the progress line
    let update_progress = |pb: &ProgressBar, in_progress: &Arc<Mutex<Vec<String>>>, completed: &Arc<Mutex<usize>>, total: usize| {
        let current = in_progress.lock().unwrap();
        let done = *completed.lock().unwrap();

        // Show at most 6 grammar names
        let display_names: Vec<_> = current.iter()
            .map(|n| n.strip_prefix("tree-sitter-").unwrap_or(n))
            .take(6)
            .collect();

        let names_str = if display_names.is_empty() {
            String::new()
        } else if current.len() > 6 {
            format!("{} +{}", display_names.join(", "), current.len() - 6)
        } else {
            display_names.join(", ")
        };

        pb.set_message(format!("[{}/{}] {}", done, total, names_str));
    };

    for (_level_idx, level) in levels.iter().enumerate() {
        level.par_iter().for_each(|grammar| {
            let name = grammar.file_name().unwrap().to_string_lossy().to_string();
            let short_name = name.strip_prefix("tree-sitter-").unwrap_or(&name).to_string();
            let grammar_start = Instant::now();

            // Add to in-progress list
            {
                in_progress.lock().unwrap().push(name.clone());
                update_progress(&pb, &in_progress, &completed, total_grammars);
            }

            // Initialize
            let init_result = init_grammar(grammar);
            if let Err(e) = init_result {
                // Remove from in-progress, increment completed
                {
                    in_progress.lock().unwrap().retain(|n| n != &name);
                    *completed.lock().unwrap() += 1;
                    update_progress(&pb, &in_progress, &completed, total_grammars);
                }
                results.lock().unwrap().push(GrammarResult {
                    name: short_name,
                    duration: grammar_start.elapsed(),
                    success: false,
                    error: Some(format!("init failed: {}", e)),
                });
                pb.inc(1);
                return;
            }

            // Regenerate
            let regen_result = regenerate_grammar(grammar);
            let duration = grammar_start.elapsed();

            // Remove from in-progress, increment completed
            {
                in_progress.lock().unwrap().retain(|n| n != &name);
                *completed.lock().unwrap() += 1;
                update_progress(&pb, &in_progress, &completed, total_grammars);
            }

            let result = match regen_result {
                Ok(()) => {
                    GrammarResult {
                        name: short_name,
                        duration,
                        success: true,
                        error: None,
                    }
                }
                Err(e) => {
                    GrammarResult {
                        name: short_name,
                        duration,
                        success: false,
                        error: Some(e.to_string()),
                    }
                }
            };

            results.lock().unwrap().push(result);
            pb.inc(1);
        });
    }

    pb.finish_with_message(format!("[{}/{}] Done", total_grammars, total_grammars));

    // Step 6: Clean up all grammars in parallel
    print!("\r{}", "Cleaning up...".dimmed());
    std::io::stdout().flush().ok();

    grammars.par_iter().for_each(|grammar| {
        let _ = clean_grammar(grammar);
    });

    println!("\r{}", "Cleanup complete.".dimmed());

    // Step 7: Copy C sources to crates
    print!("\r{}", "Copying C sources to crates...".dimmed());
    std::io::stdout().flush().ok();

    grammars.par_iter().for_each(|grammar| {
        let _ = copy_grammar_sources_to_crate(&repo_root, grammar);
    });

    println!("\r{}", "C sources copied to crates.".dimmed());

    // Summary
    let total_duration = start_time.elapsed();
    let results = results.lock().unwrap();
    let failures: Vec<_> = results.iter().filter(|r| !r.success).collect();
    let successes: Vec<_> = results.iter().filter(|r| r.success).collect();

    println!("\n{}", "=== Summary ===".cyan().bold());
    println!(
        "Total time: {:.2}s ({} grammars)",
        total_duration.as_secs_f64(),
        results.len()
    );

    // Show timing for all grammars, sorted by duration
    let mut sorted_results: Vec<_> = results.iter().collect();
    sorted_results.sort_by(|a, b| b.duration.cmp(&a.duration));

    println!("\n{}", "Timing (slowest first):".dimmed());
    for result in sorted_results.iter().take(10) {
        let status = if result.success {
            "OK".green().to_string()
        } else {
            "FAILED".red().to_string()
        };
        println!(
            "  {:>6.2}s  {}  {}",
            result.duration.as_secs_f64(),
            status,
            result.name
        );
    }
    if sorted_results.len() > 10 {
        println!("  ... and {} more", sorted_results.len() - 10);
    }

    if failures.is_empty() {
        println!(
            "\n{} All {} grammars regenerated successfully!",
            "SUCCESS:".green().bold(),
            successes.len()
        );
    } else {
        println!(
            "\n{} {} succeeded, {} failed:",
            "PARTIAL:".yellow().bold(),
            successes.len(),
            failures.len()
        );
        for f in &failures {
            println!(
                "  {} {} - {}",
                "-".red(),
                f.name,
                f.error.as_deref().unwrap_or("unknown error").dimmed()
            );
        }
        println!("\n{}", "Failed grammars will use their existing parser.c files.".dimmed());
    }
}

/// Group grammars by dependency level for parallel processing
/// Level 0: grammars with no dependencies
/// Level 1: grammars that depend only on level 0
/// etc.
fn group_by_dependency_level(grammars: &[PathBuf]) -> Result<Vec<Vec<PathBuf>>, Box<dyn std::error::Error>> {
    let deps: HashMap<&str, &[&str]> = GRAMMAR_DEPS.iter().cloned().collect();

    // Build name -> path map
    let name_to_path: HashMap<String, &PathBuf> = grammars
        .iter()
        .map(|p| (p.file_name().unwrap().to_string_lossy().to_string(), p))
        .collect();

    let all_names: HashSet<String> = name_to_path.keys().cloned().collect();

    // Calculate level for each grammar
    fn get_level(
        name: &str,
        deps: &HashMap<&str, &[&str]>,
        all_names: &HashSet<String>,
        cache: &mut HashMap<String, usize>,
    ) -> usize {
        if let Some(&level) = cache.get(name) {
            return level;
        }

        let level = if let Some(grammar_deps) = deps.get(name) {
            let max_dep_level = grammar_deps
                .iter()
                .filter(|d| all_names.contains(**d))
                .map(|d| get_level(d, deps, all_names, cache))
                .max()
                .unwrap_or(0);
            max_dep_level + 1
        } else {
            0
        };

        cache.insert(name.to_string(), level);
        level
    }

    let mut level_cache = HashMap::new();
    let mut levels: HashMap<usize, Vec<PathBuf>> = HashMap::new();

    for name in &all_names {
        let level = get_level(name, &deps, &all_names, &mut level_cache);
        levels
            .entry(level)
            .or_default()
            .push(name_to_path[name].clone());
    }

    // Convert to vec of vecs, sorted by level
    let max_level = levels.keys().max().copied().unwrap_or(0);
    let mut result = Vec::new();
    for level in 0..=max_level {
        if let Some(mut grammars) = levels.remove(&level) {
            grammars.sort();
            result.push(grammars);
        }
    }

    Ok(result)
}

fn ensure_tree_sitter_cli() -> Result<(), Box<dyn std::error::Error>> {
    // Check if tree-sitter is available
    let version = Command::new("tree-sitter")
        .arg("--version")
        .output()
        .ok()
        .filter(|o| o.status.success());

    if let Some(output) = version {
        let version_str = String::from_utf8_lossy(&output.stdout);
        println!("  Found: {}", version_str.trim());
        return Ok(());
    }

    println!("  tree-sitter CLI not found, installing...");

    // Try cargo binstall first (faster)
    let binstall = Command::new("cargo")
        .args(["binstall", "-y", "tree-sitter-cli"])
        .status();

    if binstall.is_ok() && binstall.unwrap().success() {
        println!("  Installed via cargo binstall");
        return Ok(());
    }

    // Fall back to cargo install
    println!("  cargo binstall failed, trying cargo install...");
    let install = Command::new("cargo")
        .args(["install", "tree-sitter-cli"])
        .status()?;

    if !install.success() {
        return Err("Failed to install tree-sitter-cli".into());
    }

    println!("  Installed via cargo install");
    Ok(())
}

fn find_grammars(repo_root: &Path) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let mut grammars = Vec::new();
    let grammars_dir = repo_root.join("grammars");

    for entry in fs::read_dir(&grammars_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            let name = path.file_name().unwrap().to_string_lossy();
            if name.starts_with("tree-sitter-") {
                // Verify it has a grammar.js
                if path.join("grammar.js").exists() {
                    grammars.push(path);
                }
            }
        }
    }

    grammars.sort();
    println!("  Found {} grammars:", grammars.len());
    for g in &grammars {
        println!("    {}", g.file_name().unwrap().to_string_lossy());
    }

    Ok(grammars)
}

/// Set up node_modules symlinks so grammars can find their dependencies
fn setup_node_modules(repo_root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let grammars_dir = repo_root.join("grammars");
    let node_modules = grammars_dir.join("node_modules");

    // Create node_modules if it doesn't exist
    if !node_modules.exists() {
        fs::create_dir(&node_modules)?;
        println!("  Created grammars/node_modules/");
    }

    // Create symlinks for each grammar that is a dependency
    let mut deps_needed: HashSet<&str> = HashSet::new();
    for (_, grammar_deps) in GRAMMAR_DEPS {
        for dep in *grammar_deps {
            deps_needed.insert(dep);
        }
    }

    for dep in deps_needed {
        let target = grammars_dir.join(dep);
        let link = node_modules.join(dep);

        if target.exists() {
            // Remove existing symlink if present
            if link.exists() || link.symlink_metadata().is_ok() {
                fs::remove_file(&link).or_else(|_| fs::remove_dir_all(&link))?;
            }

            #[cfg(unix)]
            std::os::unix::fs::symlink(&target, &link)?;
            #[cfg(windows)]
            std::os::windows::fs::symlink_dir(&target, &link)?;

            println!("  Symlinked {} -> {}", dep, target.display());
        } else {
            println!("  Warning: dependency {} not found at {}", dep, target.display());
        }
    }

    Ok(())
}

fn init_grammar(grammar_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    // Only run init --update if tree-sitter.json already exists
    // (otherwise it prompts interactively)
    let config_file = grammar_dir.join("tree-sitter.json");
    if config_file.exists() {
        let status = Command::new("tree-sitter")
            .args(["init", "--update"])
            .current_dir(grammar_dir)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?;

        if !status.success() {
            return Err(format!(
                "tree-sitter init --update failed for {}",
                grammar_dir.display()
            )
            .into());
        }
    }

    // Run pnpm install if package.json exists and has dependencies
    let package_json = grammar_dir.join("package.json");
    if package_json.exists() {
        // Check if node_modules already exists
        let node_modules = grammar_dir.join("node_modules");
        if !node_modules.exists() {
            let _ = Command::new("pnpm")
                .args(["install", "--ignore-scripts"])
                .current_dir(grammar_dir)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            // Non-fatal - some grammars may not need npm deps
        }
    }

    Ok(())
}

fn regenerate_grammar(grammar_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    // Set NODE_PATH to grammars/ dir so grammars can find their dependencies
    // grammar_dir is grammars/tree-sitter-*, so parent is grammars/
    let grammars_dir = grammar_dir.parent().unwrap_or(grammar_dir);

    let output = Command::new("tree-sitter")
        .args(["generate"])
        .current_dir(grammar_dir)
        .env("NODE_PATH", grammars_dir)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // Print full error to stderr for debugging
        eprintln!("\n{} {} failed:\n{}",
            "Error:".red().bold(),
            grammar_dir.file_name().unwrap().to_string_lossy(),
            stderr);

        // Return a short summary for the results table
        let error_msg = stderr
            .lines()
            .find(|l| l.contains("Cannot find module") || l.contains("error") || l.contains("Error"))
            .unwrap_or("generation failed")
            .trim();
        return Err(error_msg.to_string().into());
    }

    Ok(())
}

fn clean_grammar(grammar_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut removed_count = 0;

    // Clean root directory
    for entry in fs::read_dir(grammar_dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = path.file_name().unwrap().to_string_lossy();

        if path.is_file() {
            // Keep explicitly listed files and all .js/.mjs files (helper modules)
            let dominated = KEEP_FILES.iter().any(|&f| name == f)
                || name.ends_with(".js")
                || name.ends_with(".mjs");
            if !dominated {
                fs::remove_file(&path)?;
                removed_count += 1;
            }
        } else if path.is_dir() {
            if !KEEP_DIRS.iter().any(|&d| name == d) {
                fs::remove_dir_all(&path)?;
                removed_count += 1;
            }
        }
    }

    // Clean src/ directory
    let src_dir = grammar_dir.join("src");
    if src_dir.exists() {
        for entry in fs::read_dir(&src_dir)? {
            let entry = entry?;
            let path = entry.path();
            let name = path.file_name().unwrap().to_string_lossy();

            if path.is_file() {
                // Keep files with allowed extensions
                let has_allowed_ext = KEEP_SRC_EXTENSIONS.iter().any(|ext| name.ends_with(ext));
                if !has_allowed_ext {
                    fs::remove_file(&path)?;
                    removed_count += 1;
                }
            } else if path.is_dir() {
                if !KEEP_SRC_DIRS.iter().any(|&d| name == d) {
                    fs::remove_dir_all(&path)?;
                    removed_count += 1;
                }
            }
        }
    }

    let _ = removed_count; // suppress unused warning
    Ok(())
}

/// Copy C source files from a grammar's src/ to the corresponding crate's grammar-src/
fn copy_grammar_sources_to_crate(repo_root: &Path, grammar_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let name = grammar_dir
        .file_name()
        .unwrap()
        .to_string_lossy()
        .strip_prefix("tree-sitter-")
        .unwrap_or(&grammar_dir.file_name().unwrap().to_string_lossy())
        .to_string();

    let grammar_src_dir = grammar_dir.join("src");
    let crate_name = format!("arborium-{}", name);
    let crate_dir = repo_root.join("crates").join(&crate_name);
    let crate_grammar_src = crate_dir.join("grammar-src");

    // Skip if crate doesn't exist
    if !crate_dir.exists() {
        return Ok(());
    }

    // Skip if grammar src doesn't exist
    if !grammar_src_dir.exists() {
        return Ok(());
    }

    // Create grammar-src/ directory in crate
    if crate_grammar_src.exists() {
        fs::remove_dir_all(&crate_grammar_src)?;
    }
    fs::create_dir_all(&crate_grammar_src)?;

    // Copy all .c, .cc, .h, .json files from grammar's src/
    for entry in fs::read_dir(&grammar_src_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() {
            let name = path.file_name().unwrap().to_string_lossy();
            let should_copy = name.ends_with(".c")
                || name.ends_with(".cc")
                || name.ends_with(".h")
                || name.ends_with(".json");

            if should_copy {
                fs::copy(&path, crate_grammar_src.join(entry.file_name()))?;
            }
        } else if path.is_dir() {
            // Copy subdirectories like tree_sitter/
            let dest_dir = crate_grammar_src.join(entry.file_name());
            copy_dir_recursive(&path, &dest_dir)?;
        }
    }

    Ok(())
}

fn step<F>(name: &str, f: F)
where
    F: FnOnce() -> Result<(), Box<dyn std::error::Error>>,
{
    println!("\n==> {}", name);
    if let Err(e) = f() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn find_repo_root() -> Option<PathBuf> {
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

// =============================================================================
// Grammar tracking and updates
// =============================================================================

/// Grammar configuration from GRAMMARS.toml
#[derive(Debug)]
#[allow(dead_code)]
struct GrammarConfig {
    name: String,
    repo: String,
    commit: String,
    license: String,
}

/// Parse GRAMMARS.toml
fn parse_grammars_toml(repo_root: &Path) -> Result<BTreeMap<String, GrammarConfig>, Box<dyn std::error::Error>> {
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

/// Get the latest commit hash from a remote repository
fn get_remote_head(repo_url: &str) -> Result<String, Box<dyn std::error::Error>> {
    if repo_url == "local" {
        return Ok("local".to_string());
    }

    let output = Command::new("git")
        .args(["ls-remote", repo_url, "HEAD"])
        .output()?;

    if !output.status.success() {
        return Err(format!("Failed to query {}", repo_url).into());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let commit = stdout.split_whitespace().next()
        .ok_or_else(|| format!("No commit found for {}", repo_url))?;

    Ok(commit.to_string())
}

/// Check for updates to vendored grammars
fn check_updates() {
    let repo_root = find_repo_root().expect("Could not find repo root");
    println!("Checking for grammar updates...\n");

    let grammars = match parse_grammars_toml(&repo_root) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("Error parsing GRAMMARS.toml: {}", e);
            std::process::exit(1);
        }
    };

    let mut updates_available = Vec::new();
    let mut up_to_date = 0;
    let mut errors = 0;

    for (name, config) in &grammars {
        if config.repo == "local" {
            println!("  {} (local, skipping)", name);
            continue;
        }

        print!("  {} ... ", name);

        match get_remote_head(&config.repo) {
            Ok(remote_commit) => {
                if remote_commit == config.commit {
                    println!("up to date");
                    up_to_date += 1;
                } else {
                    println!("UPDATE AVAILABLE");
                    println!("    current: {}", &config.commit[..12.min(config.commit.len())]);
                    println!("    latest:  {}", &remote_commit[..12.min(remote_commit.len())]);
                    updates_available.push((name.clone(), config.commit.clone(), remote_commit));
                }
            }
            Err(e) => {
                println!("ERROR: {}", e);
                errors += 1;
            }
        }
    }

    println!();
    println!("Summary:");
    println!("  {} up to date", up_to_date);
    println!("  {} updates available", updates_available.len());
    if errors > 0 {
        println!("  {} errors", errors);
    }

    if !updates_available.is_empty() {
        println!();
        println!("To update a grammar, run:");
        for (name, _, _) in &updates_available {
            println!("  cargo xtask vendor {}", name);
        }
    }
}

/// Vendor (update) a specific grammar
fn vendor_grammar(name: &str) {
    let repo_root = find_repo_root().expect("Could not find repo root");

    let grammars = match parse_grammars_toml(&repo_root) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("Error parsing GRAMMARS.toml: {}", e);
            std::process::exit(1);
        }
    };

    let config = match grammars.get(name) {
        Some(c) => c,
        None => {
            eprintln!("Unknown grammar: {}", name);
            eprintln!("Available grammars:");
            for name in grammars.keys() {
                eprintln!("  {}", name);
            }
            std::process::exit(1);
        }
    };

    if config.repo == "local" {
        eprintln!("Cannot vendor local grammar: {}", name);
        std::process::exit(1);
    }

    println!("Vendoring {} from {}", name, config.repo);

    // Determine the target directory name
    let dir_name = format!("tree-sitter-{}", name);
    let target_dir = repo_root.join("grammars").join(&dir_name);

    // Clone to temp directory
    let temp_dir = std::env::temp_dir().join(format!("arborium-vendor-{}", name));
    if temp_dir.exists() {
        fs::remove_dir_all(&temp_dir).expect("Failed to clean temp dir");
    }

    println!("  Cloning {}...", config.repo);
    let status = Command::new("git")
        .args(["clone", "--depth", "1", &config.repo, temp_dir.to_str().unwrap()])
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .status()
        .expect("Failed to run git clone");

    if !status.success() {
        eprintln!("Failed to clone repository");
        std::process::exit(1);
    }

    // Get the new commit hash
    let new_commit = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(&temp_dir)
        .output()
        .expect("Failed to get commit hash");

    let new_commit = String::from_utf8_lossy(&new_commit.stdout).trim().to_string();
    println!("  New commit: {}", &new_commit[..12.min(new_commit.len())]);

    // Create target directory structure
    if target_dir.exists() {
        // Preserve queries directory if it exists and has local modifications
        let queries_backup = if target_dir.join("queries").exists() {
            let backup = std::env::temp_dir().join(format!("arborium-queries-backup-{}", name));
            if backup.exists() {
                fs::remove_dir_all(&backup).ok();
            }
            fs::rename(target_dir.join("queries"), &backup).ok();
            Some(backup)
        } else {
            None
        };

        fs::remove_dir_all(&target_dir).expect("Failed to remove old grammar dir");
        fs::create_dir_all(&target_dir).expect("Failed to create grammar dir");

        // Restore queries if we backed them up
        if let Some(backup) = queries_backup {
            fs::rename(&backup, target_dir.join("queries")).ok();
        }
    } else {
        fs::create_dir_all(&target_dir).expect("Failed to create grammar dir");
    }

    // Copy essential files
    println!("  Copying files...");

    // Copy LICENSE
    for license_file in &["LICENSE", "LICENSE.md", "COPYING.txt", "COPYING"] {
        let src = temp_dir.join(license_file);
        if src.exists() {
            fs::copy(&src, target_dir.join(license_file)).ok();
            break;
        }
    }

    // Copy grammar.js
    let grammar_js = temp_dir.join("grammar.js");
    if grammar_js.exists() {
        fs::copy(&grammar_js, target_dir.join("grammar.js")).expect("Failed to copy grammar.js");
    }

    // Copy package.json if it exists (some grammars have npm dependencies)
    let package_json = temp_dir.join("package.json");
    if package_json.exists() {
        fs::copy(&package_json, target_dir.join("package.json")).ok();
    }

    // Copy grammar/ directory if it exists (for grammars with multiple files)
    let grammar_dir = temp_dir.join("grammar");
    if grammar_dir.exists() {
        copy_dir_recursive(&grammar_dir, &target_dir.join("grammar")).expect("Failed to copy grammar/");
    }

    // Copy additional directories that some grammars need for their grammar.js
    for extra_dir in &["lib", "common", "rules"] {
        let src_dir = temp_dir.join(extra_dir);
        if src_dir.exists() {
            copy_dir_recursive(&src_dir, &target_dir.join(extra_dir))
                .unwrap_or_else(|e| eprintln!("  Warning: Failed to copy {}/: {}", extra_dir, e));
        }
    }

    // Copy all root-level JS files (helper files that grammars may need)
    if let Ok(entries) = fs::read_dir(&temp_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension() {
                    if ext == "js" {
                        let filename = path.file_name().unwrap();
                        // grammar.js is already copied separately
                        if filename != "grammar.js" {
                            fs::copy(&path, target_dir.join(filename)).ok();
                        }
                    }
                }
            }
        }
    }

    // Copy src/ directory
    let src_dir = temp_dir.join("src");
    if src_dir.exists() {
        copy_dir_recursive(&src_dir, &target_dir.join("src")).expect("Failed to copy src/");
    }

    // Copy queries/ if target doesn't already have them
    let queries_dir = temp_dir.join("queries");
    if queries_dir.exists() && !target_dir.join("queries").exists() {
        copy_dir_recursive(&queries_dir, &target_dir.join("queries")).expect("Failed to copy queries/");
    }

    // Clean up temp dir
    fs::remove_dir_all(&temp_dir).ok();

    // Update GRAMMARS.toml
    println!("  Updating GRAMMARS.toml...");
    let grammars_path = repo_root.join("GRAMMARS.toml");
    let contents = fs::read_to_string(&grammars_path).expect("Failed to read GRAMMARS.toml");

    // Replace the commit line for this grammar

    // We need to be careful to only replace within the right section
    let mut new_contents = String::new();
    let mut in_target_section = false;
    let mut replaced = false;

    for line in contents.lines() {
        let trimmed = line.trim();

        // Check for section header
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            let section_name = &trimmed[1..trimmed.len()-1];
            in_target_section = section_name == name;
        }

        // Replace commit line if in target section
        if in_target_section && trimmed.starts_with("commit = ") && !replaced {
            new_contents.push_str(&line.replace(&config.commit, &new_commit));
            replaced = true;
        } else {
            new_contents.push_str(line);
        }
        new_contents.push('\n');
    }

    fs::write(&grammars_path, new_contents).expect("Failed to write GRAMMARS.toml");

    println!();
    println!("Done! Grammar {} updated to {}", name, &new_commit[..12.min(new_commit.len())]);
    println!();
    println!("Next steps:");
    println!("  1. Run: cargo xtask regenerate");
    println!("  2. Run: cargo build");
    println!("  3. Run: cargo test");
}

/// Recursively copy a directory
fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let dest_path = dst.join(entry.file_name());

        if path.is_dir() {
            copy_dir_recursive(&path, &dest_path)?;
        } else {
            fs::copy(&path, &dest_path)?;
        }
    }

    Ok(())
}

// =============================================================================
// Grammar crate generation
// =============================================================================

/// Configuration for a grammar crate
#[derive(Debug)]
struct GrammarCrateConfig {
    /// Grammar name (e.g., "rust", "python")
    name: String,
    /// C function name suffix (e.g., "rust" for tree_sitter_rust)
    c_symbol: String,
    /// Source files to compile (relative to tree-sitter-{name}/src/)
    source_files: Vec<String>,
    /// Whether highlights.scm exists
    has_highlights: bool,
    /// Whether injections.scm exists
    has_injections: bool,
    /// Whether locals.scm exists
    has_locals: bool,
    /// Query path prefix (for grammars with nested query directories)
    query_path: String,
    /// Additional languages exported by this grammar (e.g., "tsx" for typescript)
    extra_languages: Vec<(String, String)>, // (c_symbol, export_name)
}

/// Special grammar configurations that differ from the standard pattern
fn get_special_grammar_config(name: &str) -> Option<GrammarCrateConfig> {
    match name {
        "rust" => Some(GrammarCrateConfig {
            name: "rust".to_string(),
            c_symbol: "rust".to_string(),
            source_files: vec!["parser.c".to_string(), "scanner.c".to_string()],
            has_highlights: true,
            has_injections: true,
            has_locals: false,
            query_path: "".to_string(),
            extra_languages: vec![],
        }),
        "typescript" => Some(GrammarCrateConfig {
            name: "typescript".to_string(),
            c_symbol: "typescript".to_string(),
            source_files: vec!["parser.c".to_string(), "scanner.c".to_string()],
            has_highlights: true,
            has_injections: false,
            has_locals: true,
            query_path: "".to_string(),
            extra_languages: vec![("tsx".to_string(), "tsx".to_string())],
        }),
        "markdown" => Some(GrammarCrateConfig {
            name: "markdown".to_string(),
            c_symbol: "markdown".to_string(),
            source_files: vec!["parser.c".to_string(), "scanner.c".to_string()],
            has_highlights: true,
            has_injections: false,
            has_locals: false,
            query_path: "tree-sitter-markdown/".to_string(),
            extra_languages: vec![("markdown_inline".to_string(), "markdown_inline".to_string())],
        }),
        "ocaml" => Some(GrammarCrateConfig {
            name: "ocaml".to_string(),
            c_symbol: "ocaml".to_string(),
            source_files: vec!["parser.c".to_string(), "scanner.c".to_string()],
            has_highlights: true,
            has_injections: false,
            has_locals: true,
            query_path: "".to_string(),
            extra_languages: vec![("ocaml_interface".to_string(), "ocaml_interface".to_string())],
        }),
        "xml" => Some(GrammarCrateConfig {
            name: "xml".to_string(),
            c_symbol: "xml".to_string(),
            source_files: vec!["parser.c".to_string()],
            has_highlights: true,
            has_injections: false,
            has_locals: false,
            query_path: "xml/".to_string(),
            extra_languages: vec![("dtd".to_string(), "dtd".to_string())],
        }),
        "php" => Some(GrammarCrateConfig {
            name: "php".to_string(),
            c_symbol: "php".to_string(),
            source_files: vec!["parser.c".to_string(), "scanner.c".to_string()],
            has_highlights: true,
            has_injections: true,
            has_locals: false,
            query_path: "".to_string(),
            extra_languages: vec![("php_only".to_string(), "php_only".to_string())],
        }),
        "wasm" => Some(GrammarCrateConfig {
            name: "wasm".to_string(),
            c_symbol: "wat".to_string(),  // The wasm grammar exports wat, not wasm
            source_files: vec!["parser.c".to_string()],
            has_highlights: false,
            has_injections: false,
            has_locals: false,
            query_path: "wat/".to_string(),
            extra_languages: vec![("wast".to_string(), "wast".to_string())],
        }),
        "just" => Some(GrammarCrateConfig {
            name: "just".to_string(),
            c_symbol: "just".to_string(),
            source_files: vec!["parser.c".to_string(), "scanner.c".to_string()],
            has_highlights: true,
            has_injections: true,
            has_locals: false,
            query_path: "just/".to_string(),
            extra_languages: vec![],
        }),
        "typst" => Some(GrammarCrateConfig {
            name: "typst".to_string(),
            c_symbol: "typst".to_string(),
            source_files: vec!["parser.c".to_string(), "scanner.c".to_string()],
            has_highlights: true,
            has_injections: true,
            has_locals: false,
            query_path: "typst/".to_string(),
            extra_languages: vec![],
        }),
        "asm" => Some(GrammarCrateConfig {
            name: "asm".to_string(),
            c_symbol: "asm".to_string(),
            source_files: vec!["parser.c".to_string()],
            has_highlights: true,
            has_injections: false,
            has_locals: false,
            query_path: "asm/".to_string(),
            extra_languages: vec![],
        }),
        _ => None,
    }
}

/// Detect grammar configuration from filesystem
fn detect_grammar_config(repo_root: &Path, name: &str) -> GrammarCrateConfig {
    // Check for special configuration first
    if let Some(config) = get_special_grammar_config(name) {
        return config;
    }

    let grammar_dir = repo_root.join("grammars").join(format!("tree-sitter-{}", name));
    let src_dir = grammar_dir.join("src");
    let queries_dir = grammar_dir.join("queries");

    // Detect source files
    let mut source_files = vec!["parser.c".to_string()];
    if src_dir.join("scanner.c").exists() {
        source_files.push("scanner.c".to_string());
    }
    if src_dir.join("scanner.cc").exists() {
        source_files.push("scanner.cc".to_string());
    }

    // Detect queries
    let has_highlights = queries_dir.join("highlights.scm").exists();
    let has_injections = queries_dir.join("injections.scm").exists();
    let has_locals = queries_dir.join("locals.scm").exists();

    // Convert name to C symbol (replace - with _)
    let c_symbol = name.replace('-', "_");

    GrammarCrateConfig {
        name: name.to_string(),
        c_symbol,
        source_files,
        has_highlights,
        has_injections,
        has_locals,
        query_path: "".to_string(),
        extra_languages: vec![],
    }
}

/// Generate Cargo.toml for a grammar crate
fn generate_cargo_toml(config: &GrammarCrateConfig, license: &str) -> String {
    let crate_name = format!("arborium-{}", config.name);
    let description = format!("{} grammar for arborium (tree-sitter bindings)", config.name);

    format!(
        r#"[package]
name = "{crate_name}"
version = "0.1.0"
edition = "2024"
description = "{description}"
license = "{license}"
repository = "https://github.com/bearcove/arborium"
keywords = ["tree-sitter", "{name}", "syntax-highlighting"]
categories = ["parsing", "text-processing"]

[lib]
path = "src/lib.rs"

[dependencies]
tree-sitter-patched-arborium = {{ version = "0.25.10", path = "../../tree-sitter" }}
arborium-sysroot = {{ version = "0.1.0", path = "../arborium-sysroot" }}

[build-dependencies]
cc = {{ version = "1", features = ["parallel"] }}
"#,
        crate_name = crate_name,
        description = description,
        license = license,
        name = config.name,
    )
}

/// Generate build.rs for a grammar crate
fn generate_build_rs(config: &GrammarCrateConfig) -> String {
    // Source files are now in grammar-src/ within the crate itself
    let src_dir_path = "grammar-src";

    let rerun_lines: String = config
        .source_files
        .iter()
        .map(|f| format!("    println!(\"cargo:rerun-if-changed={{}}/{}\", src_dir);", f))
        .collect::<Vec<_>>()
        .join("\n");

    let file_lines: String = config
        .source_files
        .iter()
        .map(|f| format!("    build.file(format!(\"{{}}/{}\", src_dir));", f))
        .collect::<Vec<_>>()
        .join("\n");

    let compile_name = format!("tree_sitter_{}", config.c_symbol);

    format!(
        r#"fn main() {{
    let src_dir = "{src_dir}";

{rerun_lines}

    let mut build = cc::Build::new();

    build
        .include(src_dir)
        .include(format!("{{}}/tree_sitter", src_dir))
        .warnings(false)
        .flag_if_supported("-Wno-unused-parameter")
        .flag_if_supported("-Wno-unused-but-set-variable")
        .flag_if_supported("-Wno-trigraphs");

    // For WASM builds, use our custom sysroot (provided by arborium-sysroot crate)
    if let Ok(sysroot) = std::env::var("DEP_ARBORIUM_SYSROOT_PATH") {{
        build.include(&sysroot);
    }}

{file_lines}

    build.compile("{compile_name}");
}}
"#,
        src_dir = src_dir_path,
        rerun_lines = rerun_lines,
        file_lines = file_lines,
        compile_name = compile_name,
    )
}

/// Generate lib.rs for a grammar crate
fn generate_lib_rs(config: &GrammarCrateConfig) -> String {
    let mut output = String::new();

    // Module doc comment
    output.push_str(&format!(
        r#"//! {} grammar for tree-sitter
//!
//! This crate provides the {} language grammar for use with tree-sitter.

use tree_sitter_patched_arborium::Language;

"#,
        config.name.to_uppercase(),
        config.name,
    ));

    // Main language extern
    output.push_str(&format!(
        r#"unsafe extern "C" {{
    fn tree_sitter_{}() -> Language;
}}

/// Returns the {} tree-sitter language.
pub fn language() -> Language {{
    unsafe {{ tree_sitter_{}() }}
}}

"#,
        config.c_symbol, config.name, config.c_symbol,
    ));

    // Extra languages
    for (c_symbol, export_name) in &config.extra_languages {
        output.push_str(&format!(
            r#"unsafe extern "C" {{
    fn tree_sitter_{}() -> Language;
}}

/// Returns the {} tree-sitter language.
pub fn {}_language() -> Language {{
    unsafe {{ tree_sitter_{}() }}
}}

"#,
            c_symbol, export_name, export_name, c_symbol,
        ));
    }

    // Query constants - queries are in grammars/tree-sitter-{name}/queries/
    let query_prefix = if config.query_path.is_empty() {
        format!("../../../grammars/tree-sitter-{}/queries/", config.name)
    } else {
        format!("../../../grammars/tree-sitter-{}/queries/{}", config.name, config.query_path)
    };

    if config.has_highlights {
        output.push_str(&format!(
            r#"/// The highlight query for {}.
pub const HIGHLIGHTS_QUERY: &str = include_str!("{}highlights.scm");

"#,
            config.name, query_prefix,
        ));
    } else {
        output.push_str(&format!(
            r#"/// The highlight query for {} (empty - no highlights available).
pub const HIGHLIGHTS_QUERY: &str = "";

"#,
            config.name,
        ));
    }

    if config.has_injections {
        output.push_str(&format!(
            r#"/// The injections query for {}.
pub const INJECTIONS_QUERY: &str = include_str!("{}injections.scm");

"#,
            config.name, query_prefix,
        ));
    } else {
        output.push_str(&format!(
            r#"/// The injections query for {} (empty - no injections available).
pub const INJECTIONS_QUERY: &str = "";

"#,
            config.name,
        ));
    }

    if config.has_locals {
        output.push_str(&format!(
            r#"/// The locals query for {}.
pub const LOCALS_QUERY: &str = include_str!("{}locals.scm");

"#,
            config.name, query_prefix,
        ));
    } else {
        output.push_str(&format!(
            r#"/// The locals query for {} (empty - no locals available).
pub const LOCALS_QUERY: &str = "";

"#,
            config.name,
        ));
    }

    // Tests
    output.push_str(&format!(
        r#"#[cfg(test)]
mod tests {{
    use super::*;

    #[test]
    fn test_language() {{
        let lang = language();
        assert!(lang.version() > 0);
    }}
}}
"#,
    ));

    output
}

/// Generate all grammar crates
fn generate_grammar_crates() {
    let repo_root = find_repo_root().expect("Could not find repo root");
    println!("Generating grammar crates...\n");

    // Parse GRAMMARS.toml to get license info
    let grammars = match parse_grammars_toml(&repo_root) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("Error parsing GRAMMARS.toml: {}", e);
            std::process::exit(1);
        }
    };

    // Find all grammar directories
    let grammar_dirs = find_grammars(&repo_root).expect("Failed to find grammars");

    let crates_dir = repo_root.join("crates");
    if !crates_dir.exists() {
        fs::create_dir_all(&crates_dir).expect("Failed to create crates/");
    }

    let mut generated = 0;
    let mut skipped = 0;
    let mut workspace_members = Vec::new();

    for grammar_dir in grammar_dirs {
        let dir_name = grammar_dir.file_name().unwrap().to_string_lossy();
        let name = dir_name.strip_prefix("tree-sitter-").unwrap_or(&dir_name);

        // Skip grammars we don't have in GRAMMARS.toml
        let license = grammars.get(name).map(|g| g.license.as_str()).unwrap_or("MIT");

        // Detect configuration
        let config = detect_grammar_config(&repo_root, name);

        // Create crate directory
        let crate_name = format!("arborium-{}", name);
        let crate_dir = crates_dir.join(&crate_name);
        let src_dir = crate_dir.join("src");

        // Skip if crate already exists (preserve manual edits)
        if crate_dir.exists() {
            println!("  {} (skipped - already exists)", crate_name);
            skipped += 1;
            workspace_members.push(format!("crates/{}", crate_name));
            continue;
        }

        println!("  Generating {}...", crate_name);

        fs::create_dir_all(&src_dir).expect("Failed to create src/");

        // Generate files
        let cargo_toml = generate_cargo_toml(&config, license);
        let build_rs = generate_build_rs(&config);
        let lib_rs = generate_lib_rs(&config);

        fs::write(crate_dir.join("Cargo.toml"), cargo_toml).expect("Failed to write Cargo.toml");
        fs::write(crate_dir.join("build.rs"), build_rs).expect("Failed to write build.rs");
        fs::write(src_dir.join("lib.rs"), lib_rs).expect("Failed to write lib.rs");

        workspace_members.push(format!("crates/{}", crate_name));
        generated += 1;
    }

    println!();
    println!("Generated {} crates, skipped {} existing", generated, skipped);
    println!();

    // Print workspace members for Cargo.toml
    println!("Add these to workspace.members in Cargo.toml:");
    println!();
    for member in &workspace_members {
        println!("    \"{}\",", member);
    }
}

// =============================================================================
// Demo server
// =============================================================================

/// Build and serve the WASM demo
fn serve_demo(addr: &str, specified_port: Option<u16>, dev_mode: bool) {
    let repo_root = find_repo_root().expect("Could not find repo root");
    let demo_dir = repo_root.join("demo");

    if dev_mode {
        println!("Building and serving arborium demo (DEV MODE)...\n");
    } else {
        println!("Building and serving arborium demo...\n");
    }

    // Step 0: Generate index.html from template
    step("Generating demo HTML", || {
        // Call generate_demo logic inline to avoid needing a Result return
        let template_path = demo_dir.join("template.html");
        let examples_dir = demo_dir.join("examples");
        let output_path = demo_dir.join("index.html");

        let template = fs::read_to_string(&template_path)?;
        let mut examples: BTreeMap<String, String> = BTreeMap::new();

        for entry in fs::read_dir(&examples_dir)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            let stem = path.file_stem().unwrap().to_string_lossy();
            let ext = path.extension().map(|e| e.to_string_lossy().to_string());

            let lang_id = if stem == "x86asm" {
                "x86asm".to_string()
            } else if let Some(ext) = &ext {
                extension_to_lang_id(ext).map(|s| s.to_string()).unwrap_or_else(|| stem.to_string())
            } else {
                stem.to_string()
            };

            let content = fs::read_to_string(&path)?;
            examples.insert(lang_id, content);
        }

        let mut js_obj = String::from("{\n");
        for (i, (lang_id, content)) in examples.iter().enumerate() {
            let escaped = escape_for_js(content);
            js_obj.push_str(&format!("            \"{}\": \"{}\"", lang_id, escaped));
            if i < examples.len() - 1 {
                js_obj.push(',');
            }
            js_obj.push('\n');
        }
        js_obj.push_str("        }");

        let output = template.replace("{{EXAMPLES}}", &js_obj);
        fs::write(&output_path, &output)?;
        println!("  Generated index.html ({} examples)", examples.len());
        Ok(())
    });

    // Step 1: Check for wasm-pack
    step("Checking for wasm-pack", || ensure_wasm_pack());

    if dev_mode {
        // DEV MODE: Fast build with --dev flag, skip wasm-opt
        step("Building WASM package (wasm-pack --dev)", || {
            println!("  Running wasm-pack build --dev --target web...");
            let status = Command::new("wasm-pack")
                .args(["build", "--dev", "--target", "web"])
                .current_dir(&demo_dir)
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .status()?;

            if !status.success() {
                return Err("wasm-pack build failed".into());
            }

            println!("  Dev build complete!");
            Ok(())
        });

        // Check for env imports
        step("Checking for env imports", || {
            let wasm_file = demo_dir.join("pkg/arborium_demo_bg.wasm");
            check_wasm_env_imports(&wasm_file)
        });

        // Fast compression with low quality settings
        step("Pre-compressing files (fast)", || {
            precompress_files_fast(&demo_dir)
        });
    } else {
        // RELEASE MODE: Full optimizations

        // Step 2: Cargo build in debug mode (fast, no optimizations) for early env import check
        step("Building WASM (debug, fast)", || {
            println!("  Running cargo build (debug) for WASM target...");
            let status = Command::new("cargo")
                .args([
                    "build",
                    "--target",
                    "wasm32-unknown-unknown",
                    "-p",
                    "arborium-demo",
                ])
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .status()?;

            if !status.success() {
                return Err("cargo build failed".into());
            }

            println!("  Debug build complete!");
            Ok(())
        });

        // Step 3: Check for env imports on debug build (before slow optimized build)
        step("Checking for env imports", || {
            check_wasm_env_imports_debug()
        });

        // Step 4: Build the WASM package with wasm-pack (includes wasm-opt)
        step("Building WASM package (wasm-pack)", || {
            println!("  Running wasm-pack build --target web...");
            let status = Command::new("wasm-pack")
                .args(["build", "--target", "web"])
                .current_dir(&demo_dir)
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .status()?;

            if !status.success() {
                return Err("wasm-pack build failed".into());
            }

            println!("  Build complete!");
            Ok(())
        });

        // Step 5: Verify env imports on final release build
        step("Verifying release build (env imports)", || {
            let wasm_file = demo_dir.join("pkg/arborium_demo_bg.wasm");
            check_wasm_env_imports(&wasm_file)
        });

        // Step 6: Pre-compress files with best compression
        step("Pre-compressing files", || {
            precompress_files(&demo_dir)
        });
    }

    // Step 7: Start HTTP server
    println!("\n==> Starting HTTP server");
    println!("  Demo directory: {}", demo_dir.display());
    println!();

    let (server, port) = if let Some(p) = specified_port {
        // Try the specified port only
        match tiny_http::Server::http(format!("{}:{}", addr, p)) {
            Ok(s) => (s, p),
            Err(e) => {
                eprintln!("Error: Could not bind to {}:{}: {}", addr, p, e);
                std::process::exit(1);
            }
        }
    } else {
        // Try ports 8000-8010
        let mut server = None;
        let mut port = 8000u16;

        while port <= 8010 {
            match tiny_http::Server::http(format!("{}:{}", addr, port)) {
                Ok(s) => {
                    server = Some(s);
                    break;
                }
                Err(_) => {
                    port += 1;
                }
            }
        }

        match server {
            Some(s) => (s, port),
            None => {
                eprintln!("Error: Could not bind to any port between 8000-8010 on {}", addr);
                std::process::exit(1);
            }
        }
    };

    let display_addr = if addr == "0.0.0.0" { "localhost" } else { addr };
    println!("  ┌─────────────────────────────────────────────────┐");
    println!("  │                                                 │");
    println!("  │   Demo running at: http://{}:{:<5}       │", display_addr, port);
    println!("  │                                                 │");
    println!("  │   Press Ctrl+C to stop                          │");
    println!("  │                                                 │");
    println!("  └─────────────────────────────────────────────────┘");
    println!();

    // Compute bundle info once at startup
    let bundle_info = compute_bundle_info(&demo_dir);

    // Serve files
    for request in server.incoming_requests() {
        let url_path = request.url().trim_start_matches('/');

        // Special endpoint for bundle info
        if url_path == "bundle-info.json" {
            let response = tiny_http::Response::from_string(&bundle_info)
                .with_header(
                    tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..])
                        .unwrap(),
                );
            let _ = request.respond(response);
            continue;
        }

        let file_path = if url_path.is_empty() || url_path == "/" {
            demo_dir.join("index.html")
        } else {
            demo_dir.join(url_path)
        };

        // Security: ensure path is within demo_dir
        let file_path = match file_path.canonicalize() {
            Ok(p) => p,
            Err(_) => {
                let response = tiny_http::Response::from_string("Not Found")
                    .with_status_code(404);
                let _ = request.respond(response);
                continue;
            }
        };

        if !file_path.starts_with(&demo_dir) {
            let response = tiny_http::Response::from_string("Forbidden")
                .with_status_code(403);
            let _ = request.respond(response);
            continue;
        }

        // Check what encodings the client accepts
        let accept_encoding = request
            .headers()
            .iter()
            .find(|h| h.field.as_str().to_ascii_lowercase() == "accept-encoding")
            .map(|h| h.value.as_str().to_string())
            .unwrap_or_default();

        let accepts_br = accept_encoding.contains("br");
        let accepts_gzip = accept_encoding.contains("gzip");

        // Try to serve pre-compressed files (prefer brotli over gzip)
        let br_path = PathBuf::from(format!("{}.br", file_path.display()));
        let gz_path = PathBuf::from(format!("{}.gz", file_path.display()));
        let (serve_path, encoding) = if accepts_br && br_path.exists() {
            (br_path, Some("br"))
        } else if accepts_gzip && gz_path.exists() {
            (gz_path, Some("gzip"))
        } else {
            (file_path.clone(), None)
        };

        // Read and serve the file
        match fs::read(&serve_path) {
            Ok(content) => {
                let content_type = guess_content_type(&file_path); // Use original path for content-type

                let mut response = tiny_http::Response::from_data(content)
                    .with_header(
                        tiny_http::Header::from_bytes(&b"Content-Type"[..], content_type.as_bytes())
                            .unwrap(),
                    );

                if let Some(enc) = encoding {
                    response = response.with_header(
                        tiny_http::Header::from_bytes(&b"Content-Encoding"[..], enc.as_bytes())
                            .unwrap(),
                    );
                }

                let _ = request.respond(response);
            }
            Err(_) => {
                let response = tiny_http::Response::from_string("Not Found")
                    .with_status_code(404);
                let _ = request.respond(response);
            }
        }
    }
}

/// Guess content type from file extension
fn guess_content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("js") => "application/javascript; charset=utf-8",
        Some("wasm") => "application/wasm",
        Some("json") => "application/json; charset=utf-8",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("svg") => "image/svg+xml",
        Some("ico") => "image/x-icon",
        _ => "application/octet-stream",
    }
}

/// Check if wasm-pack is installed, install if not
fn ensure_wasm_pack() -> Result<(), Box<dyn std::error::Error>> {
    if command_exists("wasm-pack") {
        let output = Command::new("wasm-pack").arg("--version").output()?;
        let version = String::from_utf8_lossy(&output.stdout);
        println!("  Found: {}", version.trim());
        return Ok(());
    }

    println!("  wasm-pack not found, installing...");

    // Try cargo binstall first
    let binstall = Command::new("cargo")
        .args(["binstall", "-y", "wasm-pack"])
        .status();

    if binstall.is_ok() && binstall.unwrap().success() {
        println!("  Installed via cargo binstall");
        return Ok(());
    }

    // Fall back to cargo install
    println!("  Trying cargo install...");
    let install = Command::new("cargo")
        .args(["install", "wasm-pack"])
        .status()?;

    if !install.success() {
        return Err("Failed to install wasm-pack".into());
    }

    println!("  Installed via cargo install");
    Ok(())
}

/// Check if a command exists in PATH
fn command_exists(cmd: &str) -> bool {
    Command::new(cmd)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok()
}

/// Compute bundle size information for the wasm files
fn compute_bundle_info(demo_dir: &Path) -> String {
    let pkg_dir = demo_dir.join("pkg");
    let wasm_file = pkg_dir.join("arborium_demo_bg.wasm");
    let js_file = pkg_dir.join("arborium_demo.js");

    let mut info = String::from("{");

    // WASM file
    if let Ok(metadata) = fs::metadata(&wasm_file) {
        let raw_size = metadata.len();
        let compressed_size = fs::read(&wasm_file)
            .ok()
            .and_then(|data| compress_gzip(&data).ok())
            .map(|c| c.len() as u64)
            .unwrap_or(0);
        info.push_str(&format!(
            "\"wasm\":{{\"raw\":{},\"compressed\":{}}},",
            raw_size, compressed_size
        ));
    }

    // JS file
    if let Ok(metadata) = fs::metadata(&js_file) {
        let raw_size = metadata.len();
        let compressed_size = fs::read(&js_file)
            .ok()
            .and_then(|data| compress_gzip(&data).ok())
            .map(|c| c.len() as u64)
            .unwrap_or(0);
        info.push_str(&format!(
            "\"js\":{{\"raw\":{},\"compressed\":{}}}",
            raw_size, compressed_size
        ));
    }

    info.push('}');
    info
}

/// Compress data using brotli level 5 (good balance of speed and compression)
fn compress_brotli(data: &[u8]) -> Result<Vec<u8>, std::io::Error> {
    let mut output = Vec::new();
    {
        let mut encoder = brotli::CompressorWriter::new(&mut output, 4096, 5, 22);
        encoder.write_all(data)?;
    }
    Ok(output)
}

/// Compress data using gzip (flate2) - best compression
fn compress_gzip(data: &[u8]) -> Result<Vec<u8>, std::io::Error> {
    use flate2::write::GzEncoder;
    use flate2::Compression;

    let mut encoder = GzEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(data)?;
    encoder.finish()
}

/// Compress data using brotli level 1 (fast)
fn compress_brotli_fast(data: &[u8]) -> Result<Vec<u8>, std::io::Error> {
    let mut output = Vec::new();
    {
        let mut encoder = brotli::CompressorWriter::new(&mut output, 4096, 1, 22);
        encoder.write_all(data)?;
    }
    Ok(output)
}

/// Compress data using gzip level 1 (fast)
fn compress_gzip_fast(data: &[u8]) -> Result<Vec<u8>, std::io::Error> {
    use flate2::write::GzEncoder;
    use flate2::Compression;

    let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
    encoder.write_all(data)?;
    encoder.finish()
}

/// Check for env imports in WASM files (these won't work in browsers)
/// This checks the cargo-built wasm BEFORE wasm-opt runs (which is slow)
fn check_wasm_env_imports_debug() -> Result<(), Box<dyn std::error::Error>> {
    let wasm_file = Path::new("target/wasm32-unknown-unknown/debug/arborium_demo.wasm");
    check_wasm_env_imports(wasm_file)
}

#[allow(dead_code)]
fn check_wasm_env_imports_cargo() -> Result<(), Box<dyn std::error::Error>> {
    let wasm_file = Path::new("target/wasm32-unknown-unknown/release/arborium_demo.wasm");
    check_wasm_env_imports(wasm_file)
}

fn check_wasm_env_imports(wasm_file: &Path) -> Result<(), Box<dyn std::error::Error>> {

    if !wasm_file.exists() {
        return Err(format!("WASM file not found: {}", wasm_file.display()).into());
    }

    println!("  Checking {} for env imports...", wasm_file.display());

    let output = Command::new("wasm-objdump")
        .args(["-j", "Import", "-x"])
        .arg(&wasm_file)
        .output()?;

    if !output.status.success() {
        // wasm-objdump might not be installed, just warn
        println!("  Warning: wasm-objdump not found, skipping env import check");
        println!("  Install wabt to enable this check: brew install wabt");
        return Ok(());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut env_imports = Vec::new();

    for line in stdout.lines() {
        if line.contains("<- env.") {
            env_imports.push(line.trim().to_string());
        }
    }

    if !env_imports.is_empty() {
        eprintln!("\n  ERROR: Found {} env imports that won't work in browsers:", env_imports.len());
        for import in &env_imports {
            eprintln!("    {}", import);
        }
        eprintln!();
        eprintln!("  These functions need to be provided in wasm-sysroot or avoided.");
        eprintln!("  Common causes:");
        eprintln!("    - Missing function in wasm-sysroot (add stub implementation)");
        eprintln!("    - External scanner using unavailable libc functions");
        eprintln!("    - Grammar not properly compiled for WASM target");
        return Err(format!("Found {} env imports in WASM", env_imports.len()).into());
    }

    println!("  No env imports found - WASM is browser-compatible!");
    Ok(())
}

/// Pre-compress compressible files in the demo directory (in parallel)
fn precompress_files(demo_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    use std::sync::{Arc, Mutex};
    use std::thread;

    let files_to_compress = [
        "index.html",
        "pkg/arborium_demo.js",
        "pkg/arborium_demo_bg.wasm",
    ];

    let results: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let mut handles = Vec::new();

    for file in files_to_compress {
        let file_path = demo_dir.join(file);
        if !file_path.exists() {
            continue;
        }

        let data = fs::read(&file_path)?;
        let raw_size = data.len();
        let file_name = file.to_string();

        // Spawn gzip compression
        {
            let results = Arc::clone(&results);
            let data = data.clone();
            let file_path = file_path.clone();
            let file_name = file_name.clone();
            handles.push(thread::spawn(move || {
                let gz_path = PathBuf::from(format!("{}.gz", file_path.display()));
                let gz_data = compress_gzip(&data).expect("gzip compression failed");
                fs::write(&gz_path, &gz_data).expect("failed to write .gz file");
                let msg = format!("  {} ({}) -> .gz ({})", file_name, format_size(raw_size), format_size(gz_data.len()));
                results.lock().unwrap().push(msg);
            }));
        }

        // Spawn brotli compression in parallel
        {
            let results = Arc::clone(&results);
            handles.push(thread::spawn(move || {
                let br_path = PathBuf::from(format!("{}.br", file_path.display()));
                let br_data = compress_brotli(&data).expect("brotli compression failed");
                fs::write(&br_path, &br_data).expect("failed to write .br file");
                let msg = format!("  {} ({}) -> .br ({})", file_name, format_size(raw_size), format_size(br_data.len()));
                results.lock().unwrap().push(msg);
            }));
        }
    }

    // Wait for all threads to complete
    for handle in handles {
        handle.join().expect("compression thread panicked");
    }

    // Print results
    let results = results.lock().unwrap();
    for msg in results.iter() {
        println!("{}", msg);
    }

    Ok(())
}

/// Pre-compress files with fast (low quality) settings for dev mode
fn precompress_files_fast(demo_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    use std::sync::{Arc, Mutex};
    use std::thread;

    let files_to_compress = [
        "index.html",
        "pkg/arborium_demo.js",
        "pkg/arborium_demo_bg.wasm",
    ];

    let results: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let mut handles = Vec::new();

    for file in files_to_compress {
        let file_path = demo_dir.join(file);
        if !file_path.exists() {
            continue;
        }

        let data = fs::read(&file_path)?;
        let raw_size = data.len();
        let file_name = file.to_string();

        // Spawn fast gzip compression
        {
            let results = Arc::clone(&results);
            let data = data.clone();
            let file_path = file_path.clone();
            let file_name = file_name.clone();
            handles.push(thread::spawn(move || {
                let gz_path = PathBuf::from(format!("{}.gz", file_path.display()));
                let gz_data = compress_gzip_fast(&data).expect("gzip compression failed");
                fs::write(&gz_path, &gz_data).expect("failed to write .gz file");
                let msg = format!("  {} ({}) -> .gz ({})", file_name, format_size(raw_size), format_size(gz_data.len()));
                results.lock().unwrap().push(msg);
            }));
        }

        // Spawn fast brotli compression in parallel
        {
            let results = Arc::clone(&results);
            handles.push(thread::spawn(move || {
                let br_path = PathBuf::from(format!("{}.br", file_path.display()));
                let br_data = compress_brotli_fast(&data).expect("brotli compression failed");
                fs::write(&br_path, &br_data).expect("failed to write .br file");
                let msg = format!("  {} ({}) -> .br ({})", file_name, format_size(raw_size), format_size(br_data.len()));
                results.lock().unwrap().push(msg);
            }));
        }
    }

    // Wait for all threads to complete
    for handle in handles {
        handle.join().expect("compression thread panicked");
    }

    // Print results
    let results = results.lock().unwrap();
    for msg in results.iter() {
        println!("{}", msg);
    }

    Ok(())
}

fn format_size(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

// =============================================================================
// Demo generation
// =============================================================================

/// Mapping from file extension to language ID
fn extension_to_lang_id(ext: &str) -> Option<&'static str> {
    match ext {
        "rs" => Some("rust"),
        "js" => Some("javascript"),
        "ts" => Some("typescript"),
        "py" => Some("python"),
        "rb" => Some("ruby"),
        "go" => Some("go"),
        "java" => Some("java"),
        "c" => Some("c"),
        "cpp" => Some("cpp"),
        "h" => Some("c"),
        "hpp" => Some("cpp"),
        "cs" => Some("csharp"),
        "fs" => Some("fsharp"),
        "hs" => Some("haskell"),
        "lua" => Some("lua"),
        "php" => Some("php"),
        "sh" => Some("bash"),
        "bash" => Some("bash"),
        "zsh" => Some("bash"),
        "html" => Some("html"),
        "css" => Some("css"),
        "scss" => Some("scss"),
        "json" => Some("json"),
        "yaml" => Some("yaml"),
        "yml" => Some("yaml"),
        "toml" => Some("toml"),
        "xml" => Some("xml"),
        "sql" => Some("sql"),
        "md" => Some("markdown"),
        "scala" => Some("scala"),
        "swift" => Some("swift"),
        "kt" => Some("kotlin"),
        "dart" => Some("dart"),
        "ex" => Some("elixir"),
        "exs" => Some("elixir"),
        "clj" => Some("clojure"),
        "elm" => Some("elm"),
        "zig" => Some("zig"),
        "nix" => Some("nix"),
        "diff" => Some("diff"),
        "dockerfile" => Some("dockerfile"),
        "ini" => Some("ini"),
        "meson" => Some("meson"),
        "vb" => Some("vb"),
        "asm" => Some("asm"),
        "gleam" => Some("gleam"),
        _ => None,
    }
}

/// Escape a string for use in a JavaScript string literal
fn escape_for_js(s: &str) -> String {
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

// =============================================================================
// README generation
// =============================================================================

/// Generate README.md from GRAMMARS.toml
fn generate_readme() {
    let repo_root = find_repo_root().expect("Could not find repo root");

    println!("Generating README.md from GRAMMARS.toml...\n");

    // Parse GRAMMARS.toml
    let grammars = match parse_grammars_toml(&repo_root) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("Error parsing GRAMMARS.toml: {}", e);
            std::process::exit(1);
        }
    };

    // Group grammars by license type
    let mut permissive: Vec<&GrammarConfig> = Vec::new();
    let mut gpl: Vec<&GrammarConfig> = Vec::new();

    for grammar in grammars.values() {
        if grammar.license.contains("GPL") {
            gpl.push(grammar);
        } else {
            permissive.push(grammar);
        }
    }

    // Sort by name
    permissive.sort_by(|a, b| a.name.cmp(&b.name));
    gpl.sort_by(|a, b| a.name.cmp(&b.name));

    let total_count = grammars.len();
    let permissive_count = permissive.len();
    let gpl_count = gpl.len();

    // Generate permissive grammars table
    let mut permissive_table = String::new();
    permissive_table.push_str("| Feature | Language | License | Source |\n");
    permissive_table.push_str("|---------|----------|---------|--------|\n");
    for grammar in &permissive {
        let feature = format!("`lang-{}`", grammar.name);
        let display_name = grammar_display_name(&grammar.name);
        let repo_link = if grammar.repo == "local" {
            "local".to_string()
        } else {
            format!("[tree-sitter-{}]({})", grammar.name, grammar.repo)
        };
        permissive_table.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            feature, display_name, grammar.license, repo_link
        ));
    }

    // Generate GPL grammars table
    let mut gpl_table = String::new();
    gpl_table.push_str("| Feature | Language | License | Source |\n");
    gpl_table.push_str("|---------|----------|---------|--------|\n");
    for grammar in &gpl {
        let feature = format!("`lang-{}`", grammar.name);
        let display_name = grammar_display_name(&grammar.name);
        let repo_link = format!("[tree-sitter-{}]({})", grammar.name, grammar.repo);
        gpl_table.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            feature, display_name, grammar.license, repo_link
        ));
    }

    // Read template
    let template_path = repo_root.join("README.template.md");
    let template = match fs::read_to_string(&template_path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Error reading README.template.md: {}", e);
            eprintln!("Please create README.template.md with placeholders:");
            eprintln!("  {{{{TOTAL_COUNT}}}} - total number of grammars");
            eprintln!("  {{{{PERMISSIVE_COUNT}}}} - number of permissive grammars");
            eprintln!("  {{{{GPL_COUNT}}}} - number of GPL grammars");
            eprintln!("  {{{{PERMISSIVE_TABLE}}}} - table of permissive grammars");
            eprintln!("  {{{{GPL_TABLE}}}} - table of GPL grammars");
            std::process::exit(1);
        }
    };

    // Replace placeholders
    let output = template
        .replace("{{TOTAL_COUNT}}", &total_count.to_string())
        .replace("{{PERMISSIVE_COUNT}}", &permissive_count.to_string())
        .replace("{{GPL_COUNT}}", &gpl_count.to_string())
        .replace("{{PERMISSIVE_TABLE}}", permissive_table.trim())
        .replace("{{GPL_TABLE}}", gpl_table.trim());

    // Write output
    let output_path = repo_root.join("README.md");
    match fs::write(&output_path, &output) {
        Ok(_) => {
            println!("  Generated README.md:");
            println!("    - {} total grammars", total_count);
            println!("    - {} permissive (MIT/Apache/CC0/Unlicense)", permissive_count);
            println!("    - {} GPL-licensed", gpl_count);
        }
        Err(e) => {
            eprintln!("Error writing README.md: {}", e);
            std::process::exit(1);
        }
    }

    println!("\nDone!");
}

/// Convert grammar name to display name
fn grammar_display_name(name: &str) -> String {
    match name {
        "asm" => "Assembly".to_string(),
        "bash" => "Bash".to_string(),
        "c" => "C".to_string(),
        "c-sharp" => "C#".to_string(),
        "clojure" => "Clojure".to_string(),
        "cpp" => "C++".to_string(),
        "css" => "CSS".to_string(),
        "dart" => "Dart".to_string(),
        "diff" => "Diff".to_string(),
        "dockerfile" => "Dockerfile".to_string(),
        "elixir" => "Elixir".to_string(),
        "elm" => "Elm".to_string(),
        "fsharp" => "F#".to_string(),
        "gleam" => "Gleam".to_string(),
        "go" => "Go".to_string(),
        "haskell" => "Haskell".to_string(),
        "html" => "HTML".to_string(),
        "idris" => "Idris".to_string(),
        "ini" => "INI".to_string(),
        "java" => "Java".to_string(),
        "javascript" => "JavaScript".to_string(),
        "jinja2" => "Jinja2".to_string(),
        "json" => "JSON".to_string(),
        "lean" => "Lean".to_string(),
        "lua" => "Lua".to_string(),
        "markdown" => "Markdown".to_string(),
        "meson" => "Meson".to_string(),
        "nginx" => "nginx".to_string(),
        "nix" => "Nix".to_string(),
        "perl" => "Perl".to_string(),
        "php" => "PHP".to_string(),
        "python" => "Python".to_string(),
        "r" => "R".to_string(),
        "ron" => "RON".to_string(),
        "ruby" => "Ruby".to_string(),
        "rust" => "Rust".to_string(),
        "scala" => "Scala".to_string(),
        "scss" => "SCSS".to_string(),
        "sql" => "SQL".to_string(),
        "toml" => "TOML".to_string(),
        "typescript" => "TypeScript".to_string(),
        "verilog" => "Verilog".to_string(),
        "wasm" => "WebAssembly".to_string(),
        "xml" => "XML".to_string(),
        "yaml" => "YAML".to_string(),
        "zig" => "Zig".to_string(),
        "zsh" => "Zsh".to_string(),
        "caddy" => "Caddyfile".to_string(),
        "devicetree" => "Device Tree".to_string(),
        "starlark" => "Starlark".to_string(),
        "hcl" => "HCL (Terraform)".to_string(),
        "glsl" => "GLSL".to_string(),
        "vim" => "Vimscript".to_string(),
        "objc" => "Objective-C".to_string(),
        "vue" => "Vue".to_string(),
        "svelte" => "Svelte".to_string(),
        "hlsl" => "HLSL".to_string(),
        "thrift" => "Thrift".to_string(),
        "capnp" => "Cap'n Proto".to_string(),
        "awk" => "AWK".to_string(),
        "batch" => "Batch".to_string(),
        "jq" => "jq".to_string(),
        "vb" => "Visual Basic".to_string(),
        "powershell" => "PowerShell".to_string(),
        "vhdl" => "VHDL".to_string(),
        "rescript" => "ReScript".to_string(),
        "postscript" => "PostScript".to_string(),
        "tlaplus" => "TLA+".to_string(),
        "prolog" => "Prolog".to_string(),
        "x86asm" => "x86 Assembly".to_string(),
        _ => {
            // Capitalize first letter
            let mut chars = name.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
            }
        }
    }
}

/// Generate demo/index.html from template and example files
fn generate_demo() {
    let repo_root = find_repo_root().expect("Could not find repo root");
    let demo_dir = repo_root.join("demo");
    let template_path = demo_dir.join("template.html");
    let examples_dir = demo_dir.join("examples");
    let output_path = demo_dir.join("index.html");

    println!("Generating demo/index.html...\n");

    // Read template
    let template = match fs::read_to_string(&template_path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Error reading template.html: {}", e);
            std::process::exit(1);
        }
    };

    // Read all example files
    let mut examples: BTreeMap<String, String> = BTreeMap::new();

    let entries = match fs::read_dir(&examples_dir) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("Error reading examples directory: {}", e);
            std::process::exit(1);
        }
    };

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let file_name = path.file_name().unwrap().to_string_lossy();
        let stem = path.file_stem().unwrap().to_string_lossy();
        let ext = path.extension().map(|e| e.to_string_lossy().to_string());

        // Determine language ID
        // First, check if the stem itself is a known language (e.g., "x86asm.asm" -> "x86asm")
        let lang_id = if stem == "x86asm" {
            "x86asm".to_string()
        } else if let Some(ext) = &ext {
            // Try to map extension to language
            if let Some(id) = extension_to_lang_id(ext) {
                id.to_string()
            } else {
                // Use stem as fallback
                stem.to_string()
            }
        } else {
            stem.to_string()
        };

        // Read file content
        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("  Warning: Could not read {}: {}", file_name, e);
                continue;
            }
        };

        println!("  {} -> {}", file_name, lang_id);
        examples.insert(lang_id, content);
    }

    println!("\n  Read {} example files", examples.len());

    // Build JavaScript object
    let mut js_obj = String::from("{\n");
    for (i, (lang_id, content)) in examples.iter().enumerate() {
        let escaped = escape_for_js(content);
        js_obj.push_str(&format!("            \"{}\": \"{}\"", lang_id, escaped));
        if i < examples.len() - 1 {
            js_obj.push(',');
        }
        js_obj.push('\n');
    }
    js_obj.push_str("        }");

    // Replace placeholder in template
    let output = template.replace("{{EXAMPLES}}", &js_obj);

    // Write output
    match fs::write(&output_path, &output) {
        Ok(_) => {
            println!("\n  Written to {}", output_path.display());
            println!("  Output size: {}", format_size(output.len()));
        }
        Err(e) => {
            eprintln!("Error writing index.html: {}", e);
            std::process::exit(1);
        }
    }

    println!("\nDone!");
}

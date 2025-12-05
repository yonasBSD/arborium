//! Generate command - regenerates crate files from arborium.kdl.
//!
//! This command reads arborium.kdl files and generates:
//! - Cargo.toml
//! - build.rs
//! - src/lib.rs
//! - grammar/src/ (by running tree-sitter generate)

use crate::cache::GrammarCache;
use crate::plan::{Operation, Plan, PlanMode, PlanSet};
use crate::tool::Tool;
use crate::types::{CrateRegistry, CrateState};
use crate::util::find_repo_root;
use camino::{Utf8Path, Utf8PathBuf};
use fs_err as fs;
// Removed indicatif imports since we no longer use spinners for fast operations
use owo_colors::OwoColorize;
use rayon::prelude::*;
use rootcause::Report;
use std::io::{IsTerminal, Write};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// Context for build operations - contains shared state and configuration
struct BuildContext<'a> {
    cache: &'a GrammarCache,
    crates_dir: &'a Utf8Path,
    repo_root: &'a Utf8Path,
    cache_hits: &'a AtomicUsize,
    cache_misses: &'a AtomicUsize,
    mode: PlanMode,
    workspace_version: &'a str,
}

/// Update root Cargo.toml with the specified version
fn update_root_cargo_toml(repo_root: &Utf8Path, version: &str) -> Result<(), Report> {
    use regex::Regex;

    let cargo_toml_path = repo_root.join("Cargo.toml");
    let content = fs::read_to_string(&cargo_toml_path)?;

    // Update [workspace.package] version
    let workspace_version_re =
        Regex::new(r#"(?m)^(\[workspace\.package\][\s\S]*?version\s*=\s*)"[^"]*""#)
            .map_err(|e| std::io::Error::other(format!("Failed to compile regex: {e}")))?;
    let content = workspace_version_re.replace(&content, format!(r#"$1"{version}""#));

    // Update all version = "X.Y.Z" in [workspace.dependencies] section
    // Match lines like: arborium-ada = { path = "...", version = "X.Y.Z" }
    // Also matches: arborium = { path = "...", version = "X.Y.Z" }
    let dep_version_re =
        Regex::new(r#"(?m)^(arborium(?:-[a-z0-9_-]+)?\s*=\s*\{[^}]*version\s*=\s*)"[^"]*""#)
            .map_err(|e| std::io::Error::other(format!("Failed to compile regex: {e}")))?;
    let content = dep_version_re.replace_all(&content, format!(r#"$1"{version}""#));

    fs::write(&cargo_toml_path, content.as_ref())?;
    Ok(())
}

/// Generate [workspace.dependencies] section from registry
fn generate_workspace_dependencies(
    repo_root: &Utf8Path,
    registry: &CrateRegistry,
    version: &str,
) -> Result<(), Report> {
    let cargo_toml_path = repo_root.join("Cargo.toml");
    let content = fs::read_to_string(&cargo_toml_path)?;

    // Collect all grammar crate names (sorted for deterministic output)
    let mut crate_names: Vec<&str> = registry
        .crates
        .keys()
        .map(|s| s.as_str())
        .filter(|name| name.starts_with("arborium-"))
        .collect();
    crate_names.sort();

    // Build the [workspace.dependencies] section
    let mut deps_section = String::from("\n[workspace.dependencies]\n");
    // Include the umbrella crate itself
    deps_section.push_str(&format!(
        "arborium = {{ path = \"crates/arborium\", version = \"{}\" }}\n",
        version
    ));
    for crate_name in &crate_names {
        deps_section.push_str(&format!(
            "{} = {{ path = \"crates/{}\", version = \"{}\" }}\n",
            crate_name, crate_name, version
        ));
    }

    // Check if [workspace.dependencies] already exists
    let new_content = if let Some(start) = content.find("\n[workspace.dependencies]") {
        // Find the next section (or end of file)
        let after_header = start + 1; // skip the leading newline
        let section_end = content[after_header..]
            .find("\n[")
            .map(|i| after_header + i)
            .unwrap_or(content.len());
        // Replace the section
        format!(
            "{}{}{}",
            &content[..start],
            deps_section,
            &content[section_end..]
        )
    } else {
        // Insert before [workspace.package]
        content.replace(
            "\n[workspace.package]",
            &format!("{}\n[workspace.package]", deps_section),
        )
    };

    fs::write(&cargo_toml_path, new_content)?;
    Ok(())
}

/// Generate crate files for all or a specific grammar.
pub fn plan_generate(
    crates_dir: &Utf8Path,
    name: Option<&str>,
    mode: PlanMode,
    version: &str,
    no_fail_fast: bool,
) -> Result<PlanSet, Report> {
    use std::time::Instant;
    let total_start = Instant::now();

    // Note: lint is run by main.rs before and after calling this function
    let registry_start = Instant::now();
    let registry = CrateRegistry::load(crates_dir)?;
    let registry_elapsed = registry_start.elapsed();

    // Set up grammar cache
    let repo_root =
        find_repo_root().ok_or_else(|| std::io::Error::other("Could not find repo root"))?;
    let repo_root = Utf8PathBuf::from_path_buf(repo_root)
        .map_err(|_| std::io::Error::other("Non-UTF8 repo root"))?;
    let cache = GrammarCache::new(&repo_root);

    // Update root Cargo.toml with the specified version
    update_root_cargo_toml(&repo_root, version)?;

    // Generate [workspace.dependencies] from registry
    generate_workspace_dependencies(&repo_root, &registry, version)?;

    // Use the provided version for generated Cargo.toml files
    let workspace_version = version.to_string();

    // Track cache stats
    let cache_hits = AtomicUsize::new(0);
    let cache_misses = AtomicUsize::new(0);

    // Collect crates to process (respecting filter)
    let crates_to_process: Vec<_> = registry
        .crates
        .iter()
        .filter(|(_name, crate_state)| {
            // Skip if a specific name was requested and this isn't it
            if let Some(filter) = name {
                let matches = crate_state.name == filter
                    || (crate_state.name.strip_prefix("arborium-") == Some(filter));
                if !matches {
                    return false;
                }
            }
            // Skip crates without arborium.kdl
            crate_state.config.is_some()
        })
        .collect();

    if crates_to_process.is_empty() {
        return Ok(PlanSet::new());
    }

    // Pre-generation grammar validation
    println!("{}", "Validating grammar dependencies...".cyan().bold());
    for (_name, crate_state) in &crates_to_process {
        if let Some(config) = &crate_state.config {
            // Only validate grammars that have external dependencies
            let has_dependencies = !get_grammar_dependencies(config).is_empty();
            if has_dependencies {
                validate_grammar_requires(crate_state, config)?;
            }
        }
    }
    println!("{} All grammar requires validated", "✓".green());
    println!();

    // Store length before potentially consuming the vector
    let num_crates_to_process = crates_to_process.len();

    // Check if we're in a terminal (for spinners) or CI (for plain output)
    let is_tty = std::io::stdout().is_terminal();

    // Set up multi-progress for parallel spinners (only used in TTY mode)
    // No longer using spinners for fast operations

    // Thread-safe collection for plans and errors
    let plans = Mutex::new(PlanSet::new());
    let errors: Mutex<Vec<(String, Report)>> = Mutex::new(Vec::new());

    // Process crates in parallel - always parallel, but handle errors differently
    if no_fail_fast {
        // Parallel processing - collect all errors
        crates_to_process
            .par_iter()
            .for_each(|(_name, crate_state)| {
                let config = crate_state.config.as_ref().unwrap();
                let crate_name = &crate_state.name;

                // Check if this crate has a grammar to generate
                let grammar_dir = crate_state.path.join("grammar");
                let needs_generation =
                    grammar_dir.exists() && grammar_dir.join("grammar.js").exists();

                // Track timing for slow operations
                let start_time = std::time::Instant::now();

                // Progress is shown later in cache miss/hit messages

                let ctx = BuildContext {
                    cache: &cache,
                    crates_dir,
                    repo_root: &repo_root,
                    cache_hits: &cache_hits,
                    cache_misses: &cache_misses,
                    mode,
                    workspace_version: &workspace_version,
                };

                match plan_crate_generation(crate_state, config, &ctx) {
                    Ok(plan) => {
                        if !plan.is_empty() {
                            plans.lock().unwrap().add(plan);
                        }

                        // Show completion message for slow operations (>1s) in TTY mode
                        let elapsed = start_time.elapsed();
                        if needs_generation && is_tty && elapsed.as_secs() >= 1 {
                            println!(
                                "{} {} completed in {:.1}s",
                                "●".green(),
                                crate_name,
                                elapsed.as_secs_f64()
                            );
                        }
                    }
                    Err(e) => {
                        // Collect error - don't fail fast
                        if needs_generation {
                            println!("{} {} {}", "●".red(), "✗".red(), crate_name);
                        }
                        errors.lock().unwrap().push((crate_name.clone(), e));
                    }
                }
            });
    } else {
        // Parallel processing with fail-fast - stop on first error
        use std::sync::atomic::AtomicBool;
        let should_stop = AtomicBool::new(false);
        let first_error = Mutex::new(None::<(String, Report)>);

        crates_to_process
            .par_iter()
            .try_for_each(|(_name, crate_state)| -> Result<(), ()> {
                // Check if we should stop due to an earlier error
                if should_stop.load(Ordering::Relaxed) {
                    return Err(());
                }

                let config = crate_state.config.as_ref().unwrap();
                let crate_name = &crate_state.name;

                // Check if this crate has a grammar to generate
                let grammar_dir = crate_state.path.join("grammar");
                let needs_generation =
                    grammar_dir.exists() && grammar_dir.join("grammar.js").exists();

                // Track timing for slow operations
                let start_time = std::time::Instant::now();

                let ctx = BuildContext {
                    cache: &cache,
                    crates_dir,
                    repo_root: &repo_root,
                    cache_hits: &cache_hits,
                    cache_misses: &cache_misses,
                    mode,
                    workspace_version: &workspace_version,
                };

                match plan_crate_generation(crate_state, config, &ctx) {
                    Ok(plan) => {
                        if !plan.is_empty() {
                            plans.lock().unwrap().add(plan);
                        }

                        // Show completion message for slow operations (>1s) in TTY mode
                        let elapsed = start_time.elapsed();
                        if needs_generation && is_tty && elapsed.as_secs() >= 1 {
                            println!(
                                "{} {} completed in {:.1}s",
                                "●".green(),
                                crate_name,
                                elapsed.as_secs_f64()
                            );
                        }
                        Ok(())
                    }
                    Err(e) => {
                        // Fail fast - signal stop and store first error
                        if needs_generation {
                            println!("{} ✗ {}", "●".red(), crate_name);
                        }
                        should_stop.store(true, Ordering::Relaxed);
                        *first_error.lock().unwrap() = Some((crate_name.clone(), e));
                        Err(())
                    }
                }
            })
            .ok(); // Ignore the Result from try_for_each, we handle errors below

        // Check if we had a fail-fast error
        if let Some((_crate_name, error)) = first_error.into_inner().unwrap() {
            return Err(error);
        }
    }

    let processing_elapsed = total_start.elapsed();

    // Print timing and cache stats
    let hits = cache_hits.load(Ordering::Relaxed);
    let misses = cache_misses.load(Ordering::Relaxed);
    let total_cached = hits + misses;
    let cache_hit_rate = if total_cached > 0 {
        (hits as f64 / total_cached as f64) * 100.0
    } else {
        0.0
    };

    println!();
    println!("{}", "=".repeat(80));
    println!("{} Generation Summary", "●".cyan().bold());
    println!("{}", "=".repeat(80));

    // What was processed
    println!(
        "{} Processed: {} crates",
        "●".cyan(),
        num_crates_to_process.to_string().bold()
    );

    // Detailed timing breakdown
    let generation_time = (processing_elapsed - registry_elapsed).as_secs_f64();

    println!(
        "{} Total time: {:.2}s",
        "●".cyan(),
        processing_elapsed.as_secs_f64().to_string().bold()
    );
    println!(
        "  - Registry loading: {:.2}s ({:.1}%)",
        registry_elapsed.as_secs_f64(),
        (registry_elapsed.as_secs_f64() / processing_elapsed.as_secs_f64()) * 100.0
    );
    println!(
        "  - Generation phase: {:.2}s ({:.1}%)",
        generation_time,
        (generation_time / processing_elapsed.as_secs_f64()) * 100.0
    );
    println!("    - includes: tree-sitter CLI, file templating, cache operations");

    // Cache statistics
    if total_cached > 0 {
        println!("{} Cache performance:", "●".green().bold());
        println!(
            "  - {} hits ({:.1}%)",
            hits.to_string().green().bold(),
            cache_hit_rate
        );
        println!(
            "  - {} misses ({:.1}%)",
            misses.to_string().yellow().bold(),
            100.0 - cache_hit_rate
        );
        println!("  - Time saved: ~{:.1}s (estimated)", hits as f64 * 2.0); // rough estimate

        if hits > 0 {
            println!("  {} Grammar files restored from cache", "✓".green());
        }
        if misses > 0 {
            println!(
                "  {} Grammar files regenerated with tree-sitter CLI",
                "⚡".yellow()
            );
        }
    }

    // Recommend next commands based on results (following PUBLISH.md process)
    println!();
    println!("{} Recommended next steps:", "💡".bright_yellow().bold());

    if hits > 0 && misses == 0 {
        println!(
            "  {} All grammars were cached - run tests to verify:",
            "●".green()
        );
        println!("    {}", "cargo nextest run".bold());
        println!(
            "  {} Or serve the demo to test syntax highlighting:",
            "●".cyan()
        );
        println!("    {}", "cargo xtask serve".bold());
    } else if misses > 0 {
        println!(
            "  {} New grammars generated - verify with tests:",
            "●".yellow()
        );
        println!("    {}", "cargo nextest run".bold());
        println!("  {} Build WASM components for web demos:", "●".blue());
        println!("    {}", "cargo xtask plugins".bold());
        println!("  {} Test syntax highlighting in browser:", "●".cyan());
        println!("    {}", "cargo xtask serve".bold());
        println!();
        println!(
            "  {} Ready to release? Follow the PUBLISH.md process:",
            "●".green().bold()
        );
        println!("    {}", "# 1. Tag and push core release".dimmed());
        println!("    {}", "cargo xtask tag --core".bold());
        println!("    {}", "# 2. Tag and push groups one by one".dimmed());
        println!("    {}", "cargo xtask tag --group squirrel".bold());
        println!("    {}", "cargo xtask tag --group deer".bold());
        println!("    {}", "# ... (repeat for other groups)".dimmed());
    } else {
        println!(
            "  {} No changes needed - you're up to date! 🎉",
            "●".green()
        );
        println!("  {} Run tests to verify everything works:", "●".cyan());
        println!("    {}", "cargo nextest run".bold());
    }

    // Check for errors - only relevant in no-fail-fast mode
    // (In fail-fast mode, we would have already returned with the first error)
    if no_fail_fast {
        let errors = errors.into_inner().unwrap();
        if !errors.is_empty() {
            eprintln!();
            for (crate_name, error) in &errors {
                eprintln!("Error: {}: {}", crate_name.bold(), error);
            }
            Err(std::io::Error::other(format!(
                "{} grammar(s) failed to generate",
                errors.len()
            )))?;
        }
    }

    Ok(plans.into_inner().unwrap())
}

fn plan_crate_generation(
    crate_state: &CrateState,
    config: &crate::types::CrateConfig,
    ctx: &BuildContext,
) -> Result<Plan, Report> {
    let mut plan = Plan::for_crate(&crate_state.name);
    let def_path = &crate_state.def_path;
    let crate_path = &crate_state.crate_path;

    // Ensure crate directory exists
    if !crate_path.exists() {
        plan.add(Operation::CreateDir {
            path: crate_path.to_owned(),
            description: "Create crate directory".to_string(),
        });
    }

    // Generate Cargo.toml
    let cargo_toml_path = crate_path.join("Cargo.toml");
    let new_cargo_toml = generate_cargo_toml(&crate_state.name, config, ctx.workspace_version);

    if cargo_toml_path.exists() {
        let old_content = fs::read_to_string(&cargo_toml_path)?;
        if old_content != new_cargo_toml {
            plan.add(Operation::UpdateFile {
                path: cargo_toml_path,
                old_content: Some(old_content),
                new_content: new_cargo_toml,
                description: "Update Cargo.toml".to_string(),
            });
        }
    } else {
        plan.add(Operation::CreateFile {
            path: cargo_toml_path,
            content: new_cargo_toml,
            description: "Create Cargo.toml".to_string(),
        });
    }

    // Generate build.rs
    let build_rs_path = crate_path.join("build.rs");
    let new_build_rs = generate_build_rs(&crate_state.name, config);

    if build_rs_path.exists() {
        let old_content = fs::read_to_string(&build_rs_path)?;
        if old_content != new_build_rs {
            plan.add(Operation::UpdateFile {
                path: build_rs_path,
                old_content: Some(old_content),
                new_content: new_build_rs,
                description: "Update build.rs".to_string(),
            });
        }
    } else {
        plan.add(Operation::CreateFile {
            path: build_rs_path,
            content: new_build_rs,
            description: "Create build.rs".to_string(),
        });
    }

    // Generate src/lib.rs
    let lib_rs_path = crate_path.join("src/lib.rs");
    let new_lib_rs = generate_lib_rs(&crate_state.name, def_path, config);

    if lib_rs_path.exists() {
        let old_content = fs::read_to_string(&lib_rs_path)?;
        if old_content != new_lib_rs {
            plan.add(Operation::UpdateFile {
                path: lib_rs_path,
                old_content: Some(old_content),
                new_content: new_lib_rs,
                description: "Update src/lib.rs".to_string(),
            });
        }
    } else {
        // Ensure src/ directory exists
        let src_dir = crate_path.join("src");
        if !src_dir.exists() {
            plan.add(Operation::CreateDir {
                path: src_dir,
                description: "Create src directory".to_string(),
            });
        }
        plan.add(Operation::CreateFile {
            path: lib_rs_path,
            content: new_lib_rs,
            description: "Create src/lib.rs".to_string(),
        });
    }

    // Generate grammar/src/ from vendored grammar sources in def/
    let grammar_dir = def_path.join("grammar");

    if grammar_dir.exists() && grammar_dir.join("grammar.js").exists() {
        plan_grammar_src_generation(&mut plan, def_path, config, ctx)?;
    }

    Ok(plan)
}

/// Get the cross-grammar dependencies for a grammar.
/// Returns a list of (npm_package_name, arborium_crate_name) tuples.
fn get_grammar_dependencies(config: &crate::types::CrateConfig) -> Vec<(String, String)> {
    let mut deps = Vec::new();

    for grammar in &config.grammars {
        for dep in &grammar.dependencies {
            deps.push((dep.npm.clone(), dep.krate.clone()));
        }
    }

    deps
}

/// Set up node_modules with copies of dependency grammars for tree-sitter generate.
/// This is only used during generation (dev time), not at crate build time.
fn setup_grammar_dependencies(
    temp_path: &Utf8Path,
    crates_dir: &Utf8Path,
    config: &crate::types::CrateConfig,
) -> Result<(), Report> {
    let deps = get_grammar_dependencies(config);
    if deps.is_empty() {
        return Ok(());
    }

    let node_modules = temp_path.join("node_modules");
    fs::create_dir_all(&node_modules)?;

    // Try to find repo root to look for langs/ directory
    let repo_root = crates_dir.parent().expect("crates_dir should have parent");
    let langs_dir = repo_root.join("langs");

    for (npm_name, arborium_name) in deps {
        let target_dir = node_modules.join(&npm_name);

        // Extract language name from arborium crate name
        let lang_name = arborium_name
            .strip_prefix("arborium-")
            .unwrap_or(&arborium_name);

        // Try new structure first: langs/group-*/lang/def/grammar
        let mut dep_grammar_dir = None;
        if langs_dir.exists() {
            // Search through all groups for this language
            if let Ok(entries) = fs::read_dir(&langs_dir) {
                for group_entry in entries.flatten() {
                    let group_path = group_entry.path();
                    if group_path.is_dir() {
                        let lang_path = group_path.join(lang_name);
                        let grammar_path = lang_path.join("def").join("grammar");
                        if grammar_path.exists() {
                            dep_grammar_dir = Some(
                                Utf8PathBuf::from_path_buf(grammar_path).expect("non-UTF8 path"),
                            );
                            break;
                        }
                    }
                }
            }
        }

        // Fall back to old structure: crates/arborium-*/grammar
        if dep_grammar_dir.is_none() {
            let old_path = crates_dir.join(&arborium_name).join("grammar");
            if old_path.exists() {
                dep_grammar_dir = Some(old_path);
            }
        }

        if let Some(grammar_dir) = dep_grammar_dir {
            // Copy the dependency's grammar files to node_modules
            copy_dir_contents(&grammar_dir, &target_dir)?;
        }
    }

    Ok(())
}

/// Plan the generation of grammar/src/ by running tree-sitter generate in a temp directory.
fn plan_grammar_src_generation(
    plan: &mut Plan,
    def_path: &Utf8Path,
    config: &crate::types::CrateConfig,
    ctx: &BuildContext,
) -> Result<(), Report> {
    let grammar_dir = def_path.join("grammar");
    let dest_src_dir = grammar_dir.join("src");
    let crate_name = def_path
        .parent()
        .and_then(|p| p.file_name())
        .unwrap_or("unknown");

    // Compute cache key from input files
    let cache_key = ctx
        .cache
        .compute_cache_key(def_path, ctx.crates_dir, config)?;

    // Check cache first
    if let Some(cached) = ctx.cache.get(crate_name, &cache_key) {
        // Cache hit! Extract to a temp dir first, then plan updates
        ctx.cache_hits.fetch_add(1, Ordering::Relaxed);

        // Print cache hit info
        let short_key = &cache_key[..8.min(cache_key.len())];
        let cache_path = ctx
            .cache
            .cache_dir
            .strip_prefix(ctx.repo_root)
            .unwrap_or(&ctx.cache.cache_dir)
            .join(crate_name)
            .join(short_key);
        println!(
            "● {} ({}: {}, re-using cache {})",
            crate_name.green(),
            "up-to-date".green(),
            short_key,
            cache_path
        );

        let temp_dir = tempfile::tempdir()?;
        let temp_src = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf())
            .map_err(|_| std::io::Error::other("Non-UTF8 temp path"))?;

        cached.extract_to(&temp_src)?;

        // Plan updates from cached files
        plan_updates_from_generated(&mut *plan, &temp_src, &dest_src_dir, ctx.mode)?;
        return Ok(());
    }

    // Cache miss - need to generate
    ctx.cache_misses.fetch_add(1, Ordering::Relaxed);
    let short_key = &cache_key[..8.min(cache_key.len())];
    println!(
        "● {} ({}: {}, regenerating)",
        crate_name.yellow(),
        "cache miss".yellow(),
        short_key
    );

    // Create a temp directory with same structure as the crate
    // Some grammars have `require('../common/...')` so we need to preserve the relative paths
    let temp_dir = tempfile::tempdir()?;
    let temp_root = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf())
        .map_err(|_| std::io::Error::other("Non-UTF8 temp path"))?;

    // Copy grammar/ to temp/grammar/
    let temp_grammar = temp_root.join("grammar");
    copy_dir_contents(&grammar_dir, &temp_grammar)?;

    // Copy common/ to temp/common/ if it exists (some grammars share code via ../common/)
    // Check both def/common (shared at language level) and def/grammar/common (local to grammar)
    let def_common_dir = def_path.join("common");
    let grammar_common_dir = def_path.join("grammar/common");

    if def_common_dir.exists() {
        let temp_common = temp_root.join("common");
        copy_dir_contents(&def_common_dir, &temp_common)?;
    }

    if grammar_common_dir.exists() {
        let temp_grammar_common = temp_grammar.join("common");
        copy_dir_contents(&grammar_common_dir, &temp_grammar_common)?;
    }

    // Set up cross-grammar dependencies if needed (in temp/grammar/node_modules/)
    setup_grammar_dependencies(&temp_grammar, ctx.crates_dir, config)?;

    // Create src/ directory for grammars that generate files there (e.g., vim's keywords.h)
    fs::create_dir_all(temp_grammar.join("src"))?;

    // Run tree-sitter generate in the temp/grammar directory
    let tree_sitter = Tool::TreeSitter.find()?;

    // Start progress reporting for slow operations
    let start_time = std::time::Instant::now();
    let crate_name_for_progress = crate_name.to_string();
    let should_stop = Arc::new(AtomicBool::new(false));
    let should_stop_clone = should_stop.clone();

    let progress_handle = thread::spawn(move || {
        loop {
            thread::sleep(Duration::from_secs(5));
            if should_stop_clone.load(Ordering::Relaxed) {
                break;
            }
            let elapsed = start_time.elapsed().as_secs();
            print!("{} ({}s) ", crate_name_for_progress, elapsed);
            let _ = std::io::stdout().flush();
        }
    });

    let output = tree_sitter
        .command()
        .args(["generate"])
        .current_dir(&temp_grammar)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()?;

    // Stop progress reporting
    should_stop.store(true, Ordering::Relaxed);
    let _ = progress_handle.join();

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // Show more context for debugging
        let error_lines: Vec<&str> = stderr.lines().take(20).collect();
        Err(std::io::Error::other(format!(
            "tree-sitter generate failed for {}:\n{}",
            crate_name,
            error_lines.join("\n")
        )))?;
    }

    // The generated files are in temp/grammar/src/
    let generated_src = temp_grammar.join("src");

    // Save to cache for next time
    if let Err(e) = ctx.cache.save(crate_name, &cache_key, &generated_src) {
        // Cache save failure is not fatal, just log it
        eprintln!("Warning: failed to cache {}: {}", crate_name, e);
    }

    // Plan updates from generated files
    plan_updates_from_generated(plan, &generated_src, &dest_src_dir, ctx.mode)?;

    Ok(())
}

/// Plan file updates from a generated source directory to the destination.
fn plan_updates_from_generated(
    plan: &mut Plan,
    generated_src: &Utf8Path,
    dest_src_dir: &Utf8Path,
    mode: PlanMode,
) -> Result<(), Report> {
    // Ensure grammar/src/ directory exists in plan
    if !dest_src_dir.exists() {
        plan.add(Operation::CreateDir {
            path: dest_src_dir.to_owned(),
            description: "Create grammar/src directory".to_string(),
        });
    }

    // Copy all generated files to grammar/src/
    // This includes parser.c, scanner.c, grammar.json, node-types.json, and any .h files
    for entry in fs::read_dir(generated_src)? {
        let entry = entry?;
        let file_name = entry.file_name().to_string_lossy().to_string();
        let generated_file = Utf8PathBuf::from_path_buf(entry.path())
            .map_err(|_| std::io::Error::other("Non-UTF8 path"))?;

        // Skip directories (tree_sitter/ is handled separately)
        if !generated_file.is_file() {
            continue;
        }

        let dest_file = dest_src_dir.join(&file_name);
        let new_content = fs::read_to_string(&generated_file)?;
        plan_file_update(
            plan,
            &dest_file,
            new_content,
            &format!("src/{}", file_name),
            mode,
        )?;
    }

    // Copy tree_sitter/ directory
    let generated_tree_sitter = generated_src.join("tree_sitter");
    let dest_tree_sitter = dest_src_dir.join("tree_sitter");
    if generated_tree_sitter.exists() {
        // Ensure tree_sitter/ directory exists
        if !dest_tree_sitter.exists() {
            plan.add(Operation::CreateDir {
                path: dest_tree_sitter.clone(),
                description: "Create src/tree_sitter directory".to_string(),
            });
        }

        // Copy each file in tree_sitter/
        for entry in fs::read_dir(&generated_tree_sitter)? {
            let entry = entry?;
            let file_name = entry.file_name().to_string_lossy().to_string();
            let generated_file = Utf8PathBuf::from_path_buf(entry.path())
                .map_err(|_| std::io::Error::other("Non-UTF8 path"))?;
            let dest_file = dest_tree_sitter.join(&file_name);

            if generated_file.is_file() {
                let new_content = fs::read_to_string(&generated_file)?;
                plan_file_update(
                    plan,
                    &dest_file,
                    new_content,
                    &format!("src/tree_sitter/{}", file_name),
                    mode,
                )?;
            }
        }
    }

    Ok(())
}

/// Helper to plan a file update (create or update based on whether content changed).
/// In dry-run mode, reads old content for diffing.
/// In normal mode, uses blake3 hashing to check if update is needed.
fn plan_file_update(
    plan: &mut Plan,
    dest_path: &Utf8Path,
    new_content: String,
    description: &str,
    mode: PlanMode,
) -> Result<(), Report> {
    if dest_path.exists() {
        // Hash the new content
        let new_hash = blake3::hash(new_content.as_bytes());

        // Read and hash existing file
        let old_bytes = fs::read(dest_path)?;
        let old_hash = blake3::hash(&old_bytes);

        // Only update if hashes differ
        if old_hash != new_hash {
            let old_content = if mode.is_dry_run() {
                // In dry-run mode, we need the content for diffing
                Some(String::from_utf8_lossy(&old_bytes).into_owned())
            } else {
                None
            };

            plan.add(Operation::UpdateFile {
                path: dest_path.to_owned(),
                old_content,
                new_content,
                description: format!("Update {}", description),
            });
        }
    } else {
        plan.add(Operation::CreateFile {
            path: dest_path.to_owned(),
            content: new_content,
            description: format!("Create {}", description),
        });
    }
    Ok(())
}

/// Validate grammar.js require() statements by running Node.js with dummy globals
fn validate_grammar_requires(
    crate_state: &CrateState,
    _config: &crate::types::CrateConfig,
) -> Result<(), Report> {
    let grammar_js = crate_state.def_path.join("grammar/grammar.js");

    if !grammar_js.exists() {
        return Ok(());
    }

    // Create a temporary wrapper script with dummy tree-sitter functions
    let temp_dir = tempfile::tempdir()?;
    let wrapper_path = temp_dir.path().join("validate_grammar.js");

    let wrapper_content = format!(
        r#"
// Dummy tree-sitter globals
global.grammar = () => {{}};
global.seq = (...args) => args;
global.choice = (...args) => args;
global.repeat = (rule) => rule;
global.repeat1 = (rule) => rule;
global.optional = (rule) => rule;
global.prec = (n, rule) => rule;
global.prec_left = (n, rule) => rule;
global.prec_right = (n, rule) => rule;
global.prec_dynamic = (n, rule) => rule;
global.token = (rule) => rule;
global.alias = (rule, name) => rule;
global.field = (name, rule) => rule;
global.$ = new Proxy({{}}, {{ get: () => "rule" }});

// Pattern constants (optional - grammars may define their own)
global.NEWLINE = 'newline';
global.WHITESPACE = 'whitespace';
global.IDENTIFIER = 'identifier';
global.NUMBER = 'number';
global.STRING = 'string';
global.COMMENT = 'comment';

// Try to require the grammar file - this will fail if requires are broken
try {{
    require('{}');
    console.log('OK');
}} catch (error) {{
    if (error.code === 'MODULE_NOT_FOUND') {{
        console.error('MISSING_MODULE:' + error.message);
        process.exit(1);
    }} else {{
        console.error('SYNTAX_ERROR:' + error.message);
        process.exit(2);
    }}
}}
"#,
        grammar_js.as_str().replace('\\', "\\\\")
    );

    fs::write(&wrapper_path, wrapper_content)?;

    // Run Node.js on the wrapper
    let output = std::process::Command::new("node")
        .arg(&wrapper_path)
        .current_dir(&crate_state.def_path)
        .output()
        .map_err(|e| std::io::Error::other(format!("Failed to run node: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);

        // Check if this is a missing dependency that should be handled by setup_grammar_dependencies
        if stderr.contains("Cannot find module 'tree-sitter-") {
            // This is expected - we have a cross-grammar dependency
            // Let setup_grammar_dependencies handle it during generation
            println!(
                "  {} {} - has cross-grammar dependencies (will be resolved during generation)",
                "→".blue(),
                crate_state
                    .name
                    .strip_prefix("arborium-")
                    .unwrap_or(&crate_state.name)
            );
            return Ok(());
        }

        if stdout.starts_with("MISSING_MODULE:") || stderr.contains("Cannot find module") {
            let error_msg = if stdout.starts_with("MISSING_MODULE:") {
                stdout.strip_prefix("MISSING_MODULE:").unwrap_or(&stdout)
            } else {
                &stderr
            };
            return Err(std::io::Error::other(format!(
                "Missing file dependency in {}: {}",
                crate_state
                    .name
                    .strip_prefix("arborium-")
                    .unwrap_or(&crate_state.name),
                error_msg.trim().lines().next().unwrap_or(error_msg.trim())
            ))
            .into());
        } else if stdout.starts_with("SYNTAX_ERROR:") {
            let error_msg = stdout.strip_prefix("SYNTAX_ERROR:").unwrap_or(&stdout);
            return Err(std::io::Error::other(format!(
                "Grammar syntax error in {}: {}",
                crate_state
                    .name
                    .strip_prefix("arborium-")
                    .unwrap_or(&crate_state.name),
                error_msg.trim()
            ))
            .into());
        } else {
            return Err(std::io::Error::other(format!(
                "Grammar validation failed for {}: {}",
                crate_state
                    .name
                    .strip_prefix("arborium-")
                    .unwrap_or(&crate_state.name),
                stderr.trim().lines().next().unwrap_or("unknown error")
            ))
            .into());
        }
    }

    Ok(())
}

fn copy_dir_contents(src_dir: &Utf8Path, dest_dir: &Utf8Path) -> Result<(), Report> {
    fs::create_dir_all(dest_dir)?;

    for entry in fs::read_dir(src_dir)? {
        let entry = entry?;
        let src_path = Utf8PathBuf::from_path_buf(entry.path())
            .map_err(|_| std::io::Error::other("Non-UTF8 path"))?;
        let dest_path = dest_dir.join(entry.file_name().to_string_lossy().as_ref());

        if src_path.is_dir() {
            copy_dir_contents(&src_path, &dest_path)?;
        } else {
            fs::copy(&src_path, &dest_path)?;
        }
    }

    Ok(())
}

/// Generate Cargo.toml content for a grammar crate.
fn generate_cargo_toml(
    crate_name: &str,
    config: &crate::types::CrateConfig,
    workspace_version: &str,
) -> String {
    let grammar_id = config
        .grammars
        .first()
        .map(|g| g.id.as_ref())
        .unwrap_or(crate_name.strip_prefix("arborium-").unwrap_or(crate_name));

    let _description = config
        .grammars
        .first()
        .and_then(|g| g.description.as_ref())
        .map(|d| d.as_ref())
        .unwrap_or_else(|| "tree-sitter grammar bindings");

    // Use license from arborium.kdl, fallback to MIT if empty
    let license: &str = {
        let l: &str = config.license.value.as_ref();
        if l.is_empty() { "MIT" } else { l }
    };

    format!(
        r#"[package]
name = "{crate_name}"
version = "{workspace_version}"
edition = "2024"
description = "{grammar_id} grammar for arborium (tree-sitter bindings)"
license = "{license}"
repository = "https://github.com/bearcove/arborium"
keywords = ["tree-sitter", "{grammar_id}", "syntax-highlighting"]
categories = ["parsing", "text-processing"]

[lib]
path = "src/lib.rs"

[dependencies]
tree-sitter-patched-arborium = {{ version = "0.25.10", path = "../../tree-sitter" }}
arborium-sysroot = {{ version = "{workspace_version}", path = "../arborium-sysroot" }}

[dev-dependencies]
arborium-test-harness = {{ version = "{workspace_version}", path = "../arborium-test-harness" }}

[build-dependencies]
cc = {{ version = "1", features = ["parallel"] }}
"#
    )
}

/// Generate build.rs content for a grammar crate.
fn generate_build_rs(crate_name: &str, config: &crate::types::CrateConfig) -> String {
    let grammar = config.grammars.first();
    let has_scanner = grammar.map(|g| g.has_scanner()).unwrap_or(false);

    let c_symbol: String = grammar
        .and_then(|g| g.c_symbol.as_ref())
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            crate_name
                .strip_prefix("arborium-")
                .unwrap_or(crate_name)
                .replace('-', "_")
        });

    let scanner_section = if has_scanner {
        r#"    println!("cargo:rerun-if-changed=grammar/scanner.c");
"#
    } else {
        ""
    };

    let scanner_compile = if has_scanner {
        r#"
    build.file("grammar/scanner.c");"#
    } else {
        ""
    };

    format!(
        r#"fn main() {{
    let src_dir = "grammar/src";

    println!("cargo:rerun-if-changed={{}}/parser.c", src_dir);
{scanner_section}
    let mut build = cc::Build::new();

    build
        .include(src_dir)
        .include("grammar") // for common/ includes like "../common/scanner.h"
        .include(format!("{{}}/tree_sitter", src_dir))
        .opt_level_str("s") // optimize for size, not speed
        .warnings(false)
        .flag_if_supported("-Wno-unused-parameter")
        .flag_if_supported("-Wno-unused-but-set-variable")
        .flag_if_supported("-Wno-trigraphs");

    // For WASM builds, use our custom sysroot (provided by arborium crate via links = "arborium")
    let target = std::env::var("TARGET").unwrap_or_default();
    if target.contains("wasm")
        && let Ok(sysroot) = std::env::var("DEP_ARBORIUM_SYSROOT_PATH")
    {{
        build.include(&sysroot);
    }}

    build.file(format!("{{}}/parser.c", src_dir));{scanner_compile}

    build.compile("tree_sitter_{c_symbol}");
}}
"#
    )
}

/// Generate src/lib.rs content for a grammar crate.
fn generate_lib_rs(
    crate_name: &str,
    crate_path: &Utf8Path,
    config: &crate::types::CrateConfig,
) -> String {
    let grammar = config.grammars.first();
    let tests_cursed = grammar.map(|g| g.tests_cursed()).unwrap_or(false);

    let grammar_id = grammar
        .map(|g| g.id.as_ref())
        .unwrap_or_else(|| crate_name.strip_prefix("arborium-").unwrap_or(crate_name));

    let grammar_name = grammar
        .map(|g| g.name.as_ref())
        .unwrap_or(grammar_id)
        .to_uppercase();

    let c_symbol = grammar
        .and_then(|g| g.c_symbol.as_ref())
        .map(|s| s.to_string())
        .unwrap_or_else(|| grammar_id.replace('-', "_"));

    // Check if queries exist
    let highlights_exists = crate_path.join("queries/highlights.scm").exists();
    let injections_exists = crate_path.join("queries/injections.scm").exists();
    let locals_exists = crate_path.join("queries/locals.scm").exists();

    let highlights_query = if highlights_exists {
        format!(
            r#"/// The highlights query for {grammar_id}.
pub const HIGHLIGHTS_QUERY: &str = include_str!("../queries/highlights.scm");"#
        )
    } else {
        format!(
            r#"/// The highlights query for {grammar_id} (empty - no highlights available).
pub const HIGHLIGHTS_QUERY: &str = "";"#
        )
    };

    let injections_query = if injections_exists {
        format!(
            r#"/// The injections query for {grammar_id}.
pub const INJECTIONS_QUERY: &str = include_str!("../queries/injections.scm");"#
        )
    } else {
        format!(
            r#"/// The injections query for {grammar_id} (empty - no injections available).
pub const INJECTIONS_QUERY: &str = "";"#
        )
    };

    let locals_query = if locals_exists {
        format!(
            r#"/// The locals query for {grammar_id}.
pub const LOCALS_QUERY: &str = include_str!("../queries/locals.scm");"#
        )
    } else {
        format!(
            r#"/// The locals query for {grammar_id} (empty - no locals available).
pub const LOCALS_QUERY: &str = "";"#
        )
    };

    let test_module = if tests_cursed {
        String::new()
    } else {
        format!(
            r#"
#[cfg(test)]
mod tests {{
    use super::*;

    #[test]
    fn test_grammar() {{
        arborium_test_harness::test_grammar(
            language(),
            "{grammar_id}",
            HIGHLIGHTS_QUERY,
            INJECTIONS_QUERY,
            LOCALS_QUERY,
            env!("CARGO_MANIFEST_DIR"),
        );
    }}
}}
"#
        )
    };

    format!(
        r#"//! {grammar_name} grammar for tree-sitter
//!
//! This crate provides the {grammar_id} language grammar for use with tree-sitter.

use tree_sitter_patched_arborium::Language;

unsafe extern "C" {{
    fn tree_sitter_{c_symbol}() -> Language;
}}

/// Returns the {grammar_id} tree-sitter language.
pub fn language() -> Language {{
    unsafe {{ tree_sitter_{c_symbol}() }}
}}

{highlights_query}

{injections_query}

{locals_query}
{test_module}"#
    )
}

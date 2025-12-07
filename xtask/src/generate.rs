/// Generate command - regenerates crate files from arborium.kdl.
///
/// This command reads arborium.kdl files and generates:
/// - Cargo.toml
/// - build.rs
/// - src/lib.rs
/// - grammar/src/ (by running tree-sitter generate)
use crate::cache::GrammarCache;
use crate::plan::{Operation, Plan, PlanMode, PlanSet};
use crate::tool::Tool;
use crate::types::{CrateRegistry, CrateState};
use crate::util::find_repo_root;
use crate::version_store;
use camino::{Utf8Path, Utf8PathBuf};

/// Options for the generate command.
pub struct GenerateOptions<'a> {
    /// Optional grammar name to regenerate (regenerates all if None)
    pub name: Option<&'a str>,
    /// Dry run mode vs execute mode
    pub mode: PlanMode,
    /// Version string for generated Cargo.toml files
    pub version: &'a str,
    /// Continue processing all crates even if some fail
    pub no_fail_fast: bool,
    /// Number of parallel jobs for tree-sitter generation
    pub jobs: usize,
}
use fs_err as fs;
use owo_colors::OwoColorize;
use rayon::prelude::*;
use rootcause::Report;
use sailfish::TemplateSimple;
use std::process::Stdio;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

// Sailfish templates - compiled at build time
#[derive(TemplateSimple)]
#[template(path = "validate_grammar.stpl.js")]
struct ValidateGrammarTemplate<'a> {
    grammar_path: &'a str,
}

#[derive(TemplateSimple)]
#[template(path = "cargo.stpl.toml")]
struct CargoTomlTemplate<'a> {
    crate_name: &'a str,
    workspace_version: &'a str,
    grammar_id: &'a str,
    grammar_name: &'a str,
    license: &'a str,
    tag: &'a str,
    shared_rel: &'a str,
    repo_rel: &'a str,
}

#[derive(TemplateSimple)]
#[template(path = "build.stpl.rs")]
struct BuildRsTemplate<'a> {
    has_scanner: bool,
    c_symbol: &'a str,
}

#[derive(TemplateSimple)]
#[template(path = "lib.stpl.rs")]
struct LibRsTemplate<'a> {
    grammar_id: &'a str,
    c_symbol: &'a str,
    highlights_exists: bool,
    injections_exists: bool,
    locals_exists: bool,
    tests_cursed: bool,
}

#[derive(TemplateSimple)]
#[template(path = "readme.stpl.md")]
struct ReadmeTemplate<'a> {
    crate_name: &'a str,
    crate_name_snake: &'a str,
    grammar_name: &'a str,
    grammar_id: &'a str,
    upstream_repo: &'a str,
    upstream_url: &'a str,
    commit: &'a str,
    license: &'a str,
    description: &'a str,
    inventor: &'a str,
    year: u16,
    language_link: &'a str,
}

// Plugin crate templates
#[derive(TemplateSimple)]
#[template(path = "plugin_cargo.stpl.toml")]
struct PluginCargoTomlTemplate<'a> {
    grammar_id: &'a str,
    grammar_crate_name: &'a str,
    crate_rel: &'a str,
    shared_rel: &'a str,
    wit_path: &'a str,
}

#[derive(TemplateSimple)]
#[template(path = "plugin_lib.stpl.rs")]
struct PluginLibRsTemplate<'a> {
    grammar_id: &'a str,
    grammar_crate_name_snake: &'a str,
    wit_path: &'a str,
}

#[derive(TemplateSimple)]
#[template(path = "plugin_package.stpl.json")]
struct PluginPackageJsonTemplate<'a> {
    grammar_id: &'a str,
    version: &'a str,
}

#[derive(TemplateSimple)]
#[template(path = "plugin_build.stpl.rs")]
struct PluginBuildRsTemplate<'a> {
    has_scanner: bool,
    c_symbol: &'a str,
}

/// Update root Cargo.toml with the specified version
fn update_root_cargo_toml(repo_root: &Utf8Path, version: &str) -> Result<(), Report> {
    use regex::Regex;

    let cargo_toml_path = repo_root.join("Cargo.toml");
    let content = fs::read_to_string(&cargo_toml_path)?;

    // Ensure [workspace.package] exists; create if missing.
    let mut content = content;
    if !content.contains("[workspace.package]") {
        // Insert at end to avoid disturbing existing sections.
        content.push_str("\n[workspace.package]\n");
        content.push_str(&format!("version = \"{}\"\n", version));
        content.push_str("edition = \"2024\"\n");
        content.push_str("license = \"MIT OR Apache-2.0\"\n");
        content.push_str("repository = \"https://github.com/bearcove/arborium\"\n");
    } else {
        // Update version inside existing workspace.package.
        let workspace_version_re =
            Regex::new(r#"(?m)^(\[workspace\.package\][\s\S]*?version\s*=\s*)"[^"]*""#)
                .map_err(|e| std::io::Error::other(format!("Failed to compile regex: {e}")))?;
        content = workspace_version_re
            .replace(&content, format!(r#"$1"{version}""#))
            .to_string();
    }

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

    // Collect all grammar crates with their paths (sorted for deterministic output)
    let mut crate_entries: Vec<(&str, &Utf8Path)> = registry
        .crates
        .values()
        .filter(|state| state.name.starts_with("arborium-"))
        .map(|state| (state.name.as_str(), state.crate_path.as_path()))
        .collect();
    crate_entries.sort_by_key(|(name, _)| *name);

    // Build the [workspace.dependencies] section
    let mut deps_section = String::from("\n[workspace.dependencies]\n");
    // Include the umbrella crate itself
    deps_section.push_str(&format!(
        "arborium = {{ path = \"crates/arborium\", version = \"{}\" }}\n",
        version
    ));
    for (crate_name, crate_path) in &crate_entries {
        // Make path relative to repo root
        let rel_path = crate_path.strip_prefix(repo_root).unwrap_or(crate_path);
        deps_section.push_str(&format!(
            "{} = {{ path = \"{}\", version = \"{}\" }}\n",
            crate_name, rel_path, version
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
///
/// This follows the 5-function generation flow from generate.md:
/// 1. Load registry
/// 2. Prepare temp structures (SHARED by validation & generation)
/// 3. Validate all grammars
/// 4. Generate all grammars (tree-sitter)
/// 5. Generate all crates (Cargo.toml, lib.rs, etc.)
pub fn plan_generate(
    crates_dir: &Utf8Path,
    options: GenerateOptions<'_>,
) -> Result<PlanSet, Report> {
    use std::time::Instant;
    let total_start = Instant::now();

    // 1. Load Registry
    let registry = load_registry(crates_dir)?;

    // 2. Prepare temp structures (SHARED by validation & generation)
    let prepared = prepare_temp_structures(&registry, crates_dir, options.name, options.version)?;

    if prepared.prepared_temps.is_empty() {
        println!("No grammars to process");
        return Ok(PlanSet::new());
    }

    // 3. Validate all grammars using prepared structures
    validate_all_grammars(&prepared)?;

    // 4. Generate all grammars using same prepared structures
    let generation_results =
        generate_all_grammars(&prepared, options.mode, options.no_fail_fast, options.jobs)?;

    // 5. Generate all crates using templates
    let plan_set = generate_all_crates(&prepared, &generation_results)?;

    let total_elapsed = total_start.elapsed();
    println!("Total time: {:.2}s", total_elapsed.as_secs_f64());
    println!(
        "Cache hits: {}, misses: {}",
        generation_results.cache_hits, generation_results.cache_misses
    );

    Ok(plan_set)
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

    // Compute npm/grammar/src/ path from def/grammar/src/
    // Structure: langs/group-*/lang/def/grammar/src/ -> langs/group-*/lang/npm/grammar/src/
    let npm_src_dir = dest_src_dir
        .parent() // grammar/
        .and_then(|p| p.parent()) // def/
        .and_then(|p| p.parent()) // lang/
        .map(|lang_dir| lang_dir.join("npm/grammar/src"));

    // Ensure npm/grammar/src/ directory exists
    if let Some(ref npm_src) = npm_src_dir {
        let npm_grammar = npm_src.parent().expect("npm/grammar");
        if !npm_grammar.exists() {
            plan.add(Operation::CreateDir {
                path: npm_grammar.to_owned(),
                description: "Create npm/grammar directory".to_string(),
            });
        }
        if !npm_src.exists() {
            plan.add(Operation::CreateDir {
                path: npm_src.to_owned(),
                description: "Create npm/grammar/src directory".to_string(),
            });
        }
    }

    // Copy all generated files to grammar/src/ and npm/grammar/src/
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

        let new_content = fs::read_to_string(&generated_file)?;

        // Copy to def/grammar/src/
        let dest_file = dest_src_dir.join(&file_name);
        plan_file_update(
            plan,
            &dest_file,
            new_content.clone(),
            &format!("src/{}", file_name),
            mode,
        )?;

        // Also copy to npm/grammar/src/
        if let Some(ref npm_src) = npm_src_dir {
            let npm_dest_file = npm_src.join(&file_name);
            plan_file_update(
                plan,
                &npm_dest_file,
                new_content,
                &format!("npm/grammar/src/{}", file_name),
                mode,
            )?;
        }
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

        // Ensure npm/grammar/src/tree_sitter/ directory exists
        let npm_tree_sitter = npm_src_dir.as_ref().map(|p| p.join("tree_sitter"));
        if let Some(ref npm_ts) = npm_tree_sitter {
            if !npm_ts.exists() {
                plan.add(Operation::CreateDir {
                    path: npm_ts.to_owned(),
                    description: "Create npm/grammar/src/tree_sitter directory".to_string(),
                });
            }
        }

        // Copy each file in tree_sitter/
        for entry in fs::read_dir(&generated_tree_sitter)? {
            let entry = entry?;
            let file_name = entry.file_name().to_string_lossy().to_string();
            let generated_file = Utf8PathBuf::from_path_buf(entry.path())
                .map_err(|_| std::io::Error::other("Non-UTF8 path"))?;

            if generated_file.is_file() {
                let new_content = fs::read_to_string(&generated_file)?;

                // Copy to def/grammar/src/tree_sitter/
                let dest_file = dest_tree_sitter.join(&file_name);
                plan_file_update(
                    plan,
                    &dest_file,
                    new_content.clone(),
                    &format!("src/tree_sitter/{}", file_name),
                    mode,
                )?;

                // Also copy to npm/grammar/src/tree_sitter/
                if let Some(ref npm_ts) = npm_tree_sitter {
                    let npm_dest_file = npm_ts.join(&file_name);
                    plan_file_update(
                        plan,
                        &npm_dest_file,
                        new_content,
                        &format!("npm/grammar/src/tree_sitter/{}", file_name),
                        mode,
                    )?;
                }
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
    shared_rel: &str,
    repo_rel: &str,
) -> String {
    let grammar = config.grammars.first();

    let grammar_id = grammar
        .map(|g| g.id.as_ref())
        .unwrap_or(crate_name.strip_prefix("arborium-").unwrap_or(crate_name));

    let grammar_name = grammar.map(|g| g.name.as_ref()).unwrap_or(grammar_id);

    let tag = grammar
        .map(|g| g.tag.value.as_str())
        .unwrap_or("programming");

    // Use license from arborium.kdl, fallback to MIT if empty
    let license: &str = {
        let l: &str = config.license.value.as_ref();
        if l.is_empty() { "MIT" } else { l }
    };

    let template = CargoTomlTemplate {
        crate_name,
        workspace_version,
        grammar_id,
        grammar_name,
        license,
        tag,
        shared_rel,
        repo_rel,
    };
    template
        .render_once()
        .expect("CargoTomlTemplate render failed")
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

    let template = BuildRsTemplate {
        has_scanner,
        c_symbol: &c_symbol,
    };
    template
        .render_once()
        .expect("BuildRsTemplate render failed")
}

/// Generate src/lib.rs content for a grammar crate.
fn generate_lib_rs(
    crate_name: &str,
    def_path: &Utf8Path,
    config: &crate::types::CrateConfig,
) -> String {
    let grammar = config.grammars.first();
    let tests_cursed = grammar.map(|g| g.tests_cursed()).unwrap_or(false);

    let grammar_id = grammar
        .map(|g| g.id.as_ref())
        .unwrap_or_else(|| crate_name.strip_prefix("arborium-").unwrap_or(crate_name));

    let c_symbol = grammar
        .and_then(|g| g.c_symbol.as_ref())
        .map(|s| s.to_string())
        .unwrap_or_else(|| grammar_id.replace('-', "_"));

    // Check if queries exist (in def/queries/)
    let highlights_exists = def_path.join("queries/highlights.scm").exists();
    let injections_exists = def_path.join("queries/injections.scm").exists();
    let locals_exists = def_path.join("queries/locals.scm").exists();

    let template = LibRsTemplate {
        grammar_id,
        c_symbol: &c_symbol,
        highlights_exists,
        injections_exists,
        locals_exists,
        tests_cursed,
    };
    template.render_once().expect("LibRsTemplate render failed")
}

/// Generate README.md content for a grammar crate.
fn generate_readme(crate_name: &str, config: &crate::types::CrateConfig) -> String {
    let grammar = config.grammars.first();

    let grammar_id = grammar
        .map(|g| g.id.as_ref())
        .unwrap_or_else(|| crate_name.strip_prefix("arborium-").unwrap_or(crate_name));

    let grammar_name = grammar.map(|g| g.name.as_ref()).unwrap_or(grammar_id);

    let upstream_url: &str = config.repo.value.as_ref();
    // Extract repo name from URL for display
    let upstream_repo = upstream_url
        .strip_prefix("https://github.com/")
        .unwrap_or(upstream_url);

    let commit: &str = config.commit.value.as_ref();

    let license: &str = {
        let l: &str = config.license.value.as_ref();
        if l.is_empty() { "MIT" } else { l }
    };

    // Extract optional grammar metadata
    let description = grammar
        .and_then(|g| g.description.as_ref())
        .map(|d| d.value.as_str())
        .unwrap_or("");

    let inventor = grammar
        .and_then(|g| g.inventor.as_ref())
        .map(|i| i.value.as_str())
        .unwrap_or("");

    let year = grammar
        .and_then(|g| g.year.as_ref())
        .map(|y| y.value)
        .unwrap_or(0);

    let language_link = grammar
        .and_then(|g| g.link.as_ref())
        .map(|l| l.value.as_str())
        .unwrap_or_else(|| upstream_url);

    let crate_name_snake = crate_name.replace('-', "_");

    let template = ReadmeTemplate {
        crate_name,
        crate_name_snake: &crate_name_snake,
        grammar_name,
        grammar_id,
        upstream_repo,
        upstream_url,
        commit,
        license,
        description,
        inventor,
        year,
        language_link,
    };
    template
        .render_once()
        .expect("ReadmeTemplate render failed")
}

/// Generate plugin Cargo.toml content.
fn generate_plugin_cargo_toml(
    grammar_id: &str,
    grammar_crate_name: &str,
    wit_path: &str,
) -> String {
    // Paths relative to npm/:
    // npm/ is at langs/group-*/lang/npm/
    // crate/ is at langs/group-*/lang/crate/ (sibling)
    // shared crates are at crates/ (repo root)
    // So: npm -> lang -> group-* -> langs -> repo-root -> crates
    let crate_rel = "../crate";
    let shared_rel = "../../../../crates";

    let template = PluginCargoTomlTemplate {
        grammar_id,
        grammar_crate_name,
        crate_rel,
        shared_rel,
        wit_path,
    };
    template
        .render_once()
        .expect("PluginCargoTomlTemplate render failed")
}

/// Generate plugin src/lib.rs content.
fn generate_plugin_lib_rs(grammar_id: &str, grammar_crate_name: &str, wit_path: &str) -> String {
    let grammar_crate_name_snake = grammar_crate_name.replace('-', "_");

    let template = PluginLibRsTemplate {
        grammar_id,
        grammar_crate_name_snake: &grammar_crate_name_snake,
        wit_path,
    };
    template
        .render_once()
        .expect("PluginLibRsTemplate render failed")
}

/// Generate plugin package.json content.
fn generate_plugin_package_json(grammar_id: &str, version: &str) -> String {
    let template = PluginPackageJsonTemplate {
        grammar_id,
        version,
    };
    template
        .render_once()
        .expect("PluginPackageJsonTemplate render failed")
}

/// Generate plugin build.rs content.
fn generate_plugin_build_rs(config: &crate::types::CrateConfig, crate_name: &str) -> String {
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

    let template = PluginBuildRsTemplate {
        has_scanner,
        c_symbol: &c_symbol,
    };
    template
        .render_once()
        .expect("PluginBuildRsTemplate render failed")
}

// Data structures for temp directory preparation (shared by validation & generation)
struct PreparedTemp {
    crate_state: CrateState,
    temp_dir: tempfile::TempDir,
    temp_root: Utf8PathBuf,
    temp_grammar: Utf8PathBuf,
    config: crate::types::CrateConfig,
}

struct PreparedStructures {
    prepared_temps: Vec<PreparedTemp>,
    repo_root: Utf8PathBuf,
    cache: GrammarCache,
    workspace_version: String,
}

struct GenerationResults {
    cache_hits: usize,
    cache_misses: usize,
    plans: PlanSet,
}

// 1. Load Registry
fn load_registry(crates_dir: &Utf8Path) -> Result<CrateRegistry, Report> {
    CrateRegistry::load(crates_dir)
}

// 2. Prepare Temp Structures (SHARED by validation & generation)
fn prepare_temp_structures(
    registry: &CrateRegistry,
    crates_dir: &Utf8Path,
    name: Option<&str>,
    version: &str,
) -> Result<PreparedStructures, Report> {
    // Set up repo root and cache
    let repo_root =
        find_repo_root().ok_or_else(|| std::io::Error::other("Could not find repo root"))?;
    let repo_root = Utf8PathBuf::from_path_buf(repo_root)
        .map_err(|_| std::io::Error::other("Non-UTF8 repo root"))?;
    let cache = GrammarCache::new(&repo_root);

    // Record canonical version once, then update root files
    version_store::write_version(&repo_root, version)
        .map_err(|e| rootcause::Report::new(std::io::Error::other(e.to_string())))?;
    update_root_cargo_toml(&repo_root, version)?;
    generate_workspace_dependencies(&repo_root, registry, version)?;

    // Prepare temp directories for all crates that have grammar.js files
    let mut prepared_temps = Vec::new();

    for crate_state in registry.crates.values() {
        // Skip if a specific name was requested and this isn't it
        if let Some(filter) = name {
            let matches = crate_state.name == filter
                || (crate_state.name.strip_prefix("arborium-") == Some(filter));
            if !matches {
                continue;
            }
        }
        // Skip crates without arborium.kdl
        let Some(config) = crate_state.config.clone() else {
            continue;
        };
        let grammar_js = crate_state.def_path.join("grammar/grammar.js");

        if !grammar_js.exists() {
            continue;
        }

        // Create temp directory with proper structure (shared by validation and generation)
        let temp_dir = tempfile::tempdir()?;
        let temp_root = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf())
            .map_err(|_| std::io::Error::other("Non-UTF8 temp path"))?;
        let temp_grammar = temp_root.join("grammar");

        // Copy grammar files to temp
        let def_path = &crate_state.def_path;
        let grammar_dir = def_path.join("grammar");
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
            let temp_common = temp_root.join("common");
            copy_dir_contents(&grammar_common_dir, &temp_common)?;
        }

        // Set up cross-grammar dependencies
        setup_grammar_dependencies(&temp_grammar, crates_dir, &config)?;

        prepared_temps.push(PreparedTemp {
            crate_state: (*crate_state).clone(),
            temp_dir,
            temp_root,
            temp_grammar,
            config,
        });
    }

    Ok(PreparedStructures {
        prepared_temps,
        repo_root,
        cache,
        workspace_version: version.to_string(),
    })
}

// 3. Validate All Grammars using prepared structures
fn validate_all_grammars(prepared: &PreparedStructures) -> Result<(), Report> {
    println!("{}", "Validating grammar requires...".cyan().bold());

    for prepared_temp in &prepared.prepared_temps {
        validate_single_grammar(prepared_temp)?;
    }

    println!("{} All grammar requires validated", "✓".green());
    println!();
    Ok(())
}

fn validate_single_grammar(prepared_temp: &PreparedTemp) -> Result<(), Report> {
    // Create wrapper script
    let wrapper_path = prepared_temp.temp_dir.path().join("validate_grammar.js");
    let temp_grammar_js = prepared_temp.temp_grammar.join("grammar.js");

    let template = ValidateGrammarTemplate {
        grammar_path: &temp_grammar_js.as_str().replace('\\', "\\\\"),
    };
    let wrapper_content = template.render_once()?;

    fs::write(&wrapper_path, wrapper_content)?;

    // Run Node.js on the wrapper
    let output = std::process::Command::new("node")
        .arg(&wrapper_path)
        .current_dir(&prepared_temp.temp_root)
        .output()
        .map_err(|e| std::io::Error::other(format!("Failed to run node: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(std::io::Error::other(format!(
            "Grammar validation failed for {}: {}",
            prepared_temp
                .crate_state
                .name
                .strip_prefix("arborium-")
                .unwrap_or(&prepared_temp.crate_state.name),
            stderr.trim()
        ))
        .into());
    }

    Ok(())
}

// 4. Generate All Grammars using same prepared structures
fn generate_all_grammars(
    prepared: &PreparedStructures,
    mode: PlanMode,
    no_fail_fast: bool,
    jobs: usize,
) -> Result<GenerationResults, Report> {
    let cache_hits = AtomicUsize::new(0);
    let cache_misses = AtomicUsize::new(0);
    let plans = Mutex::new(PlanSet::new());
    let errors: Mutex<Vec<(String, Report)>> = Mutex::new(Vec::new());
    let first_error_seen = AtomicUsize::new(0); // 0 = no error, 1 = error seen

    // Process grammars in parallel using prepared temp directories
    let process_grammar = |prepared_temp: &PreparedTemp| {
        // In fail-fast mode, skip if we already saw an error
        if !no_fail_fast && first_error_seen.load(Ordering::Relaxed) == 1 {
            return;
        }

        let crate_name = &prepared_temp.crate_state.name;
        let result = plan_grammar_generation_with_prepared_temp(
            prepared_temp,
            &prepared.cache,
            &prepared.repo_root,
            mode,
        );

        match result {
            Ok((plan, hit)) => {
                if hit {
                    cache_hits.fetch_add(1, Ordering::Relaxed);
                } else {
                    cache_misses.fetch_add(1, Ordering::Relaxed);
                }
                plans.lock().unwrap().add(plan);
            }
            Err(e) => {
                first_error_seen.store(1, Ordering::Relaxed);
                errors.lock().unwrap().push((crate_name.clone(), e));
            }
        }
    };

    // Always parallel, with configurable thread pool
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(jobs)
        .build()
        .expect("Failed to build thread pool");

    pool.install(|| {
        prepared.prepared_temps.par_iter().for_each(process_grammar);
    });

    // Check for errors
    let errors = errors.into_inner().unwrap();
    if !errors.is_empty() {
        for (crate_name, error) in &errors {
            eprintln!("Error: {}: {}", crate_name.bold(), error);
        }
        return Err(std::io::Error::other(format!(
            "{} grammar(s) failed to generate",
            errors.len()
        ))
        .into());
    }

    Ok(GenerationResults {
        cache_hits: cache_hits.load(Ordering::Relaxed),
        cache_misses: cache_misses.load(Ordering::Relaxed),
        plans: plans.into_inner().unwrap(),
    })
}

// Helper function to generate a single grammar using prepared temp directory
fn plan_grammar_generation_with_prepared_temp(
    prepared_temp: &PreparedTemp,
    cache: &GrammarCache,
    repo_root: &Utf8Path,
    mode: PlanMode,
) -> Result<(Plan, bool), Report> {
    let crate_name = &prepared_temp.crate_state.name;
    let def_path = &prepared_temp.crate_state.def_path;

    // Use the already prepared temp directory instead of creating a new one
    let temp_root = &prepared_temp.temp_root;
    let temp_grammar = &prepared_temp.temp_grammar;

    // Destination is grammar/src/ under the definition path
    let dest_src_dir = def_path.join("grammar/src");

    // Compute cache key
    // Note: compute_cache_key takes (def_path, crates_dir, config) but we only need def_path and config
    // The crates_dir is used for dependency resolution which is already handled in prepared temps
    let crates_dir = repo_root.join("crates");
    let cache_key = cache.compute_cache_key(def_path, &crates_dir, &prepared_temp.config)?;

    let short_key = &cache_key[..8.min(cache_key.len())];

    if let Some(cached_files) = cache.get(crate_name, &cache_key) {
        // Cache hit - skip tree-sitter generate, but still plan grammar/src updates
        let temp_src = temp_root.join("cached_src");
        cached_files.extract_to(&temp_src)?;

        let cache_path = cache
            .cache_dir
            .strip_prefix(repo_root)
            .unwrap_or(&cache.cache_dir)
            .join(crate_name)
            .join(short_key);
        println!(
            "● {} ({}: {}, skipping tree-sitter, cache: {})",
            crate_name.green(),
            "cache hit".green(),
            short_key,
            cache_path
        );

        let mut plan = Plan::for_crate(crate_name);
        plan_updates_from_generated(&mut plan, &temp_src, &dest_src_dir, mode)?;

        return Ok((plan, true)); // true = cache hit
    }

    // Cache miss - run tree-sitter generate in the prepared temp directory
    println!(
        "● {} ({}: {}, regenerating)",
        crate_name.yellow(),
        "cache miss".yellow(),
        short_key
    );

    // Create src/ directory for grammars that generate files there
    fs::create_dir_all(temp_grammar.join("src"))?;

    // Run tree-sitter generate
    let tree_sitter = Tool::TreeSitter.find()?;

    let output = tree_sitter
        .command()
        .args(["generate"])
        .current_dir(temp_grammar)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let error_lines: Vec<&str> = stderr.lines().take(20).collect();
        return Err(std::io::Error::other(format!(
            "tree-sitter generate failed for {}:\n{}",
            crate_name,
            error_lines.join("\n")
        ))
        .into());
    }

    // The generated files are in temp/grammar/src/
    let generated_src = temp_grammar.join("src");

    // Save to cache for next time
    if let Err(e) = cache.save(crate_name, &cache_key, &generated_src) {
        eprintln!("Warning: failed to cache {}: {}", crate_name, e);
    }

    // Plan file updates
    let mut plan = Plan::for_crate(crate_name);
    plan_updates_from_generated(&mut plan, &generated_src, &dest_src_dir, mode)?;

    Ok((plan, false)) // false = cache miss
}

// 5. Generate All Crates using templates (Cargo.toml, build.rs, lib.rs, README.md)
// NOTE: This does NOT run tree-sitter - grammar generation is done in step 4
fn generate_all_crates(
    prepared: &PreparedStructures,
    generation_results: &GenerationResults,
) -> Result<PlanSet, Report> {
    let mut final_plan = generation_results.plans.clone();

    // Generate crate files (Cargo.toml, build.rs, lib.rs, README.md) for all crates
    for prepared_temp in &prepared.prepared_temps {
        let crate_state = &prepared_temp.crate_state;
        let config = &prepared_temp.config;

        let crate_plan = plan_crate_files_only(crate_state, config, &prepared.workspace_version)?;
        final_plan.add(crate_plan);

        // Generate plugin crate files for grammars that have generate-component enabled
        let plugin_plan = plan_plugin_crate_files(
            crate_state,
            config,
            &prepared.repo_root,
            &prepared.workspace_version,
        )?;
        final_plan.add(plugin_plan);
    }

    Ok(final_plan)
}

/// Generate only the Rust crate files (Cargo.toml, build.rs, lib.rs, README.md)
/// This does NOT run tree-sitter generate - that's handled separately in step 4.
fn plan_crate_files_only(
    crate_state: &CrateState,
    config: &crate::types::CrateConfig,
    workspace_version: &str,
) -> Result<Plan, Report> {
    let mut plan = Plan::for_crate(&crate_state.name);
    let def_path = &crate_state.def_path;
    let crate_path = &crate_state.crate_path;

    // crate/ is at langs/group-*/lang/crate/
    // shared crates are at crates/ (repo root)
    // So: crate -> lang -> group-* -> langs -> repo-root -> crates
    let shared_rel = "../../../../crates";
    // repo root is 4 levels up from crate/
    let repo_rel = "../../../..";

    // Ensure crate directory exists
    if !crate_path.exists() {
        plan.add(Operation::CreateDir {
            path: crate_path.to_owned(),
            description: "Create crate directory".to_string(),
        });
    }

    // Generate Cargo.toml
    let cargo_toml_path = crate_path.join("Cargo.toml");
    let new_cargo_toml = generate_cargo_toml(
        &crate_state.name,
        config,
        workspace_version,
        shared_rel,
        repo_rel,
    );

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

    // Generate README.md
    let readme_path = crate_path.join("README.md");
    let new_readme = generate_readme(&crate_state.name, config);

    if readme_path.exists() {
        let old_content = fs::read_to_string(&readme_path)?;
        if old_content != new_readme {
            plan.add(Operation::UpdateFile {
                path: readme_path,
                old_content: Some(old_content),
                new_content: new_readme,
                description: "Update README.md".to_string(),
            });
        }
    } else {
        plan.add(Operation::CreateFile {
            path: readme_path,
            content: new_readme,
            description: "Create README.md".to_string(),
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

    Ok(plan)
}

/// Generate plugin crate files (npm/Cargo.toml, npm/src/lib.rs, npm/package.json)
/// Only generates for grammars that have generate-component enabled (default: true).
fn plan_plugin_crate_files(
    crate_state: &CrateState,
    config: &crate::types::CrateConfig,
    repo_root: &Utf8Path,
    workspace_version: &str,
) -> Result<Plan, Report> {
    let mut plan = Plan::for_crate(format!("{}-plugin", crate_state.name));

    // Check if any grammar has generate-component enabled
    let grammar = config.grammars.first();
    let should_generate = grammar.map(|g| g.generate_component()).unwrap_or(true);

    if !should_generate {
        return Ok(plan);
    }

    let grammar = match grammar {
        Some(g) => g,
        None => return Ok(plan),
    };

    let grammar_id = grammar.id();
    let crate_name = &crate_state.name;

    // Plugin crate lives in npm/ sibling to crate/
    // Structure: langs/group-*/lang/npm/
    let lang_dir = crate_state
        .crate_path
        .parent()
        .expect("crate_path should have parent");
    let npm_path = lang_dir.join("npm");
    let wit_path = repo_root.join("wit/grammar.wit");

    // Ensure npm directory exists
    if !npm_path.exists() {
        plan.add(Operation::CreateDir {
            path: npm_path.clone(),
            description: "Create npm directory".to_string(),
        });
    }

    // Generate npm/Cargo.toml
    let cargo_toml_path = npm_path.join("Cargo.toml");
    let new_cargo_toml = generate_plugin_cargo_toml(grammar_id, crate_name, wit_path.as_str());

    if cargo_toml_path.exists() {
        let old_content = fs::read_to_string(&cargo_toml_path)?;
        if old_content != new_cargo_toml {
            plan.add(Operation::UpdateFile {
                path: cargo_toml_path,
                old_content: Some(old_content),
                new_content: new_cargo_toml,
                description: "Update plugin Cargo.toml".to_string(),
            });
        }
    } else {
        plan.add(Operation::CreateFile {
            path: cargo_toml_path,
            content: new_cargo_toml,
            description: "Create plugin Cargo.toml".to_string(),
        });
    }

    // Generate npm/src/lib.rs
    let lib_rs_path = npm_path.join("src/lib.rs");
    let new_lib_rs = generate_plugin_lib_rs(grammar_id, crate_name, wit_path.as_str());

    if lib_rs_path.exists() {
        let old_content = fs::read_to_string(&lib_rs_path)?;
        if old_content != new_lib_rs {
            plan.add(Operation::UpdateFile {
                path: lib_rs_path,
                old_content: Some(old_content),
                new_content: new_lib_rs,
                description: "Update plugin src/lib.rs".to_string(),
            });
        }
    } else {
        // Ensure src/ directory exists
        let src_dir = npm_path.join("src");
        if !src_dir.exists() {
            plan.add(Operation::CreateDir {
                path: src_dir,
                description: "Create plugin src directory".to_string(),
            });
        }
        plan.add(Operation::CreateFile {
            path: lib_rs_path,
            content: new_lib_rs,
            description: "Create plugin src/lib.rs".to_string(),
        });
    }

    // Generate npm/package.json
    let package_json_path = npm_path.join("package.json");
    let new_package_json = generate_plugin_package_json(grammar_id, workspace_version);

    if package_json_path.exists() {
        let old_content = fs::read_to_string(&package_json_path)?;
        if old_content != new_package_json {
            plan.add(Operation::UpdateFile {
                path: package_json_path,
                old_content: Some(old_content),
                new_content: new_package_json,
                description: "Update plugin package.json".to_string(),
            });
        }
    } else {
        plan.add(Operation::CreateFile {
            path: package_json_path,
            content: new_package_json,
            description: "Create plugin package.json".to_string(),
        });
    }

    // Generate npm/build.rs
    let build_rs_path = npm_path.join("build.rs");
    let new_build_rs = generate_plugin_build_rs(config, crate_name);

    if build_rs_path.exists() {
        let old_content = fs::read_to_string(&build_rs_path)?;
        if old_content != new_build_rs {
            plan.add(Operation::UpdateFile {
                path: build_rs_path,
                old_content: Some(old_content),
                new_content: new_build_rs,
                description: "Update plugin build.rs".to_string(),
            });
        }
    } else {
        plan.add(Operation::CreateFile {
            path: build_rs_path,
            content: new_build_rs,
            description: "Create plugin build.rs".to_string(),
        });
    }

    // Grammar source files (parser.c, grammar.json, etc.) are now copied in
    // plan_updates_from_generated() during the grammar generation phase.
    // Here we only handle scanner.c which is separate (at def/grammar/scanner.c, not in src/)

    let npm_grammar_dir = npm_path.join("grammar");
    let npm_grammar_src = npm_grammar_dir.join("src");

    // Copy scanner.c if it exists (it's at def/grammar/scanner.c, not in src/)
    let def_scanner = crate_state.def_path.join("grammar/scanner.c");
    let npm_scanner = npm_grammar_src.join("scanner.c");

    if def_scanner.exists() {
        // Ensure npm/grammar/src/ directory exists for scanner.c
        if !npm_grammar_dir.exists() {
            plan.add(Operation::CreateDir {
                path: npm_grammar_dir.clone(),
                description: "Create plugin grammar directory".to_string(),
            });
        }
        if !npm_grammar_src.exists() {
            plan.add(Operation::CreateDir {
                path: npm_grammar_src.clone(),
                description: "Create plugin grammar/src directory".to_string(),
            });
        }

        let content = fs::read_to_string(&def_scanner)?;

        if npm_scanner.exists() {
            let old_content = fs::read_to_string(&npm_scanner)?;
            if old_content != content {
                plan.add(Operation::UpdateFile {
                    path: npm_scanner,
                    old_content: Some(old_content),
                    new_content: content,
                    description: "Update grammar/src/scanner.c".to_string(),
                });
            }
        } else {
            plan.add(Operation::CreateFile {
                path: npm_scanner,
                content,
                description: "Create grammar/src/scanner.c".to_string(),
            });
        }
    }

    Ok(plan)
}

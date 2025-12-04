//! Pack and unpack grammar sources for CI artifact transfer
//!
//! This module provides reliable archive creation and extraction for grammar
//! source files (parser.c, scanner.c, etc.) that are generated during the
//! `gen` command and need to be transferred between CI jobs.

use camino::{Utf8Path, Utf8PathBuf};
use miette::{Context, IntoDiagnostic, Result};
use owo_colors::OwoColorize;

/// Pack grammar sources into a tar.zst archive
pub fn pack_grammar_sources(repo_root: &Utf8Path, output: &Utf8Path) -> Result<()> {
    let crates_dir = repo_root.join("crates");

    println!("{} Packing grammar sources from {}", "→".cyan(), crates_dir);

    // Find all grammar/src directories
    let mut grammar_dirs: Vec<Utf8PathBuf> = Vec::new();
    for entry in fs_err::read_dir(&crates_dir)
        .into_diagnostic()
        .context("Failed to read crates directory")?
    {
        let entry = entry.into_diagnostic()?;
        let path = Utf8PathBuf::from_path_buf(entry.path()).expect("non-UTF8 path");

        if !path.is_dir() {
            continue;
        }

        let grammar_src = path.join("grammar").join("src");
        if grammar_src.is_dir() {
            grammar_dirs.push(grammar_src);
        }
    }

    if grammar_dirs.is_empty() {
        miette::bail!("No grammar/src directories found in {}", crates_dir);
    }

    println!(
        "  {} Found {} grammar directories",
        "●".green(),
        grammar_dirs.len()
    );

    // Create the tar archive
    let output_file = fs_err::File::create(output)
        .into_diagnostic()
        .context(format!("Failed to create output file: {}", output))?;

    // Wrap in zstd compression
    let zstd_encoder = zstd::stream::Encoder::new(output_file, 3)
        .into_diagnostic()
        .context("Failed to create zstd encoder")?;

    let mut tar_builder = tar::Builder::new(zstd_encoder);

    let mut file_count = 0;
    for grammar_src in &grammar_dirs {
        // Add all files in this directory
        for entry in walkdir::WalkDir::new(grammar_src)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            let file_path =
                Utf8PathBuf::from_path_buf(entry.path().to_path_buf()).expect("non-UTF8 path");
            let file_rel_path = file_path
                .strip_prefix(repo_root)
                .expect("file should be under repo_root");

            let mut file = fs_err::File::open(&file_path)
                .into_diagnostic()
                .context(format!("Failed to open file: {}", file_path))?;

            let metadata = file
                .metadata()
                .into_diagnostic()
                .context(format!("Failed to read metadata: {}", file_path))?;

            let mut header = tar::Header::new_gnu();
            header.set_size(metadata.len());
            header.set_mode(0o644);
            header.set_mtime(0); // Reproducible builds
            header.set_cksum();

            tar_builder
                .append_data(&mut header, file_rel_path, &mut file)
                .into_diagnostic()
                .context(format!("Failed to add file to archive: {}", file_rel_path))?;

            file_count += 1;
            println!("    {} {}", "+".green(), file_rel_path);
        }
    }

    // Finish the tar archive and zstd compression
    let zstd_encoder = tar_builder
        .into_inner()
        .into_diagnostic()
        .context("Failed to finish tar archive")?;

    zstd_encoder
        .finish()
        .into_diagnostic()
        .context("Failed to finish zstd compression")?;

    let output_size = fs_err::metadata(output)
        .into_diagnostic()
        .context("Failed to read output file size")?
        .len();

    println!(
        "{} Packed {} files into {} ({:.2} KB)",
        "✓".green().bold(),
        file_count,
        output,
        output_size as f64 / 1024.0
    );

    Ok(())
}

/// Unpack grammar sources from a tar.zst archive
pub fn unpack_grammar_sources(archive: &Utf8Path, target_dir: &Utf8Path) -> Result<()> {
    println!(
        "{} Unpacking grammar sources from {} to {}",
        "→".cyan(),
        archive,
        target_dir
    );

    let archive_file = fs_err::File::open(archive)
        .into_diagnostic()
        .context(format!("Failed to open archive: {}", archive))?;

    // Decompress zstd
    let zstd_decoder = zstd::stream::Decoder::new(archive_file)
        .into_diagnostic()
        .context("Failed to create zstd decoder")?;

    // Unpack tar safely (handles path traversal attacks)
    let mut tar_archive = tar::Archive::new(zstd_decoder);
    tar_archive
        .unpack(target_dir)
        .into_diagnostic()
        .context("Failed to unpack archive")?;

    println!("{} Unpacked archive", "✓".green().bold());

    // Verify some key files exist
    let crates_dir = target_dir.join("crates");
    let mut verified = 0;
    for entry in fs_err::read_dir(&crates_dir)
        .into_diagnostic()
        .context("Failed to read crates directory")?
    {
        let entry = entry.into_diagnostic()?;
        let path = Utf8PathBuf::from_path_buf(entry.path()).expect("non-UTF8 path");
        let parser_c = path.join("grammar").join("src").join("parser.c");
        if parser_c.exists() {
            verified += 1;
        }
    }

    println!(
        "  {} Verified {} crates have grammar/src/parser.c",
        "●".green(),
        verified
    );

    Ok(())
}

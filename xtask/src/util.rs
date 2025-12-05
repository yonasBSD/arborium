//! Shared utilities for xtask commands

use std::env;
use std::path::PathBuf;

/// Find repository root by looking for .git directory
pub fn find_repo_root() -> Option<PathBuf> {
    let cwd = env::current_dir().ok()?;
    let mut current = cwd.clone();

    loop {
        let git_dir = current.join(".git");
        if git_dir.exists() {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
}
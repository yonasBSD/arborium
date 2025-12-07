//! Theme support for arborium syntax highlighting.
//!
//! This crate provides:
//! - Highlight category definitions (the canonical list of syntax categories)
//! - Theme parsing from Helix-style TOML files
//! - CSS and ANSI output generation
//! - Built-in themes (catppuccin, dracula, tokyo-night, etc.)

pub mod highlights;
pub mod theme;

pub use highlights::{COUNT, HIGHLIGHTS, HighlightDef};
pub use theme::{Color, Modifiers, Style, Theme, ThemeError, builtin};

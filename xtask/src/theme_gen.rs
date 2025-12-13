//! Theme code generation - converts TOML themes to Rust code at build time.
//!
//! This eliminates the runtime TOML dependency from arborium-theme by generating
//! Rust code with all theme data pre-parsed.

use camino::Utf8Path;
use fs_err as fs;
use owo_colors::OwoColorize;
use std::collections::HashMap;
use std::fmt::Write;

/// Parse a hex color string like "#ff0000" or "ff0000" into (r, g, b).
fn parse_hex(s: &str) -> Option<(u8, u8, u8)> {
    let s = s.strip_prefix('#').unwrap_or(s);
    if s.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&s[0..2], 16).ok()?;
    let g = u8::from_str_radix(&s[2..4], 16).ok()?;
    let b = u8::from_str_radix(&s[4..6], 16).ok()?;
    Some((r, g, b))
}

/// Parsed style from TOML.
#[derive(Debug, Default, Clone)]
struct ParsedStyle {
    fg: Option<(u8, u8, u8)>,
    bg: Option<(u8, u8, u8)>,
    bold: bool,
    italic: bool,
    underline: bool,
    strikethrough: bool,
}

impl ParsedStyle {
    fn is_empty(&self) -> bool {
        self.fg.is_none()
            && self.bg.is_none()
            && !self.bold
            && !self.italic
            && !self.underline
            && !self.strikethrough
    }
}

/// Parsed theme from TOML.
#[derive(Debug)]
struct ParsedTheme {
    name: String,
    is_dark: bool,
    source_url: Option<String>,
    background: Option<(u8, u8, u8)>,
    foreground: Option<(u8, u8, u8)>,
    styles: Vec<ParsedStyle>,
}

/// The highlight definitions - must match arborium_theme::highlights::HIGHLIGHTS order.
const HIGHLIGHT_NAMES: &[&str] = &[
    "attribute",
    "constant",
    "constant.builtin",
    "constructor",
    "function.builtin",
    "function",
    "function.method",
    "keyword",
    "keyword.conditional",
    "keyword.coroutine",
    "keyword.debug",
    "keyword.exception",
    "keyword.function",
    "keyword.import",
    "keyword.operator",
    "keyword.repeat",
    "keyword.return",
    "keyword.type",
    "operator",
    "property",
    "punctuation",
    "punctuation.bracket",
    "punctuation.delimiter",
    "punctuation.special",
    "string",
    "string.special",
    "tag",
    "tag.delimiter",
    "tag.error",
    "type",
    "type.builtin",
    "type.qualifier",
    "variable",
    "variable.builtin",
    "variable.parameter",
    "comment",
    "comment.documentation",
    "macro",
    "label",
    "diff.addition",
    "diff.deletion",
    "number",
    "text.literal",
    "text.emphasis",
    "text.strong",
    "text.uri",
    "text.reference",
    "string.escape",
    "text.title",
    "text.strikethrough",
    "spell",
    "embedded",
    "error",
    "namespace",
    "include",
    "storageclass",
    "repeat",
    "conditional",
    "exception",
    "preproc",
    "none",
    "character",
    "character.special",
    "variable.member",
    "function.definition",
    "type.definition",
    "function.call",
    "keyword.modifier",
    "keyword.directive",
    "string.regexp",
    "nospell",
    "float",
    "boolean",
];

/// Extra mappings from Helix theme names to our names.
const EXTRA_MAPPINGS: &[(&str, &str)] = &[
    ("keyword.control", "keyword"),
    ("keyword.storage", "keyword"),
    ("comment.line", "comment"),
    ("comment.block", "comment"),
    ("function.macro", "macro"),
    ("diff.plus", "diff.addition"),
    ("diff.minus", "diff.deletion"),
];

/// Parse a theme from TOML content.
fn parse_theme(toml_str: &str) -> Result<ParsedTheme, String> {
    let value: toml::Value = toml_str
        .parse()
        .map_err(|e| format!("TOML parse error: {e}"))?;
    let table = value
        .as_table()
        .ok_or_else(|| "Expected table".to_string())?;

    let name = table
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let is_dark = table
        .get("variant")
        .and_then(|v| v.as_str())
        .map(|v| v != "light")
        .unwrap_or(true);

    let source_url = table
        .get("source")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // Extract palette for color lookups
    let palette: HashMap<&str, (u8, u8, u8)> = table
        .get("palette")
        .and_then(|v| v.as_table())
        .map(|t| {
            t.iter()
                .filter_map(|(k, v)| v.as_str().and_then(parse_hex).map(|c| (k.as_str(), c)))
                .collect()
        })
        .unwrap_or_default();

    let resolve_color =
        |s: &str| -> Option<(u8, u8, u8)> { parse_hex(s).or_else(|| palette.get(s).copied()) };

    // Extract background and foreground
    let mut background = None;
    let mut foreground = None;

    if let Some(bg) = table.get("ui.background") {
        if let Some(bg_table) = bg.as_table() {
            if let Some(bg_str) = bg_table.get("bg").and_then(|v| v.as_str()) {
                background = resolve_color(bg_str);
            }
        }
    }
    if let Some(bg_str) = table.get("background").and_then(|v| v.as_str()) {
        background = resolve_color(bg_str);
    }

    if let Some(fg) = table.get("ui.foreground") {
        if let Some(fg_str) = fg.as_str() {
            foreground = resolve_color(fg_str);
        } else if let Some(fg_table) = fg.as_table() {
            if let Some(fg_str) = fg_table.get("fg").and_then(|v| v.as_str()) {
                foreground = resolve_color(fg_str);
            }
        }
    }
    if let Some(fg_str) = table.get("foreground").and_then(|v| v.as_str()) {
        foreground = resolve_color(fg_str);
    }

    // Parse a style value (string or table)
    let parse_style_value = |value: &toml::Value| -> ParsedStyle {
        let mut style = ParsedStyle::default();
        match value {
            toml::Value::String(s) => {
                style.fg = resolve_color(s);
            }
            toml::Value::Table(t) => {
                if let Some(fg) = t.get("fg").and_then(|v| v.as_str()) {
                    style.fg = resolve_color(fg);
                }
                if let Some(bg) = t.get("bg").and_then(|v| v.as_str()) {
                    style.bg = resolve_color(bg);
                }
                if let Some(mods) = t.get("modifiers").and_then(|v| v.as_array()) {
                    for m in mods {
                        if let Some(s) = m.as_str() {
                            match s {
                                "bold" => style.bold = true,
                                "italic" => style.italic = true,
                                "underlined" | "underline" => style.underline = true,
                                "crossed_out" | "strikethrough" => style.strikethrough = true,
                                _ => {}
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        style
    };

    // Initialize styles array
    let mut styles: Vec<ParsedStyle> = (0..HIGHLIGHT_NAMES.len())
        .map(|_| ParsedStyle::default())
        .collect();

    // Parse each highlight rule
    for (i, name) in HIGHLIGHT_NAMES.iter().enumerate() {
        if let Some(rule) = table.get(*name) {
            styles[i] = parse_style_value(rule);
        }
    }

    // Handle extra mappings
    for (helix_name, our_name) in EXTRA_MAPPINGS {
        if let Some(rule) = table.get(*helix_name) {
            if let Some(i) = HIGHLIGHT_NAMES.iter().position(|n| n == our_name) {
                if styles[i].is_empty() {
                    styles[i] = parse_style_value(rule);
                }
            }
        }
    }

    Ok(ParsedTheme {
        name,
        is_dark,
        source_url,
        background,
        foreground,
        styles,
    })
}

/// Generate Rust code for a color option.
fn gen_color_option(color: &Option<(u8, u8, u8)>) -> String {
    match color {
        Some((r, g, b)) => format!("Some(Color::new({r}, {g}, {b}))"),
        None => "None".to_string(),
    }
}

/// Generate Rust code for a style.
fn gen_style(style: &ParsedStyle) -> String {
    if style.is_empty() {
        return "Style::new()".to_string();
    }

    let mut parts = vec!["Style::new()".to_string()];

    if let Some((r, g, b)) = style.fg {
        parts.push(format!(".fg(Color::new({r}, {g}, {b}))"));
    }

    if style.bold {
        parts.push(".bold()".to_string());
    }
    if style.italic {
        parts.push(".italic()".to_string());
    }
    if style.underline {
        parts.push(".underline()".to_string());
    }
    if style.strikethrough {
        parts.push(".strikethrough()".to_string());
    }

    parts.join("")
}

/// Theme definition for code generation.
struct ThemeDef {
    fn_name: String,
    theme: ParsedTheme,
}

/// Generate builtin_generated.rs from all theme TOML files.
pub fn generate_theme_code(crates_dir: &Utf8Path) -> Result<(), String> {
    let themes_dir = crates_dir.join("arborium-theme/themes");
    let output_path = crates_dir.join("arborium-theme/src/builtin_generated.rs");

    println!(
        "{} Generating theme Rust code from {}",
        "●".cyan(),
        themes_dir.cyan()
    );

    // Collect and parse all theme files
    let mut themes: Vec<ThemeDef> = Vec::new();

    let entries =
        fs::read_dir(&themes_dir).map_err(|e| format!("Failed to read themes dir: {e}"))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read dir entry: {e}"))?;
        let path = entry.path();

        if path.extension().is_some_and(|e| e == "toml") {
            let file_stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .ok_or_else(|| format!("Invalid file name: {:?}", path))?;

            // Convert file name to function name (e.g., "catppuccin-mocha" -> "catppuccin_mocha")
            let fn_name = file_stem.replace('-', "_");

            let content =
                fs::read_to_string(&path).map_err(|e| format!("Failed to read {:?}: {e}", path))?;

            let theme =
                parse_theme(&content).map_err(|e| format!("Failed to parse {:?}: {e}", path))?;

            themes.push(ThemeDef { fn_name, theme });
        }
    }

    // Sort by function name for deterministic output
    themes.sort_by(|a, b| a.fn_name.cmp(&b.fn_name));

    // Generate the Rust code
    let mut code = String::new();

    writeln!(
        code,
        "// Generated theme definitions - DO NOT EDIT MANUALLY."
    )
    .unwrap();
    writeln!(
        code,
        "// This file is generated by xtask from TOML theme files."
    )
    .unwrap();
    writeln!(code).unwrap();
    writeln!(code, "use super::{{Color, Style, Theme}};").unwrap();
    writeln!(code).unwrap();

    // Generate each theme as a static function
    for def in &themes {
        let theme = &def.theme;

        writeln!(code, "/// {} theme.", theme.name).unwrap();
        if let Some(ref url) = theme.source_url {
            writeln!(code, "///").unwrap();
            writeln!(code, "/// Source: {url}").unwrap();
        }
        writeln!(code, "pub fn {}() -> Theme {{", def.fn_name).unwrap();
        writeln!(code, "    Theme {{").unwrap();
        writeln!(code, "        name: {:?}.to_string(),", theme.name).unwrap();
        writeln!(code, "        is_dark: {},", theme.is_dark).unwrap();

        match &theme.source_url {
            Some(url) => {
                writeln!(code, "        source_url: Some({:?}.to_string()),", url).unwrap()
            }
            None => writeln!(code, "        source_url: None,").unwrap(),
        }

        writeln!(
            code,
            "        background: {},",
            gen_color_option(&theme.background)
        )
        .unwrap();
        writeln!(
            code,
            "        foreground: {},",
            gen_color_option(&theme.foreground)
        )
        .unwrap();

        writeln!(code, "        styles: [").unwrap();
        for (i, style) in theme.styles.iter().enumerate() {
            let trailing = if i == theme.styles.len() - 1 { "" } else { "," };
            writeln!(code, "            {}{}", gen_style(style), trailing).unwrap();
        }
        writeln!(code, "        ],").unwrap();
        writeln!(code, "    }}").unwrap();
        writeln!(code, "}}").unwrap();
        writeln!(code).unwrap();
    }

    // Generate all() function
    writeln!(code, "/// Get all built-in themes.").unwrap();
    writeln!(code, "pub fn all() -> Vec<Theme> {{").unwrap();
    writeln!(code, "    vec![").unwrap();
    for def in &themes {
        writeln!(code, "        {}(),", def.fn_name).unwrap();
    }
    writeln!(code, "    ]").unwrap();
    writeln!(code, "}}").unwrap();

    // Write the file
    fs::write(&output_path, &code).map_err(|e| format!("Failed to write output: {e}"))?;

    println!(
        "  {} Generated {} themes to {}",
        "✓".green(),
        themes.len(),
        output_path.cyan()
    );

    Ok(())
}

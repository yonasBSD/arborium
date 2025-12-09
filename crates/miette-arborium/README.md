# miette-arborium

Arborium-powered syntax highlighter for [miette](https://github.com/zkat/miette) diagnostics.

This crate integrates arborium's tree-sitter based syntax highlighting into miette's error reporting output, giving your diagnostic messages beautiful, accurate syntax highlighting across 90+ programming languages.

## Quick Start

Install the highlighter globally and miette will automatically use it:

```rust
fn main() {
    // Install the highlighter (call once at startup)
    miette_arborium::install_global().ok();

    // Now all miette errors will have syntax highlighting
    // ... your code ...
}
```

Or use it directly with a `GraphicalReportHandler`:

```rust
use miette::{GraphicalReportHandler, GraphicalTheme};
use miette_arborium::MietteHighlighter;

let handler = GraphicalReportHandler::new_themed(GraphicalTheme::unicode())
    .with_syntax_highlighting(MietteHighlighter::new())
    .with_context_lines(3);
```

## Features

- **Language detection**: Automatically detects language from file extension
- **90+ languages**: Supports all languages enabled via Cargo features (passthrough to arborium)
- **Tree-sitter powered**: Accurate syntax highlighting using tree-sitter grammars
- **ANSI terminal output**: Beautiful colors in your terminal

## Language Features

By default, no languages are included. Enable the languages you need via Cargo features:

```toml
[dependencies]
miette-arborium = { version = "0.700", features = ["lang-rust", "lang-python", "lang-javascript"] }
```

Or enable all languages:

```toml
[dependencies]
miette-arborium = { version = "0.700", features = ["all-languages"] }
```

### Available Language Features

All `lang-*` features are passthrough to arborium. Some commonly used ones:

- `lang-rust` - Rust
- `lang-python` - Python
- `lang-javascript` - JavaScript
- `lang-typescript` - TypeScript
- `lang-go` - Go
- `lang-c` / `lang-cpp` - C/C++
- `lang-json` / `lang-yaml` / `lang-toml` - Data formats
- `lang-bash` - Shell scripts
- `lang-sql` - SQL

See [arborium's documentation](https://docs.rs/arborium) for the full list of supported languages.

## Example

```rust
use miette::{Diagnostic, NamedSource, SourceSpan};
use thiserror::Error;

#[derive(Error, Debug, Diagnostic)]
#[error("syntax error")]
struct SyntaxError {
    #[source_code]
    src: NamedSource<String>,
    #[label("unexpected token here")]
    span: SourceSpan,
}

fn main() -> miette::Result<()> {
    miette_arborium::install_global().ok();

    let source = r#"fn main() {
    let x = 42
    println!("{}", x);
}"#;

    Err(SyntaxError {
        src: NamedSource::new("example.rs", source.to_string()),
        span: (32..33).into(),
    })?
}
```

## License

Licensed under either of Apache License, Version 2.0 or MIT license at your option.

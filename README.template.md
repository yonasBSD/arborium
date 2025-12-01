# arborium

Batteries-included [tree-sitter](https://tree-sitter.github.io/tree-sitter/) grammar collection with HTML rendering and WASM support.

[![Crates.io](https://img.shields.io/crates/v/arborium.svg)](https://crates.io/crates/arborium)
[![Documentation](https://docs.rs/arborium/badge.svg)](https://docs.rs/arborium)
[![License](https://img.shields.io/crates/l/arborium.svg)](LICENSE-MIT)

## Features

- **{{TOTAL_COUNT}} language grammars** included out of the box
- **{{PERMISSIVE_COUNT}} permissively licensed** (MIT/Apache-2.0/CC0/Unlicense) grammars enabled by default
- **WASM support** with custom allocator fix
- **Feature flags** for fine-grained control over included languages

## Usage

```toml
[dependencies]
arborium = "0.1"
```

By default, all permissively-licensed grammars are included. To select specific languages:

```toml
[dependencies]
arborium = { version = "0.1", default-features = false, features = ["lang-rust", "lang-javascript"] }
```

## Feature Flags

### Grammar Collections

| Feature | Description |
|---------|-------------|
| `mit-grammars` | All permissively licensed grammars (MIT, Apache-2.0, CC0) - **default** |
| `gpl-grammars` | GPL-licensed grammars (copyleft - may affect your project's license) |
| `all-grammars` | All grammars including GPL |

### Permissive Grammars ({{PERMISSIVE_COUNT}})

These grammars use permissive licenses (MIT, Apache-2.0, CC0, Unlicense) and are included by default.

{{PERMISSIVE_TABLE}}

### GPL-Licensed Grammars ({{GPL_COUNT}})

These grammars are **not included by default** due to their copyleft license.
Enabling them may have implications for your project's licensing.

{{GPL_TABLE}}

## Sponsors

CI infrastructure generously provided by [Depot](https://depot.dev).

[![Depot](https://depot.dev/badges/depot.svg)](https://depot.dev)

## License

This project is dual-licensed under [MIT](LICENSE-MIT) OR [Apache-2.0](LICENSE-APACHE).

The bundled grammar sources retain their original licenses - see [LICENSES.md](LICENSES.md) for details.

## WASM Support

Arborium supports building for `wasm32-unknown-unknown`. This requires compiling C code (tree-sitter core and grammar parsers) to WebAssembly.

### macOS

On macOS, the built-in Apple clang does **not** support the `wasm32-unknown-unknown` target. You need to install LLVM via Homebrew:

```bash
brew install llvm
```

Then ensure the Homebrew LLVM is in your PATH when building:

```bash
export PATH="$(brew --prefix llvm)/bin:$PATH"
cargo build --target wasm32-unknown-unknown
```

## FAQ

### Build fails with "No available targets are compatible with triple wasm32-unknown-unknown"

**Error message:**
```
error: unable to create target: 'No available targets are compatible with triple "wasm32-unknown-unknown"'
```

**Cause:** You're using Apple's built-in clang, which doesn't include the WebAssembly backend.

**Solution:** Install LLVM via Homebrew and use it instead:

```bash
brew install llvm
export PATH="$(brew --prefix llvm)/bin:$PATH"
cargo build --target wasm32-unknown-unknown
```

You may want to add the PATH export to your shell profile (`.zshrc`, `.bashrc`, etc.) or use a tool like [direnv](https://direnv.net/) to set it per-project.

## Development

### Regenerating Grammars

```bash
cargo xtask regenerate
```

This will:
1. Run `tree-sitter init --update` for grammars with existing config
2. Run `npm install` for grammars with npm dependencies
3. Run `tree-sitter generate` in dependency order (e.g., CSS before SCSS)
4. Clean up generated files we don't need (bindings, etc.)

### Generating README

The README is generated from `GRAMMARS.toml`. To regenerate:

```bash
cargo xtask generate-readme
```

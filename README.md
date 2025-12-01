# arborium

Batteries-included [tree-sitter](https://tree-sitter.github.io/tree-sitter/) grammar collection with HTML rendering and WASM support.

[![Crates.io](https://img.shields.io/crates/v/arborium.svg)](https://crates.io/crates/arborium)
[![Documentation](https://docs.rs/arborium/badge.svg)](https://docs.rs/arborium)
[![License](https://img.shields.io/crates/l/arborium.svg)](LICENSE-MIT)

## Features

- **69 language grammars** included out of the box
- **67 permissively licensed** (MIT/Apache-2.0/CC0/Unlicense) grammars enabled by default
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

### Permissive Grammars (67)

These grammars use permissive licenses (MIT, Apache-2.0, CC0, Unlicense) and are included by default.

| Feature | Language | License | Source |
|---------|----------|---------|--------|
| `lang-asm` | Assembly | MIT | [tree-sitter-asm](https://github.com/RubixDev/tree-sitter-asm) |
| `lang-awk` | AWK | MIT | [tree-sitter-awk](https://github.com/Beaglefoot/tree-sitter-awk) |
| `lang-bash` | Bash | MIT | [tree-sitter-bash](https://github.com/tree-sitter/tree-sitter-bash) |
| `lang-batch` | Batch | MIT | [tree-sitter-batch](https://github.com/davidevofficial/tree-sitter-batch) |
| `lang-c` | C | MIT | [tree-sitter-c](https://github.com/tree-sitter/tree-sitter-c) |
| `lang-c-sharp` | C# | MIT | [tree-sitter-c-sharp](https://github.com/tree-sitter/tree-sitter-c-sharp) |
| `lang-caddy` | Caddyfile | MIT | [tree-sitter-caddy](https://github.com/Samonitari/tree-sitter-caddy) |
| `lang-capnp` | Cap'n Proto | MIT | [tree-sitter-capnp](https://github.com/tree-sitter-grammars/tree-sitter-capnp) |
| `lang-clojure` | Clojure | Unlicense | [tree-sitter-clojure](https://github.com/sogaiu/tree-sitter-clojure) |
| `lang-cpp` | C++ | MIT | [tree-sitter-cpp](https://github.com/tree-sitter/tree-sitter-cpp) |
| `lang-css` | CSS | MIT | [tree-sitter-css](https://github.com/tree-sitter/tree-sitter-css) |
| `lang-dart` | Dart | MIT | [tree-sitter-dart](https://github.com/UserNobody14/tree-sitter-dart) |
| `lang-devicetree` | Device Tree | MIT | [tree-sitter-devicetree](https://github.com/joelspadin/tree-sitter-devicetree) |
| `lang-diff` | Diff | MIT | [tree-sitter-diff](https://github.com/the-mikedavis/tree-sitter-diff) |
| `lang-dockerfile` | Dockerfile | MIT | [tree-sitter-dockerfile](https://github.com/camdencheek/tree-sitter-dockerfile) |
| `lang-elixir` | Elixir | Apache-2.0 | [tree-sitter-elixir](https://github.com/elixir-lang/tree-sitter-elixir) |
| `lang-elm` | Elm | MIT | [tree-sitter-elm](https://github.com/elm-tooling/tree-sitter-elm) |
| `lang-fsharp` | F# | MIT | [tree-sitter-fsharp](https://github.com/ionide/tree-sitter-fsharp) |
| `lang-gleam` | Gleam | Apache-2.0 | [tree-sitter-gleam](https://github.com/gleam-lang/tree-sitter-gleam) |
| `lang-glsl` | GLSL | MIT | [tree-sitter-glsl](https://github.com/tree-sitter-grammars/tree-sitter-glsl) |
| `lang-go` | Go | MIT | [tree-sitter-go](https://github.com/tree-sitter/tree-sitter-go) |
| `lang-haskell` | Haskell | MIT | [tree-sitter-haskell](https://github.com/tree-sitter/tree-sitter-haskell) |
| `lang-hcl` | HCL (Terraform) | Apache-2.0 | [tree-sitter-hcl](https://github.com/tree-sitter-grammars/tree-sitter-hcl) |
| `lang-hlsl` | HLSL | MIT | [tree-sitter-hlsl](https://github.com/tree-sitter-grammars/tree-sitter-hlsl) |
| `lang-html` | HTML | MIT | [tree-sitter-html](https://github.com/tree-sitter/tree-sitter-html) |
| `lang-idris` | Idris | MIT | [tree-sitter-idris](https://github.com/kayhide/tree-sitter-idris) |
| `lang-ini` | INI | Apache-2.0 | [tree-sitter-ini](https://github.com/justinmk/tree-sitter-ini) |
| `lang-java` | Java | MIT | [tree-sitter-java](https://github.com/tree-sitter/tree-sitter-java) |
| `lang-javascript` | JavaScript | MIT | [tree-sitter-javascript](https://github.com/tree-sitter/tree-sitter-javascript) |
| `lang-jq` | jq | MIT | [tree-sitter-jq](https://github.com/flurie/tree-sitter-jq) |
| `lang-json` | JSON | MIT | [tree-sitter-json](https://github.com/tree-sitter/tree-sitter-json) |
| `lang-lean` | Lean | MIT | [tree-sitter-lean](https://github.com/Julian/tree-sitter-lean) |
| `lang-lua` | Lua | MIT | [tree-sitter-lua](https://github.com/tree-sitter-grammars/tree-sitter-lua) |
| `lang-markdown` | Markdown | MIT | [tree-sitter-markdown](https://github.com/tree-sitter-grammars/tree-sitter-markdown) |
| `lang-meson` | Meson | MIT | [tree-sitter-meson](https://github.com/tree-sitter-grammars/tree-sitter-meson) |
| `lang-nix` | Nix | MIT | [tree-sitter-nix](https://github.com/nix-community/tree-sitter-nix) |
| `lang-objc` | Objective-C | MIT | [tree-sitter-objc](https://github.com/tree-sitter-grammars/tree-sitter-objc) |
| `lang-perl` | Perl | MIT | [tree-sitter-perl](https://github.com/tree-sitter-perl/tree-sitter-perl) |
| `lang-php` | PHP | MIT | [tree-sitter-php](https://github.com/tree-sitter/tree-sitter-php) |
| `lang-postscript` | PostScript | MIT | [tree-sitter-postscript](https://github.com/smoeding/tree-sitter-postscript) |
| `lang-powershell` | PowerShell | MIT | [tree-sitter-powershell](https://github.com/airbus-cert/tree-sitter-powershell) |
| `lang-prolog` | Prolog | MIT | [tree-sitter-prolog](https://codeberg.org/foxy/tree-sitter-prolog) |
| `lang-python` | Python | MIT | [tree-sitter-python](https://github.com/tree-sitter/tree-sitter-python) |
| `lang-r` | R | MIT | [tree-sitter-r](https://github.com/r-lib/tree-sitter-r) |
| `lang-rescript` | ReScript | MIT | [tree-sitter-rescript](https://github.com/rescript-lang/tree-sitter-rescript) |
| `lang-ron` | RON | MIT OR Apache-2.0 | [tree-sitter-ron](https://github.com/tree-sitter-grammars/tree-sitter-ron) |
| `lang-rust` | Rust | MIT | [tree-sitter-rust](https://codeberg.org/grammar-orchard/tree-sitter-rust-orchard) |
| `lang-scala` | Scala | MIT | [tree-sitter-scala](https://github.com/tree-sitter/tree-sitter-scala) |
| `lang-scss` | SCSS | MIT | [tree-sitter-scss](https://github.com/serenadeai/tree-sitter-scss) |
| `lang-sql` | SQL | MIT | [tree-sitter-sql](https://github.com/DerekStride/tree-sitter-sql) |
| `lang-starlark` | Starlark | MIT | [tree-sitter-starlark](https://github.com/tree-sitter-grammars/tree-sitter-starlark) |
| `lang-svelte` | Svelte | MIT | [tree-sitter-svelte](https://github.com/tree-sitter-grammars/tree-sitter-svelte) |
| `lang-thrift` | Thrift | MIT | [tree-sitter-thrift](https://github.com/tree-sitter-grammars/tree-sitter-thrift) |
| `lang-tlaplus` | TLA+ | MIT | [tree-sitter-tlaplus](https://github.com/tlaplus-community/tree-sitter-tlaplus) |
| `lang-toml` | TOML | MIT | [tree-sitter-toml](https://github.com/tree-sitter-grammars/tree-sitter-toml) |
| `lang-typescript` | TypeScript | MIT | [tree-sitter-typescript](https://github.com/tree-sitter/tree-sitter-typescript) |
| `lang-vb` | Visual Basic | MIT | [tree-sitter-vb](https://github.com/CodeAnt-AI/tree-sitter-vb-dotnet) |
| `lang-verilog` | Verilog | MIT | [tree-sitter-verilog](https://github.com/tree-sitter/tree-sitter-verilog) |
| `lang-vhdl` | VHDL | MIT | [tree-sitter-vhdl](https://github.com/alemuller/tree-sitter-vhdl) |
| `lang-vim` | Vimscript | MIT | [tree-sitter-vim](https://github.com/tree-sitter-grammars/tree-sitter-vim) |
| `lang-vue` | Vue | MIT | [tree-sitter-vue](https://github.com/tree-sitter-grammars/tree-sitter-vue) |
| `lang-wasm` | WebAssembly | Apache-2.0 | [tree-sitter-wasm](https://github.com/wasm-lsp/tree-sitter-wasm) |
| `lang-x86asm` | x86 Assembly | MIT | local |
| `lang-xml` | XML | MIT | [tree-sitter-xml](https://github.com/tree-sitter-grammars/tree-sitter-xml) |
| `lang-yaml` | YAML | MIT | [tree-sitter-yaml](https://github.com/tree-sitter-grammars/tree-sitter-yaml) |
| `lang-zig` | Zig | MIT | [tree-sitter-zig](https://github.com/tree-sitter-grammars/tree-sitter-zig) |
| `lang-zsh` | Zsh | MIT | [tree-sitter-zsh](https://github.com/georgeharker/tree-sitter-zsh) |

### GPL-Licensed Grammars (2)

These grammars are **not included by default** due to their copyleft license.
Enabling them may have implications for your project's licensing.

| Feature | Language | License | Source |
|---------|----------|---------|--------|
| `lang-jinja2` | Jinja2 | GPL-3.0 | [tree-sitter-jinja2](https://github.com/dbt-labs/tree-sitter-jinja2) |
| `lang-nginx` | nginx | GPL-3.0 | [tree-sitter-nginx](https://gitlab.com/joncoole/tree-sitter-nginx) |

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

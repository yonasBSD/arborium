# arborium-highlight

Core syntax highlighting engine for arborium.

## Overview

`arborium-highlight` provides:

- **Grammar trait**: Parse text, return spans + injections
- **GrammarProvider trait**: Get grammars by language (sync or async)
- **SyncHighlighter**: For Rust native (statically linked grammars)
- **AsyncHighlighter**: For WASM browser (dynamically loaded plugins)
- **HTML rendering**: `spans_to_html()` outputs `<a-k>`, `<a-s>`, etc.

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        GrammarProvider                          │
│                                                                 │
│  async fn get(&mut self, lang: &str) -> Option<&mut Grammar>    │
└─────────────────────────────────────────────────────────────────┘
        │                                    │
        │ Rust: returns immediately          │ WASM: awaits JS Promise
        │ (statically linked)                │ (wasm-bindgen-futures)
        ▼                                    ▼
┌─────────────────┐                ┌─────────────────┐
│ SyncHighlighter │                │ AsyncHighlighter│
│                 │                │                 │
│ Polls once,     │                │ Actually awaits │
│ panics if       │                │                 │
│ Pending         │                │                 │
└─────────────────┘                └─────────────────┘
```

## Rust Usage

```rust
use arborium::Highlighter;

let mut highlighter = Highlighter::new();
let html = highlighter.highlight("rust", "fn main() {}")?;
// Output: <a-k>fn</a-k> <a-f>main</a-f>() {}
```

## WASM Usage

The host uses `wasm-bindgen` with `wasm-bindgen-futures` for async:

```rust
// In arborium-host (wasm-bindgen crate)
#[wasm_bindgen]
pub async fn highlight(language: &str, source: &str) -> String {
    let mut highlighter = AsyncHighlighter::new(WasmPluginProvider::new());
    highlighter.highlight(language, source).await.unwrap_or_default()
}
```

```javascript
import init, { highlight } from 'arborium-host';

await init();
const html = await highlight('html', '<style>h1 { color: red; }</style>');
```

The `WasmPluginProvider::get()` calls into JS to load grammar plugins.
JS returns a Promise, Rust awaits it via `wasm-bindgen-futures`.
Grammar plugins are WIT components loaded on demand.

## Key Types

```rust
pub struct Span {
    pub start: u32,
    pub end: u32,
    pub capture: String,  // e.g., "keyword", "function"
}

pub struct Injection {
    pub start: u32,
    pub end: u32,
    pub language: String,  // e.g., "css", "javascript"
    pub include_children: bool,
}

pub struct ParseResult {
    pub spans: Vec<Span>,
    pub injections: Vec<Injection>,
}

pub trait Grammar {
    fn parse(&mut self, text: &str) -> ParseResult;
}

pub trait GrammarProvider {
    type Grammar: Grammar;
    async fn get(&mut self, language: &str) -> Option<&mut Self::Grammar>;
}
```

## Injection Handling

When parsing HTML like `<style>h1 { color: red; }</style>`:

1. HTML grammar returns spans for tags + injection `{7..25, "css"}`
2. Highlighter calls `provider.get("css").await`
3. CSS grammar parses `h1 { color: red; }`
4. Spans are offset-adjusted and merged
5. Recurse for any nested injections
6. Render combined spans to HTML

## Crate Structure

```
crates/
├── arborium-highlight/    # This crate - core engine
├── arborium/              # Rust API (SyncHighlighter + StaticProvider)
├── arborium-host/         # WASM host (wasm-bindgen, AsyncHighlighter)
├── arborium-theme/        # Capture → tag mapping
└── lang-*/                # Grammar plugins (WIT components)
```

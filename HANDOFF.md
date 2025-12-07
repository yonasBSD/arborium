# Handoff: arborium-host Integration

## Current Status

We're integrating the `arborium-host` WASM component into the demo so that the demo uses the same code path that actual users would use. Previously, the demo called grammar plugins directly and did span-to-HTML conversion in JavaScript. Now it should go through the host.

## What's Been Done

1. **Build system updated** (`xtask/src/build.rs`, `xtask/src/main.rs`):
   - `cargo xtask build` now builds `arborium-host` first using `cargo-component`
   - Host is transpiled with `jco` to `demo/pkg/host.js`

2. **JS template rewritten** (`xtask/templates/app.stpl.js`):
   - Removed `spansToHtml`, `captureToTag`, `captureToTagMap` (span processing now in Rust)
   - Added `loadHost()` to load the host component
   - Added `createPluginProvider()` implementing the `plugin-provider` interface
   - `highlightCode()` now calls `host.highlight()` and gets HTML directly

3. **Host component** (`crates/arborium-host/`):
   - `src/lib.rs` - orchestrates grammar plugins via plugin-provider callbacks
   - `src/render.rs` - converts spans to HTML with deduplication
   - `wit/host.wit` - defines the interfaces

## Current Bug

The demo fails with:
```
TypeError: Cannot read properties of undefined (reading 'length')
    at trampoline14 (host.js:1059:25)
```

The error is in the jco-generated `host.js` at line 1046-1059:
```javascript
ret = { tag: 'ok', val: pluginParse(arg0 >>> 0, arg1 >>> 0)};
// ...
var {spans: v0_0, injections: v0_1 } = e;  // e is the val from above
var vec3 = v0_0;
var len3 = vec3.length;  // <-- CRASH: v0_0 (spans) is undefined
```

The issue: `trampoline14` wraps our `pluginParse` return value in `{ tag: 'ok', val: ... }`. But then it expects `val` to have `{ spans, injections }` directly.

Looking at the WIT:
```wit
plugin-parse: func(...) -> result<parse-result, parse-error>;
```

The host expects `pluginParse` to return `parse-result` (not `result<parse-result, ...>`), and jco wraps it in ok/err based on whether it throws.

## The Fix Needed

In `createPluginProvider().pluginParse()`, we should return the `parse-result` directly (not wrapped in `{ tag: 'ok', val: ... }`):

```javascript
pluginParse(pluginHandle, sessionHandle) {
    const entry = sessionHandles.get(sessionHandle);
    if (!entry) {
        throw new Error('Invalid session');  // jco will wrap in { tag: 'err' }
    }

    const result = entry.plugin.parse(entry.session);

    // Plugin returns result<parse-result, parse-error>
    // We need to unwrap it and return parse-result directly
    // (jco will re-wrap our return value)
    if (result.tag === 'err') {
        throw new Error(result.val.message);
    }

    const val = result.tag === 'ok' ? result.val : result;
    return {
        spans: val.spans || [],
        injections: val.injections || [],
    };
}
```

The key insight: jco's trampoline does its own ok/err wrapping based on whether the JS function throws. So we should:
- Return `parse-result` directly on success
- Throw an error on failure (jco converts to `{ tag: 'err' }`)

## Files to Edit

- `xtask/templates/app.stpl.js` - fix `pluginParse` to return unwrapped result

## Testing

After fixing, run:
```bash
cd /Users/amos/bearcove/arborium
cargo xtask build --no-transpile
cargo xtask serve
```

Then open the demo in browser and check console for errors.

## Architecture Overview

```
Browser
  │
  ├─► loadHost() ─► host.js (jco-transpiled arborium-host)
  │                    │
  │                    ├─► host.createDocument(lang)
  │                    ├─► host.setText(doc, text)
  │                    └─► host.highlight(doc, depth) ─► HTML string
  │                              │
  │                              │ (calls back into JS via plugin-provider)
  │                              ▼
  └─► createPluginProvider() ◄──┘
          │
          ├─► loadPlugin(lang) ─► grammarCache[lang]
          ├─► createPluginSession(handle) ─► plugin.createSession()
          ├─► pluginSetText(h, s, text) ─► plugin.setText(s, text)
          └─► pluginParse(h, s) ─► plugin.parse(s) ─► { spans, injections }
```

The host component (`arborium-host`) handles:
- Document lifecycle
- Span deduplication in `render.rs`
- HTML generation with proper tag mapping
- Injection recursion (parsing embedded languages)

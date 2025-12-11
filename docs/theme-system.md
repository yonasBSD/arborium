# Arborium Theme System

## 1. Theme Files

Theme files declare CSS variables for syntax highlighting colors. Each theme is either a **light** or **dark** variant, determined by the `variant` field in the source TOML file:

```toml
# crates/arborium-theme/themes/github-light.toml
name = "GitHub Light"
variant = "light"
background = "#ffffff"
foreground = "#24292f"

"keyword" = { fg = "red" }
"function" = { fg = "purple" }
"string" = { fg = "blue" }
# etc

[palette]
red = "#cf222e"
purple = "#8250df"
blue = "#0a3069"
```

```toml
# crates/arborium-theme/themes/tokyo-night.toml
name = "Tokyo Night"
variant = "dark"
background = "#1a1b26"
foreground = "#a9b1d6"

"keyword" = { fg = "purple" }
"function" = { fg = "blue" }
"string" = { fg = "green" }
# etc

[palette]
purple = "#bb9af7"
blue = "#7aa2f7"
green = "#9ece6a"
```

The `variant` field (`"light"` or `"dark"`) determines which CSS variable suffix the theme uses.

**Light themes** define `--arb-*-light` variables:
```css
:root {
  --arb-keyword-light: #c5222e;
  --arb-function-light: #6f42c1;
  --arb-string-light: #0a3069;
  /* etc */
}
```

**Dark themes** define `--arb-*-dark` variables:
```css
:root {
  --arb-keyword-dark: #ff7b72;
  --arb-function-dark: #d2a8ff;
  --arb-string-dark: #a5d6ff;
  /* etc */
}
```

Users include **two theme files** - one light, one dark - to get automatic switching based on system preference or `data-theme` attribute.

Example:
```html
<link rel="stylesheet" href="base.css">
<link rel="stylesheet" href="themes/github-light.css">
<link rel="stylesheet" href="themes/tokyo-night.css">
```

## 2. Base CSS

The base CSS file:
- Defines the `a-*` element selectors (e.g., `a-k`, `a-f`, `a-s`)
- Uses CSS variables from theme files
- Handles switching between light/dark via `@media (prefers-color-scheme)` and `[data-theme]` attributes

```css
/* Default: light */
a-k { color: var(--arb-keyword-light); }
a-f { color: var(--arb-function-light); }

/* System preference: dark */
@media (prefers-color-scheme: dark) {
  a-k { color: var(--arb-keyword-dark); }
  a-f { color: var(--arb-function-dark); }
}

/* Explicit data-theme overrides */
:root[data-theme="light"] {
  a-k { color: var(--arb-keyword-light); }
  a-f { color: var(--arb-function-light); }
}

:root[data-theme="dark"] {
  a-k { color: var(--arb-keyword-dark); }
  a-f { color: var(--arb-function-dark); }
}
```

## 3. docs.rs Integration

docs.rs has three themes: `light`, `dark`, and `ayu`. It uses a **different base CSS** and **JavaScript-based switching**.

### docs.rs Base CSS

The docs.rs base CSS uses **CSS variable fallback** - no media queries or `[data-theme]` selectors needed:

```css
a-k { color: var(--arb-keyword-dark, var(--arb-keyword-light)); }
a-f { color: var(--arb-function-dark, var(--arb-function-light)); }
/* etc */
```

If `--arb-keyword-dark` is defined, it's used. Otherwise, falls back to `--arb-keyword-light`.

### Rustdoc Theme Files

Theme files are **the same as all other themes** - they define either `-light` or `-dark` variables:

- `rustdoc-light.css` defines `--arb-*-light` variables
- `rustdoc-dark.css` defines `--arb-*-dark` variables  
- `rustdoc-ayu.css` defines `--arb-*-dark` variables (it's a dark theme)

### JavaScript Switching

The JavaScript:
1. Detects if running on a rustdoc page (checks for `<meta name="generator" content="rustdoc">`)
2. Reads the current theme from `data-theme` attribute (`light`, `dark`, or `ayu`)
3. Dynamically adds/removes `<link>` tags to load only the active theme's CSS
4. Watches for theme changes and swaps the CSS accordingly

Since only one theme file is loaded at a time:
- When `rustdoc-dark.css` is loaded, only `--arb-*-dark` vars exist → those are used
- When `rustdoc-light.css` is loaded, only `--arb-*-light` vars exist → fallback kicks in, those are used

## 4. IIFE Bundle Defaults

When using the IIFE bundle (`arborium.iife.js`), the default behavior is:

- **Base CSS**: `base-rustdoc.css` (uses variable fallback)
- **Light theme**: `github-light.css`
- **Dark theme**: `one-dark.css`

The IIFE automatically:
1. Detects if running on a rustdoc page (checks for `<meta name="generator" content="rustdoc">`)
2. On rustdoc: maps `light`/`dark`/`ayu` to `rustdoc-light`/`rustdoc-dark`/`rustdoc-ayu`
3. Elsewhere: uses system preference (`prefers-color-scheme`) or `data-theme` attribute
4. Loads the appropriate theme CSS from CDN
5. Watches for theme changes and swaps CSS dynamically

Users can override defaults via data attributes on the script tag:
```html
<script 
  src="https://cdn.jsdelivr.net/npm/@arborium/arborium@1/dist/arborium.iife.js"
  data-theme-light="catppuccin-latte"
  data-theme-dark="catppuccin-mocha"
></script>
```

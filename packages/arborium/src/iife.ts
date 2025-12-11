/**
 * @arborium/arborium IIFE bundle
 *
 * Drop-in auto-highlighter that runs on page load.
 * Configuration via data attributes or window.Arborium object.
 */

import { loadGrammar, highlight, getConfig, setConfig, defaultConfig } from "./loader.js";
import { detectLanguage, extractLanguageFromClass, normalizeLanguage } from "./detect.js";
import type { ArboriumConfig } from "./types.js";

// Capture current script immediately (before any async operations)
const currentScript = document.currentScript as HTMLScriptElement | null;

/** Parse query parameters from script src URL */
function getQueryParams(): URLSearchParams {
  if (!currentScript?.src) return new URLSearchParams();
  try {
    const url = new URL(currentScript.src);
    return url.searchParams;
  } catch {
    return new URLSearchParams();
  }
}

/** Parse configuration from script data attributes and query params */
function getConfigFromScript(): Partial<ArboriumConfig> {
  if (!currentScript) return {};

  const config: Partial<ArboriumConfig> = {};
  const params = getQueryParams();

  // Data attributes
  if (currentScript.hasAttribute("data-manual")) {
    config.manual = true;
  }

  const theme = currentScript.getAttribute("data-theme");
  if (theme) config.theme = theme;

  const selector = currentScript.getAttribute("data-selector");
  if (selector) config.selector = selector;

  const cdn = currentScript.getAttribute("data-cdn");
  if (cdn) config.cdn = cdn;

  const version = currentScript.getAttribute("data-version");
  if (version) config.version = version;

  // Query parameters (for local testing)
  const pluginsUrl = params.get("pluginsUrl");
  if (pluginsUrl) config.pluginsUrl = pluginsUrl;

  const hostUrl = params.get("hostUrl");
  if (hostUrl) config.hostUrl = hostUrl;

  return config;
}

/** Detect if we're running on a rustdoc-generated page */
function isRustdocEnvironment(): boolean {
  const generator = document.querySelector('meta[name="generator"]');
  return generator?.getAttribute("content") === "rustdoc";
}

/** Map rustdoc theme names to Arborium theme IDs */
function mapRustdocTheme(value?: string): string | null {
  if (!value) return null;

  const themeMap: Record<string, string> = {
    light: "rustdoc-light",
    dark: "rustdoc-dark",
    ayu: "rustdoc-ayu",
  };

  if (themeMap[value]) {
    return themeMap[value];
  }

  // Unknown theme value: return as-is or fallback to null
  return null;
}

/** Detect the current theme from rustdoc or environment */
function getAutoTheme(): string {
  if (isRustdocEnvironment()) {
    // Try data-theme attribute (used by both docs.rs and local rustdoc)
    const rustdocTheme = mapRustdocTheme(document.documentElement.dataset.theme);
    if (rustdocTheme) {
      return rustdocTheme;
    }
  }

  // Fall back to system preference
  const isLight = window.matchMedia("(prefers-color-scheme: light)").matches;
  return isLight ? "github-light" : "one-dark";
}

/** Get merged configuration from all sources and apply to loader */
function getMergedConfig(): Required<ArboriumConfig> {
  // Priority: data attributes > window.Arborium > auto-detect > defaults
  const windowConfig = window.Arborium || {};
  const scriptConfig = getConfigFromScript();
  const merged = { ...windowConfig, ...scriptConfig };

  // Auto-detect theme if not explicitly set
  if (!merged.theme) {
    merged.theme = getAutoTheme();
  }

  // Apply to loader so host loading uses correct URLs
  setConfig(merged);
  return getConfig();
}

/** Find all code blocks that need highlighting */
function findCodeBlocks(selector: string): HTMLElement[] {
  return Array.from(document.querySelectorAll(selector));
}

/** Check if a code block already has syntax highlighting or semantic markup */
function hasExistingHighlighting(block: HTMLElement): boolean {
  // Check for common highlighting library markers
  const highlightClasses = ["hljs", "highlighted", "prism-code", "shiki"];
  for (const cls of highlightClasses) {
    if (block.classList.contains(cls)) return true;
  }

  // Check if there are spans with syntax highlighting classes inside
  // (highlight.js, prism, etc. use spans with classes)
  const spans = block.querySelectorAll("span[class]");
  if (spans.length > 0) {
    // If there are multiple spans with classes, likely already highlighted
    // Be conservative: even a few spans suggest existing highlighting
    const classedSpans = Array.from(spans).filter((s) => s.className && s.className.length > 0);
    if (classedSpans.length >= 3) return true;
  }

  // Check for semantic markup (e.g., docs.rs uses <a> tags for type/function links)
  // If there are links inside the code, it has meaningful markup we shouldn't destroy
  const links = block.querySelectorAll("a");
  if (links.length >= 2) return true;

  return false;
}

/** Get the language for a code block */
function getLanguageForBlock(block: HTMLElement): string | null {
  // Check data-lang attribute
  const dataLang = block.getAttribute("data-lang");
  if (dataLang) return normalizeLanguage(dataLang);

  // Check class="language-*"
  const className = block.className;
  const classLang = extractLanguageFromClass(className);
  if (classLang) return normalizeLanguage(classLang);

  // Check parent element (often <pre> wraps <code>)
  const parent = block.parentElement;
  if (parent) {
    const parentDataLang = parent.getAttribute("data-lang");
    if (parentDataLang) return normalizeLanguage(parentDataLang);

    const parentClassLang = extractLanguageFromClass(parent.className);
    if (parentClassLang) return normalizeLanguage(parentClassLang);
  }

  // Try auto-detection
  const source = block.textContent || "";
  return detectLanguage(source);
}

/** Get the CSS base URL */
function getCssBaseUrl(): string {
  const config = getMergedConfig();

  if (config.hostUrl) {
    return `${config.hostUrl}/themes`;
  }

  const cdn = config.cdn;
  const version = config.version;

  let baseUrl: string;
  if (cdn === "jsdelivr") {
    baseUrl = "https://cdn.jsdelivr.net/npm";
  } else if (cdn === "unpkg") {
    baseUrl = "https://unpkg.com";
  } else {
    baseUrl = cdn;
  }

  const versionSuffix = version === "latest" ? "" : `@${version}`;
  return `${baseUrl}/@arborium/arborium${versionSuffix}/dist/themes`;
}

/** Inject base CSS (only once) */
function injectBaseCSS(): void {
  const baseId = "arborium-base";
  if (document.getElementById(baseId)) return;

  const cssUrl = `${getCssBaseUrl()}/base-rustdoc.css`;
  console.debug(`[arborium] Loading base CSS: ${cssUrl}`);

  const link = document.createElement("link");
  link.id = baseId;
  link.rel = "stylesheet";
  link.href = cssUrl;
  document.head.appendChild(link);
}

// Track currently loaded theme
let currentThemeId: string | null = null;

/** Inject theme CSS, removing any previously loaded theme */
function injectThemeCSS(theme: string): void {
  // Remove old theme if different
  if (currentThemeId && currentThemeId !== theme) {
    const oldLink = document.getElementById(`arborium-theme-${currentThemeId}`);
    if (oldLink) {
      oldLink.remove();
      console.debug(`[arborium] Removed theme: ${currentThemeId}`);
    }
  }

  const themeId = `arborium-theme-${theme}`;
  if (document.getElementById(themeId)) {
    currentThemeId = theme;
    return;
  }

  const cssUrl = `${getCssBaseUrl()}/${theme}.css`;
  console.debug(`[arborium] Loading theme: ${cssUrl}`);

  const link = document.createElement("link");
  link.id = themeId;
  link.rel = "stylesheet";
  link.href = cssUrl;
  document.head.appendChild(link);

  currentThemeId = theme;
}

/** Highlight a single code block */
async function highlightBlock(
  block: HTMLElement,
  language: string,
  config: ArboriumConfig,
): Promise<void> {
  const source = block.textContent || "";
  if (!source.trim()) return;

  try {
    const html = await highlight(language, source, config);
    block.innerHTML = html;
    block.setAttribute("data-highlighted", "true");
    block.setAttribute("data-lang", language);
  } catch (err) {
    console.warn(`[arborium] Failed to highlight ${language}:`, err);
    // Don't modify the block on error
  }
}

/** Main auto-highlight function */
async function autoHighlight(): Promise<void> {
  const config = getMergedConfig();

  // Inject base CSS (defines a-* selectors using variables)
  injectBaseCSS();

  // Inject theme CSS (defines the variables)
  injectThemeCSS(config.theme);

  // Find all code blocks
  const blocks = findCodeBlocks(config.selector);
  if (blocks.length === 0) return;

  // Group blocks by language
  const blocksByLanguage = new Map<string, HTMLElement[]>();
  const unknownBlocks: HTMLElement[] = [];

  for (const block of blocks) {
    // Skip already highlighted blocks
    if (block.hasAttribute("data-highlighted")) continue;

    // Skip blocks that appear to have existing syntax highlighting
    // (e.g., docs.rs uses spans with classes for highlighting)
    if (hasExistingHighlighting(block)) continue;

    const language = getLanguageForBlock(block);
    if (language) {
      const existing = blocksByLanguage.get(language) || [];
      existing.push(block);
      blocksByLanguage.set(language, existing);
    } else {
      unknownBlocks.push(block);
    }
  }

  // Load grammars in parallel for all detected languages
  const languages = Array.from(blocksByLanguage.keys());
  const loadPromises = languages.map((lang) =>
    loadGrammar(lang, config).catch((err) => {
      console.warn(`[arborium] Failed to load grammar for ${lang}:`, err);
      return null;
    }),
  );

  // Wait for all grammars to load
  const grammars = await Promise.all(loadPromises);

  // Highlight blocks for each loaded grammar
  const highlightPromises: Promise<void>[] = [];

  for (let i = 0; i < languages.length; i++) {
    const language = languages[i];
    const grammar = grammars[i];
    if (!grammar) continue;

    const languageBlocks = blocksByLanguage.get(language) || [];
    for (const block of languageBlocks) {
      highlightPromises.push(highlightBlock(block, language, config));
    }
  }

  // Wait for all highlighting to complete
  await Promise.all(highlightPromises);

  // Log summary
  const total = blocks.length;
  const highlighted = blocks.filter((b) => b.hasAttribute("data-highlighted")).length;
  const skipped = unknownBlocks.length;

  if (highlighted > 0 || skipped > 0) {
    console.debug(
      `[arborium] Highlighted ${highlighted}/${total} blocks` +
        (skipped > 0 ? ` (${skipped} unknown language)` : ""),
    );
  }
}

/** Public API for manual highlighting */
export async function highlightAll(config?: ArboriumConfig): Promise<void> {
  const mergedConfig = getConfig({ ...getMergedConfig(), ...config });
  await autoHighlight();
}

/** Public API for highlighting a specific element */
export async function highlightElement(
  element: HTMLElement,
  language?: string,
  config?: ArboriumConfig,
): Promise<void> {
  const mergedConfig = getConfig({ ...getMergedConfig(), ...config });
  const lang = language || getLanguageForBlock(element);

  if (!lang) {
    console.warn("[arborium] Could not detect language for element");
    return;
  }

  await highlightBlock(element, lang, mergedConfig);
}

// Expose public API on window
(window as any).arborium = {
  highlightAll,
  highlightElement,
  loadGrammar,
  highlight,
  detectLanguage,
  config: getMergedConfig(),
};

/** Re-highlight all blocks when theme changes */
async function onThemeChange(): Promise<void> {
  const newTheme = getAutoTheme();
  const currentConfig = getMergedConfig();

  // Only re-highlight if theme actually changed and wasn't explicitly set
  if (currentConfig.theme !== newTheme) {
    // Update config
    setConfig({ theme: newTheme });
    (window as any).arborium.config = getMergedConfig();

    // Inject new theme CSS
    injectThemeCSS(newTheme);

    // No need to re-highlight - CSS handles the colors
    // The spans are already in place, just the theme CSS changes
    console.debug(`[arborium] Theme changed to: ${newTheme}`);
  }
}

/** Set up theme change watchers */
function watchThemeChanges(): void {
  // Watch for rustdoc theme attribute changes
  const observer = new MutationObserver((mutations) => {
    for (const mutation of mutations) {
      const attr = mutation.attributeName;
      if (attr === "data-theme" && isRustdocEnvironment()) {
        onThemeChange();
        break;
      }
    }
  });
  observer.observe(document.documentElement, { attributes: true });

  // Watch for system color scheme changes
  window
    .matchMedia("(prefers-color-scheme: light)")
    .addEventListener("change", () => onThemeChange());
}

// Auto-highlight on DOMContentLoaded (unless manual mode)
const config = getMergedConfig();
if (!config.manual) {
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", () => {
      autoHighlight();
      watchThemeChanges();
    });
  } else {
    // DOM already loaded
    autoHighlight();
    watchThemeChanges();
  }
}

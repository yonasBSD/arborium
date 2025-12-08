/**
 * Arborium loader - loads the host WASM and grammar plugins.
 *
 * Architecture:
 * 1. Load arborium-host WASM (built with wasm-bindgen)
 * 2. Host imports { loadGrammar, parse, isLanguageAvailable } from window.arboriumHost
 * 3. We implement those by loading WIT grammar plugins on demand
 * 4. Host exports highlight(language, source) -> Promise<string>
 */

import type { ParseResult, ArboriumConfig, Grammar, Span, Injection } from './types.js';

// Default config
const DEFAULT_CONFIG: Required<ArboriumConfig> = {
  manual: false,
  theme: 'tokyo-night',
  selector: 'pre code',
  cdn: 'jsdelivr',
  version: 'latest',
};

// Merged config
let config: Required<ArboriumConfig> = { ...DEFAULT_CONFIG };

// Host module (loaded lazily)
let hostModule: {
  highlight: (language: string, source: string) => Promise<string>;
  isLanguageAvailable: (language: string) => boolean;
} | null = null;

// Grammar plugins cache
const grammarCache = new Map<string, GrammarPlugin>();

// Languages we know are available (from plugins.json)
let availableLanguages: Set<string> = new Set();

// Plugin manifest
let pluginsManifest: Record<string, { js: string; wasm: string }> | null = null;

// Grammar handle counter (simple incrementing ID)
let nextGrammarHandle = 1;

// Map from handle to plugin
const handleToPlugin = new Map<number, GrammarPlugin>();

/** A loaded grammar plugin (WIT component) */
interface GrammarPlugin {
  languageId: string;
  injectionLanguages: string[];
  parse: (text: string) => ParseResult;
}

/** Get the CDN base URL */
function getCdnUrl(): string {
  if (config.cdn === 'jsdelivr') {
    return `https://cdn.jsdelivr.net/npm/@anthropic-ai/arborium@${config.version}`;
  } else if (config.cdn === 'unpkg') {
    return `https://unpkg.com/@anthropic-ai/arborium@${config.version}`;
  }
  return config.cdn; // Custom URL
}

/** Load the plugins manifest */
async function loadPluginsManifest(): Promise<void> {
  if (pluginsManifest) return;

  const url = `${getCdnUrl()}/plugins.json`;
  const response = await fetch(url);
  pluginsManifest = await response.json();

  // Populate available languages
  availableLanguages = new Set(Object.keys(pluginsManifest));
}

/** Load a grammar plugin */
async function loadGrammarPlugin(language: string): Promise<GrammarPlugin | null> {
  // Check cache
  const cached = grammarCache.get(language);
  if (cached) return cached;

  // Ensure manifest is loaded
  await loadPluginsManifest();

  // Check if language exists
  const pluginInfo = pluginsManifest?.[language];
  if (!pluginInfo) return null;

  const baseUrl = getCdnUrl();

  try {
    // Load the plugin JS and WASM
    const jsUrl = `${baseUrl}/${pluginInfo.js}`;
    const wasmUrl = `${baseUrl}/${pluginInfo.wasm}`;

    // Dynamically import the JS module
    const module = await import(/* @vite-ignore */ jsUrl);

    // Fetch the WASM
    const wasmResponse = await fetch(wasmUrl);
    const wasmBytes = await wasmResponse.arrayBuffer();

    // Instantiate
    const instance = await module.instantiate(wasmBytes);

    // Wrap as GrammarPlugin
    const plugin: GrammarPlugin = {
      languageId: language,
      injectionLanguages: instance.plugin.injectionLanguages?.() ?? [],
      parse: (text: string) => {
        const result = instance.plugin.parse(text);
        return {
          spans: result.spans.map((s: any) => ({
            start: s.start,
            end: s.end,
            capture: s.capture,
          })),
          injections: result.injections.map((i: any) => ({
            start: i.start,
            end: i.end,
            language: i.language,
            includeChildren: i.includeChildren,
          })),
        };
      },
    };

    grammarCache.set(language, plugin);
    return plugin;
  } catch (e) {
    console.error(`Failed to load grammar plugin for ${language}:`, e);
    return null;
  }
}

/** Setup window.arboriumHost for the Rust host to call into */
function setupHostInterface(): void {
  (window as any).arboriumHost = {
    /** Check if a language is available (sync) */
    isLanguageAvailable(language: string): boolean {
      return availableLanguages.has(language) || grammarCache.has(language);
    },

    /** Load a grammar and return a handle (async) */
    async loadGrammar(language: string): Promise<number> {
      const plugin = await loadGrammarPlugin(language);
      if (!plugin) {
        return 0; // 0 means "not found"
      }

      // Check if we already have a handle for this plugin
      for (const [handle, p] of handleToPlugin) {
        if (p === plugin) {
          return handle;
        }
      }

      // Assign a new handle
      const handle = nextGrammarHandle++;
      handleToPlugin.set(handle, plugin);
      return handle;
    },

    /** Parse text using a grammar handle (sync) */
    parse(handle: number, text: string): ParseResult {
      const plugin = handleToPlugin.get(handle);
      if (!plugin) {
        return { spans: [], injections: [] };
      }
      return plugin.parse(text);
    },
  };
}

/** Load the host WASM module */
async function loadHost(): Promise<void> {
  if (hostModule) return;

  // Setup the interface that the host will import
  setupHostInterface();

  // Load the host WASM (built with wasm-bindgen)
  const baseUrl = getCdnUrl();
  const hostUrl = `${baseUrl}/arborium_host.js`;

  try {
    const module = await import(/* @vite-ignore */ hostUrl);
    await module.default(); // Initialize wasm-bindgen

    hostModule = {
      highlight: module.highlight,
      isLanguageAvailable: module.isLanguageAvailable,
    };
  } catch (e) {
    console.error('Failed to load arborium host:', e);
    throw e;
  }
}

/** Initialize arborium with config */
export async function init(userConfig?: Partial<ArboriumConfig>): Promise<void> {
  config = { ...DEFAULT_CONFIG, ...userConfig };
  await loadPluginsManifest();
  await loadHost();
}

/** Highlight source code */
export async function highlight(language: string, source: string): Promise<string> {
  await loadHost();
  if (!hostModule) throw new Error('Host not loaded');
  return hostModule.highlight(language, source);
}

/** Load a grammar for direct use */
export async function loadGrammar(language: string): Promise<Grammar | null> {
  const plugin = await loadGrammarPlugin(language);
  if (!plugin) return null;

  return {
    languageId: () => plugin.languageId,
    injectionLanguages: () => plugin.injectionLanguages,
    highlight: (source: string) => {
      // For direct grammar use, we just parse and render
      // This doesn't handle injections - use highlight() for that
      const result = plugin.parse(source);
      return spansToHtml(source, result.spans);
    },
    parse: (source: string) => plugin.parse(source),
    dispose: () => {
      // No-op for now, plugins are cached
    },
  };
}

/** Convert spans to HTML (simple version without injection handling) */
export function spansToHtml(source: string, spans: Span[]): string {
  // This is a simplified version - the real one is in Rust
  // Sort spans by start position
  const sorted = [...spans].sort((a, b) => a.start - b.start);

  let html = '';
  let pos = 0;

  for (const span of sorted) {
    // Add text before span
    if (span.start > pos) {
      html += escapeHtml(source.slice(pos, span.start));
    }

    // Get tag for capture
    const tag = getTagForCapture(span.capture);
    const text = escapeHtml(source.slice(span.start, span.end));

    if (tag) {
      html += `<a-${tag}>${text}</a-${tag}>`;
    } else {
      html += text;
    }

    pos = span.end;
  }

  // Add remaining text
  if (pos < source.length) {
    html += escapeHtml(source.slice(pos));
  }

  return html;
}

/** Get the short tag for a capture name */
function getTagForCapture(capture: string): string | null {
  // Map captures to short tags (from arborium-theme)
  if (capture.startsWith('keyword') || capture === 'include' || capture === 'conditional') {
    return 'k';
  }
  if (capture.startsWith('function') || capture.startsWith('method')) {
    return 'f';
  }
  if (capture.startsWith('string') || capture === 'character') {
    return 's';
  }
  if (capture.startsWith('comment')) {
    return 'c';
  }
  if (capture.startsWith('type')) {
    return 't';
  }
  if (capture.startsWith('variable')) {
    return 'v';
  }
  if (capture.startsWith('number') || capture === 'float') {
    return 'n';
  }
  if (capture.startsWith('operator')) {
    return 'o';
  }
  if (capture.startsWith('punctuation')) {
    return 'p';
  }
  return null;
}

/** Escape HTML special characters */
function escapeHtml(text: string): string {
  return text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

/** Get current config */
export function getConfig(): Required<ArboriumConfig> {
  return { ...config };
}

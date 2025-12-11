/** A span of highlighted text */
export interface Span {
  start: number;
  end: number;
  /** The capture name (e.g., "keyword", "string", "comment") */
  capture: string;
}

/** A language injection (e.g., JS inside HTML) */
export interface Injection {
  start: number;
  end: number;
  language: string;
  includeChildren: boolean;
}

/** Result of parsing source code */
export interface ParseResult {
  spans: Span[];
  injections: Injection[];
}

/** A loaded grammar plugin */
export interface Grammar {
  /** The language identifier */
  languageId(): string;
  /** Languages this grammar may inject */
  injectionLanguages(): string[];
  /** Highlight source code, returning HTML string */
  highlight(source: string): string | Promise<string>;
  /** Parse source code, returning raw spans */
  parse(source: string): ParseResult;
  /** Dispose of resources */
  dispose(): void;
}

/** Configuration for the arborium runtime */
export interface ArboriumConfig {
  /** Disable auto-highlighting on page load */
  manual?: boolean;
  /** Theme to use: "tokyo-night" | "github-light" | custom */
  theme?: string;
  /** CSS selector for code blocks */
  selector?: string;
  /** CDN to use: "jsdelivr" | "unpkg" | custom base URL */
  cdn?: string;
  /** Package version to load (default: "1" for latest 1.x.x) */
  version?: string;
  /** URL to plugins.json manifest - overrides bundled manifest (for local testing) */
  pluginsUrl?: string;
  /** Base URL for the Rust host module (for local testing) */
  hostUrl?: string;
}

/** Global config set before script loads */
declare global {
  interface Window {
    Arborium?: ArboriumConfig;
  }
}

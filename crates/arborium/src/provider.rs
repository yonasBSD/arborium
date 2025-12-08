//! Static grammar provider for the arborium crate.
//!
//! This module provides `StaticProvider`, a `GrammarProvider` implementation
//! that creates `TreeSitterGrammar` instances for enabled language features.

use std::collections::HashMap;

use arborium_highlight::{
    tree_sitter::{TreeSitterGrammar, TreeSitterGrammarConfig},
    Grammar, GrammarProvider, ParseResult,
};

/// A provider that creates tree-sitter grammars for enabled languages.
///
/// Grammars are lazily created on first use and cached.
pub struct StaticProvider {
    grammars: HashMap<&'static str, TreeSitterGrammar>,
}

impl Default for StaticProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl StaticProvider {
    /// Create a new static provider.
    pub fn new() -> Self {
        Self {
            grammars: HashMap::new(),
        }
    }

    /// Normalize a language name to its canonical form.
    fn normalize_language(language: &str) -> &'static str {
        match language {
            "js" | "jsx" | "mjs" | "cjs" => "javascript",
            "ts" | "mts" | "cts" => "typescript",
            "py" | "py3" | "python3" => "python",
            "rb" => "ruby",
            "rs" => "rust",
            "sh" | "shell" => "bash",
            "yml" => "yaml",
            "htm" => "html",
            "cs" | "csharp" => "c-sharp",
            "c++" | "cxx" | "hpp" => "cpp",
            "golang" => "go",
            "hs" => "haskell",
            "ex" | "exs" => "elixir",
            "erl" => "erlang",
            "kt" | "kts" => "kotlin",
            "ml" => "ocaml",
            "pl" | "pm" => "perl",
            "ps1" | "pwsh" => "powershell",
            "sass" => "scss",
            "tf" | "terraform" => "hcl",
            "bat" | "cmd" => "batch",
            "dockerfile" | "docker" => "dockerfile",
            "h" => "c",
            "lisp" | "cl" => "commonlisp",
            "el" | "emacs-lisp" => "elisp",
            "jl" => "julia",
            "m" => "matlab",
            "mm" | "objective-c" => "objc",
            "json" | "jsonc" => "json",
            "scm" => "query",
            "rlang" => "r",
            "res" => "rescript",
            "rq" => "sparql",
            "mysql" | "postgresql" | "postgres" | "sqlite" => "sql",
            "pbtxt" | "textpb" => "textproto",
            "tla" => "tlaplus",
            "typ" => "typst",
            "ua" => "uiua",
            "vbnet" | "visualbasic" => "vb",
            "v" | "sv" | "systemverilog" => "verilog",
            "vhd" => "vhdl",
            "nasm" | "x86" => "x86asm",
            "xsl" | "xslt" | "svg" => "xml",
            "jinja" | "j2" => "jinja2",
            "gql" => "graphql",
            "vert" | "frag" => "glsl",
            "conf" | "cfg" => "ini",
            "bzl" | "bazel" => "starlark",
            "patch" => "diff",
            "dlang" => "d",
            "f#" | "fs" => "fsharp",
            // Return original if it matches a known language
            "ada" | "agda" | "asm" | "awk" | "bash" | "batch" | "c" | "c-sharp" | "caddy"
            | "capnp" | "clojure" | "cmake" | "commonlisp" | "cpp" | "css" | "d" | "dart"
            | "devicetree" | "diff" | "dockerfile" | "dot" | "elisp" | "elixir" | "elm"
            | "erlang" | "fish" | "fsharp" | "gleam" | "glsl" | "go" | "graphql" | "haskell"
            | "hcl" | "hlsl" | "html" | "idris" | "ini" | "java" | "javascript" | "jinja2"
            | "jq" | "json" | "julia" | "kdl" | "kotlin" | "lean" | "lua" | "matlab" | "meson"
            | "nginx" | "ninja" | "nix" | "objc" | "ocaml" | "perl" | "php" | "powershell"
            | "prolog" | "python" | "query" | "r" | "rescript" | "ron" | "ruby" | "rust"
            | "scala" | "scheme" | "scss" | "sparql" | "sql" | "ssh-config" | "starlark"
            | "svelte" | "swift" | "textproto" | "thrift" | "tlaplus" | "toml" | "tsx"
            | "typescript" | "typst" | "uiua" | "vb" | "verilog" | "vhdl" | "vim" | "vue"
            | "x86asm" | "xml" | "yaml" | "yuri" | "zig" | "zsh" => {
                // Need to return a &'static str, so leak the string
                // This is fine because language names are finite and small
                Box::leak(language.to_string().into_boxed_str())
            }
            other => Box::leak(other.to_string().into_boxed_str()),
        }
    }

    /// Create a grammar for a language.
    #[allow(unused_variables)]
    fn create_grammar(language: &str) -> Option<TreeSitterGrammar> {
        macro_rules! try_lang {
            ($feature:literal, $module:ident, $primary:literal) => {
                #[cfg(feature = $feature)]
                if language == $primary {
                    let config = TreeSitterGrammarConfig {
                        language: crate::$module::language().into(),
                        highlights_query: &crate::$module::HIGHLIGHTS_QUERY,
                        injections_query: crate::$module::INJECTIONS_QUERY,
                        locals_query: crate::$module::LOCALS_QUERY,
                    };
                    return TreeSitterGrammar::new(config).ok();
                }
            };
        }

        // Core languages for injections
        try_lang!("lang-javascript", lang_javascript, "javascript");
        try_lang!("lang-css", lang_css, "css");
        try_lang!("lang-typescript", lang_typescript, "typescript");

        // All other languages
        try_lang!("lang-ada", lang_ada, "ada");
        try_lang!("lang-agda", lang_agda, "agda");
        try_lang!("lang-asm", lang_asm, "asm");
        try_lang!("lang-awk", lang_awk, "awk");
        try_lang!("lang-bash", lang_bash, "bash");
        try_lang!("lang-batch", lang_batch, "batch");
        try_lang!("lang-c", lang_c, "c");
        try_lang!("lang-c-sharp", lang_c_sharp, "c-sharp");
        try_lang!("lang-caddy", lang_caddy, "caddy");
        try_lang!("lang-capnp", lang_capnp, "capnp");
        try_lang!("lang-clojure", lang_clojure, "clojure");
        try_lang!("lang-cmake", lang_cmake, "cmake");
        try_lang!("lang-commonlisp", lang_commonlisp, "commonlisp");
        try_lang!("lang-cpp", lang_cpp, "cpp");
        try_lang!("lang-d", lang_d, "d");
        try_lang!("lang-dart", lang_dart, "dart");
        try_lang!("lang-devicetree", lang_devicetree, "devicetree");
        try_lang!("lang-diff", lang_diff, "diff");
        try_lang!("lang-dockerfile", lang_dockerfile, "dockerfile");
        try_lang!("lang-dot", lang_dot, "dot");
        try_lang!("lang-elisp", lang_elisp, "elisp");
        try_lang!("lang-elixir", lang_elixir, "elixir");
        try_lang!("lang-elm", lang_elm, "elm");
        try_lang!("lang-erlang", lang_erlang, "erlang");
        try_lang!("lang-fish", lang_fish, "fish");
        try_lang!("lang-fsharp", lang_fsharp, "fsharp");
        try_lang!("lang-gleam", lang_gleam, "gleam");
        try_lang!("lang-glsl", lang_glsl, "glsl");
        try_lang!("lang-go", lang_go, "go");
        try_lang!("lang-graphql", lang_graphql, "graphql");
        try_lang!("lang-haskell", lang_haskell, "haskell");
        try_lang!("lang-hcl", lang_hcl, "hcl");
        try_lang!("lang-hlsl", lang_hlsl, "hlsl");
        try_lang!("lang-html", lang_html, "html");
        try_lang!("lang-ini", lang_ini, "ini");
        try_lang!("lang-java", lang_java, "java");
        try_lang!("lang-jinja2", lang_jinja2, "jinja2");
        try_lang!("lang-jq", lang_jq, "jq");
        try_lang!("lang-json", lang_json, "json");
        try_lang!("lang-julia", lang_julia, "julia");
        try_lang!("lang-kdl", lang_kdl, "kdl");
        try_lang!("lang-kotlin", lang_kotlin, "kotlin");
        try_lang!("lang-lean", lang_lean, "lean");
        try_lang!("lang-lua", lang_lua, "lua");
        try_lang!("lang-matlab", lang_matlab, "matlab");
        try_lang!("lang-meson", lang_meson, "meson");
        try_lang!("lang-nginx", lang_nginx, "nginx");
        try_lang!("lang-ninja", lang_ninja, "ninja");
        try_lang!("lang-nix", lang_nix, "nix");
        try_lang!("lang-objc", lang_objc, "objc");
        try_lang!("lang-ocaml", lang_ocaml, "ocaml");
        try_lang!("lang-perl", lang_perl, "perl");
        try_lang!("lang-php", lang_php, "php");
        try_lang!("lang-powershell", lang_powershell, "powershell");
        try_lang!("lang-prolog", lang_prolog, "prolog");
        try_lang!("lang-python", lang_python, "python");
        try_lang!("lang-query", lang_query, "query");
        try_lang!("lang-r", lang_r, "r");
        try_lang!("lang-rescript", lang_rescript, "rescript");
        try_lang!("lang-ron", lang_ron, "ron");
        try_lang!("lang-ruby", lang_ruby, "ruby");
        try_lang!("lang-rust", lang_rust, "rust");
        try_lang!("lang-scala", lang_scala, "scala");
        try_lang!("lang-scheme", lang_scheme, "scheme");
        try_lang!("lang-scss", lang_scss, "scss");
        try_lang!("lang-sparql", lang_sparql, "sparql");
        try_lang!("lang-sql", lang_sql, "sql");
        try_lang!("lang-ssh-config", lang_ssh_config, "ssh-config");
        try_lang!("lang-starlark", lang_starlark, "starlark");
        try_lang!("lang-svelte", lang_svelte, "svelte");
        try_lang!("lang-swift", lang_swift, "swift");
        try_lang!("lang-textproto", lang_textproto, "textproto");
        try_lang!("lang-thrift", lang_thrift, "thrift");
        try_lang!("lang-tlaplus", lang_tlaplus, "tlaplus");
        try_lang!("lang-toml", lang_toml, "toml");
        try_lang!("lang-tsx", lang_tsx, "tsx");
        try_lang!("lang-typst", lang_typst, "typst");
        try_lang!("lang-uiua", lang_uiua, "uiua");
        try_lang!("lang-vb", lang_vb, "vb");
        try_lang!("lang-verilog", lang_verilog, "verilog");
        try_lang!("lang-vhdl", lang_vhdl, "vhdl");
        try_lang!("lang-vue", lang_vue, "vue");
        try_lang!("lang-x86asm", lang_x86asm, "x86asm");
        try_lang!("lang-xml", lang_xml, "xml");
        try_lang!("lang-yaml", lang_yaml, "yaml");
        try_lang!("lang-yuri", lang_yuri, "yuri");
        try_lang!("lang-zig", lang_zig, "zig");

        None
    }
}

impl GrammarProvider for StaticProvider {
    type Grammar = TreeSitterGrammar;

    #[cfg(not(target_arch = "wasm32"))]
    async fn get(&mut self, language: &str) -> Option<&mut Self::Grammar> {
        let normalized = Self::normalize_language(language);

        // Create grammar if not cached
        if !self.grammars.contains_key(normalized) {
            if let Some(grammar) = Self::create_grammar(normalized) {
                self.grammars.insert(normalized, grammar);
            }
        }

        self.grammars.get_mut(normalized)
    }

    #[cfg(target_arch = "wasm32")]
    async fn get(&mut self, language: &str) -> Option<&mut Self::Grammar> {
        let normalized = Self::normalize_language(language);

        // Create grammar if not cached
        if !self.grammars.contains_key(normalized) {
            if let Some(grammar) = Self::create_grammar(normalized) {
                self.grammars.insert(normalized, grammar);
            }
        }

        self.grammars.get_mut(normalized)
    }
}

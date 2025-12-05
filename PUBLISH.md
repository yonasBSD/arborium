# Publishing Guide


## Core Crates (always present)

```
crates/
├── arborium/                  ← main crate with inventory
├── tree-sitter/               ← fork to avoid upstream build errors
└── tree-sitter-highlight/     ← fork to avoid upstream build errors
```

- These are always in the repo (not generated) and are published every release.
- The tree-sitter forks track upstream but include fixes so CI builds reliably across targets.

## Release Flow

Everything is generated **once**, then we push tags **one by one** so jobs stay fast and easy to retry.

### 1. Prepare a release locally

```bash
# Pick a version and generate everything (core + all groups)
# This runs tree-sitter-cli for all grammars, but results are cached
# by tree-sitter-cli version + grammar.js hash, so reruns are cheap.

cargo xtask gen --version 0.3.0
```

`xtask gen` also records the current release version in a small metadata file
(so later commands like `xtask tag --group squirrel` don't need you to
re-type `0.3.0`).

### 2. Tag and push, one job at a time

```bash
# Core release tag (publishes core crates)
cargo xtask tag --core      # creates + pushes v0.3.0

# Group tags (about 10 animal groups, hand-crafted)
cargo xtask tag --group squirrel   # creates + pushes v0.3.0-squirrel
cargo xtask tag --group deer       # creates + pushes v0.3.0-deer
# ... fox, bear, wolf, otter, etc. (up to ~10 groups)
```

Each pushed tag triggers a CI workflow that **only publishes** based on the
already-generated, committed files. CI never runs `xtask gen --version`.

- Core tag (`v0.3.0`) → publishes `arborium`, forks of `tree-sitter` and
  `tree-sitter-highlight`, `miette-arborium`, etc.
- Group tag (`v0.3.0-squirrel`, `v0.3.0-deer`, …) → publishes that group's
  crates.io crates **and** its npm artifacts in a single job.

You can stagger tags over time to keep jobs short and make retries cheap.

### 3. Publish collection crate

After all groups are out, a final tag (or the core tag job) publishes
`arborium-collection`, which depends on the per-language crates and exposes
feature flags.

## Two Outputs, Two Registries

### 1. Native Rust Crates → crates.io

- ~98 grammar crates organized into ~10 hand-picked animal groups
- Core crates (`arborium`, `arborium-collection`, `miette-arborium`, `tree-sitter-*`) always published
- Each group publishes independently from `langs/group-{animal}/` via
  `cargo ws publish --publish-as-is`
- We use `cargo ws publish` instead of `cargo publish --workspace` because the
  latter is still brittle with partial publishes; `cargo ws publish` can resume
  cleanly.
- **Retry-safe**: crates.io warns and skips already-published versions

### 2. WASM Plugins → npm

- All grammars with `generate-component: true` in
  `langs/group-{animal}/{lang}/sources/arborium.kdl`
- Built via `cargo-component` for `wasm32-wasip2` from the same group directory
- Transpiled via `jco` for browser compatibility
- Published as per-language packages under the `@arborium` scope, e.g.
  `@arborium/rust`, `@arborium/javascript`, etc.
- **Published together with crates.io** in the same per-group CI job for version sync

## Publishing Strategy

- We publish per-group, and each group job handles **both** crates.io and npm together.
- Tags are pushed **manually**, one after another, so jobs stay fast and easy to
  retry.
- Core crates (`arborium`, `arborium-collection`, `tree-sitter-*`, `miette-arborium`) publish once per release before groups.

### Trusted Publishing (Cargo + npm)

- For **crates.io**, we use Cargo's Trusted Publishing (GitHub OIDC) so CI
  doesn't need long-lived API tokens.
- For **npm**, we likewise use GitHub's trusted publishing integration so
  publishing is tied to tags and workflows, not shared secrets.

### crates.io (per group)

Cargo handles already-published versions gracefully - it warns and continues:
```
warning: crate arborium-rust@0.3.0 already exists on crates.io
```

So retrying a group is safe; already-published crates are skipped.

### npm (per group, via xtask)

npm is **not graceful** - it hard-fails with `EPUBLISHCONFLICT`:
```
npm ERR! code EPUBLISHCONFLICT
npm ERR! Cannot publish over existing version
```

**xtask publish** (per group) must:
- Check if version exists before publishing
- Distinguish `EPUBLISHCONFLICT` (skip, continue) from real errors (fail)
- Handle retries without re-publishing successes

Each group job builds plugins from its own crates and publishes npm artifacts
immediately to keep versions in lockstep.

## What's in Git vs Generated

### Source of Truth vs Generated

The **source of truth** for each language lives under its group and language,
inside a `sources/` directory. Generated artifacts live next to it as `crate/`
(static Rust crate) and `npm/` (WASM plugin package).

```
langs/
├── group-squirrel/               (Web languages)
│   ├── rust/
│   │   ├── sources/              ← LANGUAGE DEFINITIONS (committed)
│   │   │   ├── arborium.kdl      ← SOURCE OF TRUTH
│   │   │   ├── grammar/
│   │   │   │   ├── grammar.js    ← tree-sitter grammar
│   │   │   │   └── scanner.c     ← custom scanner (if any)
│   │   │   ├── queries/
│   │   │   │   └── highlights.scm ← highlight queries
│   │   │   └── samples/          ← test samples
│   │   ├── crate/                ← Static linking crate (generated)
│   │   └── npm/                  ← WASM plugin package (generated)
│   ├── javascript/
│   ├── html/
│   └── [other web languages...]
├── group-deer/                   (C family)
│   ├── c/
│   ├── cpp/
│   ├── objc/
│   └── [other C family languages...]
├── group-fox/                    (Systems languages)
│   ├── python/
│   ├── go/
│   ├── java/
│   └── [other systems languages...]
├── group-bear/                   (Web frameworks)
│   ├── typescript/
│   ├── tsx/
│   ├── svelte/
│   ├── vue/
│   └── [other web frameworks...]
├── group-wolf/                   (Data/config)
│   ├── json/
│   ├── yaml/
│   ├── toml/
│   ├── xml/
│   └── [other data formats...]
└── group-otter/                  (Scripting/other)
    ├── bash/
    ├── perl/
    ├── php/
    ├── ruby/
    └── [other scripting languages...]
```
### Generated (gitignored)

```
langs/group-{animal}/{lang}/crate/
├── Cargo.toml            ← GENERATED by xtask gen
├── build.rs              ← GENERATED by xtask gen
├── src/lib.rs            ← GENERATED by xtask gen
└── grammar/
    └── src/              ← GENERATED by xtask gen (tree-sitter generate)
        ├── parser.c
        ├── grammar.json
        └── ...

langs/group-{animal}/{lang}/npm/
├── Cargo.toml            ← GENERATED for cargo-component
├── src/
│   └── bindings.rs      ← GENERATED bindings
└── package.json          ← GENERATED npm package
```

### Non-generated crates (hand-written, committed)

These crates don't have `arborium.kdl` and are fully hand-written:
- `arborium` (main crate)
- `arborium-test-harness`
- `arborium-sysroot`
- `arborium-host`
- `arborium-wire`
- `arborium-plugin-runtime`
- `miette-arborium`

## What `xtask gen --version X.Y.Z` Does

1. **Updates core crate versions:**
   - `arborium/Cargo.toml` version = "X.Y.Z"
   - `arborium-collection/Cargo.toml` version = "X.Y.Z"

2. **Generates group workspace files:**
   - `langs/group-{animal}/Cargo.toml` with member crates and version "X.Y.Z"

3. **Generates grammar crate files from sources:**
   - Reads `langs/group-{animal}/{lang}/sources/arborium.kdl` and friends
   - Writes `langs/group-{animal}/{lang}/crate/Cargo.toml` with version "X.Y.Z"
   - Writes `build.rs` with correct C compilation setup
   - Writes `src/lib.rs` with language exports
   - Runs tree-sitter generate into `grammar/src/*`

4. **Generates WASM plugin packages:**
   - Writes `langs/group-{animal}/{lang}/npm/Cargo.toml` for cargo-component build
   - Writes `langs/group-{animal}/{lang}/npm/package.json` for npm publishing
   - Writes `langs/group-{animal}/{lang}/npm/src/bindings.rs` generated bindings

When called without `--version`, uses `0.0.0-dev` (fine for local dev since path deps ignore versions).

## Workflows

### Local Development

```bash
# Edit arborium.kdl, grammar.js, queries, etc.

# Regenerate (uses 0.0.0-dev version, doesn't matter locally)
cargo xtask gen

# Build and test
cargo build
cargo test
```

### Release

See **Release Flow** above for the full tag + CI story.

## Artifacts Published

| Registry | Package | Count |
|----------|---------|-------|
| crates.io | `arborium` (core with inventory) | 1 |
| crates.io | `arborium-collection` (feature-gated) | 1 |
| crates.io | `arborium-{lang}` (static crates) | 98 |
| crates.io | `arborium-test-harness` | 1 |
| crates.io | `arborium-sysroot` | 1 |
| crates.io | `tree-sitter-patched-arborium` | 1 |
| crates.io | `tree-sitter-highlight-patched-arborium` | 1 |
| crates.io | `miette-arborium` | 1 |
| npmjs.com | `@arborium/arborium` (bundle) | 1 |
| npmjs.com | `@arborium/{lang}` (per-language WASM plugins) | 98 |

## TODO

- [ ] Finalize ~10 hand-crafted animal groups (squirrel, deer, fox, bear, wolf,
      otter, …) and document which languages live where
- [ ] Implement inventory system in arborium crate
- [ ] Create arborium-collection crate with feature flags
- [ ] Update `xtask publish` + `xtask tag` commands for:
  - [ ] Group-based publishing (per-animal tags like v0.3.0-squirrel)
  - [ ] Combined crates.io + npm publishing per group
  - [ ] Inventory-aware dependency resolution
- [ ] Update generate caching to tree-sitter-cli output only
- [ ] Standardize wasm-opt settings to -Oz
- [ ] Unify release.yml and npm-publish.yml into single workflow

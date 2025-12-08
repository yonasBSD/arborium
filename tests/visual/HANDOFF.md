# Handoff: Visual Regression Testing

## Current Status

Visual regression testing infrastructure is set up but not yet tested end-to-end.

## What's Done

1. **Playwright setup** (`tests/visual/`)
   - `package.json` with dependencies: playwright, pixelmatch, pngjs
   - `screenshot.mjs` - main test script

2. **Script features**:
   - Fetches language list from `/plugins.json`
   - Injects CSS to hide UI and fullscreen the code area
   - Forces monospace font for cross-platform consistency
   - Captures PNG screenshots per language
   - Uses pixelmatch for diff comparison (0.1% threshold)
   - Saves diffs to `diffs/` folder on failure

3. **Usage**:
   ```bash
   cd tests/visual
   npm install
   npx playwright install chromium

   # Start demo server in another terminal:
   cargo xtask serve

   # Run tests (compares against snapshots/):
   npm test

   # Or test specific languages:
   node screenshot.mjs rust python javascript

   # Update baseline snapshots:
   npm run update
   ```

## What's Left

1. **Test the script** - needs demo server running (`cargo xtask serve`)

2. **Generate initial snapshots** - run `npm run update` to create baseline

3. **CI integration** - Add to GitHub Actions:
   - Install playwright + chromium in Docker image
   - Run `cargo xtask serve` in background
   - Run `npm test` in `tests/visual/`
   - Upload diff artifacts on failure

4. **Docker image update** - Add to `Dockerfile.plugin-builder`:
   ```dockerfile
   # For visual regression tests
   RUN npm install -g playwright
   RUN npx playwright install chromium --with-deps
   ```

5. **Gitignore** - Add to `.gitignore`:
   ```
   tests/visual/diffs/
   tests/visual/node_modules/
   ```

   But **commit** `tests/visual/snapshots/` (the baseline images)

## Open Questions

- Should snapshots be PNG or convert to JPEG-XL for smaller repo size?
- Font consistency: current approach uses generic `monospace`, may need a specific font installed in Docker

## Files

- `tests/visual/screenshot.mjs` - main script
- `tests/visual/package.json` - npm config
- `tests/visual/snapshots/` - baseline images (to be generated)
- `tests/visual/diffs/` - diff output on failures (gitignored)

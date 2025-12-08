# Visual Regression Tests

This directory contains visual regression tests for arborium's syntax highlighting. Screenshots are captured for each supported language and compared against baseline snapshots to detect unexpected changes in highlighting output.

## Prerequisites

1. **Node.js** (v18+)
2. **ImageMagick** with JPEG-XL support (for snapshot compression)
   ```bash
   # macOS
   brew install imagemagick

   # Linux (Ubuntu/Debian)
   apt install imagemagick
   ```
3. **Playwright Chromium browser**

## Setup

```bash
cd tests/visual
npm install
npx playwright install chromium
```

## Running Tests

### 1. Start the demo server

In a separate terminal:
```bash
cargo xtask serve
```

The server must be running at `http://127.0.0.1:8000` (the default).

### 2. Run visual tests

```bash
# Run all tests (compare against baseline snapshots)
npm test

# Test specific languages only
node screenshot.mjs rust python javascript

# Update baseline snapshots (after intentional changes)
npm run update

# Update specific language snapshots
node screenshot.mjs --update rust
```

## Configuration

Environment variables:
- `DEMO_URL` - Override server URL (default: `http://127.0.0.1:8000`)
- `PARALLEL_BROWSERS` - Number of parallel browser contexts (default: `4`)

## How It Works

1. **Screenshot capture**: Uses Playwright to load each language's demo page, injects CSS to isolate the code output, and captures a 2x HiDPI screenshot (800x600 viewport, 1600x1200 actual pixels).

2. **Snapshot format**: Screenshots are stored as JPEG-XL (`.jxl`) files for efficient storage (~30-60KB per language vs ~100-200KB for PNG). ImageMagick handles conversion.

3. **Comparison**: Uses pixelmatch to compare screenshots with a 0.1% pixel difference threshold. This allows for minor anti-aliasing variations while catching actual highlighting changes.

4. **Diff output**: On failure, diff images are saved to `diffs/` showing pixel differences in red.

## Directory Structure

```
tests/visual/
├── README.md          # This file
├── package.json       # npm configuration
├── screenshot.mjs     # Main test script
├── snapshots/         # Baseline images (committed to git)
│   ├── rust.jxl
│   ├── python.jxl
│   └── ...
└── diffs/             # Diff output on failures (gitignored)
    ├── rust.png       # Pixel diff
    └── rust-actual.png # Actual screenshot
```

## Common Tasks

### Adding a new language

1. Add the grammar via `arborium.kdl`
2. Run `cargo xtask gen --version <version>`
3. Start the demo server: `cargo xtask serve`
4. Generate the snapshot: `node screenshot.mjs --update <language>`
5. Commit the new `.jxl` file

### Updating snapshots after intentional changes

If you've made intentional changes to highlighting (e.g., updated queries):

```bash
# Update all snapshots
npm run update

# Or update specific languages
node screenshot.mjs --update cpp glsl hlsl
```

Review the changes, then commit the updated `.jxl` files.

### Debugging failures

1. Check `diffs/<language>.png` for visual diff
2. Check `diffs/<language>-actual.png` for what was captured
3. Compare against `snapshots/<language>.jxl`

To view a `.jxl` file:
```bash
# Convert to PNG for viewing
magick snapshots/rust.jxl /tmp/rust.png
open /tmp/rust.png  # macOS
```

## CI Integration

Visual tests run in GitHub Actions CI. The workflow:

1. Builds the demo server
2. Starts it in the background
3. Runs `npm test`
4. Uploads diff artifacts on failure

See `.github/workflows/ci.yml` for details.

## Troubleshooting

### "ImageMagick is required for JPEG-XL support"

Install ImageMagick with JXL support:
```bash
brew install imagemagick  # macOS
apt install imagemagick   # Linux
```

### "Failed to fetch languages from http://127.0.0.1:8000/plugins.json"

The demo server isn't running. Start it with:
```bash
cargo xtask serve
```

### Tests fail on CI but pass locally

Font rendering can differ between systems. The test uses a 0.1% threshold to account for minor variations. If you see persistent failures:

1. Check if the diff shows actual highlighting changes vs font rendering differences
2. Consider updating the baseline if the change is acceptable
3. The CI uses Chromium in headless mode which should be deterministic

### High pixel diff percentages

If you see >1% differences, there's likely an actual change in highlighting. This could be:
- Updated highlight queries
- Changed theme colors
- Modified sample code
- Font/CSS changes in the demo

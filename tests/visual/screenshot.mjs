#!/usr/bin/env node
// Visual regression testing for arborium demo
// Captures screenshots of each language's syntax highlighting

import { chromium } from 'playwright';
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';
import { PNG } from 'pngjs';
import pixelmatch from 'pixelmatch';

const __dirname = dirname(fileURLToPath(import.meta.url));
const SNAPSHOTS_DIR = join(__dirname, 'snapshots');
const DIFF_DIR = join(__dirname, 'diffs');

// Config
const VIEWPORT = { width: 800, height: 600 };
const BASE_URL = process.env.DEMO_URL || 'http://127.0.0.1:8000';
const THRESHOLD = 0.1; // 0.1% pixel difference allowed

// Force consistent font rendering across platforms
const FONT_CSS = `
    * {
        font-family: 'monospace' !important;
        -webkit-font-smoothing: none !important;
        text-rendering: geometricPrecision !important;
    }
    /* Hide UI elements for cleaner screenshots */
    .language-selector, .controls, header, footer, nav {
        display: none !important;
    }
    /* Make code area fill viewport */
    .code-display {
        position: fixed !important;
        inset: 0 !important;
        margin: 0 !important;
        padding: 16px !important;
        overflow: auto !important;
    }
`;

async function getLanguages(page) {
    // Fetch the registry to get all available languages
    const response = await page.goto(`${BASE_URL}/plugins.json`);
    const registry = await response.json();
    return registry.entries.map(e => e.language);
}

async function captureScreenshot(page, language) {
    // Navigate to language
    await page.goto(`${BASE_URL}/#${language}`);

    // Inject consistent styling
    await page.addStyleTag({ content: FONT_CSS });

    // Wait for highlighting to complete (code element should have children)
    await page.waitForSelector('.code-display pre code span', { timeout: 10000 }).catch(() => {
        console.warn(`  Warning: No highlighting spans found for ${language}`);
    });

    // Small delay for rendering
    await page.waitForTimeout(100);

    // Capture full page (code area is now fullscreen)
    return await page.screenshot({ type: 'png' });
}

function compareImages(img1Buffer, img2Buffer) {
    const img1 = PNG.sync.read(img1Buffer);
    const img2 = PNG.sync.read(img2Buffer);

    // If dimensions differ, images are definitely different
    if (img1.width !== img2.width || img1.height !== img2.height) {
        return { match: false, diffPercent: 100, diff: null };
    }

    const { width, height } = img1;
    const diff = new PNG({ width, height });

    const numDiffPixels = pixelmatch(
        img1.data,
        img2.data,
        diff.data,
        width,
        height,
        { threshold: 0.1 }
    );

    const totalPixels = width * height;
    const diffPercent = (numDiffPixels / totalPixels) * 100;

    return {
        match: diffPercent <= THRESHOLD,
        diffPercent,
        diff: PNG.sync.write(diff)
    };
}

async function main() {
    const args = process.argv.slice(2);
    const updateMode = args.includes('--update');
    const specificLangs = args.filter(a => !a.startsWith('--'));

    // Ensure directories exist
    mkdirSync(SNAPSHOTS_DIR, { recursive: true });
    mkdirSync(DIFF_DIR, { recursive: true });

    console.log('Launching browser...');
    const browser = await chromium.launch({ headless: true });
    const context = await browser.newContext({ viewport: VIEWPORT });
    const page = await context.newPage();

    // Get languages
    let languages;
    try {
        languages = await getLanguages(page);
    } catch (e) {
        console.error(`Failed to fetch languages from ${BASE_URL}/plugins.json`);
        console.error('Make sure the demo server is running: cargo xtask serve');
        process.exit(1);
    }

    if (specificLangs.length > 0) {
        languages = languages.filter(l => specificLangs.includes(l));
    }

    console.log(`Testing ${languages.length} languages...`);

    const results = { passed: [], failed: [], new: [] };

    for (const lang of languages) {
        process.stdout.write(`  ${lang}... `);

        const snapshotPath = join(SNAPSHOTS_DIR, `${lang}.png`);
        const diffPath = join(DIFF_DIR, `${lang}.png`);

        try {
            const screenshot = await captureScreenshot(page, lang);
            if (!screenshot) {
                console.log('SKIP (no content)');
                continue;
            }

            if (updateMode || !existsSync(snapshotPath)) {
                // Save new snapshot
                writeFileSync(snapshotPath, screenshot);
                if (updateMode) {
                    console.log('UPDATED');
                    results.passed.push(lang);
                } else {
                    console.log('NEW');
                    results.new.push(lang);
                }
            } else {
                // Compare with existing
                const existing = readFileSync(snapshotPath);
                const { match, diffPercent, diff } = compareImages(existing, screenshot);

                if (match) {
                    console.log('PASS');
                    results.passed.push(lang);
                } else {
                    console.log(`FAIL (${diffPercent.toFixed(2)}% diff)`);
                    results.failed.push({ lang, diffPercent });

                    // Save diff image
                    if (diff) {
                        writeFileSync(diffPath, diff);
                    }
                    // Save actual screenshot for comparison
                    writeFileSync(join(DIFF_DIR, `${lang}-actual.png`), screenshot);
                }
            }
        } catch (e) {
            console.log(`ERROR: ${e.message}`);
            results.failed.push({ lang, error: e.message });
        }
    }

    await browser.close();

    // Summary
    console.log('\n--- Summary ---');
    console.log(`Passed: ${results.passed.length}`);
    console.log(`Failed: ${results.failed.length}`);
    console.log(`New:    ${results.new.length}`);

    if (results.failed.length > 0) {
        console.log('\nFailed languages:');
        for (const { lang, diffPercent, error } of results.failed) {
            if (error) {
                console.log(`  - ${lang}: ${error}`);
            } else {
                console.log(`  - ${lang}: ${diffPercent.toFixed(2)}% difference`);
            }
        }
        console.log('\nRun with --update to accept current screenshots as new baseline.');
        process.exit(1);
    }

    if (results.new.length > 0) {
        console.log('\nNew snapshots created. Review and commit them.');
    }
}

main().catch(e => {
    console.error(e);
    process.exit(1);
});

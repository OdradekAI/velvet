import { writeFile } from 'node:fs/promises';
import { spawnSync } from 'node:child_process';
import { resolve } from 'node:path';

const projectRoot = resolve(new URL('../..', import.meta.url).pathname, '..');
const outputPath = resolve(projectRoot, '.scratch/velvet-ui-evidence-alignment/research/host-projection-qualification.json');

const script = `
const task = await taskSpace("velvet host projection qualification");
const page = task.page("p1");
await page.goto("http://127.0.0.1:5173/?fixture=host-projections");
await page.cdp("Emulation.setDeviceMetricsOverride", {width:1280,height:800,deviceScaleFactor:1,mobile:false});
await page.waitForSelector('loc=css:[data-ui-kit-fixture="host-projections"]', {state:"visible"});

const firstCell = 'loc=css:[data-ui-kit-probe="asset-grid"] [role="gridcell"]:nth-of-type(1)';
const secondCell = 'loc=css:[data-ui-kit-probe="asset-grid"] [role="gridcell"]:nth-of-type(2)';
await page.focus(firstCell);
await page.keyboard.press("ArrowRight");
const assetMoved = await page.evaluate(() => ({
  selected: document.querySelector('[data-ui-kit-observation="asset-selection"]')?.textContent ?? null,
  focused: document.activeElement?.getAttribute('aria-label') ?? null,
}));
await page.focus(secondCell);
await page.keyboard.press("Enter");
const assetOpened = await page.evaluate(() => document.querySelector('[data-ui-kit-observation="asset-selection"]')?.textContent ?? null);

const firstRow = 'loc=css:[data-ui-kit-probe="data-table"] tbody tr:nth-child(1)';
await page.focus(firstRow);
await page.keyboard.press("Enter");
const tableSelected = await page.evaluate(() => ({
  state: document.querySelector('[data-ui-kit-observation="table-selection"]')?.textContent ?? null,
  focused: document.activeElement?.textContent?.trim() ?? null,
}));
await page.click('loc=css:[data-ui-kit-probe="data-table"] .velvet-data-table__sort');
const tableSorted = await page.evaluate(() => document.querySelector('[data-ui-kit-observation="table-selection"]')?.textContent ?? null);

await page.click('loc=css:[data-ui-test-id="command-palette-trigger"]');
await page.waitForSelector('loc=css:[data-ui-kit-probe="command-palette"] [role="dialog"]', {state:"visible"});
const paletteOpen = await page.evaluate(() => ({
  role: document.querySelector('[data-ui-kit-probe="command-palette"] [role="dialog"]')?.getAttribute('role') ?? null,
  active: document.activeElement?.getAttribute('aria-label') ?? document.activeElement?.getAttribute('placeholder') ?? null,
}));
await page.keyboard.press("ArrowDown");
await page.keyboard.press("Enter");
const paletteSelected = await page.evaluate(() => document.querySelector('[data-ui-kit-observation="command-selection"]')?.textContent ?? null);
await page.click('loc=css:[data-ui-test-id="command-palette-trigger"]');
await page.waitForSelector('loc=css:[data-ui-kit-probe="command-palette"] [role="dialog"]', {state:"visible"});
await page.keyboard.press("Escape");
await page.waitForFunction(() => !document.querySelector('[data-ui-kit-probe="command-palette"] [role="dialog"]'));
const paletteClosed = await page.evaluate(() => ({
  closed: !document.querySelector('[data-ui-kit-probe="command-palette"] [role="dialog"]'),
  restoredFocus: document.activeElement?.getAttribute('data-ui-test-id') ?? document.activeElement?.textContent?.trim() ?? null,
}));

const result = {
  fixture: "host-projections",
  viewport: {width: await page.evaluate(() => innerWidth), height: await page.evaluate(() => innerHeight), dpr: await page.evaluate(() => devicePixelRatio)},
  assetGrid: {moved: assetMoved, opened: assetOpened},
  dataTable: {selected: tableSelected, sorted: tableSorted},
  commandPalette: {open: paletteOpen, selected: paletteSelected, closed: paletteClosed},
};
await (await import("node:fs/promises")).writeFile(${JSON.stringify(outputPath)}, JSON.stringify(result, null, 2));
console.log(JSON.stringify(result, null, 2));
`;

const result = spawnSync('ego-browser', ['nodejs'], { input: script, encoding: 'utf8', stdio: ['pipe', 'inherit', 'inherit'] });
if (result.error) throw new Error(`ego-browser is required for host projection qualification: ${result.error.message}`);
if (result.status !== 0) process.exit(result.status ?? 1);

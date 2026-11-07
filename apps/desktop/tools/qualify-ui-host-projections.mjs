import { writeFile } from 'node:fs/promises';
import { spawnSync } from 'node:child_process';
import { resolve } from 'node:path';

const projectRoot = resolve(new URL('../..', import.meta.url).pathname, '..');
const outputPath = resolve(projectRoot, '.scratch/velvet-ui-evidence-alignment/research/host-projection-qualification.json');

const script = `
const task = await taskSpace("velvet host projection qualification");
const page = task.page("p1");
const assert = (condition, message) => { if (!condition) throw new Error(message); };
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
assert(assetMoved.selected === 'selected:asset-2;opened:', 'AssetGrid ArrowRight selection contract failed');
assert(assetOpened === 'selected:asset-2;opened:asset-2', 'AssetGrid Enter open contract failed');
assert(tableSelected.state === 'selected:row-1;sorted:', 'DataTable row selection contract failed');
assert(tableSorted === 'selected:row-1;sorted:name', 'DataTable sort contract failed');
assert(paletteOpen.role === 'dialog', 'CommandPalette dialog role contract failed');
assert(paletteSelected === 'selected:command.pin', 'CommandPalette selection contract failed');
assert(paletteClosed.closed && paletteClosed.restoredFocus === 'command-palette-trigger', 'CommandPalette focus restoration contract failed');

const inspector = 'loc=css:[data-ui-kit-probe="inspector"]';
await page.click(inspector + ' [aria-label="Pin inspector"]');
await page.waitForFunction(() => document.querySelector('[data-ui-kit-probe="inspector"] [aria-label="Unpin inspector"]')?.getAttribute('aria-pressed') === 'true');
const inspectorPinned = await page.evaluate(() => ({
  pressed: document.querySelector('[data-ui-kit-probe="inspector"] [aria-label="Unpin inspector"]')?.getAttribute('aria-pressed') ?? null,
  state: document.querySelector('[data-ui-kit-observation="inspector-state"]')?.textContent ?? null,
}));
await page.fill(inspector + ' input[aria-label="Name"]', 'Renamed poster');
await page.click(inspector + ' [data-inspector-action="save"]');
await page.waitForFunction(() => document.querySelector('[data-ui-kit-probe="inspector"] [role="status"]')?.textContent === 'Saved');
const inspectorSaved = await page.evaluate(() => ({
  status: document.querySelector('[data-ui-kit-probe="inspector"] [role="status"]')?.textContent ?? null,
  state: document.querySelector('[data-ui-kit-observation="inspector-state"]')?.textContent ?? null,
}));
assert(inspectorPinned.pressed === 'true', 'Inspector pin contract failed');
assert(inspectorPinned.state === 'name:Astronaut poster;pinned:true;save:idle', 'Inspector pin observation failed');
assert(inspectorSaved.status === 'Saved', 'Inspector save status contract failed');
assert(inspectorSaved.state === 'name:Renamed poster;pinned:true;save:saved', 'Inspector save observation failed');

const result = {
  fixture: "host-projections",
  viewport: {width: await page.evaluate(() => innerWidth), height: await page.evaluate(() => innerHeight), dpr: await page.evaluate(() => devicePixelRatio)},
  assetGrid: {moved: assetMoved, opened: assetOpened},
  dataTable: {selected: tableSelected, sorted: tableSorted},
  commandPalette: {open: paletteOpen, selected: paletteSelected, closed: paletteClosed},
  inspector: {pinned: inspectorPinned, saved: inspectorSaved},
};
await (await import("node:fs/promises")).writeFile(${JSON.stringify(outputPath)}, JSON.stringify(result, null, 2));
console.log(JSON.stringify(result, null, 2));
`;

const result = spawnSync('ego-browser', ['nodejs'], { input: script, encoding: 'utf8', stdio: ['pipe', 'inherit', 'inherit'] });
if (result.error) throw new Error(`ego-browser is required for host projection qualification: ${result.error.message}`);
if (result.status !== 0) process.exit(result.status ?? 1);

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

await page.focus('loc=css:[data-ui-kit-probe="activity-bar"] [aria-label="Library"]');
await page.keyboard.press("ArrowRight");
const activityBar = await page.evaluate(() => ({
  selected: document.querySelector('[data-ui-kit-observation="activity-bar-state"]')?.textContent ?? null,
  focused: document.activeElement?.getAttribute('aria-label') ?? null,
  navigation: document.querySelector('[data-ui-kit-probe="activity-bar"] [data-ui-kit-navigation="activity-bar"]')?.getAttribute('aria-label') ?? null,
}));

const paneSemantics = await page.evaluate(() => {
  const pane = document.querySelector('[data-ui-kit-probe="pane"] [data-ui-kit-region="navigation"]');
  const heading = pane?.querySelector('h2');
  return {role: pane?.getAttribute('role') ?? null, labelledBy: pane?.getAttribute('aria-labelledby') ?? null, heading: heading?.textContent?.trim() ?? null, body: pane?.querySelector('.velvet-pane__body')?.textContent?.trim() ?? null};
});
const panelSemantics = await page.evaluate(() => {
  const panel = document.querySelector('[data-ui-kit-probe="panel"] [data-ui-kit-component="panel"]');
  const heading = panel?.querySelector('h2');
  return {role: panel?.getAttribute('role') ?? null, labelledBy: panel?.getAttribute('aria-labelledby') ?? null, heading: heading?.textContent?.trim() ?? null, raised: panel?.classList.contains('velvet-panel--surface-raised') ?? false};
});

const previewError = await page.evaluate(() => ({
  state: document.querySelector('[data-ui-kit-observation="preview-host-state"]')?.textContent ?? null,
  alert: document.querySelector('[data-ui-kit-probe="preview-host"] [role="alert"]')?.textContent?.trim() ?? null,
}));
await page.click('loc=css:[data-ui-kit-probe="preview-host"] button');
await page.waitForFunction(() => document.querySelector('[data-ui-kit-observation="preview-host-state"]')?.textContent === 'ready');
const previewReady = await page.evaluate(() => ({
  state: document.querySelector('[data-ui-kit-observation="preview-host-state"]')?.textContent ?? null,
  region: document.querySelector('[data-ui-kit-probe="preview-host"] .velvet-preview-host')?.getAttribute('role') ?? null,
}));

await page.focus('loc=css:[data-ui-kit-probe="zoom-control"] input[aria-label="Zoom level"]');
await page.keyboard.press("ArrowRight");
const zoomControl = await page.evaluate(() => ({
  value: document.querySelector('[data-ui-kit-observation="zoom-control-value"]')?.textContent ?? null,
  inputValue: document.querySelector('[data-ui-kit-probe="zoom-control"] input[aria-label="Zoom level"]')?.value ?? null,
  live: document.querySelector('[data-ui-kit-probe="zoom-control"] .velvet-zoom-control__value')?.getAttribute('aria-live') ?? null,
}));

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
await page.evaluate(() => document.querySelector('[data-ui-kit-probe="asset-grid"] [role="gridcell"]:nth-of-type(2)')?.dispatchEvent(new MouseEvent('click', { bubbles: true, shiftKey: true })));
const assetRangeSelected = await page.evaluate(() => document.querySelector('[data-ui-kit-observation="asset-selection-mode"]')?.textContent ?? null);
const assetGridSemantics = await page.evaluate(() => {
  const grid = document.querySelector('[data-ui-kit-probe="asset-grid"] [role="grid"]');
  return 'multi:' + (grid?.getAttribute('aria-multiselectable') ?? '') + ';rows:' + (grid?.getAttribute('aria-rowcount') ?? '') + ';cols:' + (grid?.getAttribute('aria-colcount') ?? '');
});

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
await page.fill('loc=css:[data-ui-kit-probe="command-palette"] input[aria-label="Search commands"]', 'Pin');
await page.waitForFunction(() => document.querySelectorAll('[data-ui-kit-probe="command-palette"] [role="option"]').length === 1);
const paletteFiltered = await page.evaluate(() => [...document.querySelectorAll('[data-ui-kit-probe="command-palette"] [role="option"]')].map((node) => node.textContent?.trim() ?? null));
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
assert(assetRangeSelected === 'replace:range', 'AssetGrid Shift range selection contract failed');
assert(assetGridSemantics === 'multi:true;rows:2;cols:2', 'AssetGrid grid semantics contract failed');
assert(activityBar.selected === 'selected:feed' && activityBar.focused === 'Feed' && activityBar.navigation === 'Qualification workspaces', 'ActivityBar focus/navigation contract failed');
assert(paneSemantics.role === 'region' && paneSemantics.labelledBy && paneSemantics.heading === 'Qualification navigation' && paneSemantics.body === 'Pane content', 'Pane accessibility contract failed');
assert(panelSemantics.role === 'region' && panelSemantics.labelledBy && panelSemantics.heading === 'Qualification panel' && panelSemantics.raised, 'Panel accessibility contract failed');
assert(previewError.state === 'error' && previewError.alert?.includes('Preview failed'), 'PreviewHost error announcement contract failed');
assert(previewReady.state === 'ready' && previewReady.region === 'region', 'PreviewHost retry/region contract failed');
assert(zoomControl.value === '185' && zoomControl.inputValue === '185' && zoomControl.live === 'polite', 'ZoomControl keyboard/value contract failed');
assert(tableSelected.state === 'selected:row-1;sorted:', 'DataTable row selection contract failed');
assert(tableSorted === 'selected:row-1;sorted:name', 'DataTable sort contract failed');
assert(paletteOpen.role === 'dialog', 'CommandPalette dialog role contract failed');
assert(paletteFiltered.length === 1 && paletteFiltered[0]?.startsWith('Pin selected asset'), 'CommandPalette query filter contract failed: ' + JSON.stringify(paletteFiltered));
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
  activityBar,
  pane: paneSemantics,
  panel: panelSemantics,
  previewHost: {error: previewError, ready: previewReady},
  zoomControl,
  assetGrid: {moved: assetMoved, opened: assetOpened, rangeSelected: assetRangeSelected, semantics: assetGridSemantics},
  dataTable: {selected: tableSelected, sorted: tableSorted},
  commandPalette: {open: paletteOpen, filtered: paletteFiltered, selected: paletteSelected, closed: paletteClosed},
  inspector: {pinned: inspectorPinned, saved: inspectorSaved},
};
await (await import("node:fs/promises")).writeFile(${JSON.stringify(outputPath)}, JSON.stringify(result, null, 2));
console.log(JSON.stringify(result, null, 2));
`;

const result = spawnSync('ego-browser', ['nodejs'], { input: script, encoding: 'utf8', stdio: ['pipe', 'inherit', 'inherit'] });
if (result.error) throw new Error(`ego-browser is required for host projection qualification: ${result.error.message}`);
if (result.status !== 0) process.exit(result.status ?? 1);

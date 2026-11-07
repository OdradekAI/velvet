import assert from 'node:assert/strict';
import { chromium } from 'playwright';

const baseUrl = 'http://127.0.0.1:5173';
const browser = await chromium.launch({ headless: true });
const page = await browser.newPage({ viewport: { width: 1280, height: 800 }, deviceScaleFactor: 1 });

async function fixtureReady(fixture) {
  await page.goto(`${baseUrl}/?fixture=${fixture}`);
  const fixtureId = fixture === 'visual-baseline' ? 'visual-baseline' : fixture === 'host-projections' ? 'host-projections' : 'primitives-overlays';
  await page.locator(`[data-ui-kit-fixture="${fixtureId}"]`).waitFor({ state: 'visible' });
  await page.waitForTimeout(150);
}

async function observation(name) {
  return page.locator(`[data-ui-kit-observation="${name}"]`).textContent();
}

const overlay = {};
await fixtureReady('visual-baseline');
const dialogTrigger = page.locator('[data-ui-kit-probe="dialog-trigger"]');
const dialog = page.locator('[role="dialog"]');
await dialogTrigger.click();
await dialog.waitFor({ state: 'visible' });
overlay.dialogOpen = await page.evaluate(() => ({
  open: Boolean(document.querySelector('[role="dialog"]')),
  active: document.activeElement?.className ?? null,
}));
await page.locator('[data-ui-test-id="dialog-action-apply"]').click();
await page.waitForFunction(() => document.querySelector('[data-ui-kit-observation="dialog-action"]')?.textContent === 'dialog:submitted');
overlay.dialogSubmitted = await observation('dialog-action');
await page.locator('[data-ui-test-id="dialog-cancel"]').click();
await dialog.waitFor({ state: 'hidden' });
overlay.dialogCancelled = await observation('dialog-action');
await dialogTrigger.click();
await dialog.waitFor({ state: 'visible' });
await page.keyboard.press('Escape');
await dialog.waitFor({ state: 'hidden' });
overlay.dialogEscapeCancelled = await page.evaluate(() => ({
  closed: !document.querySelector('[role="dialog"]'),
  action: document.querySelector('[data-ui-kit-observation="dialog-action"]')?.textContent ?? null,
  restoredFocus: document.activeElement?.getAttribute('data-ui-kit-probe') ?? null,
}));
assert.equal(overlay.dialogOpen.open, true);
assert.equal(overlay.dialogSubmitted, 'dialog:submitted');
assert.equal(overlay.dialogCancelled, 'dialog:cancelled');
assert.deepEqual(overlay.dialogEscapeCancelled, { closed: true, action: 'dialog:cancelled', restoredFocus: 'dialog-trigger' });

const contextTrigger = page.locator('[data-ui-kit-probe="context-trigger"]');
const contextMenu = page.locator('.velvet-context-menu');
const contextBox = await contextTrigger.boundingBox();
assert.ok(contextBox, 'context trigger rectangle unavailable');
await page.mouse.click(contextBox.x + contextBox.width / 2, contextBox.y + contextBox.height / 2, { button: 'right' });
await contextMenu.waitFor({ state: 'visible' });
const contextItems = await contextMenu.locator('.velvet-context-menu__item').evaluateAll((items) => items.map((item) => ({ label: item.textContent?.trim() ?? '', checked: item.getAttribute('aria-checked') })));
await page.keyboard.press('ArrowDown');
await page.keyboard.press('Enter');
await contextMenu.waitFor({ state: 'hidden' });
const contextSelected = await observation('context-action');
await page.goto(`${baseUrl}/?fixture=visual-baseline`);
await page.locator('[data-ui-kit-fixture="visual-baseline"]').waitFor({ state: 'visible' });
await page.waitForTimeout(150);
const reloadedContextTrigger = page.locator('[data-ui-kit-probe="context-trigger"]');
const reloadedContextBox = await reloadedContextTrigger.boundingBox();
assert.ok(reloadedContextBox, 'context trigger rectangle unavailable after reload');
await page.mouse.click(reloadedContextBox.x + reloadedContextBox.width / 2, reloadedContextBox.y + reloadedContextBox.height / 2, { button: 'right' });
await contextMenu.waitFor({ state: 'visible' });
await page.waitForTimeout(100);
await page.mouse.click(300, 36);
await contextMenu.waitFor({ state: 'hidden' });
const contextOutsideCancelled = await observation('context-action');
overlay.context = { items: contextItems, selected: contextSelected, outsideCancelled: contextOutsideCancelled };
assert.equal(contextItems.length, 2);
assert.equal(contextItems[0].checked, 'true');
assert.equal(contextSelected, 'context:pin');
assert.equal(contextOutsideCancelled, 'context:idle');

const host = {};
await fixtureReady('host-projections');
const firstCell = page.locator('[data-ui-kit-probe="asset-grid"] [role="gridcell"]').nth(0);
const secondCell = page.locator('[data-ui-kit-probe="asset-grid"] [role="gridcell"]').nth(1);
await firstCell.focus();
await page.keyboard.press('ArrowRight');
host.assetMoved = await observation('asset-selection');
await secondCell.focus();
await page.keyboard.press('Enter');
host.assetOpened = await observation('asset-selection');

const firstRow = page.locator('[data-ui-kit-probe="data-table"] tbody tr').first();
await firstRow.focus();
await page.keyboard.press('Enter');
host.tableSelected = await observation('table-selection');
await page.locator('[data-ui-kit-probe="data-table"] .velvet-data-table__sort').click();
host.tableSorted = await observation('table-selection');

const paletteTrigger = page.locator('[data-ui-test-id="command-palette-trigger"]');
const paletteDialog = page.locator('[data-ui-kit-probe="command-palette"] [role="dialog"]');
await paletteTrigger.click();
await paletteDialog.waitFor({ state: 'visible' });
await paletteDialog.locator('input[aria-label="Search commands"]').fill('Pin');
await page.waitForFunction(() => document.querySelectorAll('[data-ui-kit-probe="command-palette"] [role="option"]').length === 1);
host.paletteFiltered = await paletteDialog.locator('[role="option"]').allTextContents();
host.paletteOpen = await page.evaluate(() => ({
  role: document.querySelector('[data-ui-kit-probe="command-palette"] [role="dialog"]')?.getAttribute('role') ?? null,
  active: document.activeElement?.getAttribute('aria-label') ?? null,
}));
await page.keyboard.press('ArrowDown');
await page.keyboard.press('Enter');
host.paletteSelected = await observation('command-selection');
await paletteTrigger.click();
await paletteDialog.waitFor({ state: 'visible' });
await page.keyboard.press('Escape');
await paletteDialog.waitFor({ state: 'hidden' });
host.paletteClosed = await page.evaluate(() => ({
  closed: !document.querySelector('[data-ui-kit-probe="command-palette"] [role="dialog"]'),
  restoredFocus: document.activeElement?.getAttribute('data-ui-test-id') ?? null,
}));
assert.deepEqual(host.paletteFiltered, ['Pin selected assetKeep the asset visible']);
assert.equal(host.paletteOpen.role, 'dialog');
assert.equal(host.paletteSelected, 'selected:command.pin');
assert.deepEqual(host.paletteClosed, { closed: true, restoredFocus: 'command-palette-trigger' });

const inspector = page.locator('[data-ui-kit-probe="inspector"]');
await inspector.locator('[aria-label="Pin inspector"]').click();
await inspector.locator('input[aria-label="Name"]').fill('Renamed poster');
await inspector.locator('[data-inspector-action="save"]').click();
await page.waitForFunction(() => document.querySelector('[data-ui-kit-probe="inspector"] [role="status"]')?.textContent === 'Saved');
host.inspector = {
  state: await observation('inspector-state'),
  status: await inspector.locator('[role="status"]').textContent(),
};
assert.equal(host.inspector.state, 'name:Renamed poster;pinned:true;save:saved');
assert.equal(host.inspector.status, 'Saved');

const filter = page.locator('[data-ui-kit-probe="filter-panel"]');
await filter.locator('input[aria-label="Annotation"]').fill('poster');
await page.waitForFunction(() => document.querySelector('[data-ui-kit-observation="filter-rules"]')?.textContent?.includes('poster'));
host.filterChanged = await observation('filter-rules');
await filter.locator('.velvet-filter-panel__actions button').nth(0).click();
host.filterReset = await observation('filter-rules');
await filter.locator('input[aria-label="Annotation"]').fill('poster');
await filter.locator('.velvet-filter-panel__actions button').nth(1).click();
host.filterSaved = await observation('filter-saved');
await filter.locator('.velvet-filter-panel__header button').click();
host.filterClosed = await observation('filter-closed');
assert.ok(host.filterChanged.includes('poster'));
assert.equal(host.filterReset, '[]');
assert.ok(host.filterSaved.includes('poster'));
assert.equal(host.filterClosed, 'true');

await fixtureReady('visual-baseline');
const zoom = page.locator('[data-ui-kit-probe="workspace"] [role="group"][aria-label="Asset zoom"]');
const zoomSlider = zoom.locator('input[aria-label="Zoom level"]');
assert.equal(await zoomSlider.inputValue(), '180');
await zoomSlider.focus();
await page.keyboard.press('ArrowRight');
assert.equal(await zoomSlider.inputValue(), '185');
await zoom.locator('button[aria-label="Zoom out"]').click();
assert.equal(await zoomSlider.inputValue(), '180');
await zoom.locator('button[aria-label="Zoom in"]').click();
assert.equal(await zoomSlider.inputValue(), '185');
host.zoom = {
  value: await zoomSlider.inputValue(),
  focused: await zoomSlider.evaluate((node) => document.activeElement === node),
  step: await zoomSlider.getAttribute('step'),
};
assert.deepEqual(host.zoom, { value: '185', focused: false, step: '5' });

const primitives = {};
await fixtureReady('primitives-overlays');
primitives.staticSemantics = await page.locator('[data-ui-kit-probe="static-primitives"]').evaluate((node) => ({
  badgeTag: node.querySelector('.velvet-badge')?.tagName ?? null,
  iconRole: node.querySelector('.velvet-icon')?.getAttribute('role') ?? null,
  iconLabel: node.querySelector('.velvet-icon')?.getAttribute('aria-label') ?? null,
  heading: node.querySelector('h1')?.textContent?.trim() ?? null,
  separator: node.querySelector('[role="separator"]')?.getAttribute('aria-orientation') ?? null,
  progressLabel: node.querySelector('progress')?.getAttribute('aria-label') ?? null,
  progressValue: node.querySelector('progress')?.value ?? null,
}));
await page.locator('[data-ui-kit-probe="button"] [aria-label="Run action"]').click();
await page.locator('[data-ui-kit-probe="button"] [aria-label="Add item"]').click();
primitives.button = await page.locator('[data-ui-kit-probe="button"]').evaluate((node) => ({
  activated: node.querySelector('[data-ui-kit-observation="button-state"]')?.textContent ?? null,
  busyDisabled: node.querySelector('[aria-label="Busy action"]')?.hasAttribute('disabled') ?? false,
  busy: node.querySelector('[aria-label="Busy action"]')?.getAttribute('aria-busy') ?? null,
}));
await page.locator('[data-ui-kit-probe="native-controls"] input[aria-label="Primitive input"]').fill('Ada');
await page.locator('[data-ui-kit-probe="native-controls"] select[aria-label="Primitive select"]').selectOption('name');
await page.locator('[data-ui-kit-probe="native-controls"] input[type="checkbox"]').click();
primitives.nativeControls = await page.locator('[data-ui-kit-observation="native-values"]').textContent();
await page.locator('[data-ui-kit-probe="tabs"] [role="tab"]').first().focus();
await page.keyboard.press('ArrowRight');
primitives.tabs = await page.locator('[data-ui-kit-probe="tabs"]').evaluate((node) => ({
  selected: node.querySelector('[data-ui-kit-observation="tab-state"]')?.textContent ?? null,
  focused: document.activeElement?.textContent?.trim() ?? null,
  orientation: node.querySelector('[role="tablist"]')?.getAttribute('aria-orientation') ?? null,
}));
primitives.stateRole = await page.locator('[data-ui-kit-probe="state"] [role="alert"]').getAttribute('role');
await page.locator('[data-ui-kit-probe="state"] button').click();
primitives.stateRetry = await page.locator('[data-ui-kit-observation="state-retry"]').textContent();
primitives.toast = await page.locator('[data-ui-kit-probe="toast"]').evaluate((node) => ({
  role: node.querySelector('[role="status"]')?.getAttribute('role') ?? null,
  live: node.querySelector('[role="status"]')?.getAttribute('aria-live') ?? null,
  count: node.querySelector('[data-ui-kit-observation="toast-count"]')?.textContent ?? null,
}));
await page.locator('[data-ui-kit-probe="toast"] [aria-label="Dismiss notification"]').click();
primitives.toastDismissed = await page.locator('[data-ui-kit-observation="toast-count"]').textContent();
assert.equal(primitives.staticSemantics.badgeTag, 'SPAN');
assert.equal(primitives.staticSemantics.iconRole, 'img');
assert.equal(primitives.staticSemantics.iconLabel, 'Favorite');
assert.equal(primitives.staticSemantics.heading, 'Primitive semantics');
assert.equal(primitives.staticSemantics.separator, 'vertical');
assert.equal(primitives.staticSemantics.progressLabel, 'Progress');
assert.equal(primitives.staticSemantics.progressValue, 60);
assert.deepEqual(primitives.button, { activated: '2', busyDisabled: true, busy: 'true' });
assert.equal(primitives.nativeControls, 'input:Ada;select:name;checked:true');
assert.deepEqual(primitives.tabs, { selected: 'recent', focused: 'Recent', orientation: 'horizontal' });
assert.equal(primitives.stateRole, 'alert');
assert.equal(primitives.stateRetry, '1');
assert.deepEqual(primitives.toast, { role: 'status', live: 'polite', count: '1' });
assert.equal(primitives.toastDismissed, '0');

console.log(JSON.stringify({ engine: 'playwright', overlay, host, primitives }, null, 2));
await browser.close();

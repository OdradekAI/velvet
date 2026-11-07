import assert from 'node:assert/strict';
import { chromium } from 'playwright';

const baseUrl = 'http://127.0.0.1:5173';
const browser = await chromium.launch({ headless: true });
const page = await browser.newPage({ viewport: { width: 1280, height: 800 }, deviceScaleFactor: 1 });

async function fixtureReady(fixture) {
  await page.goto(`${baseUrl}/?fixture=${fixture}`);
  await page.locator(`[data-ui-kit-fixture="${fixture === 'eagle' ? 'eagle-baseline' : 'host-projections'}"]`).waitFor({ state: 'visible' });
  await page.waitForTimeout(150);
}

async function observation(name) {
  return page.locator(`[data-ui-kit-observation="${name}"]`).textContent();
}

const overlay = {};
await fixtureReady('eagle');
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
await page.goto(`${baseUrl}/?fixture=eagle`);
await page.locator('[data-ui-kit-fixture="eagle-baseline"]').waitFor({ state: 'visible' });
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

console.log(JSON.stringify({ engine: 'playwright', overlay, host }, null, 2));
await browser.close();

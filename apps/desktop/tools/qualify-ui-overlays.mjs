import { spawnSync } from 'node:child_process';
import { resolve } from 'node:path';

const projectRoot = resolve(new URL('../..', import.meta.url).pathname, '..');
const evidenceRoot = resolve(projectRoot, '.scratch/velvet-ui-kit-and-plugin-boundaries/research');

const script = `
const task = await taskSpace("velvet eagle overlay qualification");
const page = task.page("p1");
const assert = (condition, message) => { if (!condition) throw new Error(message); };
const waitFor = async (label, predicate) => {
  try {
    await page.waitForFunction(predicate);
  } catch (error) {
    throw new Error(label + ': ' + String(error));
  }
};
await page.goto("http://127.0.0.1:5173/?fixture=eagle");
await page.cdp("Emulation.setDeviceMetricsOverride", {width:1280,height:800,deviceScaleFactor:1,mobile:false});
await page.waitForSelector('loc=css:[data-ui-kit-fixture="eagle-baseline"]', {state:"visible"});
await new Promise((resolve) => setTimeout(resolve, 150));

const dialogTrigger = 'loc=css:[data-ui-kit-probe="dialog-trigger"]';
await page.click(dialogTrigger);
await page.waitForSelector('loc=css:[role="dialog"]', {state:"visible"});
await page.waitForSelector('loc=css:[data-ui-test-id="dialog-action-apply"]', {state:"visible"});
const dialogOpen = await page.evaluate(() => ({
  open: Boolean(document.querySelector('[role="dialog"]')),
  active: document.activeElement?.getAttribute('data-ui-kit-probe') ?? document.activeElement?.className ?? null,
}));
const dialogScreenshot = await page.screenshot({path:${JSON.stringify(resolve(evidenceRoot, 'velvet-dialog-overlay.png'))}});
await page.click('loc=css:[data-ui-test-id="dialog-action-apply"]');
await waitFor('dialog submit', () => document.querySelector('[data-ui-kit-observation="dialog-action"]')?.textContent === 'dialog:submitted');
const dialogSubmitted = await page.evaluate(() => document.querySelector('[data-ui-kit-observation="dialog-action"]')?.textContent ?? null);
await page.click('loc=css:[data-ui-test-id="dialog-cancel"]');
await waitFor('dialog cancel button', () => !document.querySelector('[role="dialog"]'));
const dialogCancelled = await page.evaluate(() => document.querySelector('[data-ui-kit-observation="dialog-action"]')?.textContent ?? null);
await page.click(dialogTrigger);
await page.waitForSelector('loc=css:[role="dialog"]', {state:"visible"});
await page.keyboard.press("Escape");
await waitFor('dialog Escape', () => !document.querySelector('[role="dialog"]'));
const dialogEscapeCancelled = await page.evaluate(() => ({
  closed: !document.querySelector('[role="dialog"]'),
  action: document.querySelector('[data-ui-kit-observation="dialog-action"]')?.textContent ?? null,
  restoredFocus: document.activeElement?.getAttribute('data-ui-kit-probe') ?? document.activeElement?.className ?? null,
}));
assert(dialogOpen.open && dialogOpen.active !== null, 'Dialog open/focus contract failed');
assert(dialogSubmitted === 'dialog:submitted', 'Dialog submit action contract failed');
assert(dialogCancelled === 'dialog:cancelled', 'Dialog cancel action contract failed');
assert(dialogEscapeCancelled.closed && dialogEscapeCancelled.action === 'dialog:cancelled' && dialogEscapeCancelled.restoredFocus === 'dialog-trigger', 'Dialog Escape cancellation/focus restoration contract failed');

const contextTrigger = 'loc=css:[data-ui-kit-probe="context-trigger"]';
await page.focus(contextTrigger);
const contextRect = await page.evaluate(() => {
  const node = document.querySelector('[data-ui-kit-probe="context-trigger"]');
  const rect = node?.getBoundingClientRect();
  return rect ? {x:rect.x + rect.width / 2, y:rect.y + rect.height / 2} : null;
});
if (!contextRect) throw new Error("context trigger rectangle unavailable");
await page.cdp("Input.dispatchMouseEvent", {type:"mousePressed",x:contextRect.x,y:contextRect.y,button:"right",clickCount:1});
await page.cdp("Input.dispatchMouseEvent", {type:"mouseReleased",x:contextRect.x,y:contextRect.y,button:"right",clickCount:1});
await page.waitForSelector('loc=css:.velvet-context-menu', {state:"visible"});
const contextOpen = await page.evaluate(() => ({
  open: Boolean(document.querySelector('.velvet-context-menu')),
  items: [...document.querySelectorAll('.velvet-context-menu__item')].map((node) => ({label:node.textContent?.trim(), highlighted:node.hasAttribute('data-highlighted'), checked:node.getAttribute('aria-checked')})),
}));
const contextScreenshot = await page.screenshot({path:${JSON.stringify(resolve(evidenceRoot, 'velvet-context-menu.png'))}});
await page.keyboard.press("ArrowDown");
await page.keyboard.press("Enter");
await waitFor('context menu selection', () => !document.querySelector('.velvet-context-menu'));
const contextKeyboardDismissed = await page.evaluate(() => !document.querySelector('.velvet-context-menu'));
const contextSelected = await page.evaluate(() => document.querySelector('[data-ui-kit-observation="context-action"]')?.textContent ?? null);
assert(contextOpen.open && contextOpen.items.length === 2 && contextOpen.items[0]?.checked === 'true', 'Context menu open/checked contract failed');
assert(contextKeyboardDismissed && contextSelected === 'context:pin', 'Context menu keyboard selection contract failed: ' + contextKeyboardDismissed + '/' + contextSelected);

await page.focus(contextTrigger);
const secondRect = await page.evaluate(() => { const node = document.querySelector('[data-ui-kit-probe="context-trigger"]'); const rect = node?.getBoundingClientRect(); return rect ? {x:rect.x + rect.width / 2, y:rect.y + rect.height / 2} : null; });
await page.cdp("Input.dispatchMouseEvent", {type:"mousePressed",x:secondRect.x,y:secondRect.y,button:"right",clickCount:1});
await page.cdp("Input.dispatchMouseEvent", {type:"mouseReleased",x:secondRect.x,y:secondRect.y,button:"right",clickCount:1});
await page.waitForSelector('loc=css:.velvet-context-menu', {state:"visible"});
await page.keyboard.press("Escape");
await waitFor('context menu Escape', () => !document.querySelector('.velvet-context-menu'));
const contextEscapeCancelled = await page.evaluate(() => ({
  closed: !document.querySelector('.velvet-context-menu'),
  action: document.querySelector('[data-ui-kit-observation="context-action"]')?.textContent ?? null,
}));

await page.goto("http://127.0.0.1:5173/?fixture=eagle");
await page.cdp("Emulation.setDeviceMetricsOverride", {width:1280,height:800,deviceScaleFactor:1,mobile:false});
await page.waitForSelector('loc=css:[data-ui-kit-fixture="eagle-baseline"]', {state:"visible"});
await new Promise((resolve) => setTimeout(resolve, 150));
await page.focus(contextTrigger);
const thirdRect = await page.evaluate(() => { const node = document.querySelector('[data-ui-kit-probe="context-trigger"]'); const rect = node?.getBoundingClientRect(); return rect ? {x:rect.x + rect.width / 2, y:rect.y + rect.height / 2} : null; });
if (!thirdRect) throw new Error('context trigger rectangle unavailable after reload');
await page.cdp("Input.dispatchMouseEvent", {type:"mousePressed",x:thirdRect.x,y:thirdRect.y,button:"right",clickCount:1});
await page.cdp("Input.dispatchMouseEvent", {type:"mouseReleased",x:thirdRect.x,y:thirdRect.y,button:"right",clickCount:1});
await page.waitForSelector('loc=css:.velvet-context-menu', {state:"visible"});
await new Promise((resolve) => setTimeout(resolve, 100));
await page.cdp("Input.dispatchMouseEvent", {type:"mousePressed",x:300,y:36,button:"left",clickCount:1});
await page.cdp("Input.dispatchMouseEvent", {type:"mouseReleased",x:300,y:36,button:"left",clickCount:1});
await waitFor('context menu outside click', () => !document.querySelector('.velvet-context-menu'));
const contextOutsideDismissed = await page.evaluate(() => !document.querySelector('.velvet-context-menu'));
const contextOutsideCancelled = await page.evaluate(() => document.querySelector('[data-ui-kit-observation="context-action"]')?.textContent ?? null);
assert(contextEscapeCancelled.closed && contextEscapeCancelled.action === 'context:pin', 'Context menu Escape cancellation contract failed: ' + JSON.stringify(contextEscapeCancelled));
assert(contextOutsideDismissed && contextOutsideCancelled === 'context:idle', 'Context menu outside dismissal contract failed: ' + contextOutsideDismissed + '/' + contextOutsideCancelled);

console.log(JSON.stringify({viewport:{width:await page.evaluate(() => innerWidth),height:await page.evaluate(() => innerHeight)},dialogOpen,dialogSubmitted,dialogCancelled,dialogEscapeCancelled,contextOpen,contextSelected,contextKeyboardDismissed,contextEscapeCancelled,contextOutsideDismissed,contextOutsideCancelled,dialogScreenshot,contextScreenshot}, null, 2));
`;

const result = spawnSync('ego-browser', ['nodejs'], { input: script, encoding: 'utf8', stdio: ['pipe', 'inherit', 'inherit'] });
if (result.error) {
  throw new Error(`ego-browser is required for overlay qualification: ${result.error.message}`);
}
if (result.status !== 0) {
  process.exit(result.status ?? 1);
}

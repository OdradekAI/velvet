import { spawnSync } from 'node:child_process';
import { resolve } from 'node:path';

const projectRoot = resolve(new URL('../..', import.meta.url).pathname, '..');
const evidenceRoot = resolve(projectRoot, '.scratch/velvet-ui-kit-and-plugin-boundaries/research');

const script = `
const task = await taskSpace("velvet eagle overlay qualification");
const page = task.page("p1");
const assert = (condition, message) => { if (!condition) throw new Error(message); };
await page.goto("http://127.0.0.1:5173/?fixture=eagle");
await page.cdp("Emulation.setDeviceMetricsOverride", {width:1280,height:800,deviceScaleFactor:1,mobile:false});
await page.waitForSelector('loc=css:[data-ui-kit-fixture="eagle-baseline"]', {state:"visible"});

const dialogTrigger = 'loc=css:[data-ui-kit-probe="dialog-trigger"]';
await page.click(dialogTrigger);
await page.waitForSelector('loc=css:[role="dialog"]', {state:"visible"});
const dialogOpen = await page.evaluate(() => ({
  open: Boolean(document.querySelector('[role="dialog"]')),
  active: document.activeElement?.getAttribute('data-ui-kit-probe') ?? document.activeElement?.className ?? null,
}));
const dialogScreenshot = await page.screenshot({path:${JSON.stringify(resolve(evidenceRoot, 'velvet-dialog-overlay.png'))}});
await page.keyboard.press("Escape");
await page.waitForFunction(() => !document.querySelector('[role="dialog"]'));
const dialogClosed = await page.evaluate(() => ({
  closed: !document.querySelector('[role="dialog"]'),
  restoredFocus: document.activeElement?.getAttribute('data-ui-kit-probe') ?? document.activeElement?.className ?? null,
}));
assert(dialogOpen.open && dialogOpen.active !== null, 'Dialog open/focus contract failed');
assert(dialogClosed.closed && dialogClosed.restoredFocus === 'dialog-trigger', 'Dialog close/focus restoration contract failed');

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
await page.waitForFunction(() => !document.querySelector('.velvet-context-menu'));
const contextKeyboardDismissed = await page.evaluate(() => !document.querySelector('.velvet-context-menu'));
assert(contextOpen.open && contextOpen.items.length === 2 && contextOpen.items[0]?.checked === 'true', 'Context menu open/checked contract failed');
assert(contextKeyboardDismissed, 'Context menu keyboard selection contract failed');

await page.focus(contextTrigger);
const secondRect = await page.evaluate(() => { const node = document.querySelector('[data-ui-kit-probe="context-trigger"]'); const rect = node?.getBoundingClientRect(); return rect ? {x:rect.x + rect.width / 2, y:rect.y + rect.height / 2} : null; });
await page.cdp("Input.dispatchMouseEvent", {type:"mousePressed",x:secondRect.x,y:secondRect.y,button:"right",clickCount:1});
await page.cdp("Input.dispatchMouseEvent", {type:"mouseReleased",x:secondRect.x,y:secondRect.y,button:"right",clickCount:1});
await page.waitForSelector('loc=css:.velvet-context-menu', {state:"visible"});
const facetsRect = await page.evaluate(() => { const node = document.querySelector('[data-ui-kit-probe="facets"]'); const rect = node?.getBoundingClientRect(); return rect ? {x:rect.x + 10, y:rect.y + 10} : null; });
await page.cdp("Input.dispatchMouseEvent", {type:"mousePressed",x:facetsRect.x,y:facetsRect.y,button:"left",clickCount:1});
await page.cdp("Input.dispatchMouseEvent", {type:"mouseReleased",x:facetsRect.x,y:facetsRect.y,button:"left",clickCount:1});
await page.waitForFunction(() => !document.querySelector('.velvet-context-menu'));
const contextOutsideDismissed = await page.evaluate(() => !document.querySelector('.velvet-context-menu'));
assert(contextOutsideDismissed, 'Context menu outside dismissal contract failed');

console.log(JSON.stringify({viewport:{width:await page.evaluate(() => innerWidth),height:await page.evaluate(() => innerHeight)},dialogOpen,dialogClosed,contextOpen,contextKeyboardDismissed,contextOutsideDismissed,dialogScreenshot,contextScreenshot}, null, 2));
`;

const result = spawnSync('ego-browser', ['nodejs'], { input: script, encoding: 'utf8', stdio: ['pipe', 'inherit', 'inherit'] });
if (result.error) {
  throw new Error(`ego-browser is required for overlay qualification: ${result.error.message}`);
}
if (result.status !== 0) {
  process.exit(result.status ?? 1);
}

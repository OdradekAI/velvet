const task = await taskSpace("velvet primitives and overlays qualification");
const page = task.page("p1");
const assert = (condition, message) => { if (!condition) throw new Error(message); };

await page.goto("http://127.0.0.1:5173/?fixture=primitives-overlays");
await page.cdp("Emulation.setDeviceMetricsOverride", {width:1280,height:800,deviceScaleFactor:1,mobile:false});
await page.waitForSelector('loc=css:[data-ui-kit-fixture="primitives-overlays"]', {state:"visible"});

const staticSemantics = await page.evaluate(() => ({
  badgeTag: document.querySelector('[data-ui-kit-probe="static-primitives"] .velvet-badge')?.tagName ?? null,
  iconRole: document.querySelector('[data-ui-kit-probe="static-primitives"] .velvet-icon')?.getAttribute('role') ?? null,
  iconLabel: document.querySelector('[data-ui-kit-probe="static-primitives"] .velvet-icon')?.getAttribute('aria-label') ?? null,
  heading: document.querySelector('[data-ui-kit-probe="static-primitives"] h1')?.textContent?.trim() ?? null,
  stackClass: document.querySelector('[data-ui-kit-probe="static-primitives"] .velvet-stack')?.className ?? null,
  inlineClass: document.querySelector('[data-ui-kit-probe="static-primitives"] .velvet-stack--row')?.className ?? null,
  separator: document.querySelector('[data-ui-kit-probe="static-primitives"] [role="separator"]')?.getAttribute('aria-orientation') ?? null,
  progress: document.querySelector('[data-ui-kit-probe="static-primitives"] progress') ? {
    label: document.querySelector('[data-ui-kit-probe="static-primitives"] progress')?.getAttribute('aria-label') ?? null,
    value: document.querySelector('[data-ui-kit-probe="static-primitives"] progress')?.value ?? null,
  } : null,
}));

await page.click('loc=css:[data-ui-kit-probe="button"] [aria-label="Run action"]');
await page.click('loc=css:[data-ui-kit-probe="button"] [aria-label="Add item"]');
const buttonState = await page.evaluate(() => ({
  activated: document.querySelector('[data-ui-kit-observation="button-state"]')?.textContent ?? null,
  busyDisabled: document.querySelector('[data-ui-kit-probe="button"] [aria-label="Busy action"]')?.hasAttribute('disabled') ?? false,
  busy: document.querySelector('[data-ui-kit-probe="button"] [aria-label="Busy action"]')?.getAttribute('aria-busy') ?? null,
}));

await page.fill('loc=css:[data-ui-kit-probe="native-controls"] input[aria-label="Primitive input"]', 'Ada');
await page.evaluate(() => { const select = document.querySelector('[data-ui-kit-probe="native-controls"] select[aria-label="Primitive select"]'); if (select) { select.value = 'name'; select.dispatchEvent(new Event('change', { bubbles: true })); } });
await page.click('loc=css:[data-ui-kit-probe="native-controls"] input[type="checkbox"]');
const nativeControls = await page.evaluate(() => ({
  values: document.querySelector('[data-ui-kit-observation="native-values"]')?.textContent ?? null,
  inputFocused: document.activeElement?.getAttribute('aria-label') === 'Primitive select',
  checkboxLabel: document.querySelector('[data-ui-kit-probe="native-controls"] label')?.textContent?.trim() ?? null,
}));

await page.focus('loc=css:[data-ui-kit-probe="tabs"] [role="tab"]:first-of-type');
await page.keyboard.press("ArrowRight");
const tabs = await page.evaluate(() => ({
  selected: document.querySelector('[data-ui-kit-observation="tab-state"]')?.textContent ?? null,
  focused: document.activeElement?.textContent?.trim() ?? null,
  orientation: document.querySelector('[data-ui-kit-probe="tabs"] [role="tablist"]')?.getAttribute('aria-orientation') ?? null,
}));

const stateBefore = await page.evaluate(() => ({
  role: document.querySelector('[data-ui-kit-probe="state"] [role="alert"]')?.getAttribute('role') ?? null,
  busy: document.querySelector('[data-ui-kit-probe="state"] [role="alert"]')?.getAttribute('aria-busy') ?? null,
}));
await page.click('loc=css:[data-ui-kit-probe="state"] button');
const stateAfter = await page.evaluate(() => document.querySelector('[data-ui-kit-observation="state-retry"]')?.textContent ?? null);

const toast = await page.evaluate(() => ({
  role: document.querySelector('[data-ui-kit-probe="toast"] [role="status"]')?.getAttribute('role') ?? null,
  live: document.querySelector('[data-ui-kit-probe="toast"] [role="status"]')?.getAttribute('aria-live') ?? null,
  count: document.querySelector('[data-ui-kit-observation="toast-count"]')?.textContent ?? null,
}));
await page.click('loc=css:[data-ui-kit-probe="toast"] [aria-label="Dismiss notification"]');
const toastDismissed = await page.evaluate(() => document.querySelector('[data-ui-kit-observation="toast-count"]')?.textContent ?? null);

assert(staticSemantics.badgeTag === 'SPAN' && staticSemantics.iconRole === 'img' && staticSemantics.iconLabel === 'Favorite', 'Static badge/icon semantics contract failed');
assert(staticSemantics.heading === 'Primitive semantics' && staticSemantics.stackClass?.includes('velvet-stack') && staticSemantics.inlineClass?.includes('velvet-stack--row'), 'Text/Stack/Inline semantics contract failed');
assert(staticSemantics.separator === 'vertical' && staticSemantics.progress?.label === 'Progress' && staticSemantics.progress?.value === 60, 'Separator/Progress semantics contract failed');
assert(buttonState.activated === '2' && buttonState.busyDisabled && buttonState.busy === 'true', 'Button/IconButton busy semantics contract failed');
assert(nativeControls.values === 'input:Ada;select:name;checked:true' && nativeControls.checkboxLabel?.includes('Include archived'), 'Input/Select/Checkbox native contract failed');
assert(tabs.selected === 'recent' && tabs.focused === 'Recent' && tabs.orientation === 'horizontal', 'Tabs keyboard/focus contract failed');
assert(stateBefore.role === 'alert' && stateAfter === '1', 'State alert/retry contract failed');
assert(toast.role === 'status' && toast.live === 'polite' && toast.count === '1' && toastDismissed === '0', 'Toast live/dismissal contract failed');

console.log(JSON.stringify({fixture:"primitives-overlays",staticSemantics,buttonState,nativeControls,tabs,state:{before:stateBefore,after:stateAfter},toast:{...toast,dismissed:toastDismissed}}, null, 2));

import { spawnSync } from 'node:child_process';
import { resolve } from 'node:path';

const projectRoot = resolve(new URL('../..', import.meta.url).pathname, '..');
const outputPath = resolve(projectRoot, '.scratch/velvet-ui-kit-and-plugin-boundaries/research/velvet-eagle-fixture.png');

const script = `
const task = await taskSpace("velvet eagle visual qualification");
const page = task.page("p1");
await page.goto("http://127.0.0.1:5173/?fixture=eagle");
await page.cdp("Emulation.setDeviceMetricsOverride", {width:1280,height:800,deviceScaleFactor:1,mobile:false});
await page.waitForSelector('loc=css:[data-ui-kit-fixture="eagle-baseline"]', {state:"visible"});
const probes = await page.evaluate(() => {
  const names = ["title-bar","navigation","toolbar","facets","workspace","grid","inspector","selection-preview","operations"];
  const regions = Object.fromEntries(names.map((name) => {
    const node = document.querySelector('[data-ui-kit-probe="' + name + '"]');
    if (!node) return [name, null];
    const style = getComputedStyle(node);
    const rect = node.getBoundingClientRect();
    return [name, {rect:{x:rect.x,y:rect.y,width:rect.width,height:rect.height},background:style.backgroundColor,color:style.color,border:style.border,borderRadius:style.borderRadius,fontSize:style.fontSize}];
  }));
  const selected = document.querySelector('[role="gridcell"][aria-selected="true"]');
  if (selected) {
    const style = getComputedStyle(selected);
    const rect = selected.getBoundingClientRect();
    regions['selected-asset'] = {rect:{x:rect.x,y:rect.y,width:rect.width,height:rect.height},border:style.border,borderRadius:style.borderRadius,background:style.backgroundColor};
  }
  return regions;
});
console.log(JSON.stringify({viewport:await page.evaluate(() => ({width:innerWidth,height:innerHeight,dpr:devicePixelRatio})),probes,screenshot:await page.screenshot({path:${JSON.stringify(outputPath)}})}, null, 2));
`;

const result = spawnSync('ego-browser', ['nodejs'], { input: script, encoding: 'utf8', stdio: ['pipe', 'inherit', 'inherit'] });
if (result.error) {
  throw new Error(`ego-browser is required for visual capture: ${result.error.message}`);
}
if (result.status !== 0) {
  process.exit(result.status ?? 1);
}

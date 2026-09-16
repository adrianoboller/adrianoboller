import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';
import { resolve } from 'node:path';
const b = await chromium.launch();
for (const [w,h,mob] of [[412,915,true],[1440,900,false]]) {
  const c = await b.newContext({ viewport:{width:w,height:h}, isMobile:mob, hasTouch:mob });
  const p = await c.newPage(); await p.goto('file://'+resolve('mock/a1.html')); await p.waitForTimeout(500);
  const r = await p.evaluate(() => { const st=document.querySelector('.pp__stage').getBoundingClientRect(); const im=document.querySelector('.pp__slide.is-active img').getBoundingClientRect(); return {palco:[Math.round(st.width),Math.round(st.height)], img:[Math.round(im.width),Math.round(im.height)], dentro: im.left>=st.left-1 && im.right<=st.right+1 && im.top>=st.top-1 && im.bottom<=st.bottom+1}; });
  console.log(mob?'celular':'desktop', JSON.stringify(r)); await c.close();
}
await b.close();

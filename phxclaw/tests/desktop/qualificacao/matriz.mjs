// Matriz: 4 larguras x 2 idiomas x 8 telas. Mede e captura (numeros brutos; o veredito e do
// qualificar.mjs). Uso: node tests/desktop/qualificacao/matriz.mjs [--ui DIR]
// Saida: tests/desktop/out/qualificacao/matriz.json e cap/.
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { join } from 'node:path';
import { subir } from './servidor.mjs';
import { stubTauriFn as stubTauri } from './stub.mjs';
import { chromium, UI, OUT, CAP, MEDIR, GRADE, CONFIG } from './comum.mjs';
const medir = MEDIR;
const grade = GRADE;
const { srv, porta } = await subir(UI, { config: CONFIG });
const ORIG = `http://127.0.0.1:${porta}`;


const LARG = [[1920, 1080], [1366, 768], [768, 1024], [390, 844]];
const TELAS = ['geral', 'agentes', 'ide', 'ferramentas', 'absorcao', 'tarefas', 'config'];
const res = [];
const browser = await chromium.launch();
for (const [w, h] of LARG) {
  for (const lang of ['pt', 'en']) {
    const ctx = await browser.newContext({ viewport: { width: w, height: h }, locale: 'pt-BR', isMobile: w === 390, hasTouch: w <= 768, deviceScaleFactor: 1 });
    await ctx.addInitScript(stubTauri, grade);
    await ctx.addInitScript(l => { try { localStorage.setItem('phxclaw.token', 'token-de-teste'); localStorage.setItem('phxclaw.idioma', l); } catch {} }, lang);
    const page = await ctx.newPage();
    const erros = [];
    page.on('pageerror', e => erros.push(String(e)));
    page.on('console', m => { if (m.type() === 'error') erros.push(m.text()); });
    // splash a meio caminho
    await page.goto(`${ORIG}/index.html?screen=splash`);
    await page.waitForTimeout(1600);
    let m = await page.evaluate(`(${medir})()`);
    await page.screenshot({ path: join(CAP, `splash_${w}_${lang}.png`) });
    res.push({ w, lang, tela: 'splash', ...m, erros: [...erros] });
    await page.goto(`${ORIG}/index.html?screen=dashboard`);
    await page.waitForTimeout(900);
    for (const tela of TELAS) {
      erros.length = 0;
      await page.evaluate(t => { location.hash = t; }, tela);
      await page.waitForTimeout(700);
      if (tela === 'ide') { await page.click('#ideAbrirBash').catch(e => erros.push('clique ide: ' + e.message.split('\n')[0])); await page.waitForTimeout(500); }
      await page.mouse.move(1, 1); m = await page.evaluate(`(${medir})()`);
      await page.screenshot({ path: join(CAP, `${tela}_${w}_${lang}.png`) });
      res.push({ w, lang, tela, ...m, erros: [...erros] });
      console.log(w, lang, tela, 'contraste<', m.contraste.length, 'cortado', m.cortado.length, 'sobreposto', m.sobreposto.length, 'toque', m.toque.length, 'docW', m.scroll.docScrollW, '/', m.scroll.docClientW, 'erros', erros.length);
    }
    await ctx.close();
  }
}
writeFileSync(join(OUT, 'matriz.json'), JSON.stringify(res, null, 1));
await browser.close(); srv.close();
